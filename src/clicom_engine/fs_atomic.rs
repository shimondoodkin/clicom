//! Single source of truth for atomic file writes (write `*.tmp` then rename).

use std::fs;
use std::path::Path;

/// Atomically write `bytes` to `path`. Creates parent dirs as needed.
pub fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(
        path.extension()
            .map(|e| format!("{}.tmp", e.to_string_lossy()))
            .unwrap_or_else(|| "tmp".into()),
    );
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_atomically_via_tmp_rename() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("foo.json");
        write(&target, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
        // tmp file should be gone
        assert!(!td.path().join("foo.json.tmp").exists());
    }

    #[test]
    fn overwrites_existing_file() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("x.txt");
        std::fs::write(&target, b"old").unwrap();
        write(&target, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn creates_parent_dirs() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("a/b/c.txt");
        write(&target, b"x").unwrap();
        assert!(target.exists());
    }
}
