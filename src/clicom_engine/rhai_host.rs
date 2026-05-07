//! Rhai engine setup + host-fn registration shared shape.
//!
//! Limits per §6.3. Host fns are registered by `register_host_fns` (Task 22+).

use rhai::{Engine, Scope};
use std::sync::Arc;
use crate::clicom_engine::screen::ScreenBuffer;

// ── Input helpers ─────────────────────────────────────────────────────────────

/// Translate bare `\n` → `\r` while leaving existing `\r\n` intact.
fn translate_newlines(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let prev = if i == 0 { 0u8 } else { bytes[i - 1] };
            if prev == b'\r' {
                out.push(b'\n');
            } else {
                out.push(b'\r');
            }
        } else {
            out.push(b);
        }
    }
    out
}

/// Parse a key-spec string with bracketed tokens (e.g. `"hi[Enter][Ctrl+C]"`) into bytes.
fn parse_key_spec(spec: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            let mut tok = String::new();
            let mut closed = false;
            while let Some(&nc) = chars.peek() {
                chars.next();
                if nc == ']' {
                    closed = true;
                    break;
                }
                tok.push(nc);
            }
            if !closed {
                return Err(format!("unterminated key token starting at '[{tok}'"));
            }
            let bytes = lookup_key(&tok)?;
            out.extend_from_slice(&bytes);
        } else {
            // Plain char — UTF-8 encode it verbatim (no newline translation).
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            out.extend_from_slice(s.as_bytes());
        }
    }
    Ok(out)
}

fn lookup_key(tok: &str) -> Result<Vec<u8>, String> {
    let t = tok.to_lowercase();
    // Modifiers: ctrl+X, alt+X
    if let Some(rest) = t.strip_prefix("ctrl+") {
        let mut chs = rest.chars();
        let c = chs.next().ok_or_else(|| "empty Ctrl+".to_string())?;
        if chs.next().is_some() {
            return Err(format!("unsupported Ctrl chord: '{tok}'"));
        }
        let b = (c.to_ascii_uppercase() as u8) & 0x1f;
        return Ok(vec![b]);
    }
    if let Some(rest) = t.strip_prefix("alt+") {
        if rest.is_empty() {
            return Err("empty Alt+".to_string());
        }
        let mut bytes = vec![0x1b];
        bytes.extend_from_slice(rest.as_bytes());
        return Ok(bytes);
    }
    let bytes: &[u8] = match t.as_str() {
        "enter"              => b"\r",
        "tab"                => b"\t",
        "backspace"          => b"\x7f",
        "esc" | "escape"     => b"\x1b",
        "space"              => b" ",
        "up"                 => b"\x1b[A",
        "down"               => b"\x1b[B",
        "right"              => b"\x1b[C",
        "left"               => b"\x1b[D",
        "home"               => b"\x1b[H",
        "end"                => b"\x1b[F",
        "pageup" | "page_up"     => b"\x1b[5~",
        "pagedown" | "page_down" => b"\x1b[6~",
        "insert"             => b"\x1b[2~",
        "delete"             => b"\x1b[3~",
        "f1"                 => b"\x1bOP",
        "f2"                 => b"\x1bOQ",
        "f3"                 => b"\x1bOR",
        "f4"                 => b"\x1bOS",
        "f5"                 => b"\x1b[15~",
        "f6"                 => b"\x1b[17~",
        "f7"                 => b"\x1b[18~",
        "f8"                 => b"\x1b[19~",
        "f9"                 => b"\x1b[20~",
        "f10"                => b"\x1b[21~",
        "f11"                => b"\x1b[23~",
        "f12"                => b"\x1b[24~",
        _ => return Err(format!("unknown key '{tok}'")),
    };
    Ok(bytes.to_vec())
}

pub struct HostContext {
    pub screen: Arc<ScreenBuffer>,
    pub status: Arc<std::sync::Mutex<crate::clicom_engine::meta::Status>>,
    pub nudge_tx: crossbeam_channel::Sender<Vec<u8>>,
    /// The wrapper's process cwd (per spec §4 — used to resolve relative paths in host fns).
    pub instance_cwd: std::path::PathBuf,
    pub idle_observer: Arc<std::sync::Mutex<crate::clicom_engine::idle::IdleDetector>>,
    pub script_timeout_override: Arc<std::sync::Mutex<Option<u64>>>,
    /// Wall-clock deadline for the currently executing script. Set by execute_script_to_files,
    /// read by the on_progress callback registered in register_host_fns.
    pub current_deadline: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    /// Buffer for print() / debug() output; drained after each script execution.
    pub print_buffer: Arc<std::sync::Mutex<String>>,
}

pub fn build_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(env_or_default("CLICOM_MAX_OPS", 10_000_000) as u64);
    engine.set_max_call_levels(64);
    engine.set_max_string_size(4 * 1024 * 1024);
    engine.set_max_array_size(10_000);
    engine.set_max_map_size(10_000);
    engine.disable_symbol("eval");
    engine
}

fn env_or_default(name: &str, def: usize) -> usize {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(def)
}

