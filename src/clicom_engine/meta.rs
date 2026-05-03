//! `meta.json` and `status.json` types and serialization.
//!
//! Schemas: `clicom-meta/1`, `clicom-status/1` (§3.1, §3.2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::clicom_engine::fs_atomic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub schema: String,        // "clicom-meta/1"
    pub pid: u32,
    pub name: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub started_at: DateTime<Utc>,
}

impl Meta {
    pub const SCHEMA: &'static str = "clicom-meta/1";

    pub fn new(pid: u32, name: String, command: Vec<String>, cwd: PathBuf) -> Self {
        Meta {
            schema: Self::SCHEMA.to_string(),
            pid,
            name,
            command,
            cwd,
            started_at: Utc::now(),
        }
    }

    pub fn write_to(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        fs_atomic::write(path, &json)
    }

    pub fn read_from(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State { Idle, Busy, Exited, Died }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub schema: String,        // "clicom-status/1"
    pub state: State,
    pub last_activity: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub exited_at: Option<DateTime<Utc>>,
}

impl Status {
    pub const SCHEMA: &'static str = "clicom-status/1";

    pub fn initial_busy() -> Self {
        Status {
            schema: Self::SCHEMA.to_string(),
            state: State::Busy,
            last_activity: Utc::now(),
            exit_code: None,
            exited_at: None,
        }
    }

    pub fn write_to(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        fs_atomic::write(path, &json)
    }

    pub fn read_from(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn meta_round_trips() {
        let td = TempDir::new().unwrap();
        let path = td.path().join("meta.json");
        let m = Meta::new(123, "alice".into(), vec!["claude".into(), "code".into()], td.path().to_path_buf());
        m.write_to(&path).unwrap();
        let read = Meta::read_from(&path).unwrap();
        assert_eq!(read.schema, "clicom-meta/1");
        assert_eq!(read.pid, 123);
        assert_eq!(read.name, "alice");
        assert_eq!(read.command, vec!["claude", "code"]);
    }

    #[test]
    fn status_round_trips_and_uses_lowercase_state() {
        let td = TempDir::new().unwrap();
        let path = td.path().join("status.json");
        let s = Status::initial_busy();
        s.write_to(&path).unwrap();
        let read = Status::read_from(&path).unwrap();
        assert_eq!(read.state, State::Busy);
        // Verify on-disk format uses lowercase strings (per spec example)
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"busy\""), "state should serialize as lowercase: {raw}");
    }
}
