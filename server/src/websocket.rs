use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use tokio::sync::{mpsc, broadcast, RwLock};
use serde_json::{json, Value};
use futures_util::{StreamExt, SinkExt};
use std::collections::HashMap;
use std::sync::Arc;
use crate::db::{Database, Agent, AgentMetrics};
use crate::registry::AgentRegistry;
use crate::sessions::SessionManager;
use crate::logger::{log_info, log_warn};
use crate::auth;
use chrono::Utc;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct PendingCommand {
    pub id: String,
    pub command_type: String,
    pub payload: String,
}

#[derive(Clone)]
pub struct ServerState {
    pub db: Database,
    pub registry: AgentRegistry,
    pub session_manager: SessionManager,
    pub tx: broadcast::Sender<Value>,
    pub command_queue: Arc<RwLock<HashMap<String, Vec<PendingCommand>>>>,
}

pub async fn agent_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_agent_ws(socket, state))
}

async fn handle_agent_ws(socket: WebSocket, state: ServerState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    let mut agent_id = String::new();
    let mut authenticated = false;
    let mut nonce = String::new();
    
    let auth_timeout = tokio::time::sleep(tokio::time::Duration::from_secs(10));
    tokio::pin!(auth_timeout);
    
    loop {
        tokio::select! {
            _ = &mut auth_timeout => {
                let fail_msg = json!({
                    "type": "AuthFail",
                    "payload": { "reason": "Authentication timeout" }
                });
                let _ = ws_sender.send(Message::Text(fail_msg.to_string())).await;
                return;
            }
            msg = ws_receiver.next() => {
                let msg = match msg {
                    Some(Ok(Message::Text(txt))) => txt,
                    _ => return, 
                };
                
                let parsed: Value = match serde_json::from_str(&msg) {
                    Ok(val) => val,
                    Err(_) => return,
                };
                
                let msg_type = parsed["type"].as_str().unwrap_or("");
                let payload = &parsed["payload"];
                
                if !authenticated {
                    if msg_type == "Register" {
                        agent_id = payload["agent_id"].as_str().unwrap_or("").to_string();
                        let hostname = payload["hostname"].as_str().unwrap_or("").to_string();
                        let os = payload["os"].as_str().unwrap_or("").to_string();
                        
                        if agent_id.is_empty() {
                            return;
                        }
                        
                        log_info(&state.db, &state.tx, "Server", Some(agent_id.clone()), &format!("Registering agent on host: {}", hostname));
                        
                        let agent = Agent {
                            id: agent_id.clone(),
                            hostname,
                            os,
                            status: "Offline".to_string(),
                            last_seen: Utc::now().to_rfc3339(),
                        };
                        let _ = state.db.upsert_agent(&agent);
                        
                        nonce = auth::generate_nonce();
                        let challenge_msg = json!({
                            "type": "Challenge",
                            "payload": { "nonce": nonce }
                        });
                        if ws_sender.send(Message::Text(challenge_msg.to_string())).await.is_err() {
                            return;
                        }
                    } else if msg_type == "Proof" {
                        if agent_id.is_empty() {
                            return;
                        }
                        let signature = payload["signature"].as_str().unwrap_or("");
                        if auth::verify_proof(&nonce, signature) {
                            authenticated = true;
                            log_info(&state.db, &state.tx, "Server", Some(agent_id.clone()), "Authentication successful!");
                            
                            let auth_ok_msg = json!({
                                "type": "AuthOk",
                                "payload": {}
                            });
                            if ws_sender.send(Message::Text(auth_ok_msg.to_string())).await.is_err() {
                                return;
                            }
                            
                            let now_str = Utc::now().to_rfc3339();
                            let _ = state.db.update_agent_status(&agent_id, "Online", &now_str);
                            let _ = state.session_manager.start_session(&agent_id);
                            
                            let _ = state.tx.send(json!({
                                "type": "AgentStatus",
                                "payload": {
                                    "id": agent_id.clone(),
                                    "status": "Online",
                                    "last_seen": now_str
                                }
                            }));
                            
                            break; 
                        } else {
                            log_warn(&state.db, &state.tx, "Server", Some(agent_id.clone()), "Authentication failed (invalid signature)!");
                            let fail_msg = json!({
                                "type": "AuthFail",
                                "payload": { "reason": "Invalid signature proof" }
                            });
                            let _ = ws_sender.send(Message::Text(fail_msg.to_string())).await;
                            return;
                        }
                    } else {
                        return; 
                    }
                }
            }
        }
    }

    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<Message>();
    state.registry.register(agent_id.clone(), agent_tx);

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = agent_rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let state_clone = state.clone();
    let agent_id_clone = agent_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            let txt = match msg {
                Message::Text(t) => t,
                _ => continue,
            };

            let parsed: Value = match serde_json::from_str(&txt) {
                Ok(val) => val,
                Err(_) => continue,
            };

            let msg_type = parsed["type"].as_str().unwrap_or("");
            let payload = &parsed["payload"];

            if msg_type == "Heartbeat" {
                let now_str = Utc::now().to_rfc3339();
                let _ = state_clone.db.update_agent_status(&agent_id_clone, "Online", &now_str);
                
                let _ = state_clone.tx.send(json!({
                    "type": "AgentStatus",
                    "payload": {
                        "id": agent_id_clone.clone(),
                        "status": "Online",
                        "last_seen": now_str
                    }
                }));
            } else if msg_type == "SystemInfo" {
                let cpu_usage = payload["cpu_usage"].as_f64().unwrap_or(0.0);
                let memory_usage = payload["memory_usage"].as_f64().unwrap_or(0.0);
                let disk_usage = payload["disk_usage"].as_f64().unwrap_or(0.0);
                let now_str = Utc::now().to_rfc3339();

                let metrics = AgentMetrics {
                    id: None,
                    agent_id: agent_id_clone.clone(),
                    cpu_usage,
                    memory_usage,
                    disk_usage,
                    timestamp: now_str.clone(),
                };

                let _ = state_clone.db.insert_metrics(&metrics);
                
                let _ = state_clone.tx.send(json!({
                    "type": "Metrics",
                    "payload": metrics
                }));
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    state.registry.unregister(&agent_id);
    let _ = state.session_manager.end_session(&agent_id);
    let now_str = Utc::now().to_rfc3339();
    let _ = state.db.update_agent_status(&agent_id, "Offline", &now_str);
    log_info(&state.db, &state.tx, "Server", Some(agent_id.clone()), "Agent disconnected");

    let _ = state.tx.send(json!({
        "type": "AgentStatus",
        "payload": {
            "id": agent_id,
            "status": "Offline",
            "last_seen": now_str
        }
    }));
}

pub async fn dashboard_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_dashboard_ws(socket, state))
}

async fn handle_dashboard_ws(socket: WebSocket, state: ServerState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    if let Ok(agents) = state.db.get_agents() {
        let initial_agents_msg = json!({
            "type": "InitialAgents",
            "payload": agents
        });
        if let Ok(txt) = serde_json::to_string(&initial_agents_msg) {
            let _ = ws_sender.send(Message::Text(txt)).await;
        }
    }

    if let Ok(logs) = state.db.get_logs(50) {
        let initial_logs_msg = json!({
            "type": "InitialLogs",
            "payload": logs
        });
        if let Ok(txt) = serde_json::to_string(&initial_logs_msg) {
            let _ = ws_sender.send(Message::Text(txt)).await;
        }
    }

    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(txt) = serde_json::to_string(&event) {
                if ws_sender.send(Message::Text(txt)).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_)) = ws_receiver.next().await {
            // Read and discard messages from dashboard
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
