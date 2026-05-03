use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn ambiguous_run_lists_candidates() {
    let td = TempDir::new().unwrap();
    let mut a = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    let mut b = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    // Pass "" as partial so both instances match, revealing the ambiguity.
    // Source "1" is provided so read_script_source succeeds before resolution.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ambiguous"));
    let _ = a.kill(); let _ = a.wait();
    let _ = b.kill(); let _ = b.wait();
}

#[test]
fn partial_match_resolves_one() {
    let td = TempDir::new().unwrap();
    let mut a = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    let mut b = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    // Pick the first instance dir's dir-name as a partial.
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap().next().unwrap().unwrap();
    let dir_name = inst.file_name().to_string_lossy().to_string();
    let pid = dir_name.split('-').next().unwrap();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", pid, "1"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let _ = a.kill(); let _ = a.wait();
    let _ = b.kill(); let _ = b.wait();
}
