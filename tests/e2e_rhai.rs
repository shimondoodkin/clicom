use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn eval_is_disabled_at_compile_time() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Pass "" as partial so clap routes the script to `source`.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "eval(\"type_text(\\\"x\\\")\")"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("parse"));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn wait_ms_above_cap_throws() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Pass "" as partial so clap routes the script to `source`.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "wait_ms(700000)"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn loop_caught_by_max_operations() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Pass "" as partial so clap routes the script to `source`.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "loop {}"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn set_timeout_overrides_default() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let start = std::time::Instant::now();
    // Script: set short timeout then spin in a loop. Should abort with ERR timeout.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["run", "", "set_timeout(500); let i = 0; loop { wait_ms(10); i += 1; if i > 300 { break; } }"])
        .output().unwrap();
    let elapsed = start.elapsed();
    // Should exit 3 (script error) and complete well within 2s.
    assert_eq!(out.status.code(), Some(3), "expected exit 3, got {:?}", out.status.code());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("timeout"), "expected ERR timeout in stderr, got: {stderr:?}");
    assert!(elapsed < Duration::from_secs(2), "should abort fast, took {:?}", elapsed);
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn print_lands_on_driver_stderr() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["run", "", "print(\"hello from script\"); 42"])
        .output().unwrap();
    assert_eq!(out.status.code(), Some(0), "expected exit 0, got {:?}\nstderr: {}", out.status.code(), String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("42"), "expected 42 in stdout, got: {stdout:?}");
    assert!(stderr.contains("hello from script"), "expected print output in stderr, got: {stderr:?}");
    let _ = child.kill(); let _ = child.wait();
}
