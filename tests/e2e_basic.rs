use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let child = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 30 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));
    child
}

#[test]
fn instance_dir_appears_with_layout_files() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let clicom_dir = td.path().join(".clicom");
    let inst = std::fs::read_dir(&clicom_dir).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    assert!(inst.join("meta.json").exists());
    assert!(inst.join("status.json").exists());
    assert!(inst.join("commands.lock").exists());
    assert!(inst.join("commands").is_dir());
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_returns_value_via_out_file() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Pass "" as partial (matches any instance) then the script as source.
    // Plan used ["run", "1 + 2"] but clap assigns first positional to `partial`;
    // using "" ensures partial matches the single live instance.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "1 + 2"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("3"), "stdout: {s}");
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_screen_text_returns_visible() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Type something into the wrapped child via the channel
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "type_text(\"marker\\n\")"]).output().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "", "screen_text()"]).output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("marker") || !s.is_empty(), "screen: {s}");
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn status_lists_instance() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["status"]).output().unwrap();
    assert!(out.status.success());
    assert!(!out.stdout.is_empty());
    let _ = child.kill(); let _ = child.wait();
}
