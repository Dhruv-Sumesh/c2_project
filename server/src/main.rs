use axum::{
    routing::{get, post},
    Router,
    extract::{Path, State},
    Json,
    http::HeaderMap,
};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
};
use std::net::SocketAddr;
use tokio::sync::{broadcast, RwLock};
use serde_json::Value;
use chrono::Utc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

mod db;
mod logger;
mod registry;
mod sessions;
mod auth;
mod websocket;

use db::{Database, Command, CommandResult};
use registry::AgentRegistry;
use sessions::SessionManager;
use websocket::{ServerState, dashboard_ws_handler}; // Removed agent_ws_handler
use logger::log_info;

#[derive(Deserialize)]
struct BeaconData {
    id: String,
    status: String,
    hostname: Option<String>,
    os: Option<String>,
}

#[derive(Deserialize)]
struct CommandResultData {
    agent_id: String,
    command_id: String,
    output: String,
    status: String, // "completed", "failed", "running"
}

#[derive(Deserialize)]
struct QueueCommandRequest {
    agent_id: String,
    command_type: String,
    payload: String,
}

#[derive(Serialize, Clone)]
struct PendingCommand {
    id: String,
    command_type: String,
    payload: String,
}

// Extended ServerState with command queue
pub struct ServerState {
    pub db: Database,
    pub registry: AgentRegistry,
    pub session_manager: SessionManager,
    pub tx: broadcast::Sender<Value>,
    pub command_queue: RwLock<HashMap<String, Vec<PendingCommand>>>, // agent_id -> commands
}

