use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use axum::extract::ws::Message;

#[derive(Clone)]
pub struct AgentRegistry {
    agents: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        AgentRegistry {
            agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, agent_id: String, tx: mpsc::UnboundedSender<Message>) {
        self.agents.lock().unwrap().insert(agent_id, tx);
    }

    pub fn unregister(&self, agent_id: &str) {
        self.agents.lock().unwrap().remove(agent_id);
    }

    pub fn send_to_agent(&self, agent_id: &str, msg: Message) -> bool {
        if let Some(tx) = self.agents.lock().unwrap().get(agent_id) {
            tx.send(msg).is_ok()
        } else {
            false
        }
    }

    pub fn is_connected(&self, agent_id: &str) -> bool {
        self.agents.lock().unwrap().contains_key(agent_id)
    }

    pub fn get_connected_agents(&self) -> Vec<String> {
        self.agents.lock().unwrap().keys().cloned().collect()
    }
}
