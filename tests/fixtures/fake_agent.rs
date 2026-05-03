// tests/fixtures/fake_agent.rs
//! Tiny binary used by integration tests: cat stdin to stdout until EOF, then exit.

use std::io::{Read, Write};

fn main() {
    let mut buf = [0u8; 1024];
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    loop {
        let n = match handle.read(&mut buf) { Ok(n) if n > 0 => n, _ => break };
        std::io::stdout().write_all(&buf[..n]).ok();
        std::io::stdout().flush().ok();
    }
}
