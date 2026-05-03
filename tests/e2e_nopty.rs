use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn nopty_echo_captures_stdout_and_exits_clean() {
    let td = TempDir::new().unwrap();
    let st = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "echo GOODBYE"])
        .status().unwrap();
    assert!(st.success() || st.code() == Some(0));
    // Find the instance dir
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let screen = std::fs::read_to_string(inst.join("screen.txt")).unwrap();
    assert!(screen.contains("GOODBYE"), "screen: {screen}");
    let status: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(inst.join("status.json")).unwrap()).unwrap();
    assert_eq!(status["state"], "exited");
}
