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
