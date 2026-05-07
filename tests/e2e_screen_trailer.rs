//! Integration tests for the dead-instance fallback path of
//! `clicom screen` / `clicom screen-after` / `clicom screen-after-re`.

use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Manually craft a `.clicom/<pid>-<rand>/` directory whose PID is dead
/// (so `discovery::list_instances` flips its state to `Died`).
fn craft_dead_instance(td: &TempDir, screen_body: &str) -> std::path::PathBuf {
    // 4_000_000 is guaranteed not a real process on any supported platform.
    let dead_pid: u32 = 4_000_000;
    let dot_clicom = td.path().join(".clicom");
    let inst = dot_clicom.join(format!("{dead_pid}-deadbe"));
    fs::create_dir_all(inst.join("commands")).unwrap();

    let meta = serde_json::json!({
        "schema": "clicom-meta/1",
        "pid": dead_pid,
        "name": "agent",
        "command": ["fake"],
        "cwd": td.path(),
        "started_at": "2026-05-07T01:00:00Z",
    });
    fs::write(inst.join("meta.json"), serde_json::to_vec_pretty(&meta).unwrap()).unwrap();

    let status = serde_json::json!({
        "schema": "clicom-status/1",
        "state": "busy",
        "last_activity": "2026-05-07T01:34:12Z",
        "exit_code": null,
        "exited_at": null,
    });
    fs::write(inst.join("status.json"), serde_json::to_vec_pretty(&status).unwrap()).unwrap();

    fs::write(inst.join("commands.lock"), b"").unwrap();
    fs::write(inst.join("screen.txt"), screen_body.as_bytes()).unwrap();

    inst
}

#[test]
fn screen_dead_instance_appends_died_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "line one\nline two\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen"])
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("line one\nline two"), "screen body missing: {stdout:?}");
    assert!(stdout.contains("[clicom: state=died"), "trailer missing: {stdout:?}");
    assert!(stdout.contains("last_activity=2026-05-07T01:34:12Z"), "ts wrong: {stdout:?}");
    assert!(stdout.contains("visible_rows=2]"), "rows wrong: {stdout:?}");
}

#[test]
fn screen_dead_instance_no_status_omits_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "only line\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen", "--no-status"])
        .output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("only line"));
    assert!(!stdout.contains("[clicom:"), "trailer should be suppressed: {stdout:?}");
}

#[test]
fn screen_after_dead_instance_applies_marker_then_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "before-MARK-after\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen-after", "MARK"])
        .output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first_line = stdout.lines().next().unwrap_or("");
    assert!(first_line.starts_with("-after"), "marker transform missing: {stdout:?}");
    assert!(stdout.contains("[clicom: state=died"), "trailer missing: {stdout:?}");
}

#[test]
fn screen_after_re_dead_instance_applies_regex_then_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "abc-12345-tail\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen-after-re", r"\d+"])
        .output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first_line = stdout.lines().next().unwrap_or("");
    assert!(first_line.starts_with("-tail"), "regex transform missing: {stdout:?}");
    assert!(stdout.contains("[clicom: state=died"), "trailer missing: {stdout:?}");
}

#[test]
fn screen_after_re_invalid_regex_against_dead_instance_errors() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "abc-123-tail\n");

    // "(" is an unterminated group — regex compile fails.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen-after-re", "("])
        .output().unwrap();
    assert!(!out.status.success(), "invalid regex must produce non-zero exit; stdout: {:?}, stderr: {:?}",
        String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("regex"), "stderr should mention 'regex': {stderr:?}");
}