pub fn register_host_fns(engine: &mut Engine, ctx: Arc<HostContext>) {
    // type_text(s) — default: translate bare \n to \r
    let c = Arc::clone(&ctx);
    engine.register_fn("type_text", move |s: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        c.nudge_tx.send(translate_newlines(s))
            .map_err(|_| Box::new(rhai::EvalAltResult::ErrorRuntime("type_text: channel closed".into(), rhai::Position::NONE)))?;
        Ok(())
    });

    // type_text(s, translate) — explicit translate flag
    let c = Arc::clone(&ctx);
    engine.register_fn("type_text", move |s: &str, translate: bool| -> Result<(), Box<rhai::EvalAltResult>> {
        let bytes = if translate { translate_newlines(s) } else { s.as_bytes().to_vec() };
        c.nudge_tx.send(bytes)
            .map_err(|_| Box::new(rhai::EvalAltResult::ErrorRuntime("type_text: channel closed".into(), rhai::Position::NONE)))?;
        Ok(())
    });

    // type_keys — bracketed shortcut key spec
    let c = Arc::clone(&ctx);
    engine.register_fn("type_keys", move |spec: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        let bytes = parse_key_spec(spec)
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("type_keys: {e}").into(), rhai::Position::NONE)))?;
        c.nudge_tx.send(bytes)
            .map_err(|_| Box::new(rhai::EvalAltResult::ErrorRuntime("type_keys: channel closed".into(), rhai::Position::NONE)))?;
        Ok(())
    });

    // screen_text
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_text", move || -> String { c.screen.to_plain_text() });

    // screen_save
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_save", move |path: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        let body = c.screen.to_plain_text();
        let resolved = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&resolved, body.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("fs: {e}").into(), rhai::Position::NONE)))?;
        Ok(body.as_bytes().len() as i64)
    });

    // screen_last_after
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_last_after", move |marker: &str| -> String {
        let lifetime = c.screen.lifetime_text();
        match lifetime.rfind(marker) {
            Some(idx) => lifetime[idx + marker.len()..].to_string(),
            None => String::new(),
        }
    });

    // screen_save_last_after
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_save_last_after", move |path: &str, marker: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        let lifetime = c.screen.lifetime_text();
        let body = match lifetime.rfind(marker) { Some(i) => lifetime[i + marker.len()..].to_string(), None => String::new() };
        let resolved = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&resolved, body.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("fs: {e}").into(), rhai::Position::NONE)))?;
        Ok(body.as_bytes().len() as i64)
    });

    // screen_last_after_re
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_last_after_re", move |pattern: &str| -> Result<String, Box<rhai::EvalAltResult>> {
        let re = regex::Regex::new(pattern)
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("regex compile: {e}").into(), rhai::Position::NONE)))?;
        let lifetime = c.screen.lifetime_text();
        let mut last_end: Option<usize> = None;
        for m in re.find_iter(&lifetime) { last_end = Some(m.end()); }
        Ok(last_end.map(|i| lifetime[i..].to_string()).unwrap_or_default())
    });

    // screen_save_last_after_re
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_save_last_after_re", move |path: &str, pattern: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        let re = regex::Regex::new(pattern)
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("regex compile: {e}").into(), rhai::Position::NONE)))?;
        let lifetime = c.screen.lifetime_text();
        let mut last_end: Option<usize> = None;
        for m in re.find_iter(&lifetime) { last_end = Some(m.end()); }
        let body = last_end.map(|i| lifetime[i..].to_string()).unwrap_or_default();
        let resolved = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&resolved, body.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("fs: {e}").into(), rhai::Position::NONE)))?;
        Ok(body.as_bytes().len() as i64)
    });

    // screen_tail_text
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_tail_text", move |from: i64, to: i64| -> Result<String, Box<rhai::EvalAltResult>> {
        let (a, b) = resolve_indexes(&c.screen, from, to)?;
        let r = c.screen.read_range(a, b);
        Ok(r.lines.join("\n"))
    });

    // wait_ms
    let _c = Arc::clone(&ctx);
    engine.register_fn("wait_ms", move |ms: i64| -> Result<(), Box<rhai::EvalAltResult>> {
        if ms > 600_000 {
            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime("wait_ms: cap exceeded".into(), rhai::Position::NONE)));
        }
        std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
        Ok(())
    });

    // wait_idle (1-arg) — default timeout 60_000
    let c = Arc::clone(&ctx);
    engine.register_fn("wait_idle", move |ms: i64| -> Result<(), Box<rhai::EvalAltResult>> {
        wait_idle_impl(&c, ms, 60_000)
    });

    // wait_idle (2-arg)
    let c = Arc::clone(&ctx);
    engine.register_fn("wait_idle", move |ms: i64, timeout_ms: i64| -> Result<(), Box<rhai::EvalAltResult>> {
        wait_idle_impl(&c, ms, timeout_ms)
    });

    // status
    let c = Arc::clone(&ctx);
    engine.register_fn("status", move || -> rhai::Map {
        let mut m = rhai::Map::new();
        let (st, last_sys) = c.idle_observer.lock()
            .map(|d| (d.state(), d.last_activity()))
            .unwrap_or((crate::clicom_engine::idle::IdleState::Busy, std::time::SystemTime::now()));
        m.insert("state".into(), format!("{:?}", st).to_lowercase().into());
        let last_activity_str = chrono::DateTime::<chrono::Utc>::from(last_sys).to_rfc3339();
        m.insert("last_activity".into(), last_activity_str.into());
        let (lt, tb) = c.screen.lifetime_info();
        m.insert("lifetime_lines".into(), (lt as i64).into());
        m.insert("trimmed_below".into(), (tb as i64).into());
        let (rows, cols) = c.screen.visible_dims();
        m.insert("visible_rows".into(), (rows as i64).into());
        m.insert("visible_cols".into(), (cols as i64).into());
        m
    });

    // set_timeout
    let c = Arc::clone(&ctx);
    engine.register_fn("set_timeout", move |ms: i64| -> Result<(), Box<rhai::EvalAltResult>> {
        if ms > 3_600_000 {
            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime("set_timeout: cap exceeded".into(), rhai::Position::NONE)));
        }
        let new_deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms.max(0) as u64);
        // Update both: current_deadline for immediate on_progress enforcement,
        // and script_timeout_override in case execute_script_to_files hasn't set current_deadline yet.
        *c.current_deadline.lock().unwrap() = Some(new_deadline);
        *c.script_timeout_override.lock().unwrap() = Some(ms.max(0) as u64);
        Ok(())
    });

    // on_progress: abort script if wall-clock deadline has passed.
    let deadline_holder = Arc::clone(&ctx.current_deadline);
    engine.on_progress(move |_ops| {
        if let Ok(guard) = deadline_holder.lock() {
            if let Some(dl) = *guard {
                if std::time::Instant::now() > dl {
                    return Some(rhai::Dynamic::from(()));
                }
            }
        }
        None
    });

    // screen_tail_save
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_tail_save", move |path: &str, from: i64, to: i64| -> Result<rhai::Map, Box<rhai::EvalAltResult>> {
        let (a, b) = resolve_indexes(&c.screen, from, to)?;
        let r = c.screen.read_range(a, b);
        let header = format!("# requested: {from}..{to}  actual: {}..{}  total_lifetime: {}  trimmed_below: {}\n",
                             r.actual_from, r.actual_to, r.total_lifetime, r.trimmed_below);
        let body = format!("{}{}", header, r.lines.join("\n"));
        let resolved = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&resolved, body.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("fs: {e}").into(), rhai::Position::NONE)))?;
        let mut m = rhai::Map::new();
        m.insert("actual_from".into(), (r.actual_from as i64).into());
        m.insert("actual_to".into(),   (r.actual_to as i64).into());
        m.insert("total_lifetime".into(), (r.total_lifetime as i64).into());
        m.insert("trimmed_below".into(),  (r.trimmed_below as i64).into());
        m.insert("bytes".into(),       (body.as_bytes().len() as i64).into());
        Ok(m)
    });

    // ── File I/O ──────────────────────────────────────────────────────────────

    // read_file(path) -> String
    let c = Arc::clone(&ctx);
    engine.register_fn("read_file", move |path: &str| -> Result<String, Box<rhai::EvalAltResult>> {
        let p = resolve_path(&c.instance_cwd, path);
        std::fs::read_to_string(&p).map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
            format!("fs: read_file({}): {e}", p.display()).into(), rhai::Position::NONE)))
    });

    // write_file(path, content) -> i64
    let c = Arc::clone(&ctx);
    engine.register_fn("write_file", move |path: &str, content: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        let p = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&p, content.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("fs: write_file({}): {e}", p.display()).into(), rhai::Position::NONE)))?;
        Ok(content.as_bytes().len() as i64)
    });

    // append_file(path, content) -> i64
    let c = Arc::clone(&ctx);
    engine.register_fn("append_file", move |path: &str, content: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        use std::io::Write;
        let p = resolve_path(&c.instance_cwd, path);
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p)
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("fs: append_file open({}): {e}", p.display()).into(), rhai::Position::NONE)))?;
        f.write_all(content.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("fs: append_file write({}): {e}", p.display()).into(), rhai::Position::NONE)))?;
        Ok(content.as_bytes().len() as i64)
    });

    // delete_file(path) -> ()  — no error if file is absent
    let c = Arc::clone(&ctx);
    engine.register_fn("delete_file", move |path: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        let p = resolve_path(&c.instance_cwd, path);
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("fs: delete_file({}): {e}", p.display()).into(), rhai::Position::NONE))),
        }
    });

    // mkdirp(path) -> ()
    let c = Arc::clone(&ctx);
    engine.register_fn("mkdirp", move |path: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        let p = resolve_path(&c.instance_cwd, path);
        std::fs::create_dir_all(&p)
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("fs: mkdirp({}): {e}", p.display()).into(), rhai::Position::NONE)))
    });

    // ── Network ───────────────────────────────────────────────────────────────

    // fetch_url(url) -> Map { status, body }
    engine.register_fn("fetch_url", move |url: &str| -> Result<rhai::Map, Box<rhai::EvalAltResult>> {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build();
        let mut m = rhai::Map::new();
        match agent.get(url).call() {
            Ok(r) => {
                m.insert("status".into(), (r.status() as i64).into());
                let body = r.into_string().map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("fetch_url: read body: {e}").into(), rhai::Position::NONE)))?;
                m.insert("body".into(), body.into());
            }
            Err(ureq::Error::Status(code, r)) => {
                m.insert("status".into(), (code as i64).into());
                m.insert("body".into(), r.into_string().unwrap_or_default().into());
            }
            Err(e) => return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("fetch_url: {e}").into(), rhai::Position::NONE))),
        }
        Ok(m)
    });

    // ── Shell ─────────────────────────────────────────────────────────────────

    // shell_execute(cmd) -> Map { exit_code, stdout, stderr }
    engine.register_fn("shell_execute", move |cmd: &str| -> Result<rhai::Map, Box<rhai::EvalAltResult>> {
        use std::process::Command;
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").arg("/C").arg(cmd).output()
        } else {
            Command::new("sh").arg("-c").arg(cmd).output()
        }
        .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(
            format!("shell_execute: spawn: {e}").into(), rhai::Position::NONE)))?;
        let mut m = rhai::Map::new();
        m.insert("exit_code".into(), (output.status.code().unwrap_or(-1) as i64).into());
        m.insert("stdout".into(), String::from_utf8_lossy(&output.stdout).to_string().into());
        m.insert("stderr".into(), String::from_utf8_lossy(&output.stderr).to_string().into());
        Ok(m)
    });

    // parse_json
    engine.register_fn("parse_json", |s: &str| -> Result<rhai::Dynamic, Box<rhai::EvalAltResult>> {
        let v: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("parse_json: {e}").into(), rhai::Position::NONE)))?;
        Ok(json_value_to_dynamic(v))
    });

    // to_json
    engine.register_fn("to_json", |v: rhai::Dynamic| -> Result<String, Box<rhai::EvalAltResult>> {
        dyn_to_json(&v).map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("to_json: {e}").into(), rhai::Position::NONE)))
    });

    // on_print: capture print() output to print_buffer
    let c = Arc::clone(&ctx);
    engine.on_print(move |s| {
        if let Ok(mut buf) = c.print_buffer.lock() {
            buf.push_str(s);
            buf.push('\n');
        }
    });

    // on_debug: capture debug() output to print_buffer
    let c = Arc::clone(&ctx);
    engine.on_debug(move |s, _src, _pos| {
        if let Ok(mut buf) = c.print_buffer.lock() {
            buf.push_str("[debug] ");
            buf.push_str(s);
            buf.push('\n');
        }
    });
}

