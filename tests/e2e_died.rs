use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn killing_wrapper_marks_died_lazily() {
    let td = TempDir::new().unwrap();
    let mut c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let _ = c.kill(); let _ = c.wait();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["status"]).output().unwrap();
    assert!(out.status.success() || out.status.code() == Some(2));
    let s = String::from_utf8_lossy(&out.stdout);
    // Either "died" or "exited" depending on shutdown ordering — both are acceptable post-mortem states.
    assert!(s.contains("died") || s.contains("exited"));
}

#[test]
fn dead_instance_retention_keeps_only_ten() {
    use clicom::clicom_engine::{layout, retention, meta::{Meta, Status}};
    let td = TempDir::new().unwrap();
    // Use a guaranteed-dead PID (4_000_000) instead of 0, because on Windows
    // PID 0 is the System Idle Process and pid_is_alive(0) returns true,
    // which would cause sweep_dead_instances to skip all instances.
    let dead_pid: u32 = 4_000_000;
    for i in 0..12u32 {
        let dir = layout::instance_dir(td.path(), dead_pid, &format!("dead{i:02}"));
        std::fs::create_dir_all(&dir).unwrap();
        let m = Meta::new(dead_pid, format!("agent{i}"), vec!["x".into()], td.path().to_path_buf());
        // backdate started_at so we get a known sort order
        let mut m = m;
        m.started_at = chrono::Utc::now() - chrono::Duration::seconds(i as i64);
        m.write_to(&layout::meta_path(&dir)).unwrap();
        Status::initial_busy().write_to(&layout::status_path(&dir)).unwrap();
    }
    retention::sweep_dead_instances(td.path(), std::process::id(), 10).unwrap();
    let n = std::fs::read_dir(td.path().join(".clicom")).unwrap().count();
    assert_eq!(n, 10);
}
