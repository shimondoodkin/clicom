//! Shorthand drivers: build a one-line Rhai script and pump it through cmd_run.
//! Uses serde_json to safely escape user text into Rhai string literals.

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::cmd_run::{self, BusyMode, RunArgs};
use crate::clicom_cli::discovery::{filter_by_partial, list_instances};
use crate::clicom_engine::{layout, meta::State, status_trailer::{self, TrailerState}};

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

/// If exactly one instance matches and its state is Died or Exited, read its
/// persisted `screen.txt`, apply `transform`, optionally append the trailer,
/// print, and return `Some(0)`. Otherwise return `None` so the caller falls
/// through to the live (script-drop) path.
fn try_dead_instance_fallback<F>(
    cwd: &Path,
    partial: Option<&str>,
    no_status: bool,
    transform: F,
) -> Result<Option<i32>>
where
    F: FnOnce(&str) -> String,
{
    let candidates = filter_by_partial(list_instances(cwd), partial);
    if candidates.len() != 1 {
        return Ok(None);
    }
    let inst = &candidates[0];
    if !matches!(inst.status.state, State::Died | State::Exited) {
        return Ok(None);
    }
    let screen_path = layout::screen_path(&inst.dir);
    let body = std::fs::read_to_string(&screen_path).map_err(|e| {
        anyhow::anyhow!(
            "dead instance {} has no readable screen.txt: {e}",
            inst.dir_name
        )
    })?;
    let transformed = transform(&body);
    if no_status {
        print!("{transformed}");
    } else {
        let rows: u16 = body.lines().count().min(u16::MAX as usize) as u16;
        let trailer = status_trailer::format(
            TrailerState::from(inst.status.state),
            inst.status.last_activity,
            rows,
        );
        println!("{transformed}\n{trailer}");
    }
    Ok(Some(0))
}

pub fn screen(cwd: &Path, partial: Option<&str>, no_status: bool) -> Result<i32> {
    if let Some(code) = try_dead_instance_fallback(cwd, partial, no_status, |s| s.to_string())? {
        return Ok(code);
    }
    let src = if no_status { "screen_text()" } else { "screen_text(true)" };
    run_with(cwd, partial, src.to_string())
}

pub fn screen_after(cwd: &Path, partial: Option<&str>, marker: &str, no_status: bool) -> Result<i32> {
    let m = marker.to_string();
    if let Some(code) = try_dead_instance_fallback(cwd, partial, no_status, |s| match s.rfind(&m) {
        Some(idx) => s[idx + m.len()..].to_string(),
        None => String::new(),
    })? {
        return Ok(code);
    }
    let src = if no_status {
        format!("screen_last_after({})", rhai_str_lit(marker))
    } else {
        format!("screen_last_after({}, true)", rhai_str_lit(marker))
    };
    run_with(cwd, partial, src)
}

pub fn screen_after_re(cwd: &Path, partial: Option<&str>, pattern: &str, no_status: bool) -> Result<i32> {
    let pat = pattern.to_string();
    if let Some(code) = try_dead_instance_fallback(cwd, partial, no_status, |s| {
        match regex::Regex::new(&pat) {
            Ok(re) => {
                let mut last_end: Option<usize> = None;
                for m in re.find_iter(s) { last_end = Some(m.end()); }
                last_end.map(|i| s[i..].to_string()).unwrap_or_default()
            }
            Err(_) => String::new(),
        }
    })? {
        return Ok(code);
    }
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