fn resolve_path(cwd: &std::path::Path, p: &str) -> std::path::PathBuf {
    let pp = std::path::Path::new(p);
    if pp.is_absolute() { pp.to_path_buf() } else { cwd.join(pp) }
}

fn wait_idle_impl(ctx: &HostContext, ms: i64, timeout_ms: i64) -> Result<(), Box<rhai::EvalAltResult>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms.max(0) as u64);
    let needed = std::time::Duration::from_millis(ms.max(0) as u64);
    let mut idle_since: Option<std::time::Instant> = None;
    loop {
        let now = std::time::Instant::now();
        let st = ctx.idle_observer.lock().map(|d| d.state()).unwrap_or(crate::clicom_engine::idle::IdleState::Busy);
        match st {
            crate::clicom_engine::idle::IdleState::Idle => {
                let s = idle_since.get_or_insert(now);
                if now.duration_since(*s) >= needed { return Ok(()); }
            }
            crate::clicom_engine::idle::IdleState::Busy => { idle_since = None; }
        }
        if now >= deadline {
            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("wait_idle: timeout after {timeout_ms}ms").into(), rhai::Position::NONE)));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn resolve_indexes(buf: &ScreenBuffer, from: i64, to: i64) -> Result<(u64, u64), Box<rhai::EvalAltResult>> {
    // Reject obviously inverted ranges before clamping.
    if from > to {
        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime("bad range".into(), rhai::Position::NONE)));
    }
    let (total, _trim) = buf.lifetime_info();
    let resolve = |x: i64| -> u64 {
        if x >= 0 { (x as u64).min(total) } else {
            let off = (-x) as u64;
            if off > total { 0 } else { total - off }
        }
    };
    let a = resolve(from);
    let b = resolve(to);
    if a > b {
        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime("bad range".into(), rhai::Position::NONE)));
    }
    if buf.range_wholly_trimmed(a, b) {
        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime("requested below trim watermark".into(), rhai::Position::NONE)));
    }
    Ok((a, b))
}

