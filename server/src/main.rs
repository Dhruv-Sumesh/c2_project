use axum::{
    routing::{get, post},
    Router,
    extract::{Path, State},
    Json,
    http::{HeaderMap, StatusCode},
};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use serde_json::Value;
use chrono::Utc;
use serde::{Deserialize, Serialize};

mod db;
mod logger;
mod registry;
mod sessions;
mod auth;
mod crypto;
mod websocket;

use db::{Database, AgentMetrics, CommandResult};
use registry::AgentRegistry;
use sessions::SessionManager;
use websocket::{ServerState, dashboard_ws_handler, PendingCommand};
use logger::{log_info, log_warn};

#[derive(Deserialize, Serialize)]
struct EncryptedEnvelope {
    payload: String,
}

#[derive(Deserialize)]
struct BeaconData {
    id: String,
    status: String,
    hostname: Option<String>,
    os: Option<String>,
    cpu_usage: Option<f64>,
    memory_usage: Option<f64>,
    disk_usage: Option<f64>,
}

#[derive(Serialize)]
struct BeaconResponse {
    success: bool,
    timestamp: String,
    commands: Vec<PendingCommand>,
}

#[derive(Deserialize)]
struct CommandResultData {
    agent_id: String,
    command_id: String,
    output: String,
    status: String,
}

#[derive(Deserialize)]
struct QueueCommandRequest {
    agent_id: String,
    command_type: String,
    payload: String,
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

fn dashboard_dir() -> &'static str {
    if std::path::Path::new("./dashboard-react/dist").exists() {
        "./dashboard-react/dist"
    } else if std::path::Path::new("../dashboard-react/dist").exists() {
        "../dashboard-react/dist"
    } else if std::path::Path::new("./dashboard").exists() {
        "./dashboard"
    } else {
        "../dashboard"
    }
}

#[tokio::main]
async fn main() {
    let db_path = "c2_simulator.db";
    let db = Database::new(db_path);

    let now_str = Utc::now().to_rfc3339();
    let _ = db.end_all_active_sessions(&now_str);

    let registry = AgentRegistry::new();
    let session_manager = SessionManager::new(db.clone());

    let (tx, _) = broadcast::channel::<Value>(1024);

    let state = ServerState {
        db: db.clone(),
        registry,
        session_manager,
        tx: tx.clone(),
        command_queue: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };

    log_info(&db, &tx, "Server", None, "Starting Educational Multi-Agent C2 Server (HTTPS Beacon Mode)...");

    let dash = dashboard_dir();
    log_info(&db, &tx, "Server", None, &format!("Serving dashboard from: {}", dash));

    // Background task: mark stale agents offline (no beacon for 90s)
    let offline_db = db.clone();
    let offline_tx = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            if let Ok(agents) = offline_db.get_agents() {
                let now = Utc::now();
                for agent in agents {
                    if agent.status != "Online" {
                        continue;
                    }
                    if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&agent.last_seen) {
                        let elapsed = now.signed_duration_since(last).num_seconds();
                        if elapsed > 90 {
                            let ts = now.to_rfc3339();
                            let _ = offline_db.update_agent_status(&agent.id, "Offline", &ts);
                            log_info(
                                &offline_db,
                                &offline_tx,
                                "Server",
                                Some(agent.id.clone()),
                                "Agent marked offline (beacon timeout)",
                            );
                            let _ = offline_tx.send(serde_json::json!({
                                "type": "AgentStatus",
                                "payload": {
                                    "id": agent.id,
                                    "status": "Offline",
                                    "last_seen": ts
                                }
                            }));
                        }
                    }
                }
            }
        }
    });

    let app = Router::new()
        .route("/api/agents", get(get_agents))
        .route("/api/agents/:id/metrics", get(get_agent_metrics))
        .route("/api/agents/:id/logs", get(get_agent_logs))
        .route("/api/agents/:id/results", get(get_command_results))
        .route("/api/logs", get(get_logs))
        .route("/api/beacon", post(receive_beacon))
        .route("/api/result", post(receive_result))
        .route("/api/command/queue", post(queue_command))
        .route("/api/dashboard/ws", get(dashboard_ws_handler))
        .fallback_service(ServeDir::new(dash))
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
    log_info(&db, &tx, "Server", None, "Beacon endpoint: POST /api/beacon (AES-GCM encrypted)");
    axum::serve(listener, app).await.unwrap();
}

