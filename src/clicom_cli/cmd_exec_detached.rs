//! `clicom exec-detached` — spawn a command as a detached process.
//!
//! Useful for launching wrapped agents from scripts/MCP tools without
//! tying their lifetime to the caller's terminal.

use anyhow::Result;

pub fn run(cmd: Vec<String>) -> Result<i32> {
    if cmd.is_empty() {
        eprintln!("clicom exec-detached: missing command after `--`");
        return Ok(2);
    }
    let pid = spawn_detached(&cmd)?;
    println!("{pid}");
    Ok(0)
}

#[cfg(windows)]
pub fn spawn_detached(cmd: &[String]) -> Result<u32> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    // CREATE_NEW_CONSOLE — give the child its own window so its stdio doesn't
    // attach to ours. (DETACHED_PROCESS would be silent but is mutually exclusive
    // with CREATE_NEW_CONSOLE per Win32 docs.)
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

    let mut command = Command::new(&cmd[0]);
    for a in &cmd[1..] {
        command.arg(a);
    }
    command
        .creation_flags(CREATE_NEW_CONSOLE)
        // Sever inheritance so the new console fully owns the child's stdio
        // (otherwise Rust's default-inherit reuses the launcher's pipe handles).
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn failed: {e}"))?;
    Ok(child.id())
}

#[cfg(not(windows))]
pub fn spawn_detached(cmd: &[String]) -> Result<u32> {
    use std::process::Command;

    let child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn failed: {e}"))?;
    Ok(child.id())
}
