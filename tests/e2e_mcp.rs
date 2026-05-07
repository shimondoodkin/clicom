use assert_cmd::prelude::*;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn send(stdin: &mut impl Write, req: &str) {
    writeln!(stdin, "{req}").unwrap();
    stdin.flush().unwrap();
}

fn recv(stdout: &mut impl BufRead) -> String {
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    line
}

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

/// Spawn `clicom mcp` in the given tempdir and return (child, stdin, stdout).
fn spawn_mcp(td: &TempDir) -> (std::process::Child, impl Write, BufReader<std::process::ChildStdout>) {
    let mut child = Command::cargo_bin("clicom")
        .unwrap()
        .current_dir(td.path())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    (child, stdin, stdout)
}

/// Send initialize + discard the response, return stdin+stdout for further use.
fn init_mcp(stdin: &mut impl Write, stdout: &mut impl BufRead) {
    send(stdin, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    let _ = recv(stdout);
}

#[test]
fn mcp_initialize_and_list_tools() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom")
        .unwrap()
        .current_dir(td.path())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    );
    let resp1 = recv(&mut stdout);
    assert!(resp1.contains("\"protocolVersion\""), "got: {resp1}");
    assert!(resp1.contains("\"clicom\""), "got: {resp1}");

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    );
    let resp2 = recv(&mut stdout);
    assert!(resp2.contains("clicom_type"), "got: {resp2}");
    assert!(resp2.contains("clicom_screen"), "got: {resp2}");
    assert!(resp2.contains("clicom_keys"), "got: {resp2}");

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

/// ──────────────────────────────────────────────────────────────────────────
/// Dead-instance fallback tests for clicom_screen / clicom_screen_after /
/// clicom_screen_after_re over MCP.
/// ──────────────────────────────────────────────────────────────────────────

#[test]
fn mcp_screen_dead_instance_appends_died_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "line one\nline two\n");

    let (mut child, mut stdin, mut stdout) = spawn_mcp(&td);
    init_mcp(&mut stdin, &mut stdout);

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"clicom_screen","arguments":{}}}"#,
    );
    let resp = recv(&mut stdout);

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    assert!(resp.contains("\"isError\":false"), "expected success: {resp}");
    assert!(resp.contains("line one"), "screen body missing: {resp}");
    assert!(resp.contains("[clicom: state=died"), "trailer missing: {resp}");
    assert!(resp.contains("last_activity=2026-05-07T01:34:12Z"), "ts wrong: {resp}");
}

#[test]
fn mcp_screen_dead_instance_no_status_omits_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "only line\n");

    let (mut child, mut stdin, mut stdout) = spawn_mcp(&td);
    init_mcp(&mut stdin, &mut stdout);

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"clicom_screen","arguments":{"no_status":true}}}"#,
    );
    let resp = recv(&mut stdout);

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    assert!(resp.contains("\"isError\":false"), "expected success: {resp}");
    assert!(resp.contains("only line"), "screen body missing: {resp}");
    assert!(!resp.contains("[clicom:"), "trailer should be suppressed: {resp}");
}

#[test]
fn mcp_screen_after_dead_instance_applies_marker_then_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "before-MARK-after\n");

    let (mut child, mut stdin, mut stdout) = spawn_mcp(&td);
    init_mcp(&mut stdin, &mut stdout);

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"clicom_screen_after","arguments":{"marker":"MARK"}}}"#,
    );
    let resp = recv(&mut stdout);

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    assert!(resp.contains("\"isError\":false"), "expected success: {resp}");
    assert!(resp.contains("-after"), "marker transform missing: {resp}");
    assert!(resp.contains("[clicom: state=died"), "trailer missing: {resp}");
}

#[test]
fn mcp_tool_call_status_with_no_instances_returns_empty_array() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom")
        .unwrap()
        .current_dir(td.path())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    );
    let _ = recv(&mut stdout);
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"clicom_status","arguments":{}}}"#,
    );
    let resp = recv(&mut stdout);
    assert!(resp.contains("\"isError\":false"), "got: {resp}");

    drop(stdin);
    let _ = child.kill();
}

#[test]
fn mcp_unknown_tool_returns_error() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom")
        .unwrap()
        .current_dir(td.path())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    );
    let _ = recv(&mut stdout);
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nonsense","arguments":{}}}"#,
    );
    let resp = recv(&mut stdout);
    assert!(resp.contains("\"error\""), "got: {resp}");
    assert!(resp.contains("unknown tool"), "got: {resp}");

    drop(stdin);
    let _ = child.kill();
}
