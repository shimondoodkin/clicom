//! Retention policies (§3.7).
//! 3.7.1 — dead-instance dirs: keep 10 most recent, prune older.
//! 3.7.2 — result-triple cap: per commands/, keep 10 most recent triples.

use std::fs;
use std::path::{Path, PathBuf};

use crate::clicom_engine::layout;
use crate::clicom_engine::meta::{Meta, State, Status};
use crate::clicom_engine::process::pid_is_alive;

/// Run the dead-instance sweep over `<cwd>/.clicom/` (§3.7.1).
///
/// `self_pid` is the calling wrapper's PID, used to skip live peers.
pub fn sweep_dead_instances(cwd: &Path, self_pid: u32, keep: usize) -> anyhow::Result<()> {
    let root = layout::dot_clicom(cwd);
    if !root.exists() { return Ok(()); }
    let mut dead: Vec<(chrono::DateTime<chrono::Utc>, PathBuf)> = Vec::new();

    for entry in fs::read_dir(&root)? {
        let e = entry?;
        let p = e.path();
        if !p.is_dir() { continue; }
        let meta_path = layout::meta_path(&p);
        let status_path = layout::status_path(&p);
        let m = match Meta::read_from(&meta_path) { Ok(m) => m, Err(_) => continue };
        let alive = pid_is_alive(m.pid);
        if alive { continue; }   // skip live (own or peer)
        // Pid dead: rewrite status if it still claims idle/busy.
        if let Ok(mut s) = Status::read_from(&status_path) {
            if matches!(s.state, State::Idle | State::Busy) {
                s.state = State::Died;
                s.exited_at = Some(chrono::Utc::now());
                s.exit_code = None;
                let _ = s.write_to(&status_path);
            }
        }
        dead.push((m.started_at, p));
    }

    let _ = self_pid; // currently unused; reserved for future filtering
    dead.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, dir) in dead.into_iter().skip(keep) {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// Enforce the result-triple cap on a single live `commands/` dir (§3.7.2).
/// Keeps the `keep` most-recent triples (sorted by `<id>` ascii — drop-time order).
pub fn evict_result_triples(commands_dir: &Path, keep: usize) -> anyhow::Result<()> {
    if !commands_dir.exists() { return Ok(()); }
    let mut done_ids: Vec<String> = Vec::new();
    for entry in fs::read_dir(commands_dir)? {
        let e = entry?;
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".done") {
            done_ids.push(id.to_string());
        }
    }
    done_ids.sort();
    if done_ids.len() <= keep { return Ok(()); }
    for id in done_ids.iter().take(done_ids.len() - keep) {
        for ext in &[".out", ".err", ".done", ".log"] {
            let _ = fs::remove_file(commands_dir.join(format!("{}{}", id, ext)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(p: &Path, body: &str) { fs::write(p, body).unwrap(); }

    #[test]
    fn evict_keeps_newest_triples() {
        let td = TempDir::new().unwrap();
        let cmds = td.path().to_path_buf();
        for i in 0..12u32 {
            let id = format!("{:020}-aaaaaa", i);
            touch(&cmds.join(format!("{id}.out")), "x");
            touch(&cmds.join(format!("{id}.done")), "OK");
        }
        evict_result_triples(&cmds, 10).unwrap();
        let mut survivors: Vec<String> = fs::read_dir(&cmds).unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".done"))
            .collect();
        survivors.sort();
        assert_eq!(survivors.len(), 10);
        assert!(survivors.iter().all(|n| !n.starts_with(&format!("{:020}-aaaaaa", 0))));
        assert!(survivors.iter().all(|n| !n.starts_with(&format!("{:020}-aaaaaa", 1))));
    }

    #[test]
    fn evict_ignores_rhai_files() {
        let td = TempDir::new().unwrap();
        let cmds = td.path().to_path_buf();
        touch(&cmds.join("0001-aaaaaa.rhai"), "type_text(\"x\")");
        evict_result_triples(&cmds, 10).unwrap();
        assert!(cmds.join("0001-aaaaaa.rhai").exists());
    }
}
