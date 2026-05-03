use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn exec_detached_spawns_child_and_prints_pid() {
    let td = TempDir::new().unwrap();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["exec-detached", "--", "cmd", "/C", "ping", "-n", "1", "127.0.0.1"])
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pid: u32 = stdout.trim().parse().expect(&format!("expected pid, got: {stdout:?}"));
    assert!(pid > 0);
}

#[test]
fn exec_detached_missing_command_errors_2() {
    let td = TempDir::new().unwrap();
    // No `--` and no positional → clap rejects with non-zero exit.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["exec-detached"])
        .output().unwrap();
    assert!(!out.status.success(), "expected failure for missing args");
}
