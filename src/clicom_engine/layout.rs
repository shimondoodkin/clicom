//! Path layout helpers (§3) + partial-match for instance discovery (§5.3).

use std::path::{Path, PathBuf};

pub fn dot_clicom(cwd: &Path) -> PathBuf { cwd.join(".clicom") }

pub fn instance_dir(cwd: &Path, pid: u32, rand6: &str) -> PathBuf {
    dot_clicom(cwd).join(format!("{}-{}", pid, rand6))
}

pub fn instance_dir_name(pid: u32, rand6: &str) -> String {
    format!("{}-{}", pid, rand6)
}

pub fn meta_path(instance: &Path) -> PathBuf { instance.join("meta.json") }
pub fn status_path(instance: &Path) -> PathBuf { instance.join("status.json") }
pub fn screen_path(instance: &Path) -> PathBuf { instance.join("screen.txt") }
pub fn lock_path(instance: &Path) -> PathBuf { instance.join("commands.lock") }
pub fn commands_dir(instance: &Path) -> PathBuf { instance.join("commands") }
pub fn rhai_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.rhai", id))
}
pub fn out_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.out", id))
}
pub fn err_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.err", id))
}
pub fn done_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.done", id))
}
pub fn log_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.log", id))
}

/// Substring match of `<partial>` against a dir name like "12345-a3f9c2".
pub fn partial_matches(dir_name: &str, partial: &str) -> bool {
    dir_name.contains(partial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_name_format() {
        assert_eq!(instance_dir_name(12345, "a3f9c2"), "12345-a3f9c2");
    }

    #[test]
    fn partial_matches_pid_or_rand_or_combined() {
        let name = "12345-a3f9c2";
        assert!(partial_matches(name, "12345"));
        assert!(partial_matches(name, "a3f9"));
        assert!(partial_matches(name, "12345-a3"));
        assert!(partial_matches(name, "f9c2"));
        assert!(!partial_matches(name, "9999"));
    }

    #[test]
    fn paths_compose_correctly() {
        let cwd = Path::new("/tmp/work");
        let inst = instance_dir(cwd, 99, "abcdef");
        assert_eq!(inst, Path::new("/tmp/work/.clicom/99-abcdef"));
        assert_eq!(rhai_path(&inst, "1-deadbe"), Path::new("/tmp/work/.clicom/99-abcdef/commands/1-deadbe.rhai"));
    }
}
