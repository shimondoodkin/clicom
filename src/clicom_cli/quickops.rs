//! Shorthand drivers: build a one-line Rhai script and pump it through cmd_run.
//! Uses serde_json to safely escape user text into Rhai string literals.

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::cmd_run::{self, BusyMode, RunArgs};

pub fn rhai_str_lit(s: &str) -> String {
    // serde_json string serialization yields a valid Rhai string literal
    // (escapes \", \\, \n, \r, \t, control chars).
    serde_json::to_string(s).unwrap_or_else(|_| String::from("\"\""))
}

fn run_with(cwd: &Path, partial: Option<&str>, source: String) -> Result<i32> {
    cmd_run::run(cwd, RunArgs {
        partial: partial.map(String::from),
        source,
        mode: BusyMode::Default,
        timeout_ms: 600_000,
    })
}

pub fn type_text(cwd: &Path, partial: Option<&str>, text: &str, translate: bool) -> Result<i32> {
    run_with(cwd, partial, format!("type_text({}, {})", rhai_str_lit(text), translate))
}

pub fn type_keys(cwd: &Path, partial: Option<&str>, spec: &str) -> Result<i32> {
    run_with(cwd, partial, format!("type_keys({})", rhai_str_lit(spec)))
}

pub fn screen(cwd: &Path, partial: Option<&str>, no_status: bool) -> Result<i32> {
    let src = if no_status { "screen_text()" } else { "screen_text(true)" };
    run_with(cwd, partial, src.to_string())
}

pub fn screen_after(cwd: &Path, partial: Option<&str>, marker: &str, no_status: bool) -> Result<i32> {
    let src = if no_status {
        format!("screen_last_after({})", rhai_str_lit(marker))
    } else {
        format!("screen_last_after({}, true)", rhai_str_lit(marker))
    };
    run_with(cwd, partial, src)
}

pub fn screen_after_re(cwd: &Path, partial: Option<&str>, pattern: &str, no_status: bool) -> Result<i32> {
    let src = if no_status {
        format!("screen_last_after_re({})", rhai_str_lit(pattern))
    } else {
        format!("screen_last_after_re({}, true)", rhai_str_lit(pattern))
    };
    run_with(cwd, partial, src)
}

pub fn wait_idle(cwd: &Path, partial: Option<&str>, ms: u64, timeout_ms: Option<u64>) -> Result<i32> {
    let to = timeout_ms.unwrap_or(60_000);
    run_with(cwd, partial, format!("wait_idle({}, {})", ms, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhai_str_lit_escapes_quotes_and_backslashes() {
        assert_eq!(rhai_str_lit("hi"), "\"hi\"");
        assert_eq!(rhai_str_lit("he said \"hi\""), "\"he said \\\"hi\\\"\"");
        assert_eq!(rhai_str_lit("a\\b"), "\"a\\\\b\"");
        assert_eq!(rhai_str_lit("line1\nline2"), "\"line1\\nline2\"");
    }
}
