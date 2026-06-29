use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use serde_json::{json, Value};
use uuid::Uuid;
use std::fs;
use std::time::Duration;
use sysinfo::System;

mod auth;
mod heartbeat;
mod reconnect;
mod system_info;

#[tokio::main]
async fn main() {
    println!("Educational Agent Simulator starting...");

    // Get or create persistent Agent ID
    let agent_id = get_or_create_agent_id();
    println!("Agent ID: {}", agent_id);

    let hostname = system_info::get_hostname();
    let os = system_info::get_os_name();
    println!("Host: {}, OS: {}", hostname, os);

    let ws_url = std::env::var("C2_SERVER_WS_URL")
        .unwrap_or_else(|_| "ws://localhost:3000/api/agent/ws".to_string());

    let mut backoff = reconnect::ReconnectBackoff::new();

    loop {
        println!("Connecting to C2 server at {}...", ws_url);
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                println!("Connected to C2 server!");
                backoff.reset();

                let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                // 1. Send REGISTER message
                let register_msg = json!({
                    "type": "Register",
                    "payload": {
                        "agent_id": agent_id,
                        "hostname": hostname,
                        "os": os
                    }
                });

                if let Err(e) = ws_sender.send(Message::Text(register_msg.to_string())).await {
                    println!("Failed to send register message: {:?}", e);
                    continue;
                }

                // 2. Wait for CHALLENGE message
                let nonce = match ws_receiver.next().await {
                    Some(Ok(Message::Text(txt))) => {
                        match serde_json::from_str::<Value>(&txt) {
                            Ok(val) if val["type"] == "Challenge" => {
                                val["payload"]["nonce"].as_str().unwrap_or("").to_string()
                            }
                            _ => {
                                println!("Unexpected response during registration");
                                continue;
                            }
                        }
                    }
                    _ => {
                        println!("Connection closed during registration");
                        continue;
                    }
                };

                // 3. Solve challenge and send PROOF message
                let proof_signature = auth::solve_challenge(&nonce);
                let proof_msg = json!({
                    "type": "Proof",
                    "payload": {
                        "signature": proof_signature
                    }
                });

                if let Err(e) = ws_sender.send(Message::Text(proof_msg.to_string())).await {
                    println!("Failed to send proof message: {:?}", e);
                    continue;
                }

                // 4. Wait for AUTH_OK or AUTH_FAIL
                match ws_receiver.next().await {
                    Some(Ok(Message::Text(txt))) => {
                        match serde_json::from_str::<Value>(&txt) {
                            Ok(val) if val["type"] == "AuthOk" => {
                                println!("Handshake completed. Authenticated successfully!");
                            }
                            Ok(val) if val["type"] == "AuthFail" => {
                                println!("Authentication failed by server: {}", val["payload"]["reason"]);
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                continue;
                            }
                            _ => {
                                println!("Unexpected response during authentication");
                                continue;
                            }
                        }
                    }
                    _ => {
                        println!("Connection closed during authentication");
                        continue;
                    }
                }

                // Spawning periodic heartbeat and system info tasks
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
                
                let mut ws_write_task = tokio::spawn(async move {
                    while let Some(msg) = rx.recv().await {
                        if ws_sender.send(msg).await.is_err() {
                            break;
                        }
                    }
                });

                let tx_heartbeat = tx.clone();
                let agent_id_heartbeat = agent_id.clone();
                let mut heartbeat_task = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(5));
                    loop {
                        interval.tick().await;
                        let payload = heartbeat::generate_heartbeat_payload(&agent_id_heartbeat);
                        let msg = Message::Text(payload.to_string());
                        if tx_heartbeat.send(msg).is_err() {
                            break;
                        }
                    }
                });

                let tx_metrics = tx.clone();
                let agent_id_metrics = agent_id.clone();
                let mut metrics_task = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(3));
                    let mut local_sys = System::new_all();
                    loop {
                        interval.tick().await;
                        let metrics = system_info::get_system_metrics(&mut local_sys);
                        let metrics_payload = json!({
                            "type": "SystemInfo",
                            "payload": {
                                "agent_id": agent_id_metrics,
                                "cpu_usage": metrics.cpu_usage,
                                "memory_usage": metrics.memory_usage,
                                "disk_usage": metrics.disk_usage
                            }
                        });
                        let msg = Message::Text(metrics_payload.to_string());
                        if tx_metrics.send(msg).is_err() {
                            break;
                        }
                    }
                });

                let mut ws_read_task = tokio::spawn(async move {
                    while let Some(result) = ws_receiver.next().await {
                        match result {
                            Ok(Message::Close(_)) | Err(_) => {
                                break;
                            }
                            _ => {} 
                        }
                    }
                });

                tokio::select! {
                    _ = &mut ws_write_task => println!("WebSocket write loop exited"),
                    _ = &mut ws_read_task => println!("WebSocket read loop exited"),
                    _ = &mut heartbeat_task => println!("Heartbeat loop exited"),
                    _ = &mut metrics_task => println!("Metrics loop exited"),
                }

                ws_write_task.abort();
                ws_read_task.abort();
                heartbeat_task.abort();
                metrics_task.abort();

                println!("Disconnected. Attempting reconnection...");
            }
            Err(e) => {
                println!("Connection error: {:?}", e);
                let delay = backoff.next_delay();
                println!("Sleeping for {:?} before retrying...", delay);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

fn get_or_create_agent_id() -> String {
    let id_file = "agent_id.txt";
    if let Ok(id) = fs::read_to_string(id_file) {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let new_id = Uuid::new_v4().to_string();
    let _ = fs::write(id_file, &new_id);
    new_id
}
