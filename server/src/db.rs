use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

pub const DEFAULT_BEACON_INTERVAL_SECS: u64 = 30;
pub const MIN_BEACON_INTERVAL_SECS: u64 = 5;
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PayloadUpload {
    pub id: String,
    pub file_name: String,
    pub file_size: i64,
    pub status: String,
    pub uploaded_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentBuild {
    pub id: String,
    pub target_os: String,
    pub server_url: String,
    pub psk: String,
    pub beacon_interval: i64,
    pub file_path: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileTransfer {
    pub id: String,
    pub agent_id: String,
    pub direction: String,
    pub file_path: String,
    pub file_size: i64,
    pub chunks_total: i64,
    pub chunks_received: i64,
    pub checksum: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BroadcastHistory {
    pub id: String,
    pub command: String,
    pub filters: String,
    pub agent_count: i64,
    pub created_at: String,
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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS payload_uploads (
                id TEXT PRIMARY KEY,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                status TEXT NOT NULL,
                uploaded_at TEXT NOT NULL,
                storage_path TEXT NOT NULL
            )",
            [],
        ).expect("failed to create payload_uploads table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_builds (
                id TEXT PRIMARY KEY,
                target_os TEXT NOT NULL,
                server_url TEXT NOT NULL,
                psk TEXT NOT NULL,
                beacon_interval INTEGER NOT NULL,
                file_path TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'building',
                created_at TEXT NOT NULL
            )",
            [],
        ).expect("failed to create agent_builds table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_transfers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                direction TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                chunks_total INTEGER NOT NULL DEFAULT 0,
                chunks_received INTEGER NOT NULL DEFAULT 0,
                checksum TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                completed_at TEXT,
                FOREIGN KEY (agent_id) REFERENCES agents (id)
            )",
            [],
        ).expect("failed to create file_transfers table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS broadcast_history (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                filters TEXT NOT NULL,
                agent_count INTEGER NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        ).expect("failed to create broadcast_history table");
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

    pub fn insert_payload_upload(
        &self,
        upload: &PayloadUpload,
        storage_path: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO payload_uploads (id, file_name, file_size, status, uploaded_at, storage_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                upload.id,
                upload.file_name,
                upload.file_size,
                upload.status,
                upload.uploaded_at,
                storage_path,
            ],
        )?;
        Ok(())
    }

    pub fn get_payload_uploads(&self, limit: usize) -> Result<Vec<PayloadUpload>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_name, file_size, status, uploaded_at
             FROM payload_uploads
             ORDER BY uploaded_at DESC
             LIMIT ?1",
        )?;
        let iter = stmt.query_map(params![limit], |row| {
            Ok(PayloadUpload {
                id: row.get(0)?,
                file_name: row.get(1)?,
                file_size: row.get(2)?,
                status: row.get(3)?,
                uploaded_at: row.get(4)?,
            })
        })?;

        let mut uploads = Vec::new();
        for upload in iter {
            uploads.push(upload?);
        }
        Ok(uploads)
    }

    pub fn update_payload_status(&self, id: &str, status: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE payload_uploads SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn insert_agent_build(&self, build: &AgentBuild) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agent_builds (id, target_os, server_url, psk, beacon_interval, file_path, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                build.id,
                build.target_os,
                build.server_url,
                build.psk,
                build.beacon_interval,
                build.file_path,
                build.status,
                build.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_agent_build_status(
        &self,
        id: &str,
        status: &str,
        file_path: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        if let Some(path) = file_path {
            conn.execute(
                "UPDATE agent_builds SET status = ?1, file_path = ?2 WHERE id = ?3",
                params![status, path, id],
            )?;
        } else {
            conn.execute(
                "UPDATE agent_builds SET status = ?1 WHERE id = ?2",
                params![status, id],
            )?;
        }
        Ok(())
    }

    pub fn get_agent_builds(&self, limit: usize) -> Result<Vec<AgentBuild>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, target_os, server_url, psk, beacon_interval, file_path, status, created_at
             FROM agent_builds ORDER BY created_at DESC LIMIT ?1",
        )?;
        let iter = stmt.query_map(params![limit], |row| {
            Ok(AgentBuild {
                id: row.get(0)?,
                target_os: row.get(1)?,
                server_url: row.get(2)?,
                psk: row.get(3)?,
                beacon_interval: row.get(4)?,
                file_path: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut builds = Vec::new();
        for b in iter {
            builds.push(b?);
        }
        Ok(builds)
    }

    pub fn get_agent_build(&self, id: &str) -> Result<Option<AgentBuild>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, target_os, server_url, psk, beacon_interval, file_path, status, created_at
             FROM agent_builds WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(AgentBuild {
                id: row.get(0)?,
                target_os: row.get(1)?,
                server_url: row.get(2)?,
                psk: row.get(3)?,
                beacon_interval: row.get(4)?,
                file_path: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }


    pub fn insert_file_transfer(&self, transfer: &FileTransfer) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO file_transfers (id, agent_id, direction, file_path, file_size, chunks_total, chunks_received, checksum, status, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                transfer.id,
                transfer.agent_id,
                transfer.direction,
                transfer.file_path,
                transfer.file_size,
                transfer.chunks_total,
                transfer.chunks_received,
                transfer.checksum,
                transfer.status,
                transfer.created_at,
                transfer.completed_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_file_transfer_progress(
        &self,
        id: &str,
        chunks_received: i64,
        status: &str,
        completed_at: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE file_transfers SET chunks_received = ?1, status = ?2, completed_at = ?3 WHERE id = ?4",
            params![chunks_received, status, completed_at, id],
        )?;
        Ok(())
    }

    pub fn get_file_transfer(&self, id: &str) -> Result<Option<FileTransfer>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, direction, file_path, file_size, chunks_total, chunks_received, checksum, status, created_at, completed_at
             FROM file_transfers WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(FileTransfer {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                direction: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                chunks_total: row.get(5)?,
                chunks_received: row.get(6)?,
                checksum: row.get(7)?,
                status: row.get(8)?,
                created_at: row.get(9)?,
                completed_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_file_transfers_for_agent(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<FileTransfer>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, direction, file_path, file_size, chunks_total, chunks_received, checksum, status, created_at, completed_at
             FROM file_transfers WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let iter = stmt.query_map(params![agent_id, limit], |row| {
            Ok(FileTransfer {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                direction: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                chunks_total: row.get(5)?,
                chunks_received: row.get(6)?,
                checksum: row.get(7)?,
                status: row.get(8)?,
                created_at: row.get(9)?,
                completed_at: row.get(10)?,
            })
        })?;
        let mut transfers = Vec::new();
        for t in iter {
            transfers.push(t?);
        }
        Ok(transfers)
    }


    pub fn insert_broadcast(&self, record: &BroadcastHistory) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO broadcast_history (id, command, filters, agent_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.id,
                record.command,
                record.filters,
                record.agent_count,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_broadcast_history(&self, limit: usize) -> Result<Vec<BroadcastHistory>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, command, filters, agent_count, created_at
             FROM broadcast_history ORDER BY created_at DESC LIMIT ?1",
        )?;
        let iter = stmt.query_map(params![limit], |row| {
            Ok(BroadcastHistory {
                id: row.get(0)?,
                command: row.get(1)?,
                filters: row.get(2)?,
                agent_count: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut records = Vec::new();
        for r in iter {
            records.push(r?);
        }
        Ok(records)
    }
}