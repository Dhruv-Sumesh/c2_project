//! Educational file transfer service — chunked upload/download between dashboard and agents.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::db::FileTransfer;
use crate::logger::log_info;
use crate::websocket::{PendingCommand, ServerState};
use crate::auth;
use crate::crypto;

const CHUNK_SIZE: usize = 64 * 1024;
const TRANSFERS_DIR: &str = "./transfers";

#[derive(Deserialize)]
pub struct InitiateDownloadRequest {
    pub file_path: String,
}

#[derive(Deserialize)]
struct ChunkPayload {
    transfer_id: String,
    agent_id: String,
    chunk_index: usize,
    chunks_total: Option<usize>,
    data_b64: String,
    checksum: String,
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn chunk_file(data: &[u8]) -> (usize, String) {
    let chunks_total = data.len().div_ceil(CHUNK_SIZE);
    (chunks_total, sha256_hex(data))
}

async fn ensure_transfers_dir() -> Result<(), StatusCode> {
    tokio::fs::create_dir_all(TRANSFERS_DIR)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn transfer_storage_path(transfer_id: &str) -> String {
    format!("{}/{}", TRANSFERS_DIR, transfer_id)
}

/// POST /api/files/upload/:agent_id — push a server-side file to an agent.
pub async fn initiate_upload_to_agent(
    State(state): State<ServerState>,
    Path(agent_id): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<Value>, StatusCode> {
    ensure_transfers_dir().await?;

    let mut file_bytes: Vec<u8> = Vec::new();
    let mut file_name = "upload.bin".to_string();
    let mut dest_path = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        match field.name() {
            Some("file") => {
                file_name = field
                    .file_name()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "upload.bin".to_string());
                file_bytes = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?.to_vec();
            }
            Some("dest_path") => {
                dest_path = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            }
            _ => {}
        }
    }

    if file_bytes.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if dest_path.is_empty() {
        dest_path = file_name.clone();
    }

    let transfer_id = uuid::Uuid::new_v4().to_string();
    let (chunks_total, checksum) = chunk_file(&file_bytes);
    let now = Utc::now().to_rfc3339();

    let storage = transfer_storage_path(&transfer_id);
    tokio::fs::write(&storage, &file_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let transfer = FileTransfer {
        id: transfer_id.clone(),
        agent_id: agent_id.clone(),
        direction: "upload".to_string(),
        file_path: dest_path.clone(),
        file_size: file_bytes.len() as i64,
        chunks_total: chunks_total as i64,
        chunks_received: 0,
        checksum: checksum.clone(),
        status: "pending".to_string(),
        created_at: now,
        completed_at: None,
    };

    state
        .db
        .insert_file_transfer(&transfer)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cmd_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "transfer_id": transfer_id,
        "dest_path": dest_path,
        "chunks_total": chunks_total,
        "checksum": checksum,
    })
    .to_string();

    let command = PendingCommand {
        id: cmd_id,
        command_type: "file_upload".to_string(),
        payload,
    };

    {
        let mut queue = state.command_queue.write().await;
        queue.entry(agent_id.clone()).or_default().push(command);
    }

    log_info(
        &state.db,
        &state.tx,
        "FileTransfer",
        Some(agent_id.clone()),
        &format!(
            "Queued upload {} ({} bytes, {} chunks)",
            transfer.id, file_bytes.len(), chunks_total
        ),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "transfer_id": transfer.id,
        "chunks_total": chunks_total,
        "file_size": file_bytes.len(),
    })))
}

/// POST /api/files/download/:agent_id — request a file from an agent.
pub async fn initiate_download_from_agent(
    State(state): State<ServerState>,
    Path(agent_id): Path<String>,
    Json(body): Json<InitiateDownloadRequest>,
) -> Result<Json<Value>, StatusCode> {
    ensure_transfers_dir().await?;

    if body.file_path.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let transfer_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let transfer = FileTransfer {
        id: transfer_id.clone(),
        agent_id: agent_id.clone(),
        direction: "download".to_string(),
        file_path: body.file_path.clone(),
        file_size: 0,
        chunks_total: 0,
        chunks_received: 0,
        checksum: String::new(),
        status: "in_progress".to_string(),
        created_at: now,
        completed_at: None,
    };

    state
        .db
        .insert_file_transfer(&transfer)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cmd_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "transfer_id": transfer_id,
        "file_path": body.file_path,
    })
    .to_string();

    let command = PendingCommand {
        id: cmd_id,
        command_type: "file_download".to_string(),
        payload,
    };

    {
        let mut queue = state.command_queue.write().await;
        queue.entry(agent_id.clone()).or_default().push(command);
    }

    log_info(
        &state.db,
        &state.tx,
        "FileTransfer",
        Some(agent_id),
        &format!("Requested download {} for {}", transfer_id, body.file_path),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "transfer_id": transfer_id,
    })))
}

