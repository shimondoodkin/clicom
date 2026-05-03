//! Instance discovery + lazy died-detection (§5.3, §3.7.1 step 3 lazy form).

use std::path::{Path, PathBuf};

use crate::clicom_engine::layout;
use crate::clicom_engine::meta::{Meta, State, Status};
use crate::clicom_engine::process::pid_is_alive;

#[derive(Debug, Clone)]
pub struct InstanceInfo {
    pub dir: PathBuf,
    pub dir_name: String,
    pub meta: Meta,
    pub status: Status,
}

/// List every subdir under `<cwd>/.clicom/`. Tolerates corrupt dirs by skipping.
/// Performs lazy died-detection per §5.3 step 2.
pub fn list_instances(cwd: &Path) -> Vec<InstanceInfo> {
    let root = layout::dot_clicom(cwd);
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&root) { Ok(e) => e, Err(_) => return out };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue; }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let meta = match Meta::read_from(&layout::meta_path(&dir)) { Ok(m) => m, Err(_) => continue };
        let status_path = layout::status_path(&dir);
        let mut status = match Status::read_from(&status_path) { Ok(s) => s, Err(_) => continue };
        if matches!(status.state, State::Idle | State::Busy) && !pid_is_alive(meta.pid) {
            status.state = State::Died;
            status.exited_at = Some(chrono::Utc::now());
            status.exit_code = None;
            let _ = status.write_to(&status_path);
        }
        out.push(InstanceInfo { dir, dir_name, meta, status });
    }
    out
}

pub fn filter_by_partial(items: Vec<InstanceInfo>, partial: Option<&str>) -> Vec<InstanceInfo> {
    match partial {
        None => items,
        Some(p) => items.into_iter().filter(|i| layout::partial_matches(&i.dir_name, p)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_returns_no_instances() {
        let td = TempDir::new().unwrap();
        let v = list_instances(td.path());
        assert!(v.is_empty());
    }

    #[test]
    fn lazy_died_detection_rewrites_status() {
        let td = TempDir::new().unwrap();
        // Manually craft an instance dir whose pid is dead.
        // Use 4_000_000 — guaranteed not a real process on any supported platform.
        let dead_pid: u32 = 4_000_000;
        let dir = layout::instance_dir(td.path(), dead_pid, "deadbe");
        std::fs::create_dir_all(&dir).unwrap();
        let meta = Meta::new(dead_pid, "x".into(), vec!["a".into()], td.path().to_path_buf());
        meta.write_to(&layout::meta_path(&dir)).unwrap();
        Status::initial_busy().write_to(&layout::status_path(&dir)).unwrap();
        let v = list_instances(td.path());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].status.state, State::Died);
        // Verify it was persisted.
        let on_disk = Status::read_from(&layout::status_path(&dir)).unwrap();
        assert_eq!(on_disk.state, State::Died);
    }
}
