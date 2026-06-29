use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::Utc;
use crate::db::{Database, Session};

#[derive(Clone)]
pub struct SessionManager {
    db: Database,
    active_sessions: Arc<Mutex<HashMap<String, String>>>, // Agent ID -> Session ID
}

impl SessionManager {
    pub fn new(db: Database) -> Self {
        SessionManager {
            db,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_session(&self, agent_id: &str) -> Result<String, rusqlite::Error> {
        if let Some(existing) = self.active_sessions.lock().unwrap().get(agent_id) {
            return Ok(existing.clone());
        }

        let session_id = Uuid::new_v4().to_string();
        let started_at = Utc::now().to_rfc3339();

        let session = Session {
            id: session_id.clone(),
            agent_id: agent_id.to_string(),
            started_at,
            ended_at: None,
        };

        self.db.insert_session(&session)?;
        self.active_sessions.lock().unwrap().insert(agent_id.to_string(), session_id.clone());

        Ok(session_id)
    }

    pub fn end_session(&self, agent_id: &str) -> Result<Option<String>, rusqlite::Error> {
        let session_id = self.active_sessions.lock().unwrap().remove(agent_id);
        if let Some(ref id) = session_id {
            let ended_at = Utc::now().to_rfc3339();
            self.db.end_session(id, &ended_at)?;
        }
        Ok(session_id)
    }
}
