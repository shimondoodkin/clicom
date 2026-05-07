//! `clicom start` — spawn a child, wire screen tap, idle detector, snapshot writer.
//! Stays in the foreground until the child exits, then writes status="exited" + screen.txt.

use anyhow::Result;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

use crate::clicom_engine::{self, ClicomChannel};
use crate::clicom_engine::{layout, retention, gitignore, rhai_host, watcher};
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

    // STEP 1: Switch host console to raw VT mode immediately. Any keystroke
    // typed from this point flows through our input forwarder rather than
    // sitting in the line-input buffer or being eaten when the mode flips.
    // RAII guard restores the original modes on Drop.
    let _console_guard = clicom_engine::console_mode::enter_raw()?;

    // STEP 2: Lightweight in-memory state required by PTY forwarders.
    let screen = Arc::new(ScreenBuffer::new(40, 120));
    let stop = Arc::new(AtomicBool::new(false));
    let detector = Arc::new(std::sync::Mutex::new(IdleDetector::new(1, Instant::now())));
    let (idle_tx, idle_rx) = crossbeam_channel::unbounded::<IdleEvent>();
    let (nudge_tx, nudge_rx) = crossbeam_channel::unbounded::<Vec<u8>>();

    // STEP 3: PTY spawn + forwarders BEFORE any file I/O.
    // Why: ChicomChannel::create + retention sweep + gitignore + watcher init
    // can take tens of ms. If the user starts typing during that window with
    // the input forwarder not yet alive, we lose those keystrokes (they hit
    // the OS input buffer in a mode that gets flipped/discarded). Spawning
    // the PTY+forwarders first means the input pipeline is live from the
    // very first keystroke.
    let pty_fwd = if !args.nopty {
        Some(spawn_pty_and_forwarders(
            &args.command, args.mouse, nudge_rx.clone(),
            &screen, &detector,
        )?)
    } else {
        None
    };

    // STEP 4: File I/O + ancillary thread setup (formerly the head of run()).
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

    // Snapshot writer thread: write screen.txt on each idle transition + at most every 250ms.
    {
        let screen = Arc::clone(&screen);
        let stop = Arc::clone(&stop);
        let inst_dir = ch.instance_dir.clone();
        let status = Arc::clone(&ch.status);
        thread::spawn(move || {
            let mut last_write = Instant::now() - Duration::from_secs(1);
            while !stop.load(Ordering::SeqCst) {
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

    // Build Rhai engine + host context, then spawn the commands/ watcher.
    let ctx = std::sync::Arc::new(rhai_host::HostContext {
        screen: Arc::clone(&screen),
        status: Arc::clone(&ch.status),
        nudge_tx: nudge_tx.clone(),
        instance_cwd: cwd.to_path_buf(),
        idle_observer: Arc::clone(&detector),
        script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
        current_deadline: Arc::new(std::sync::Mutex::new(None)),
        print_buffer: Arc::new(std::sync::Mutex::new(String::new())),
    });
    let mut engine = rhai_host::build_engine();
    rhai_host::register_host_fns(&mut engine, Arc::clone(&ctx));
    let engine = Arc::new(engine);
    let _watcher_guard = watcher::spawn_watcher(ch.instance_dir.clone(), Arc::clone(&engine), Arc::clone(&ctx), 60_000)?;

    // STEP 5: Wait for the child.
    let exit_code = match pty_fwd {
        Some(mut p) => p.wait()?,
        None => spawn_and_forward_nopty(&args.command, nudge_rx, &screen, &detector, &idle_tx)?,
    };
    trace_exit("child exited");

    // Final snapshot before flipping to exited.
    let body = screen.to_plain_text();
    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&ch.instance_dir), body.as_bytes());
    trace_exit("snapshot written");
    ch.on_shutdown(exit_code)?;
    trace_exit("on_shutdown done");
    stop.store(true, Ordering::SeqCst);
    trace_exit("returning to main");
    Ok(exit_code)
}

/// Breadcrumb to stderr when `CLICOM_TRACE_EXIT` is set. The PTY child has
/// already exited by the time we use this, but the host console may still be
/// in raw VT mode (the `_console_guard` is still alive), so we emit `\r\n`
/// instead of bare `\n` to render correctly under DISABLE_NEWLINE_AUTO_RETURN.
fn trace_exit(msg: &str) {
    if std::env::var_os("CLICOM_TRACE_EXIT").is_some() {
        eprint!("[clicom-exit] {msg}\r\n");
    }
}

/// Active PTY + forwarder threads. Hold this struct alive while the rest of
/// `run()` does its slow setup; call `wait()` to block on the child.
///
/// Forwarder/bridge join handles are intentionally kept underscore-prefixed
/// and never joined: on Windows ConPTY the master reader doesn't reliably
/// deliver EOF when the child exits, so the output forwarder can stay blocked
/// on `read()` indefinitely — and the bridge thread, which receives from a
/// channel the output forwarder feeds, would block with it. Joining them
/// would hang the wrapper on exit. They're detached; the OS reaps them when
/// the process exits.
struct PtyForward {
    _pair: portable_pty::PtyPair,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    _bridge: thread::JoinHandle<()>,
    _input_h: thread::JoinHandle<anyhow::Result<()>>,
    _output_h: thread::JoinHandle<anyhow::Result<()>>,
}

impl PtyForward {
    fn wait(&mut self) -> Result<i32> {
        let status = self.child.wait()?;
        Ok(status.exit_code() as i32)
    }
}

fn spawn_pty_and_forwarders(
    command: &[String],
    mouse_allow: bool,
    nudge_rx: crossbeam_channel::Receiver<Vec<u8>>,
    screen: &Arc<ScreenBuffer>,
    detector: &Arc<std::sync::Mutex<IdleDetector>>,
) -> Result<PtyForward> {
    use crate::clicom_engine::forwarding::{spawn_input_forwarder, spawn_output_forwarder, AgentBytes};
    use crate::clicom_engine::pty::{spawn as pty_spawn, current_terminal_size};
    let strip_mouse = !mouse_allow;
    let pty = pty_spawn(command.to_vec(), current_terminal_size())?;
    let writer = pty.pair.master.take_writer()?;
    let reader = pty.pair.master.try_clone_reader()?;
    let (tap_tx, tap_rx) = crossbeam_channel::unbounded::<AgentBytes>();
    let in_h = spawn_input_forwarder(writer, nudge_rx);
    let out_h = spawn_output_forwarder(reader, tap_tx, strip_mouse);

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

    Ok(PtyForward {
        _pair: pty.pair,
        child: pty.child,
        _bridge: bridge,
        _input_h: in_h,
        _output_h: out_h,
    })
}

fn spawn_and_forward_nopty(
    command: &[String],
    nudge_rx: crossbeam_channel::Receiver<Vec<u8>>,
    screen: &Arc<ScreenBuffer>,
    detector: &Arc<std::sync::Mutex<IdleDetector>>,
    _idle_tx: &crossbeam_channel::Sender<IdleEvent>,
) -> Result<i32> {
    use std::io::{Read, Write};
    use std::sync::Mutex;
    let mut child = clicom_engine::nopty::spawn(command)?;
    // Share child stdin between the host-stdin forwarder and the nudge forwarder.
    let child_stdin = Arc::new(Mutex::new(child.stdin));

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

    // Thread A: forward host stdin → child stdin (best effort).
    {
        let cs = Arc::clone(&child_stdin);
        thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut local = [0u8; 8192];
            let mut handle = stdin.lock();
            loop {
                let n = match handle.read(&mut local) { Ok(n) if n > 0 => n, _ => break };
                if let Ok(mut w) = cs.lock() {
                    if w.write_all(&local[..n]).is_err() { break; }
                }
            }
        });
    }

    // Thread B: drain nudge_rx → child stdin (script-injected bytes).
    {
        let cs = Arc::clone(&child_stdin);
        thread::spawn(move || {
            while let Ok(bytes) = nudge_rx.recv() {
                if let Ok(mut w) = cs.lock() {
                    let _ = w.write_all(&bytes);
                }
            }
        });
    }

    let status = child.child.wait()?;
    stop_reader.store(true, Ordering::SeqCst);
    let _ = reader_handle.join();
    Ok(status.code().unwrap_or(0))
}
