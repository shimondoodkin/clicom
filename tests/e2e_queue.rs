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
fn queue_returns_id_and_emits_results() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Pass "" as partial (matches any instance) then the script as source.
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "1 + 1"]).output().unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(!id.is_empty());

    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let done = inst.join("commands").join(format!("{id}.done"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done.exists() {
        if Instant::now() > deadline { panic!("done file did not appear"); }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(inst.join("commands").join(format!("{id}.out")).exists());

    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn done_appears_after_out() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Slow script so we have time to observe the order
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "wait_ms(500); 42"]).output().unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let done = inst.join("commands").join(format!("{id}.done"));
    let out_p = inst.join("commands").join(format!("{id}.out"));

    let mut saw_done_with_out = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if done.exists() {
            saw_done_with_out = out_p.exists();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(saw_done_with_out, ".done landed without .out present");
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn result_triple_cap_evicts_oldest() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Drop 12 quick scripts via queue
    let mut ids = Vec::new();
    for _ in 0..12 {
        let out = Command::cargo_bin("clicom").unwrap()
            .current_dir(td.path()).args(["queue", "", "1"]).output().unwrap();
        ids.push(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    // Wait for all .done files to land
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let cmds = inst.join("commands");
    let last_done = cmds.join(format!("{}.done", ids.last().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(15);
    while !last_done.exists() {
        if Instant::now() > deadline { panic!("scripts did not finish"); }
        std::thread::sleep(Duration::from_millis(50));
    }
    // After eviction, only the 10 newest should remain
    let dones: Vec<_> = std::fs::read_dir(&cmds).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "done").unwrap_or(false))
        .collect();
    assert_eq!(dones.len(), 10, "expected 10 .done files post-eviction, got {}", dones.len());

    let _ = child.kill(); let _ = child.wait();
}
