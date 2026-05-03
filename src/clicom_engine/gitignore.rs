//! Idempotent ".clicom/" append to <cwd>/.gitignore (§3.8).

use std::fs;
use std::io::Write;
use std::path::Path;

const ENTRY: &str = ".clicom/";

/// If `<cwd>/.gitignore` exists and does not already contain a line equal to
/// ".clicom/" (after trim), append it on its own line. If `.gitignore` does
/// not exist, do nothing. Idempotent.
pub fn ensure_clicom_ignored(cwd: &Path) -> anyhow::Result<()> {
    let gi = cwd.join(".gitignore");
    if !gi.exists() {
        return Ok(());
    }
    let body = fs::read_to_string(&gi)?;
    if body.lines().any(|l| l.trim() == ENTRY) {
        return Ok(());
    }
    let needs_newline = !body.ends_with('\n') && !body.is_empty();
    let mut f = fs::OpenOptions::new().append(true).open(&gi)?;
    if needs_newline { f.write_all(b"\n")?; }
    f.write_all(ENTRY.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_gitignore_does_nothing() {
        let td = TempDir::new().unwrap();
        ensure_clicom_ignored(td.path()).unwrap();
        assert!(!td.path().join(".gitignore").exists());
    }

    #[test]
    fn appends_when_absent() {
        let td = TempDir::new().unwrap();
        let gi = td.path().join(".gitignore");
        fs::write(&gi, "/target\n").unwrap();
        ensure_clicom_ignored(td.path()).unwrap();
        let body = fs::read_to_string(&gi).unwrap();
        assert!(body.contains(".clicom/"));
        assert!(body.starts_with("/target"));
    }

    #[test]
    fn idempotent_when_present() {
        let td = TempDir::new().unwrap();
        let gi = td.path().join(".gitignore");
        fs::write(&gi, "/target\n.clicom/\n").unwrap();
        ensure_clicom_ignored(td.path()).unwrap();
        let body = fs::read_to_string(&gi).unwrap();
        assert_eq!(body.matches(".clicom/").count(), 1);
    }

    #[test]
    fn handles_no_trailing_newline() {
        let td = TempDir::new().unwrap();
        let gi = td.path().join(".gitignore");
        fs::write(&gi, "/target").unwrap();   // no \n
        ensure_clicom_ignored(td.path()).unwrap();
        let body = fs::read_to_string(&gi).unwrap();
        assert!(body.contains("\n.clicom/\n"));
    }
}
