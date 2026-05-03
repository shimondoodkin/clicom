use assert_cmd::prelude::*;
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
