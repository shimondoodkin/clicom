//! Rhai engine setup + host-fn registration shared shape.
//!
//! Limits per §6.3. Host fns are registered by `register_host_fns` (Task 22+).

use rhai::{Engine, Scope};
use std::sync::Arc;
use crate::clicom_engine::screen::ScreenBuffer;

pub struct HostContext {
    pub screen: Arc<ScreenBuffer>,
    pub nudge_tx: crossbeam_channel::Sender<Vec<u8>>,
    pub instance_cwd: std::path::PathBuf,
    pub idle_observer: Arc<std::sync::Mutex<crate::clicom_engine::idle::IdleDetector>>,
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

    // screen_tail_text
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_tail_text", move |from: i64, to: i64| -> Result<String, Box<rhai::EvalAltResult>> {
        let (a, b) = resolve_indexes(&c.screen, from, to)?;
        let r = c.screen.read_range(a, b);
        Ok(r.lines.join("\n"))
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

    fn make_ctx(screen: Arc<ScreenBuffer>) -> Arc<HostContext> {
        let (tx, _rx) = crossbeam_channel::unbounded();
        Arc::new(HostContext {
            screen,
            nudge_tx: tx,
            instance_cwd: std::env::temp_dir(),
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
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
        });
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let v = run_script(&e, "screen_text()").unwrap();
        let s = v.into_string().unwrap();
        assert!(s.contains("hello"), "got: {s:?}");
    }

    #[test]
    fn type_text_pushes_into_channel() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let ctx = Arc::new(HostContext {
            screen: Arc::new(ScreenBuffer::new(10, 80)),
            nudge_tx: tx,
            instance_cwd: std::env::temp_dir(),
            idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
        });
        let mut e = build_engine();
        register_host_fns(&mut e, ctx);
        let _ = run_script(&e, "type_text(\"hi\\n\")").unwrap();
        let bytes = rx.recv().unwrap();
        assert_eq!(bytes, b"hi\n");
    }
}
