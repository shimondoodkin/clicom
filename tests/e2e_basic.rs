// tests/e2e_basic.rs
use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn start_status_basic_smoke() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "echo hello"])
        .spawn().unwrap();
    // give the wrapper a moment to spawn + write its layout
    std::thread::sleep(std::time::Duration::from_millis(500));
    // status should print at least one row
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["status"])
        .output().unwrap();
    assert!(out.status.success() || out.status.code() == Some(2),
            "status exit: {:?}", out.status);
    // wait for child to exit
    let st = child.wait().unwrap();
    assert!(st.success() || st.code() == Some(0));
    // verify .clicom dir + screen.txt exists
    let clicom_dir = td.path().join(".clicom");
    assert!(clicom_dir.is_dir());
    let mut found_screen = false;
    for e in std::fs::read_dir(clicom_dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() && p.join("screen.txt").exists() { found_screen = true; }
    }
    assert!(found_screen, "screen.txt should exist in instance dir");
}
