use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Agent {
    pub id: String,
    pub hostname: String,
    pub os: String,
    pub status: String,
    pub last_seen: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentMetrics {
    pub id: Option<i64>,
    pub agent_id: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogItem {
    pub id: Option<i64>,
    pub level: String,
    pub source: String,
    pub agent_id: Option<String>,
    pub message: String,
    pub timestamp: String,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Self {
        let conn = Connection::open(path).expect("failed to open database");
        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema();
        db
    }

    fn init_schema(&self) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                hostname TEXT NOT NULL,
                os TEXT NOT NULL,
                status TEXT NOT NULL,
                last_seen TEXT NOT NULL
            )",
            [],
        ).expect("failed to create agents table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                cpu_usage REAL NOT NULL,
                memory_usage REAL NOT NULL,
                disk_usage REAL NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )",
            [],
        ).expect("failed to create metrics table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )",
            [],
        ).expect("failed to create sessions table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                level TEXT NOT NULL,
                source TEXT NOT NULL,
                agent_id TEXT,
                message TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        ).expect("failed to create logs table");
    }

    pub fn upsert_agent(&self, agent: &Agent) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agents (id, hostname, os, status, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                hostname = excluded.hostname,
                os = excluded.os,
                status = excluded.status,
                last_seen = excluded.last_seen",
            params![agent.id, agent.hostname, agent.os, agent.status, agent.last_seen],
        )?;
        Ok(())
    }

    pub fn update_agent_status(&self, agent_id: &str, status: &str, last_seen: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agents SET status = ?1, last_seen = ?2 WHERE id = ?3",
            params![status, last_seen, agent_id],
        )?;
        Ok(())
    }

    pub fn get_agents(&self) -> Result<Vec<Agent>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, hostname, os, status, last_seen FROM agents ORDER BY last_seen DESC")?;
        let agent_iter = stmt.query_map([], |row| {
            Ok(Agent {
                id: row.get(0)?,
                hostname: row.get(1)?,
                os: row.get(2)?,
                status: row.get(3)?,
                last_seen: row.get(4)?,
            })
        })?;

        let mut agents = Vec::new();
        for agent in agent_iter {
            agents.push(agent?);
        }
        Ok(agents)
    }

    pub fn get_agent(&self, id: &str) -> Result<Option<Agent>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, hostname, os, status, last_seen FROM agents WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Agent {
                id: row.get(0)?,
                hostname: row.get(1)?,
                os: row.get(2)?,
                status: row.get(3)?,
                last_seen: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn insert_metrics(&self, metrics: &AgentMetrics) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agent_metrics (agent_id, cpu_usage, memory_usage, disk_usage, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![metrics.agent_id, metrics.cpu_usage, metrics.memory_usage, metrics.disk_usage, metrics.timestamp],
        )?;
        Ok(())
    }

    pub fn get_agent_metrics(&self, agent_id: &str, limit: usize) -> Result<Vec<AgentMetrics>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, cpu_usage, memory_usage, disk_usage, timestamp 
             FROM agent_metrics 
             WHERE agent_id = ?1 
             ORDER BY timestamp DESC 
             LIMIT ?2"
        )?;
        let metric_iter = stmt.query_map(params![agent_id, limit], |row| {
            Ok(AgentMetrics {
                id: Some(row.get(0)?),
                agent_id: row.get(1)?,
                cpu_usage: row.get(2)?,
                memory_usage: row.get(3)?,
                disk_usage: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut metrics = Vec::new();
        for metric in metric_iter {
            metrics.push(metric?);
        }
        // Return in chronological order
        metrics.reverse();
        Ok(metrics)
    }

    pub fn insert_session(&self, session: &Session) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, agent_id, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![session.id, session.agent_id, session.started_at, session.ended_at],
        )?;
        Ok(())
    }

    pub fn end_session(&self, session_id: &str, ended_at: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE id = ?2",
            params![ended_at, session_id],
        )?;
        Ok(())
    }

    pub fn end_all_active_sessions(&self, ended_at: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE ended_at IS NULL",
            params![ended_at],
        )?;
        // Also mark all agents as offline on server startup (reboot)
        conn.execute(
            "UPDATE agents SET status = 'Offline'",
            [],
        )?;
        Ok(())
    }

    pub fn insert_log(&self, log: &LogItem) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO logs (level, source, agent_id, message, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![log.level, log.source, log.agent_id, log.message, log.timestamp],
        )?;
        Ok(())
    }

    pub fn get_logs(&self, limit: usize) -> Result<Vec<LogItem>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, level, source, agent_id, message, timestamp 
             FROM logs 
             ORDER BY timestamp DESC 
             LIMIT ?1"
        )?;
        let log_iter = stmt.query_map(params![limit], |row| {
            Ok(LogItem {
                id: Some(row.get(0)?),
                level: row.get(1)?,
                source: row.get(2)?,
                agent_id: row.get(3)?,
                message: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut logs = Vec::new();
        for log in log_iter {
            logs.push(log?);
        }
        Ok(logs)
    }

    pub fn get_agent_logs(&self, agent_id: &str, limit: usize) -> Result<Vec<LogItem>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, level, source, agent_id, message, timestamp 
             FROM logs 
             WHERE agent_id = ?1 
             ORDER BY timestamp DESC 
             LIMIT ?2"
        )?;
        let log_iter = stmt.query_map(params![agent_id, limit], |row| {
            Ok(LogItem {
                id: Some(row.get(0)?),
                level: row.get(1)?,
                source: row.get(2)?,
                agent_id: row.get(3)?,
                message: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut logs = Vec::new();
        for log in log_iter {
            logs.push(log?);
        }
        Ok(logs)
    }
}
