pub mod rhai_host;
pub mod ids;
pub mod fs_atomic;
pub mod meta;
pub mod layout;
pub mod process;
pub mod gitignore;
pub mod screen;
pub mod idle;
pub mod nudge;
pub mod pty;
pub mod nopty;
pub mod console_mode;
pub mod status_trailer;
pub mod forwarding;
pub mod retention;
pub mod mouse_filter;
pub mod watcher;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::clicom_engine::meta::{Meta, State, Status};

#[derive(Debug, Clone)]
pub enum SpawnMode {
    Pty { strip_mouse: bool },
    NoPty,
}

/// Orchestrates one `<cwd>/.clicom/<pid>-<rand6>/` lifecycle (§6.2).
pub struct ClicomChannel {
    pub instance_dir: PathBuf,
    pub meta: Meta,
    pub status: Arc<Mutex<Status>>,
}

impl ClicomChannel {
    /// Create the on-disk layout and write initial meta+status. Does NOT spawn the child;
    /// that is the caller's responsibility (§5.1 step 5). The child + threads are wired
    /// by `cmd_start` in M1 / Task 18.
    pub fn create(cwd: &std::path::Path, pid: u32, name: String, command: Vec<String>) -> anyhow::Result<Self> {
        let rand6 = ids::rand6();
        let instance_dir = layout::instance_dir(cwd, pid, &rand6);
        std::fs::create_dir_all(&instance_dir)?;
        std::fs::create_dir_all(layout::commands_dir(&instance_dir))?;
        // Touch the lock file so writers can lock it.
        std::fs::OpenOptions::new().create(true).write(true).open(layout::lock_path(&instance_dir))?;

        let meta = Meta::new(pid, name, command, cwd.to_path_buf());
        meta.write_to(&layout::meta_path(&instance_dir))?;

        let status = Status::initial_busy();
        status.write_to(&layout::status_path(&instance_dir))?;

        Ok(ClicomChannel {
            instance_dir,
            meta,
            status: Arc::new(Mutex::new(status)),
        })
    }

    pub fn set_state(&self, state: State) -> anyhow::Result<()> {
        let mut s = self.status.lock().map_err(|_| anyhow::anyhow!("status mutex poisoned"))?;
        s.state = state;
        s.last_activity = chrono::Utc::now();
        s.write_to(&layout::status_path(&self.instance_dir))
    }

    pub fn write_screen(&self, content: &str) -> anyhow::Result<()> {
        crate::clicom_engine::fs_atomic::write(&layout::screen_path(&self.instance_dir), content.as_bytes())
    }

    pub fn on_shutdown(&self, exit_code: i32) -> anyhow::Result<()> {
        let mut s = self.status.lock().map_err(|_| anyhow::anyhow!("status mutex poisoned"))?;
        s.state = State::Exited;
        s.exit_code = Some(exit_code);
        s.exited_at = Some(chrono::Utc::now());
        s.write_to(&layout::status_path(&self.instance_dir))
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_writes_layout_files() {
        let td = TempDir::new().unwrap();
        let ch = ClicomChannel::create(td.path(), 999, "alice".into(), vec!["echo".into(), "hi".into()]).unwrap();
        assert!(ch.instance_dir.starts_with(td.path().join(".clicom")));
        assert!(layout::meta_path(&ch.instance_dir).exists());
        assert!(layout::status_path(&ch.instance_dir).exists());
        assert!(layout::lock_path(&ch.instance_dir).exists());
        assert!(layout::commands_dir(&ch.instance_dir).is_dir());
        let m = Meta::read_from(&layout::meta_path(&ch.instance_dir)).unwrap();
        assert_eq!(m.pid, 999);
        assert_eq!(m.name, "alice");
    }

    #[test]
    fn set_state_persists() {
        let td = TempDir::new().unwrap();
        let ch = ClicomChannel::create(td.path(), 1, "x".into(), vec!["a".into()]).unwrap();
        ch.set_state(State::Idle).unwrap();
        let s = Status::read_from(&layout::status_path(&ch.instance_dir)).unwrap();
        assert_eq!(s.state, State::Idle);
    }

    #[test]
    fn on_shutdown_writes_exited() {
        let td = TempDir::new().unwrap();
        let ch = ClicomChannel::create(td.path(), 1, "x".into(), vec!["a".into()]).unwrap();
        ch.on_shutdown(0).unwrap();
        let s = Status::read_from(&layout::status_path(&ch.instance_dir)).unwrap();
        assert_eq!(s.state, State::Exited);
        assert_eq!(s.exit_code, Some(0));
    }
}
