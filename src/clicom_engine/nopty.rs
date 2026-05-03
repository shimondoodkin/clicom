//! Pipe-based spawn (no PTY). Wires child stdin/stdout/stderr to plain pipes;
//! host stdin → child stdin, child stdout → host stdout. Used by `clicom start --nopty`.

use std::io::Read;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct NoPtyChild {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
}

pub fn spawn(command: &[String]) -> anyhow::Result<NoPtyChild> {
    let (head, tail) = command.split_first().ok_or_else(|| anyhow::anyhow!("empty command"))?;
    let mut cmd = Command::new(head);
    cmd.args(tail)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn()?;
    let stdin  = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    Ok(NoPtyChild { child, stdin, stdout })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn echo_cmd() -> Vec<String> {
        vec!["cmd".into(), "/C".into(), "echo hello".into()]
    }
    #[cfg(unix)]
    fn echo_cmd() -> Vec<String> {
        vec!["sh".into(), "-c".into(), "echo hello".into()]
    }

    #[test]
    fn spawns_child_and_captures_stdout() {
        let mut p = spawn(&echo_cmd()).unwrap();
        let mut s = String::new();
        p.stdout.read_to_string(&mut s).unwrap();
        assert!(s.contains("hello"), "got: {s:?}");
        let st = p.child.wait().unwrap();
        assert!(st.success());
    }
}
