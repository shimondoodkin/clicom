//! Common drop sequence shared by `clicom run` and `clicom queue` (§5.4).

use anyhow::Result;
use fs2::FileExt;
use std::fs::OpenOptions;
use std::path::Path;

use crate::clicom_engine::{layout, ids};

pub struct LockGuard { file: std::fs::File }
impl Drop for LockGuard { fn drop(&mut self) { let _ = self.file.unlock(); } }

pub fn acquire_lock(instance_dir: &Path) -> Result<LockGuard> {
    let f = OpenOptions::new().read(true).write(true).create(true).open(layout::lock_path(instance_dir))?;
    f.lock_exclusive()?;
    Ok(LockGuard { file: f })
}

pub fn drop_rhai(instance_dir: &Path, source: &str) -> Result<String> {
    let id = ids::make_command_id();
    let final_path = layout::rhai_path(instance_dir, &id);
    let tmp = final_path.with_extension("rhai.tmp");
    std::fs::write(&tmp, source)?;
    std::fs::rename(&tmp, &final_path)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    #[test]
    fn drop_rhai_writes_full_filename() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(layout::commands_dir(td.path())).unwrap();
        let id = drop_rhai(td.path(), "1+1").unwrap();
        assert!(layout::rhai_path(td.path(), &id).exists());
        assert!(!layout::rhai_path(td.path(), &id).with_extension("rhai.tmp").exists());
    }
}
