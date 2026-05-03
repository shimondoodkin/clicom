//! Bidirectional byte forwarding between the host terminal and the ConPTY.

use std::io::{Read, Write};
use std::thread;

/// Bytes captured from the agent's stdout, sent to a sink for both
/// (a) writing to our stdout, and (b) feeding the screen-buffer parser later.
#[derive(Debug)]
pub enum AgentBytes {
    Chunk(Vec<u8>),
    Eof,
}

/// Spawn the host-stdin → pty-stdin forwarder. Multiplexes stdin reads with a
/// `nudge_rx` channel so out-of-band nudge bytes (from the screen consumer) can
/// be injected into the ConPTY's stdin alongside real keystrokes.
///
/// Implementation note: `StdinLock<'static>` is `!Send`, so we can't move it
/// across thread boundaries. Instead, the inner thread opens `std::io::stdin()`
/// and locks it locally, forwarding bytes to the outer thread via a channel.
pub fn spawn_input_forwarder(
    mut writer: Box<dyn Write + Send>,
    nudge_rx: crossbeam_channel::Receiver<Vec<u8>>,
) -> thread::JoinHandle<anyhow::Result<()>> {
    thread::Builder::new()
        .name("input_fwd".into())
        .spawn(move || -> anyhow::Result<()> {
            // Read stdin from a separate thread that locks stdin locally
            // (StdinLock is !Send, so it can't cross thread boundaries — but
            // calling stdin().lock() inside the thread is fine).
            let (stdin_tx, stdin_rx) = crossbeam_channel::unbounded::<Vec<u8>>();
            let _ = thread::Builder::new()
                .name("input_fwd_stdin".into())
                .spawn(move || {
                    let stdin = std::io::stdin();
                    let mut handle = stdin.lock();
                    let mut buf = [0u8; 4096];
                    loop {
                        match handle.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                if stdin_tx.send(buf[..n].to_vec()).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

            loop {
                crossbeam_channel::select! {
                    recv(stdin_rx) -> msg => match msg {
                        Ok(bytes) => { writer.write_all(&bytes)?; writer.flush()?; },
                        Err(_) => break, // stdin closed
                    },
                    recv(nudge_rx) -> msg => match msg {
                        Ok(bytes) => { writer.write_all(&bytes)?; writer.flush()?; },
                        Err(_) => {
                            // nudge channel disconnected — keep going on stdin only
                            // by entering a simpler loop. But typically wrap.rs holds
                            // a tx forever, so this branch should not fire.
                        },
                    },
                }
            }
            Ok(())
        })
        .expect("spawn input_fwd")
}

/// Spawn the pty-stdout → host-stdout forwarder. Sends chunks through `tap`
/// for the screen-buffer parser (Task 14). Returns the JoinHandle.
///
/// When `strip_mouse` is true, VT mouse-tracking enable/disable sequences are
/// removed from the host-stdout stream so the host terminal does not enter
/// mouse-capture mode (which breaks click-drag text selection). The tap stream
/// stays unfiltered so the snapshot vt100 parser sees what the agent intended.
pub fn spawn_output_forwarder(
    mut reader: Box<dyn Read + Send>,
    tap: crossbeam_channel::Sender<AgentBytes>,
    strip_mouse: bool,
) -> thread::JoinHandle<anyhow::Result<()>> {
    thread::Builder::new()
        .name("output_fwd".into())
        .spawn(move || -> anyhow::Result<()> {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            let mut buf = [0u8; 4096];
            let mut filter = if strip_mouse {
                Some(crate::clicom_engine::mouse_filter::MouseFilter::new())
            } else {
                None
            };
            loop {
                let n = match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        eprintln!("[output_fwd] read Err: {e}");
                        return Err(e.into());
                    }
                };
                if let Some(f) = filter.as_mut() {
                    let filtered = f.process(&buf[..n]);
                    // Forward unfiltered bytes to the tap first (so snapshots see them).
                    let _ = tap.send(AgentBytes::Chunk(buf[..n].to_vec()));
                    // Then write the filtered bytes to host stdout.
                    out.write_all(&filtered)?;
                    out.flush()?;
                    continue;
                }
                out.write_all(&buf[..n])?;
                out.flush()?;
                let _ = tap.send(AgentBytes::Chunk(buf[..n].to_vec()));
            }
            let _ = tap.send(AgentBytes::Eof);
            Ok(())
        })
        .expect("spawn output_fwd")
}
