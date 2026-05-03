use assert_cmd::prelude::*;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn run_default_fails_busy_with_queued_script() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Queue a slow script. Pass "" as partial so clap routes the script to `source`.
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "wait_ms(2000)"]).output().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "type_text(\"hi\")"]).output().unwrap();
    assert_eq!(out.status.code(), Some(5), "expected exit 5; stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("busy"));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_default_competing_run_does_not_busy_fail() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // First run with a wait_ms inside; should not block the second from succeeding (it just queues on lock)
    let td_path = td.path().to_path_buf();
    let h = std::thread::spawn(move || {
        Command::cargo_bin("clicom").unwrap()
            .current_dir(&td_path).args(["run", "", "wait_ms(1500); 1"]).output().unwrap()
    });
    std::thread::sleep(Duration::from_millis(200));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "2"]).output().unwrap();
    assert!(out.status.success(), "second run failed; stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("2"));
    let _ = h.join();
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_wait_blocks_until_queue_empty() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "wait_ms(1500)"]).output().unwrap();
    let start = Instant::now();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "--wait", "", "1"]).output().unwrap();
    let elapsed = start.elapsed();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(elapsed >= Duration::from_millis(1200), "should have waited; elapsed {:?}", elapsed);
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn timeout_combined_budget_under_wait() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "wait_ms(2000)"]).output().unwrap();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "--wait", "--timeout", "1500", "", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    let _ = child.kill(); let _ = child.wait();
}
