use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn clean_with_id_deletes_triple() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Pass "" as partial so clap routes the script to `source`.
    let id = String::from_utf8_lossy(&Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "1"]).output().unwrap().stdout).trim().to_string();
    std::thread::sleep(Duration::from_millis(500));
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let cmds = inst.join("commands");
    assert!(cmds.join(format!("{id}.done")).exists());
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["clean", "", &id]).output().unwrap();
    assert!(out.status.success());
    assert!(!cmds.join(format!("{id}.done")).exists());
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn clean_idempotent_for_missing_id() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["clean", "", "1234567890-aaaaaa"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn clean_sweep_skips_triples_without_done() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Wait for the wrapper, then craft a synthetic .out without .done
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let cmds = inst.join("commands");
    let synthetic = "9999999999999999999-aaaaaa";
    std::fs::write(cmds.join(format!("{synthetic}.out")), "lonely").unwrap();

    // Drop a real queue script and let it complete
    let id = String::from_utf8_lossy(&Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "", "1"]).output().unwrap().stdout).trim().to_string();
    std::thread::sleep(Duration::from_millis(500));
    assert!(cmds.join(format!("{id}.done")).exists());

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["clean"]).output().unwrap();
    assert!(out.status.success());

    // Real triple gone, synthetic .out preserved
    assert!(!cmds.join(format!("{id}.done")).exists());
    assert!(cmds.join(format!("{synthetic}.out")).exists());

    let _ = child.kill(); let _ = child.wait();
}
