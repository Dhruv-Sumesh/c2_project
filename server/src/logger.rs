use crate::db::{Database, LogItem};
use chrono::Utc;
use tokio::sync::broadcast;
use serde_json::json;

pub fn log_info(db: &Database, tx: &broadcast::Sender<serde_json::Value>, source: &str, agent_id: Option<String>, message: &str) {
    write_log(db, tx, "INFO", source, agent_id, message);
}

pub fn log_warn(db: &Database, tx: &broadcast::Sender<serde_json::Value>, source: &str, agent_id: Option<String>, message: &str) {
    write_log(db, tx, "WARN", source, agent_id, message);
}

pub fn log_error(db: &Database, tx: &broadcast::Sender<serde_json::Value>, source: &str, agent_id: Option<String>, message: &str) {
    write_log(db, tx, "ERROR", source, agent_id, message);
}

fn write_log(
    db: &Database,
    tx: &broadcast::Sender<serde_json::Value>,
    level: &str,
    source: &str,
    agent_id: Option<String>,
    message: &str,
) {
    let timestamp = Utc::now().to_rfc3339();
    let log_item = LogItem {
        id: None,
        level: level.to_string(),
        source: source.to_string(),
        agent_id: agent_id.clone(),
        message: message.to_string(),
        timestamp: timestamp.clone(),
    };

    let formatted_agent = agent_id.as_ref().map(|id| format!(" [Agent: {}]", id)).unwrap_or_default();
    println!("[{}]{} [{}] {}: {}", timestamp, formatted_agent, level, source, message);

    if let Err(e) = db.insert_log(&log_item) {
        eprintln!("Failed to write log to database: {:?}", e);
    }

    let event = json!({
        "type": "Log",
        "payload": log_item
    });
    let _ = tx.send(event);
}
