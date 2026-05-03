//! `clicom whoami` — walks the parent-PID chain to identify which clicom-wrapped
//! instance we're running inside, then prints its dir name, path, pid, name, state.
//!
//! Useful when an agent (or a script the agent spawned) wants to know its own
//! clicom context — e.g. to write files into its own instance dir, or to pass
//! its dir-name as `--partial` to other clicom commands.

use anyhow::Result;
use serde_json::json;
use std::path::Path;
use sysinfo::{Pid, System};

use crate::clicom_cli::discovery::{self, InstanceInfo};

/// Walk up the process tree from `start_pid`. Return the first ancestor PID
/// that matches an instance under `<cwd>/.clicom/`.
pub fn resolve_self(cwd: &Path, start_pid: u32) -> Option<InstanceInfo> {
    let instances = discovery::list_instances(cwd);
    if instances.is_empty() {
        return None;
    }
    let mut sys = System::new();
    sys.refresh_processes();

    let mut current = Some(Pid::from_u32(start_pid));
    let mut depth = 0;
    while let Some(pid) = current {
        let pid_u32 = pid.as_u32();
        if let Some(inst) = instances.iter().find(|i| i.meta.pid == pid_u32) {
            return Some(inst.clone());
        }
        current = sys.process(pid).and_then(|p| p.parent());
        depth += 1;
        if depth > 64 {
            break;
        }
    }
    None
}

pub fn run(cwd: &Path, json_out: bool) -> Result<i32> {
    let me = match resolve_self(cwd, std::process::id()) {
        Some(i) => i,
        None => {
            eprintln!(
                "clicom whoami: not running inside a clicom-wrapped process in {}",
                cwd.display()
            );
            return Ok(2);
        }
    };
    let state_str = format!("{:?}", me.status.state).to_lowercase();
    if json_out {
        let v = json!({
            "dir_name": me.dir_name,
            "path": me.dir.display().to_string(),
            "wrapper_pid": me.meta.pid,
            "name": me.meta.name,
            "state": state_str,
            "started_at": me.meta.started_at.to_rfc3339(),
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        println!("dir_name:    {}", me.dir_name);
        println!("path:        {}", me.dir.display());
        println!("wrapper_pid: {}", me.meta.pid);
        println!("name:        {}", me.meta.name);
        println!("state:       {}", state_str);
        println!("started_at:  {}", me.meta.started_at.to_rfc3339());
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn returns_none_when_no_clicom_dir() {
        let td = TempDir::new().unwrap();
        let r = resolve_self(td.path(), std::process::id());
        assert!(r.is_none());
    }

    #[test]
    fn returns_none_when_no_ancestor_matches() {
        // Hand-craft an instance dir whose pid is unrelated to the test process.
        use crate::clicom_engine::{layout, meta::{Meta, Status}};
        let td = TempDir::new().unwrap();
        let dir = layout::instance_dir(td.path(), 4_000_000, "deadbe");
        std::fs::create_dir_all(&dir).unwrap();
        let m = Meta::new(4_000_000, "ghost".into(), vec!["x".into()], td.path().to_path_buf());
        m.write_to(&layout::meta_path(&dir)).unwrap();
        Status::initial_busy().write_to(&layout::status_path(&dir)).unwrap();
        let r = resolve_self(td.path(), std::process::id());
        assert!(r.is_none(), "no ancestor of test process should match pid 4_000_000");
    }
}
