//! `clicom start` — spawn a child, wire screen tap, idle detector, snapshot writer.
//! Stays in the foreground until the child exits, then writes status="exited" + screen.txt.

use anyhow::Result;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

use crate::clicom_engine::{self, ClicomChannel};
use crate::clicom_engine::{layout, retention, gitignore};
use crate::clicom_engine::idle::{IdleDetector, IdleEvent};
use crate::clicom_engine::meta::State;
use crate::clicom_engine::screen::ScreenBuffer;

pub struct StartArgs {
    pub mouse: bool,
    pub nopty: bool,
    pub name: Option<String>,
    pub command: Vec<String>,
}

pub fn run(cwd: &std::path::Path, args: StartArgs) -> Result<i32> {
    if args.command.is_empty() {
        eprintln!("clicom start: missing command after `--`");
        return Ok(2);
    }
    let pid = std::process::id();
    let name = args.name.clone().unwrap_or_else(|| {
        std::path::Path::new(&args.command[0])
            .file_stem().and_then(|s| s.to_str()).unwrap_or("clicom").to_string()
    });
    let ch = ClicomChannel::create(cwd, pid, name, args.command.clone())?;
    retention::sweep_dead_instances(cwd, pid, 10)?;
    let _ = gitignore::ensure_clicom_ignored(cwd);

    // Hourly retention sweep
    {
        let cwd = cwd.to_path_buf();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(3600));
            let _ = retention::sweep_dead_instances(&cwd, pid, 10);
        });
    }

    let screen = Arc::new(ScreenBuffer::new(40, 120));
    let stop = Arc::new(AtomicBool::new(false));

    // Snapshot writer thread: write screen.txt on each idle transition + at most every 250ms.
    let (idle_tx, idle_rx) = crossbeam_channel::unbounded::<IdleEvent>();
    {
        let screen = Arc::clone(&screen);
        let stop = Arc::clone(&stop);
        let inst_dir = ch.instance_dir.clone();
        let status = Arc::clone(&ch.status);
        thread::spawn(move || {
            let mut last_write = Instant::now() - Duration::from_secs(1);
            while !stop.load(Ordering::SeqCst) {
                // Drain idle events (state transitions).
                while let Ok(ev) = idle_rx.try_recv() {
                    let s = match ev { IdleEvent::BecameIdle => State::Idle, IdleEvent::BecameBusy => State::Busy };
                    if let Ok(mut st) = status.lock() {
                        st.state = s;
                        st.last_activity = chrono::Utc::now();
                        let _ = st.write_to(&layout::status_path(&inst_dir));
                    }
                    let body = screen.to_plain_text();
                    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&inst_dir), body.as_bytes());
                    last_write = Instant::now();
                }
                // Throttled snapshot.
                if last_write.elapsed() >= Duration::from_millis(250) {
                    let body = screen.to_plain_text();
                    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&inst_dir), body.as_bytes());
                    last_write = Instant::now();
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
    }

    // Idle detector ticker.
    let detector = Arc::new(std::sync::Mutex::new(IdleDetector::new(1, Instant::now())));
    {
        let det = Arc::clone(&detector);
        let stop = Arc::clone(&stop);
        let tx = idle_tx.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(200));
                let now = Instant::now();
                if let Ok(mut d) = det.lock() {
                    if let Some(ev) = d.tick(now) { let _ = tx.send(ev); }
                }
            }
        });
    }

    // Spawn child + forwarding loop. Uses pty/nopty per args.
    let exit_code = if args.nopty {
        spawn_and_forward_nopty(&args.command, &screen, &detector, &idle_tx)?
    } else {
        spawn_and_forward_pty(&args.command, args.mouse, &screen, &detector, &idle_tx)?
    };

    // Final snapshot before flipping to exited.
    let body = screen.to_plain_text();
    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&ch.instance_dir), body.as_bytes());
    ch.on_shutdown(exit_code)?;
    stop.store(true, Ordering::SeqCst);
    Ok(exit_code)
}