/// GET /api/files/:transfer_id — transfer status and metadata.
pub async fn get_transfer_status(
    State(state): State<ServerState>,
    Path(transfer_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let transfer = state
        .db
        .get_file_transfer(&transfer_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let progress = if transfer.chunks_total > 0 {
        transfer.chunks_received as f64 / transfer.chunks_total as f64
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "id": transfer.id,
        "agent_id": transfer.agent_id,
        "direction": transfer.direction,
        "file_path": transfer.file_path,
        "file_size": transfer.file_size,
        "chunks_total": transfer.chunks_total,
        "chunks_received": transfer.chunks_received,
        "checksum": transfer.checksum,
        "status": transfer.status,
        "progress": progress,
        "created_at": transfer.created_at,
        "completed_at": transfer.completed_at,
    })))
}

/// GET /api/files/:transfer_id/chunks/:chunk_index — agent fetches upload chunk (encrypted).
pub async fn get_transfer_chunk(
    State(state): State<ServerState>,
    Path((transfer_id, chunk_index)): Path<(String, usize)>,
    headers: HeaderMap,
) -> Result<Json<crate::EncryptedEnvelope>, StatusCode> {
    let agent_id = headers
        .get("X-Agent-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth::verify_agent_token(agent_id, token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let transfer = state
        .db
        .get_file_transfer(&transfer_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if transfer.agent_id != agent_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let storage = transfer_storage_path(&transfer_id);
    let data = tokio::fs::read(&storage)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let start = chunk_index * CHUNK_SIZE;
    if start >= data.len() {
        return Err(StatusCode::NOT_FOUND);
    }
    let end = (start + CHUNK_SIZE).min(data.len());
    let chunk = &data[start..end];
    let data_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, chunk);
    let checksum = sha256_hex(chunk);

    let keys = state.session_keys.read().await;
    let hex_key = keys.get(agent_id).ok_or(StatusCode::UNAUTHORIZED)?;
    let session_key = crypto::key_from_hex(hex_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let body = serde_json::json!({ "data_b64": data_b64, "checksum": checksum });
    let bytes = serde_json::to_vec(&body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let encrypted = crypto::encrypt(&bytes, &session_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(crate::EncryptedEnvelope { payload: encrypted }))
}

/// POST /api/files/chunk — agent posts download chunk (encrypted).
pub async fn receive_agent_chunk(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(envelope): Json<crate::EncryptedEnvelope>,
) -> Result<Json<Value>, StatusCode> {
    let agent_id = headers
        .get("X-Agent-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_string();

    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth::verify_agent_token(&agent_id, token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let keys = state.session_keys.read().await;
    let hex_key = keys.get(&agent_id).ok_or(StatusCode::UNAUTHORIZED)?;
    let session_key = crypto::key_from_hex(hex_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let plaintext = crypto::decrypt(&envelope.payload, &session_key).map_err(|_| StatusCode::BAD_REQUEST)?;
    let chunk: ChunkPayload = serde_json::from_slice(&plaintext).map_err(|_| StatusCode::BAD_REQUEST)?;

    if chunk.agent_id != agent_id {
        return Err(StatusCode::FORBIDDEN);
    }

    ensure_transfers_dir().await?;
    let storage = transfer_storage_path(&chunk.transfer_id);

    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &chunk.data_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if sha256_hex(&bytes) != chunk.checksum {
        return Err(StatusCode::BAD_REQUEST);
    }

    if chunk.chunk_index == 0 {
        tokio::fs::write(&storage, &bytes)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&storage)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        file.write_all(&bytes)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let transfer = state
        .db
        .get_file_transfer(&chunk.transfer_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let received = (chunk.chunk_index + 1) as i64;
    let total = chunk
        .chunks_total
        .map(|t| t as i64)
        .filter(|&t| t > 0)
        .unwrap_or(transfer.chunks_total.max(received));

    let progress = if total > 0 {
        received as f32 / total as f32
    } else {
        0.0
    };

    let status = if total > 0 && received >= total {
        "completed"
    } else {
        "in_progress"
    };

    let completed_at = if status == "completed" {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };

    state
        .db
        .update_file_transfer_progress(&chunk.transfer_id, received, status, completed_at.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = state.tx.send(serde_json::json!({
        "type": "FileTransferProgress",
        "payload": {
            "agent_id": agent_id,
            "transfer_id": chunk.transfer_id,
            "progress": progress,
            "chunks_received": received,
            "chunks_total": total,
            "status": status,
        }
    }));

    if status == "completed" {
        let _ = state.tx.send(serde_json::json!({
            "type": "FileTransferComplete",
            "payload": {
                "agent_id": agent_id,
                "transfer_id": chunk.transfer_id,
                "success": true,
            }
        }));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// GET /api/files/agent/:agent_id — list transfers for an agent.
pub async fn list_agent_transfers(
    State(state): State<ServerState>,
    Path(agent_id): Path<String>,
) -> Json<Value> {
    match state.db.get_file_transfers_for_agent(&agent_id, 50) {
        Ok(transfers) => Json(serde_json::to_value(transfers).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}
