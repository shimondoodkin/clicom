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
}

fn resolve_path(cwd: &std::path::Path, p: &str) -> std::path::PathBuf {
    let pp = std::path::Path::new(p);
    if pp.is_absolute() { pp.to_path_buf() } else { cwd.join(pp) }
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