#[tokio::main]
async fn main() {
    let db_path = "c2_simulator.db";
    let db = Database::new(db_path);
    
    // Reset all agent statuses to Offline and end active sessions on startup
    let now_str = Utc::now().to_rfc3339();
    let _ = db.end_all_active_sessions(&now_str);

    let registry = AgentRegistry::new();
    let session_manager = SessionManager::new(db.clone());
    
    // Broadcast channel for real-time dashboard events
    let (tx, _) = broadcast::channel::<Value>(1024);

    let state = ServerState {
        db: db.clone(),
        registry,
        session_manager,
        tx: tx.clone(),
        command_queue: RwLock::new(HashMap::new()),
    };

    log_info(&db, &tx, "Server", None, "Starting Educational Multi-Agent C2 Server (HTTPS Beacon Mode)...");

    // Determine dashboard assets path dynamically
    let dashboard_dir = if std::path::Path::new("./dashboard").exists() {
        "./dashboard"
    } else {
        "../dashboard"
    };
    log_info(&db, &tx, "Server", None, &format!("Serving dashboard from: {}", dashboard_dir));

    let app = Router::new()
        // API routes
        .route("/api/agents", get(get_agents))
        .route("/api/agents/:id/metrics", get(get_agent_metrics))
        .route("/api/agents/:id/logs", get(get_agent_logs))
        .route("/api/logs", get(get_logs))
        // NEW: Beacon endpoint (replaces WebSocket)
        .route("/api/beacon", post(receive_beacon))
        // NEW: Command result submission
        .route("/api/result", post(receive_result))
        // NEW: Queue command for agent
        .route("/api/command/queue", post(queue_command))
        // WebSockets - REMOVED agent_ws_handler, kept dashboard
        .route("/api/dashboard/ws", get(dashboard_ws_handler))
        // Static dashboard UI serving
        .fallback_service(ServeDir::new(dashboard_dir))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!(
                "Error: port 3000 is already in use.\n\
                 Stop the existing server with: ./scripts/run-server.sh\n\
                 Or manually: lsof -tiTCP:3000 -sTCP:LISTEN | xargs kill"
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    log_info(&db, &tx, "Server", None, &format!("Server listening on http://{}", addr));
    log_info(&db, &tx, "Server", None, "Beacon endpoint: POST /api/beacon");
    axum::serve(listener, app).await.unwrap();
}

// NEW: Beacon handler - replaces WebSocket agent connection
async fn receive_beacon(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(data): Json<BeaconData>,
) -> Json<Value> {
    let now = Utc::now().to_rfc3339();
    
    // TODO: Validate token from Authorization header
    // let auth_token = headers.get("Authorization").and_then(|v| v.to_str().ok());
    
    // Update agent in database
    let hostname = data.hostname.unwrap_or_else(|| "unknown".to_string());
    let os = data.os.unwrap_or_else(|| "unknown".to_string());
    
    let agent = db::Agent {
        id: data.id.clone(),
        hostname: hostname.clone(),
        os: os.clone(),
        status: "Online".to_string(),
        last_seen: now.clone(),
    };
    
    let _ = state.db.upsert_agent(&agent);
    
    // Log beacon received
    log_info(
        &state.db,
        &state.tx,
        "Beacon",
        Some(data.id.clone()),
        &format!("Beacon received from {} ({}, {})", data.id, hostname, os)
    );
    
    // Broadcast to dashboard
    let dashboard_update = serde_json::json!({
        "type": "AgentOnline",
        "agent_id": data.id,
        "timestamp": now,
        "hostname": hostname,
        "os": os
    });
    let _ = state.tx.send(dashboard_update);
    
    // Check for pending commands
    let commands = {
        let mut queue = state.command_queue.write().await;
        queue.remove(&data.id).unwrap_or_default()
    };
    
    if !commands.is_empty() {
        log_info(
            &state.db,
            &state.tx,
            "Command",
            Some(data.id.clone()),
            &format!("Sending {} commands to agent", commands.len())
        );
    }
    
    // Return commands to agent
    Json(serde_json::json!({
        "success": true,
        "timestamp": now,
        "commands": commands
    }))
}

// NEW: Receive command results from agent
async fn receive_result(
    State(state): State<ServerState>,
    Json(data): Json<CommandResultData>,
) -> Json<Value> {
    let now = Utc::now().to_rfc3339();
    
    // Store result in database
    let result = CommandResult {
        command_id: data.command_id.clone(),
        agent_id: data.agent_id.clone(),
        output: data.output.clone(),
        status: data.status.clone(),
        timestamp: now.clone(),
    };
    
    let _ = state.db.store_command_result(&result);
    
    log_info(
        &state.db,
        &state.tx,
        "Result",
        Some(data.agent_id.clone()),
        &format!("Command {} {}: {}", data.command_id, data.status, 
                if data.output.len() > 50 { &data.output[..50] } else { &data.output })
    );
    
    // Broadcast to dashboard
    let dashboard_update = serde_json::json!({
        "type": "CommandResult",
        "agent_id": data.agent_id,
        "command_id": data.command_id,
        "status": data.status,
        "timestamp": now
    });
    let _ = state.tx.send(dashboard_update);
    
    Json(serde_json::json!({ "success": true }))
}

// NEW: Operator endpoint to queue commands for agents
async fn queue_command(
    State(state): State<ServerState>,
    Json(data): Json<QueueCommandRequest>,
) -> Json<Value> {
    let cmd_id = uuid::Uuid::new_v4().to_string();
    
    let command = PendingCommand {
        id: cmd_id.clone(),
        command_type: data.command_type.clone(),
        payload: data.payload.clone(),
    };
    
    {
        let mut queue = state.command_queue.write().await;
        queue.entry(data.agent_id.clone())
            .or_insert_with(Vec::new)
            .push(command);
    }
    
    log_info(
        &state.db,
        &state.tx,
        "CommandQueued",
        Some(data.agent_id.clone()),
        &format!("Command {} ({}) queued for agent", cmd_id, data.command_type)
    );
    
    Json(serde_json::json!({
        "success": true,
        "command_id": cmd_id
    }))
}

// Existing handlers remain unchanged
async fn get_agents(State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_agents() {
        Ok(agents) => Json(serde_json::to_value(agents).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}

async fn get_agent_metrics(Path(id): Path<String>, State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_agent_metrics(&id, 100) {
        Ok(metrics) => Json(serde_json::to_value(metrics).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}

async fn get_agent_logs(Path(id): Path<String>, State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_agent_logs(&id, 100) {
        Ok(logs) => Json(serde_json::to_value(logs).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}

async fn get_logs(State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_logs(100) {
        Ok(logs) => Json(serde_json::to_value(logs).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}