//! commands/ watcher: drains *.rhai files in oldest-first order, runs each script,
//! writes result triples atomically, deletes the .rhai, then enforces the result-triple cap.

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::clicom_engine::{layout, retention, rhai_host};

pub struct WatcherHandle {
    pub _stop: Arc<std::sync::atomic::AtomicBool>,
}

pub fn spawn_watcher(
    instance_dir: PathBuf,
    engine_with_hostfns: Arc<rhai::Engine>,
    default_timeout_ms: u64,
) -> Result<WatcherHandle> {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cmd_dir = layout::commands_dir(&instance_dir);
    std::fs::create_dir_all(&cmd_dir)?;
    let s = Arc::clone(&stop);
    let cmd_dir_clone = cmd_dir.clone();
    let engine = engine_with_hostfns;

    std::thread::spawn(move || {
        // Set up notify; tolerate failures by falling back to pure polling.
        let (tx, rx) = crossbeam_channel::unbounded::<()>();
        let mut watcher_opt: Option<notify::RecommendedWatcher> = None;
        if let Ok(mut w) = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
            let _ = tx.send(());
        }) {
            let _ = w.watch(&cmd_dir_clone, RecursiveMode::NonRecursive);
            watcher_opt = Some(w);
        }
        let _ = watcher_opt;

        let mut last_poll = Instant::now() - Duration::from_secs(1);
        while !s.load(std::sync::atomic::Ordering::SeqCst) {
            // Wake on notify or timeout.
            let _ = rx.recv_timeout(Duration::from_millis(250));
            if last_poll.elapsed() < Duration::from_millis(50) {
                continue;
            }
            last_poll = Instant::now();

            // Collect *.rhai files, sorted ascending by name.
            let mut rhai_files: Vec<PathBuf> = match std::fs::read_dir(&cmd_dir_clone) {
                Ok(rd) => rd
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "rhai").unwrap_or(false))
                    .map(|e| e.path())
                    .collect(),
                Err(_) => continue,
            };
            rhai_files.sort();

            for rhai in rhai_files {
                let id = match rhai.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let source = match std::fs::read_to_string(&rhai) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let out_p = layout::out_path(&instance_dir, &id);
                let err_p = layout::err_path(&instance_dir, &id);
                let done_p = layout::done_path(&instance_dir, &id);
                let deadline = Instant::now() + Duration::from_millis(default_timeout_ms);
                let _ = rhai_host::execute_script_to_files(
                    &engine, &source, &out_p, &err_p, &done_p, deadline,
                );
                let _ = std::fs::remove_file(&rhai);
                let _ = retention::evict_result_triples(&cmd_dir_clone, 10);
            }
        }
    });

    Ok(WatcherHandle { _stop: stop })
}