// ── JSON helpers ─────────────────────────────────────────────────────────────

fn json_value_to_dynamic(v: serde_json::Value) -> rhai::Dynamic {
    use serde_json::Value;
    match v {
        Value::Null => rhai::Dynamic::UNIT,
        Value::Bool(b) => rhai::Dynamic::from(b),
        Value::Number(n) => {
            if n.is_i64() {
                rhai::Dynamic::from(n.as_i64().unwrap())
            } else if n.is_f64() {
                rhai::Dynamic::from(n.as_f64().unwrap())
            } else {
                rhai::Dynamic::from(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => rhai::Dynamic::from(s),
        Value::Array(arr) => {
            rhai::Dynamic::from(arr.into_iter().map(json_value_to_dynamic).collect::<rhai::Array>())
        }
        Value::Object(map) => {
            rhai::Dynamic::from(
                map.into_iter()
                    .map(|(k, v)| (k.into(), json_value_to_dynamic(v)))
                    .collect::<rhai::Map>()
            )
        }
    }
}

fn dynamic_to_json_value(v: &rhai::Dynamic) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    if v.is_unit() { return Ok(Value::Null); }
    if let Some(b) = v.clone().try_cast::<bool>() { return Ok(Value::Bool(b)); }
    if let Some(i) = v.clone().try_cast::<i64>() { return Ok(Value::Number(i.into())); }
    if let Some(f) = v.clone().try_cast::<f64>() {
        return serde_json::Number::from_f64(f)
            .map(Value::Number)
            .ok_or_else(|| format!("non-finite float: {f}"));
    }
    if let Some(s) = v.clone().try_cast::<String>() { return Ok(Value::String(s)); }
    if let Some(arr) = v.clone().try_cast::<rhai::Array>() {
        let mut out = Vec::with_capacity(arr.len());
        for item in arr.iter() { out.push(dynamic_to_json_value(item)?); }
        return Ok(Value::Array(out));
    }
    if let Some(map) = v.clone().try_cast::<rhai::Map>() {
        let mut out = serde_json::Map::with_capacity(map.len());
        for (k, val) in map.iter() { out.insert(k.to_string(), dynamic_to_json_value(val)?); }
        return Ok(Value::Object(out));
    }
    // Fallback: stringify unknown types (Char, etc).
    Ok(Value::String(v.to_string()))
}

fn dyn_to_json(v: &rhai::Dynamic) -> Result<String, String> {
    let json = dynamic_to_json_value(v)?;
    serde_json::to_string(&json).map_err(|e| e.to_string())
}

// ── Per-script execution ─────────────────────────────────────────────────────

/// Outcome of executing a single Rhai script through `execute_script_to_files`.
pub enum ScriptOutcome {
    Ok,
    /// Short code: "parse" | "runtime" | "timeout" | "host_fn" | "fs" | "range" | "internal"
    Err(&'static str),
}

/// Parse + run `source`, then write `.out`, optionally `.err`, and `.done` atomically.
/// The `.done` file is the readiness barrier (always written last).
/// If any print()/debug() output was generated, writes it to `log_path`.
pub fn execute_script_to_files(
    engine: &Engine,
    source: &str,
    out_path: &std::path::Path,
    err_path: &std::path::Path,
    done_path: &std::path::Path,
    log_path: &std::path::Path,
    deadline: std::time::Instant,
    host_ctx: &Arc<HostContext>,
) -> ScriptOutcome {
    // Compute effective deadline: script_timeout_override takes priority over default.
    let effective_deadline = {
        let mut ov = host_ctx.script_timeout_override.lock().unwrap();
        if let Some(ms) = ov.take() {
            std::time::Instant::now() + std::time::Duration::from_millis(ms)
        } else {
            deadline
        }
    };
    // Arm the on_progress deadline checker.
    *host_ctx.current_deadline.lock().unwrap() = Some(effective_deadline);

    let ast = match engine.compile(source) {
        Ok(ast) => ast,
        Err(e) => {
            *host_ctx.current_deadline.lock().unwrap() = None;
            flush_print_buffer(host_ctx, log_path);
            return write_failure(out_path, err_path, done_path, "parse", &e.to_string());
        }
    };
    let mut scope = rhai::Scope::new();
    let result: Result<rhai::Dynamic, Box<rhai::EvalAltResult>> =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.eval_ast_with_scope(&mut scope, &ast)
        })) {
            Ok(r) => r,
            Err(_payload) => {
                *host_ctx.current_deadline.lock().unwrap() = None;
                flush_print_buffer(host_ctx, log_path);
                return write_failure(out_path, err_path, done_path, "internal",
                                     "internal panic in script evaluation");
            }
        };
    // Disarm deadline before processing result.
    *host_ctx.current_deadline.lock().unwrap() = None;
    // Flush print buffer regardless of success/failure.
    flush_print_buffer(host_ctx, log_path);
    match result {
        Ok(v) => {
            let json = match dyn_to_json(&v) {
                Ok(j) => j,
                Err(e) => return write_failure(out_path, err_path, done_path, "internal",
                                               &format!("json encode: {e}")),
            };
            if let Err(e) = crate::clicom_engine::fs_atomic::write(out_path, json.as_bytes()) {
                return write_failure(out_path, err_path, done_path, "fs", &format!("{e}"));
            }
            let _ = crate::clicom_engine::fs_atomic::write(done_path, b"OK\n");
            ScriptOutcome::Ok
        }
        Err(e) => {
            let code = classify_error(&e);
            write_failure(out_path, err_path, done_path, code, &format!("{e}"))
        }
    }
}