fn spawn_and_forward_pty(
    command: &[String],
    mouse_allow: bool,  // true = --mouse flag was passed (allow passthrough); false = strip
    screen: &Arc<ScreenBuffer>,
    detector: &Arc<std::sync::Mutex<IdleDetector>>,
    idle_tx: &crossbeam_channel::Sender<IdleEvent>,
) -> Result<i32> {
    use crate::clicom_engine::forwarding::{spawn_input_forwarder, spawn_output_forwarder, AgentBytes};
    use crate::clicom_engine::pty::{spawn as pty_spawn, current_terminal_size};
    let strip_mouse = !mouse_allow;
    let mut pty = pty_spawn(command.to_vec(), current_terminal_size())?;
    let writer = pty.pair.master.take_writer()?;
    let reader = pty.pair.master.try_clone_reader()?;
    let (_nudge_tx, nudge_rx) = crossbeam_channel::unbounded::<Vec<u8>>();
    let (tap_tx, tap_rx) = crossbeam_channel::unbounded::<AgentBytes>();
    let _in_h = spawn_input_forwarder(writer, nudge_rx);
    let _out_h = spawn_output_forwarder(reader, tap_tx, strip_mouse);
    // Bridge tap → screen + idle.
    let screen_clone = Arc::clone(screen);
    let det_clone = Arc::clone(detector);
    let bridge = std::thread::spawn(move || {
        while let Ok(msg) = tap_rx.recv() {
            match msg {
                AgentBytes::Chunk(bytes) => {
                    screen_clone.advance_bytes(&bytes);
                    if let Ok(mut d) = det_clone.lock() {
                        let _ = d.note_byte(std::time::Instant::now());
                    }
                }
                AgentBytes::Eof => break,
            }
        }
    });
    let status = pty.child.wait()?;
    let _ = bridge.join();
    let _ = idle_tx; // detector ticker thread will react
    Ok(status.exit_code() as i32)
}

fn spawn_and_forward_nopty(
    command: &[String],
    screen: &Arc<ScreenBuffer>,
    detector: &Arc<std::sync::Mutex<IdleDetector>>,
    _idle_tx: &crossbeam_channel::Sender<IdleEvent>,
) -> Result<i32> {
    use std::io::{Read, Write};
    let mut child = clicom_engine::nopty::spawn(command)?;
    let buf = [0u8; 8192];
    // Read child stdout → screen tap + host stdout.
    let stop_reader = Arc::new(AtomicBool::new(false));
    let r_stop = Arc::clone(&stop_reader);
    let screen_clone = Arc::clone(screen);
    let det_clone = Arc::clone(detector);
    let mut stdout = child.stdout;
    let reader_handle = thread::spawn(move || -> anyhow::Result<()> {
        let mut local = [0u8; 8192];
        while !r_stop.load(Ordering::SeqCst) {
            let n = match stdout.read(&mut local) { Ok(n) if n > 0 => n, _ => break };
            screen_clone.advance_bytes(&local[..n]);
            std::io::stdout().write_all(&local[..n]).ok();
            std::io::stdout().flush().ok();
            if let Ok(mut d) = det_clone.lock() {
                let _ = d.note_byte(Instant::now());
            }
        }
        Ok(())
    });
    // Forward host stdin → child stdin in a separate thread (best effort).
    let mut child_stdin = child.stdin;
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut local = [0u8; 8192];
        let mut handle = stdin.lock();
        loop {
            let n = match handle.read(&mut local) { Ok(n) if n > 0 => n, _ => break };
            if child_stdin.write_all(&local[..n]).is_err() { break; }
        }
    });

    let status = child.child.wait()?;
    stop_reader.store(true, Ordering::SeqCst);
    let _ = reader_handle.join();
    let _ = buf;
    Ok(status.code().unwrap_or(0))
}
