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
}