/// Drain the print buffer and write to log_path if non-empty. Always resets the buffer.
fn flush_print_buffer(host_ctx: &Arc<HostContext>, log_path: &std::path::Path) {
    let contents = {
        let mut buf = host_ctx.print_buffer.lock().unwrap();
        std::mem::take(&mut *buf)
    };
    if !contents.is_empty() {
        let _ = crate::clicom_engine::fs_atomic::write(log_path, contents.as_bytes());
    }
}

fn classify_error(e: &rhai::EvalAltResult) -> &'static str {
    use rhai::EvalAltResult::*;
    match e {
        ErrorParsing(_, _) => "parse",
        ErrorTerminated(_, _) => "timeout",
        ErrorRuntime(msg, _) => {
            let s = msg.to_string();
            if s.contains("timeout") { "host_fn" }
            else if s.contains("requested below trim watermark") { "range" }
            else if s.starts_with("fs:") { "fs" }
            else if s.contains("cap exceeded") || s.contains("type_text:") || s.contains("type_keys:") || s.contains("shell_execute:") { "host_fn" }
            else { "runtime" }
        }
        ErrorTooManyOperations(_) => "runtime",
        _ => "runtime",
    }
}

fn write_failure(out: &std::path::Path, err: &std::path::Path, done: &std::path::Path,
                 code: &'static str, message: &str) -> ScriptOutcome {
    let _ = crate::clicom_engine::fs_atomic::write(out, b"null\n");
    let _ = crate::clicom_engine::fs_atomic::write(err, format!("{code}\n{message}\n").as_bytes());
    let _ = crate::clicom_engine::fs_atomic::write(done, format!("ERR {code}\n").as_bytes());
    ScriptOutcome::Err(code)
}