async fn receive_beacon(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(envelope): Json<EncryptedEnvelope>,
) -> Result<Json<EncryptedEnvelope>, StatusCode> {
    let psk = auth::get_psk();
    let plaintext = crypto::decrypt(&envelope.payload, &psk).map_err(|e| {
        log_warn(&state.db, &state.tx, "Beacon", None, &format!("Decrypt failed: {}", e));
        StatusCode::BAD_REQUEST
    })?;

    let data: BeaconData = serde_json::from_slice(&plaintext).map_err(|_| StatusCode::BAD_REQUEST)?;

    let token = extract_bearer_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    if !auth::verify_agent_token(&data.id, &token) {
        log_warn(&state.db, &state.tx, "Beacon", Some(data.id.clone()), "Invalid auth token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let now = Utc::now().to_rfc3339();
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
    let _ = state.session_manager.start_session(&data.id);

    log_info(
        &state.db,
        &state.tx,
        "Beacon",
        Some(data.id.clone()),
        &format!("Beacon [{}] from {} ({}, {})", data.status, data.id, hostname, os),
    );

    let _ = state.tx.send(serde_json::json!({
        "type": "AgentStatus",
        "payload": {
            "id": data.id,
            "hostname": hostname,
            "os": os,
            "status": "Online",
            "last_seen": now
        }
    }));

    if let (Some(cpu), Some(mem), Some(disk)) = (data.cpu_usage, data.memory_usage, data.disk_usage) {
        let metrics = AgentMetrics {
            id: None,
            agent_id: agent.id.clone(),
            cpu_usage: cpu,
            memory_usage: mem,
            disk_usage: disk,
            timestamp: now.clone(),
        };
        let _ = state.db.insert_metrics(&metrics);
        let _ = state.tx.send(serde_json::json!({
            "type": "Metrics",
            "payload": metrics
        }));
    }

    let commands = {
        let mut queue = state.command_queue.write().await;
        queue.remove(&agent.id).unwrap_or_default()
    };

    if !commands.is_empty() {
        log_info(
            &state.db,
            &state.tx,
            "Command",
            Some(agent.id.clone()),
            &format!("Sending {} commands to agent", commands.len()),
        );
    }

    let response = BeaconResponse {
        success: true,
        timestamp: now,
        commands,
    };

    let response_bytes = serde_json::to_vec(&response).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = crypto::encrypt(&response_bytes, &psk).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(EncryptedEnvelope { payload: encrypted }))
}

async fn receive_result(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(envelope): Json<EncryptedEnvelope>,
) -> Result<Json<EncryptedEnvelope>, StatusCode> {
    let psk = auth::get_psk();
    let plaintext = crypto::decrypt(&envelope.payload, &psk).map_err(|_| StatusCode::BAD_REQUEST)?;
    let data: CommandResultData = serde_json::from_slice(&plaintext).map_err(|_| StatusCode::BAD_REQUEST)?;

    let token = extract_bearer_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    if !auth::verify_agent_token(&data.agent_id, &token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let now = Utc::now().to_rfc3339();

    let result = CommandResult {
        id: None,
        command_id: data.command_id.clone(),
        agent_id: data.agent_id.clone(),
        output: data.output.clone(),
        status: data.status.clone(),
        timestamp: now.clone(),
    };
    let _ = state.db.store_command_result(&result);

    let preview = if data.output.len() > 50 {
        &data.output[..50]
    } else {
        &data.output
    };
    log_info(
        &state.db,
        &state.tx,
        "Result",
        Some(data.agent_id.clone()),
        &format!("Command {} {}: {}", data.command_id, data.status, preview),
    );

    let _ = state.tx.send(serde_json::json!({
        "type": "CommandResult",
        "payload": {
            "agent_id": data.agent_id,
            "command_id": data.command_id,
            "status": data.status,
            "output": data.output,
            "timestamp": now
        }
    }));

    let ack = serde_json::json!({ "success": true });
    let ack_bytes = serde_json::to_vec(&ack).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = crypto::encrypt(&ack_bytes, &psk).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(EncryptedEnvelope { payload: encrypted }))
}

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
        queue
            .entry(data.agent_id.clone())
            .or_default()
            .push(command);
    }

    log_info(
        &state.db,
        &state.tx,
        "CommandQueued",
        Some(data.agent_id.clone()),
        &format!("Command {} ({}) queued for agent", cmd_id, data.command_type),
    );

    Json(serde_json::json!({
        "success": true,
        "command_id": cmd_id
    }))
}

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

async fn get_command_results(Path(id): Path<String>, State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_command_results(&id, 50) {
        Ok(results) => Json(serde_json::to_value(results).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}

async fn get_logs(State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_logs(100) {
        Ok(logs) => Json(serde_json::to_value(logs).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}
