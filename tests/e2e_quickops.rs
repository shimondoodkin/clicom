use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));
    c
}

#[test]
fn quick_type_succeeds() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["type", "hello"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn quick_keys_succeeds() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["keys", "[Up]"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn quick_screen_runs() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["screen"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let _ = child.kill(); let _ = child.wait();
}