pub fn run_script(engine: &Engine, source: &str) -> Result<rhai::Dynamic, rhai::EvalAltResult> {
    let ast = engine.compile(source).map_err(|e| *Box::new(rhai::EvalAltResult::ErrorParsing(*e.0, e.1)))?;
    let mut scope = Scope::new();
    engine.eval_ast_with_scope::<rhai::Dynamic>(&mut scope, &ast).map_err(|e| *e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_runs_pure_script() {
        let e = build_engine();
        let v = run_script(&e, "1 + 2").unwrap();
        assert_eq!(v.as_int().unwrap(), 3);
    }

    #[test]
    fn eval_is_disabled() {
        let e = build_engine();
        let r = run_script(&e, "eval(\"1+1\")");
        assert!(r.is_err(), "expected eval to be disabled");
    }

    #[test]
    fn wait_ms_above_cap_throws() {
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::clone(&screen)));
        assert!(run_script(&e, "wait_ms(700000)").is_err());
    }

    #[test]
    fn set_timeout_above_cap_throws() {
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::clone(&screen)));
        assert!(run_script(&e, "set_timeout(7200000)").is_err());
    }

    #[test]
    fn last_after_literal_returns_post_marker_tail() {
        let screen = Arc::new(ScreenBuffer::new(20, 80));
        screen.advance_bytes(b"prelude marker tail\n");
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::clone(&screen)));
        let v = run_script(&e, "screen_last_after(\"marker\")").unwrap();
        let s = v.into_string().unwrap();
        assert!(s.contains("tail"));
    }

    #[test]
    fn last_after_marker_not_found_returns_empty() {
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        screen.advance_bytes(b"nothing here\n");
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::clone(&screen)));
        let v = run_script(&e, "screen_last_after(\"absent\")").unwrap();
        assert_eq!(v.into_string().unwrap(), "");
    }

    #[test]
    fn last_after_re_compile_error_throws() {
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::clone(&screen)));
        let r = run_script(&e, "screen_last_after_re(\"(\")");
        assert!(r.is_err());
    }

    fn make_ctx(screen: Arc<ScreenBuffer>) -> Arc<HostContext> {
        make_ctx_with_cwd(screen, std::env::temp_dir())
    }

    fn make_ctx_with_cwd(screen: Arc<ScreenBuffer>, cwd: std::path::PathBuf) -> Arc<HostContext> {
        let (tx, _rx) = crossbeam_channel::unbounded();
        Arc::new(HostContext {
            screen,
            status: Arc::new(std::sync::Mutex::new(crate::clicom_engine::meta::Status::initial_busy())),
            nudge_tx: tx,
            instance_cwd: cwd,
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
            script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
            current_deadline: Arc::new(std::sync::Mutex::new(None)),
            print_buffer: Arc::new(std::sync::Mutex::new(String::new())),
        })
    }

    /// Build a fresh ctx with its own accessible rx channel.
    fn make_ctx_with_rx(cwd: std::path::PathBuf) -> (Arc<HostContext>, crossbeam_channel::Receiver<Vec<u8>>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let ctx = Arc::new(HostContext {
            screen: Arc::new(ScreenBuffer::new(10, 80)),
            status: Arc::new(std::sync::Mutex::new(crate::clicom_engine::meta::Status::initial_busy())),
            nudge_tx: tx,
            instance_cwd: cwd,
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
            script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
            current_deadline: Arc::new(std::sync::Mutex::new(None)),
            print_buffer: Arc::new(std::sync::Mutex::new(String::new())),
        });
        (ctx, rx)
    }

    #[test]
    fn screen_tail_text_negative_index() {
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        for i in 0..3 { screen.advance_bytes(format!("L{i}\n").as_bytes()); }
        let ctx = make_ctx(Arc::clone(&screen));
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, "screen_tail_text(-3, -1)").unwrap();
        let s = v.into_string().unwrap();
        assert!(s.lines().count() <= 3);
    }

    #[test]
    fn screen_tail_text_bad_range_throws() {
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let r = run_script(&e, "screen_tail_text(10, 5)");
        assert!(r.is_err());
    }

    #[test]
    fn screen_text_returns_visible_text() {
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        screen.advance_bytes(b"hello world\n");
        let ctx = make_ctx(Arc::clone(&screen));
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, "screen_text()").unwrap();
        let s = v.into_string().unwrap();
        assert!(s.contains("hello"), "got: {s:?}");
    }

    #[test]
    fn execute_writes_done_after_out() {
        let td = tempfile::TempDir::new().unwrap();
        let out = td.path().join("id.out");
        let err = td.path().join("id.err");
        let done = td.path().join("id.done");
        let log = td.path().join("id.log");
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        let outcome = execute_script_to_files(
            &e, "1 + 2", &out, &err, &done, &log,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &ctx,
        );
        assert!(matches!(outcome, ScriptOutcome::Ok));
        let out_body = std::fs::read_to_string(&out).unwrap();
        let done_body = std::fs::read_to_string(&done).unwrap();
        assert!(out_body.trim().contains("3"));
        assert!(done_body.trim_end() == "OK");
        assert!(!err.exists());
    }

    #[test]
    fn execute_failure_writes_err_and_done_err() {
        let td = tempfile::TempDir::new().unwrap();
        let out = td.path().join("id.out");
        let err = td.path().join("id.err");
        let done = td.path().join("id.done");
        let log = td.path().join("id.log");
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        let outcome = execute_script_to_files(
            &e, "let x: int = \"bad\";", &out, &err, &done, &log,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &ctx,
        );
        assert!(matches!(outcome, ScriptOutcome::Err(_)));
        assert!(std::fs::read_to_string(&done).unwrap().starts_with("ERR "));
        assert!(std::fs::read_to_string(&err).unwrap().lines().next().is_some());
    }

    #[test]
    fn type_text_pushes_into_channel() {
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(10, 80)));
        // Grab rx before ctx is moved into register_host_fns.
        // We need a fresh channel with rx accessible — use make_ctx and reconstruct.
        let (tx, rx) = crossbeam_channel::unbounded();
        let ctx2 = Arc::new(HostContext {
            screen: Arc::new(ScreenBuffer::new(10, 80)),
            status: Arc::new(std::sync::Mutex::new(crate::clicom_engine::meta::Status::initial_busy())),
            nudge_tx: tx,
            instance_cwd: std::env::temp_dir(),
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
            script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
            current_deadline: Arc::new(std::sync::Mutex::new(None)),
            print_buffer: Arc::new(std::sync::Mutex::new(String::new())),
        });
        let _ = ctx; // unused, suppress warning
        let mut e = build_engine();
        register_host_fns(&mut e, ctx2);
        let _ = run_script(&e, "type_text(\"hi\\n\")").unwrap();
        let bytes = rx.recv().unwrap();
        // Default translation: bare \n → \r
        assert_eq!(bytes, b"hi\r");
    }

    #[test]
    fn panic_in_host_fn_yields_internal() {
        let td = tempfile::TempDir::new().unwrap();
        let out = td.path().join("id.out");
        let err = td.path().join("id.err");
        let done = td.path().join("id.done");
        let log = td.path().join("id.log");
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        // Register a host fn that panics.
        e.register_fn("do_panic", || -> () { panic!("deliberate test panic"); });
        let outcome = execute_script_to_files(
            &e, "do_panic()", &out, &err, &done, &log,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &ctx,
        );
        assert!(matches!(outcome, ScriptOutcome::Err("internal")));
        let done_body = std::fs::read_to_string(&done).unwrap();
        assert!(done_body.starts_with("ERR internal"), "got: {done_body:?}");
    }

    #[test]
    fn set_timeout_aborts_long_wait_ms() {
        let td = tempfile::TempDir::new().unwrap();
        let out = td.path().join("id.out");
        let err = td.path().join("id.err");
        let done = td.path().join("id.done");
        let log = td.path().join("id.log");
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        let start = std::time::Instant::now();
        // set_timeout(500) then wait_ms(2000): should abort within ~600ms.
        // Note: wait_ms sleeps in a tight loop; on_progress fires between rhai ops.
        // The script will sleep 2s unless the deadline aborts it.
        // We use a very short wait_ms loop to give on_progress a chance to fire.
        let outcome = execute_script_to_files(
            &e,
            "set_timeout(500); let i = 0; loop { wait_ms(10); i += 1; if i > 300 { break; } }",
            &out, &err, &done, &log,
            std::time::Instant::now() + std::time::Duration::from_secs(60),
            &ctx,
        );
        let elapsed = start.elapsed();
        assert!(matches!(outcome, ScriptOutcome::Err("timeout")),
                "expected timeout, got {:?}", std::fs::read_to_string(&done).ok());
        assert!(elapsed < std::time::Duration::from_secs(3),
                "should abort fast, took {:?}", elapsed);
        let done_body = std::fs::read_to_string(&done).unwrap();
        assert!(done_body.starts_with("ERR timeout"), "got: {done_body:?}");
    }

    // ── New tests: parse_json / to_json / print capture ───────────────────────

    #[test]
    fn parse_json_object_yields_map() {
        let mut e = build_engine();
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, r#"parse_json("{\"a\":1,\"b\":[2,3]}")"#).unwrap();
        assert!(v.clone().try_cast::<rhai::Map>().is_some(), "expected Map, got: {v:?}");
    }

    #[test]
    fn to_json_round_trip() {
        let mut e = build_engine();
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, r#"to_json(parse_json("{\"x\":42}"))"#).unwrap();
        let s = v.into_string().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["x"], serde_json::json!(42));
    }

    #[test]
    fn to_json_handles_array_and_map_correctly() {
        let mut e = build_engine();
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, r#"to_json([1,"two",#{three:3}])"#).unwrap();
        let s = v.into_string().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).expect("should be valid JSON");
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr[0], serde_json::json!(1));
        assert_eq!(arr[1], serde_json::json!("two"));
        assert_eq!(arr[2]["three"], serde_json::json!(3));
    }

    #[test]
    fn parse_json_invalid_throws() {
        let mut e = build_engine();
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        register_host_fns(&mut e, ctx);
        let r = run_script(&e, r#"parse_json("not json")"#);
        assert!(r.is_err(), "expected parse_json to throw on invalid JSON");
    }

    #[test]
    fn print_appends_to_buffer() {
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        let _ = run_script(&e, r#"print("hi"); print("ok")"#).unwrap();
        let buf = ctx.print_buffer.lock().unwrap().clone();
        assert_eq!(buf, "hi\nok\n");
    }

    #[test]
    fn execute_writes_log_when_print_used() {
        let td = tempfile::TempDir::new().unwrap();
        let out = td.path().join("id.out");
        let err = td.path().join("id.err");
        let done = td.path().join("id.done");
        let log = td.path().join("id.log");
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        let outcome = execute_script_to_files(
            &e, r#"print("hello from script"); 42"#, &out, &err, &done, &log,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &ctx,
        );
        assert!(matches!(outcome, ScriptOutcome::Ok));
        assert!(log.exists(), "log file should exist");
        let log_body = std::fs::read_to_string(&log).unwrap();
        assert!(log_body.contains("hello from script"), "got: {log_body:?}");
    }

    #[test]
    fn execute_no_log_when_no_print() {
        let td = tempfile::TempDir::new().unwrap();
        let out = td.path().join("id.out");
        let err = td.path().join("id.err");
        let done = td.path().join("id.done");
        let log = td.path().join("id.log");
        let ctx = make_ctx(Arc::new(ScreenBuffer::new(5, 80)));
        let mut e = build_engine();
        register_host_fns(&mut e, Arc::clone(&ctx));
        let outcome = execute_script_to_files(
            &e, "1 + 1", &out, &err, &done, &log,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &ctx,
        );
        assert!(matches!(outcome, ScriptOutcome::Ok));
        assert!(!log.exists(), "log file should NOT exist when no print/debug used");
    }

    // ── type_text translate tests ─────────────────────────────────────────────

    #[test]
    fn type_text_raw_passthrough() {
        let (ctx, rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_text(\"hi\\n\", false)").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"hi\n");
    }

    #[test]
    fn type_text_preserves_existing_crlf() {
        let (ctx, rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        // \r\n in the string literal: translate should NOT double the \r
        let _ = run_script(&e, "type_text(\"hi\\r\\n\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"hi\r\n");
    }

    // ── type_keys tests ───────────────────────────────────────────────────────

    #[test]
    fn type_keys_basic_chord() {
        let (ctx, rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_keys(\"[Ctrl+C]\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"\x03");
    }

    #[test]
    fn type_keys_arrow_keys() {
        let (ctx, rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_keys(\"[Up][Down]\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"\x1b[A\x1b[B");
    }

    #[test]
    fn type_keys_mixed_text_and_keys() {
        let (ctx, rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_keys(\"hi[Enter]\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"hi\r");
    }

    #[test]
    fn type_keys_unknown_token_errors() {
        let (ctx, _rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let r = run_script(&e, "type_keys(\"[NoSuchKey]\")");
        assert!(r.is_err(), "expected error for unknown key token");
    }

    #[test]
    fn type_keys_alt_chord() {
        let (ctx, rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_keys(\"[Alt+a]\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"\x1ba");
    }

    #[test]
    fn type_keys_unterminated_bracket_errors() {
        let (ctx, _rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let r = run_script(&e, "type_keys(\"[Up\")");
        assert!(r.is_err(), "expected error for unterminated bracket");
    }

    // ── File I/O tests ────────────────────────────────────────────────────────

    #[test]
    fn read_write_file_round_trip() {
        let td = tempfile::TempDir::new().unwrap();
        let (ctx, _rx) = make_ctx_with_rx(td.path().to_path_buf());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "write_file(\"rw.txt\", \"hello world\")").unwrap();
        let v = run_script(&e, "read_file(\"rw.txt\")").unwrap();
        let s = v.into_string().unwrap();
        assert_eq!(s, "hello world");
    }

    #[test]
    fn append_file_appends() {
        let td = tempfile::TempDir::new().unwrap();
        let (ctx, _rx) = make_ctx_with_rx(td.path().to_path_buf());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "write_file(\"app.txt\", \"part1\")").unwrap();
        let _ = run_script(&e, "append_file(\"app.txt\", \"part2\")").unwrap();
        let v = run_script(&e, "read_file(\"app.txt\")").unwrap();
        let s = v.into_string().unwrap();
        assert!(s.contains("part1") && s.contains("part2"), "got: {s:?}");
    }

    #[test]
    fn delete_file_idempotent_for_missing() {
        let td = tempfile::TempDir::new().unwrap();
        let (ctx, _rx) = make_ctx_with_rx(td.path().to_path_buf());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        // Deleting a non-existent file must not error
        let _ = run_script(&e, "delete_file(\"does_not_exist.txt\")").unwrap();
    }

    #[test]
    fn mkdirp_creates_nested_dirs() {
        let td = tempfile::TempDir::new().unwrap();
        let (ctx, _rx) = make_ctx_with_rx(td.path().to_path_buf());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "mkdirp(\"a/b/c\")").unwrap();
        assert!(td.path().join("a/b/c").is_dir());
        // Calling twice must also be OK
        let _ = run_script(&e, "mkdirp(\"a/b/c\")").unwrap();
    }

    // ── Shell test ────────────────────────────────────────────────────────────

    #[test]
    fn shell_execute_runs_echo() {
        let (ctx, _rx) = make_ctx_with_rx(std::env::temp_dir());
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, "shell_execute(\"echo hello\")").unwrap();
        let m = v.try_cast::<rhai::Map>().unwrap();
        let exit_code = m["exit_code"].clone().as_int().unwrap();
        let stdout = m["stdout"].clone().into_string().unwrap();
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("hello"), "stdout: {stdout:?}");
    }
}
