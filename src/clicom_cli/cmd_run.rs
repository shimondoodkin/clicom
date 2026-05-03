//! `clicom run` — synchronous drop + wait + read + delete.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::clicom_cli::{discovery, drop as drop_h};
use crate::clicom_engine::layout;
use crate::clicom_engine::meta::State;

pub enum BusyMode { Default, Wait, Force }

pub struct RunArgs {
    pub partial: Option<String>,
    pub source: String,
    pub mode: BusyMode,
    pub timeout_ms: u64,
}

pub fn run(cwd: &Path, args: RunArgs) -> Result<i32> {
    let inst = match resolve_instance(cwd, args.partial.as_deref())? {
        Some(p) => p, None => return Ok(2),
    };

    let deadline = Instant::now() + Duration::from_millis(args.timeout_ms);
    let mut guard = Some(drop_h::acquire_lock(&inst)?);

    // Busy check
    let cmds = layout::commands_dir(&inst);
    match args.mode {
        BusyMode::Default => {
            let pending = count_rhai(&cmds)?;
            if pending > 0 {
                eprintln!("busy: {pending} pending script(s)");
                return Ok(5);
            }
        }
        BusyMode::Wait => {
            while count_rhai(&cmds)? > 0 {
                if Instant::now() >= deadline { return Ok(4); }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        BusyMode::Force => { /* skip */ }
    }

    let id = drop_h::drop_rhai(&inst, &args.source)?;

    // For --force, release the lock before waiting for .done
    if matches!(args.mode, BusyMode::Force) {
        guard.take();
    }

    let done = layout::done_path(&inst, &id);
    while !done.exists() {
        if Instant::now() >= deadline { return Ok(4); }
        std::thread::sleep(Duration::from_millis(50));
    }

    // For --force, re-acquire the lock for read+delete (§5.4 step 7)
    if guard.is_none() {
        guard = Some(drop_h::acquire_lock(&inst)?);
    }

    let body = std::fs::read_to_string(&done)?.trim().to_string();
    let exit = if body.starts_with("OK") {
        let out = std::fs::read_to_string(layout::out_path(&inst, &id))?;
        print_out(&out)?;
        0
    } else {
        let err = std::fs::read_to_string(layout::err_path(&inst, &id))?;
        eprintln!("{}", err.trim_end());
        3
    };
    let _ = std::fs::remove_file(layout::out_path(&inst, &id));
    let _ = std::fs::remove_file(layout::err_path(&inst, &id));
    let _ = std::fs::remove_file(&done);
    drop(guard);
    Ok(exit)
}

fn resolve_instance(cwd: &Path, partial: Option<&str>) -> Result<Option<PathBuf>> {
    let candidates: Vec<_> = discovery::filter_by_partial(discovery::list_instances(cwd), partial)
        .into_iter().filter(|i| matches!(i.status.state, State::Idle | State::Busy)).collect();
    Ok(match candidates.len() {
        0 => { eprintln!("no live wrapped agent in {}", cwd.display()); None }
        1 => Some(candidates.into_iter().next().unwrap().dir),
        _ => {
            eprintln!("ambiguous match — candidates:");
            for i in &candidates { eprintln!("  {}", i.dir_name); }
            None
        }
    })
}

fn count_rhai(cmds: &Path) -> Result<usize> {
    let n = std::fs::read_dir(cmds)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rhai").unwrap_or(false))
        .count();
    Ok(n)
}

fn print_out(json_line: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(json_line.trim()).unwrap_or(serde_json::Value::Null);
    match v {
        serde_json::Value::String(s) => { println!("{s}"); }
        serde_json::Value::Null => { /* unit → no output */ }
        other => { println!("{}", serde_json::to_string_pretty(&other)?); }
    }
    Ok(())
}
