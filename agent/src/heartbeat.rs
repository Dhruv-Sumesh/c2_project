use serde_json::{json, Value};

pub fn generate_heartbeat_payload(agent_id: &str) -> Value {
    json!({
        "type": "Heartbeat",
        "payload": {
            "agent_id": agent_id
        }
    })
}
