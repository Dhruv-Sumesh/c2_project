use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

/// Default beacon polling interval in seconds (server-wide fallback).
pub const DEFAULT_BEACON_INTERVAL_SECS: u64 = 30;
/// Minimum allowed beacon interval enforced on server and agent.
pub const MIN_BEACON_INTERVAL_SECS: u64 = 5;
/// Maximum allowed beacon interval enforced on server and agent.
pub const MAX_BEACON_INTERVAL_SECS: u64 = 3600;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Agent {
    pub id: String,
    pub hostname: String,
    pub os: String,
    pub status: String,
    pub last_seen: String,
    #[serde(default = "default_beacon_interval")]
    pub beacon_interval_secs: u64,
}

fn default_beacon_interval() -> u64 {
    DEFAULT_BEACON_INTERVAL_SECS
}

/// Clamp and validate a requested beacon interval.
pub fn validate_beacon_interval(secs: u64) -> Result<u64, String> {
    if secs < MIN_BEACON_INTERVAL_SECS || secs > MAX_BEACON_INTERVAL_SECS {
        return Err(format!(
            "interval must be between {} and {} seconds",
            MIN_BEACON_INTERVAL_SECS, MAX_BEACON_INTERVAL_SECS
        ));
    }
    Ok(secs)
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommandResult {
    pub id: Option<i64>,
    pub command_id: String,
    pub agent_id: String,
    pub output: String,
    pub status: String,
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
                last_seen TEXT NOT NULL,
                beacon_interval_secs INTEGER NOT NULL DEFAULT 30
            )",
            [],
        ).expect("failed to create agents table");

        // Migrate older databases that lack the interval column.
        let _ = conn.execute(
            "ALTER TABLE agents ADD COLUMN beacon_interval_secs INTEGER NOT NULL DEFAULT 30",
            [],
        );

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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS command_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                output TEXT NOT NULL,
                status TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )",
            [],
        ).expect("failed to create command_results table");
    }

    pub fn upsert_agent(&self, agent: &Agent) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agents (id, hostname, os, status, last_seen, beacon_interval_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                hostname = excluded.hostname,
                os = excluded.os,
                status = excluded.status,
                last_seen = excluded.last_seen,
                beacon_interval_secs = excluded.beacon_interval_secs",
            params![
                agent.id,
                agent.hostname,
                agent.os,
                agent.status,
                agent.last_seen,
                agent.beacon_interval_secs,
            ],
        )?;
        Ok(())
    }

    pub fn update_beacon_interval(&self, agent_id: &str, interval_secs: u64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agents SET beacon_interval_secs = ?1 WHERE id = ?2",
            params![interval_secs, agent_id],
        )?;
        Ok(())
    }

    pub fn get_beacon_interval(&self, agent_id: &str) -> Result<u64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT beacon_interval_secs FROM agents WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![agent_id])?;
        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(DEFAULT_BEACON_INTERVAL_SECS)
        }
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
        let mut stmt = conn.prepare(
            "SELECT id, hostname, os, status, last_seen, beacon_interval_secs FROM agents ORDER BY last_seen DESC",
        )?;
        let agent_iter = stmt.query_map([], |row| {
            Ok(Agent {
                id: row.get(0)?,
                hostname: row.get(1)?,
                os: row.get(2)?,
                status: row.get(3)?,
                last_seen: row.get(4)?,
                beacon_interval_secs: row.get(5)?,
            })
        })?;

        let mut agents = Vec::new();
        for agent in agent_iter {
            agents.push(agent?);
        }
        Ok(agents)
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

    pub fn store_command_result(&self, result: &CommandResult) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO command_results (command_id, agent_id, output, status, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                result.command_id,
                result.agent_id,
                result.output,
                result.status,
                result.timestamp,
            ],
        )?;
        Ok(())
    }

    pub fn get_command_results(&self, agent_id: &str, limit: usize) -> Result<Vec<CommandResult>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, command_id, agent_id, output, status, timestamp 
             FROM command_results 
             WHERE agent_id = ?1 
             ORDER BY timestamp DESC 
             LIMIT ?2"
        )?;
        let result_iter = stmt.query_map(params![agent_id, limit], |row| {
            Ok(CommandResult {
                id: Some(row.get(0)?),
                command_id: row.get(1)?,
                agent_id: row.get(2)?,
                output: row.get(3)?,
                status: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut results = Vec::new();
        for result in result_iter {
            results.push(result?);
        }
        Ok(results)
    }
}