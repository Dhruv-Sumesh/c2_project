use axum::{
    routing::get,
    Router,
    extract::{Path, State},
    Json,
};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use serde_json::Value;
use chrono::Utc;

mod db;
mod logger;
mod registry;
mod sessions;
mod auth;
mod websocket;

use db::Database;
use registry::AgentRegistry;
use sessions::SessionManager;
use websocket::{ServerState, agent_ws_handler, dashboard_ws_handler};
use logger::log_info;

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
    };

    log_info(&db, &tx, "Server", None, "Starting Educational Multi-Agent C2 Server...");

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
        // WebSockets
        .route("/api/agent/ws", get(agent_ws_handler))
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
    axum::serve(listener, app).await.unwrap();
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

async fn get_logs(State(state): State<ServerState>) -> Json<Value> {
    match state.db.get_logs(100) {
        Ok(logs) => Json(serde_json::to_value(logs).unwrap_or(Value::Null)),
        Err(_) => Json(Value::Null),
    }
}
