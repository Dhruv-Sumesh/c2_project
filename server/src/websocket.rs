use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use tokio::sync::{broadcast, RwLock};
use serde_json::{json, Value};
use futures_util::{StreamExt, SinkExt};
use std::collections::HashMap;
use std::sync::Arc;
use crate::db::Database;
use crate::sessions::SessionManager;
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
    pub session_manager: SessionManager,
    pub tx: broadcast::Sender<Value>,
    pub command_queue: Arc<RwLock<HashMap<String, Vec<PendingCommand>>>>,
    pub session_keys: Arc<RwLock<HashMap<String, String>>>,
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
        while let Some(Ok(_)) = ws_receiver.next().await {}
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
