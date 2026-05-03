use assert_cmd::prelude::*;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[test]
fn wrapper_executes_dropped_rhai() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom")
        .unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 5 127.0.0.1 >nul"])
        .spawn()
        .unwrap();
    // Give the wrapper time to start and create the instance dir + watcher.
    std::thread::sleep(Duration::from_millis(800));

    // Find the instance directory under .clicom/
    let inst_dir = std::fs::read_dir(td.path().join(".clicom"))
        .unwrap()
        .find_map(|e| {
            let p = e.unwrap().path();
            if p.is_dir() { Some(p) } else { None }
        })
        .expect("no instance dir found");

    let cmd_dir = inst_dir.join("commands");
    let id = format!(
        "{}-aaaaaa",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let rhai = cmd_dir.join(format!("{id}.rhai"));
    std::fs::write(&rhai, "1 + 1").unwrap();

    // Wait up to 5 seconds for the .done file to appear.
    let deadline = Instant::now() + Duration::from_secs(5);
    let done = cmd_dir.join(format!("{id}.done"));
    while !done.exists() {
        if Instant::now() > deadline {
            panic!("done file did not appear within 5 seconds");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let body = std::fs::read_to_string(&done).unwrap();
    assert_eq!(body.trim(), "OK");

    let out = std::fs::read_to_string(cmd_dir.join(format!("{id}.out"))).unwrap();
    assert!(out.trim().contains("2"), "expected '2' in .out, got: {out:?}");

    let _ = child.kill();
    let _ = child.wait();
}
