//! Rhai engine setup + host-fn registration shared shape.
//!
//! Limits per §6.3. Host fns are registered by `register_host_fns` (Task 22+).

use rhai::{Engine, Scope};
use std::sync::Arc;
use crate::clicom_engine::screen::ScreenBuffer;

pub struct HostContext {
    pub screen: Arc<ScreenBuffer>,
    pub nudge_tx: crossbeam_channel::Sender<Vec<u8>>,
    /// The wrapper's process cwd (per spec §4 — used to resolve relative paths in host fns).
    pub instance_cwd: std::path::PathBuf,
    pub idle_observer: Arc<std::sync::Mutex<crate::clicom_engine::idle::IdleDetector>>,
    pub script_timeout_override: Arc<std::sync::Mutex<Option<u64>>>,
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
    // type_text
    let c = Arc::clone(&ctx);
    engine.register_fn("type_text", move |s: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        c.nudge_tx.send(s.as_bytes().to_vec())
            .map_err(|_| Box::new(rhai::EvalAltResult::ErrorRuntime("type_text: channel closed".into(), rhai::Position::NONE)))?;
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
        let st = c.idle_observer.lock().map(|d| d.state()).unwrap_or(crate::clicom_engine::idle::IdleState::Busy);
        m.insert("state".into(), format!("{:?}", st).to_lowercase().into());
        m.insert("last_activity".into(), chrono::Utc::now().to_rfc3339().into());
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
        *c.script_timeout_override.lock().unwrap() = Some(ms.max(0) as u64);
        Ok(())
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

// ── Per-script execution ─────────────────────────────────────────────────────

/// Outcome of executing a single Rhai script through `execute_script_to_files`.
pub enum ScriptOutcome {
    Ok,
    /// Short code: "parse" | "runtime" | "timeout" | "host_fn" | "fs" | "range" | "internal"
    Err(&'static str),
}

/// Parse + run `source`, then write `.out`, optionally `.err`, and `.done` atomically.
/// The `.done` file is the readiness barrier (always written last).
pub fn execute_script_to_files(
    engine: &Engine,
    source: &str,
    out_path: &std::path::Path,
    err_path: &std::path::Path,
    done_path: &std::path::Path,
    deadline: std::time::Instant,
) -> ScriptOutcome {
    let ast = match engine.compile(source) {
        Ok(ast) => ast,
        Err(e) => return write_failure(out_path, err_path, done_path, "parse", &e.to_string()),
    };
    let mut scope = rhai::Scope::new();
    let result: Result<rhai::Dynamic, Box<rhai::EvalAltResult>> =
        engine.eval_ast_with_scope(&mut scope, &ast);
    let _ = deadline; // wall-clock enforcement is a follow-up; max_operations caps it for now
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

fn classify_error(e: &rhai::EvalAltResult) -> &'static str {
    use rhai::EvalAltResult::*;
    match e {
        ErrorParsing(_, _) => "parse",
        ErrorRuntime(msg, _) => {
            let s = msg.to_string();
            if s.contains("timeout") { "host_fn" }
            else if s.contains("requested below trim watermark") { "range" }
            else if s.starts_with("fs:") { "fs" }
            else if s.contains("cap exceeded") || s.contains("type_text:") { "host_fn" }
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

fn dyn_to_json(v: &rhai::Dynamic) -> Result<String, String> {
    if v.is_unit() { return Ok("null".into()); }
    if let Some(s) = v.clone().try_cast::<String>() {
        return Ok(serde_json::to_string(&s).map_err(|e| e.to_string())?);
    }
    let json: serde_json::Value = serde_json::from_str(&v.to_string())
        .or_else(|_| Ok::<_, serde_json::Error>(serde_json::Value::String(v.to_string())))
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&json).map_err(|e| e.to_string())
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
        let (tx, _rx) = crossbeam_channel::unbounded();
        Arc::new(HostContext {
            screen,
            nudge_tx: tx,
            instance_cwd: std::env::temp_dir(),
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
            script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
        })
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
        let (tx, _rx) = crossbeam_channel::unbounded();
        let screen = Arc::new(ScreenBuffer::new(5, 80));
        screen.advance_bytes(b"hello world\n");
        let ctx = Arc::new(HostContext {
            screen: Arc::clone(&screen),
            nudge_tx: tx,
            instance_cwd: std::env::temp_dir(),
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
            script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
        });
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
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::new(ScreenBuffer::new(5, 80))));
        let outcome = execute_script_to_files(
            &e, "1 + 2", &out, &err, &done,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
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
        let mut e = build_engine();
        register_host_fns(&mut e, make_ctx(Arc::new(ScreenBuffer::new(5, 80))));
        let outcome = execute_script_to_files(
            &e, "let x: int = \"bad\";", &out, &err, &done,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        );
        assert!(matches!(outcome, ScriptOutcome::Err(_)));
        assert!(std::fs::read_to_string(&done).unwrap().starts_with("ERR "));
        assert!(std::fs::read_to_string(&err).unwrap().lines().next().is_some());
    }

    #[test]
    fn type_text_pushes_into_channel() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let ctx = Arc::new(HostContext {
            screen: Arc::new(ScreenBuffer::new(10, 80)),
            nudge_tx: tx,
            instance_cwd: std::env::temp_dir(),
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
            script_timeout_override: Arc::new(std::sync::Mutex::new(None)),
        });
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_text(\"hi\\n\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"hi\n");
    }
}
