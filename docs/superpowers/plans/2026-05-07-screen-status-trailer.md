# Screen Status Trailer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Append a one-line `[clicom: state=…  last_activity=…  visible_rows=…]` trailer to read-only screen-query tools so a calling supervisor sees lifecycle state in the same response as the screen text. Default ON for `clicom screen{,-after,-after-re}` + their MCP equivalents (opt-out via `--no-status`); default OFF for Rhai script callers (opt-in via `screen_text(true)` or composition with `clicom_status_trailer()`).

**Architecture:** Two execution paths share one format helper. Live path: a new `clicom_engine::status_trailer` module formats the trailer; `HostContext` gains an `Arc<Mutex<Status>>` field; existing Rhai screen fns get sibling 1-arg/2-arg overloads taking a `prepend_status: bool`. CLI-side dead-instance fallback: when `discovery::list_instances` resolves the target to `state ∈ {Died, Exited}`, the CLI/MCP path skips the script-drop and reads `screen.txt` + `status.json` from disk, applies the transform, and builds the trailer locally.

**Tech Stack:** Rust 2021, `rhai` (script engine), `chrono` (DateTime<Utc>), `anyhow`, existing `clicom_engine` modules (`meta::{State, Status}`, `screen::ScreenBuffer`, `layout`, `discovery`).

**Spec:** `docs/superpowers/specs/2026-05-07-screen-status-trailer-design.md`

---

## File Structure

**Created:**
- `src/clicom_engine/status_trailer.rs` — `TrailerState` enum, `From<State>` impl, `format(state, last_activity, visible_rows) -> String`, unit tests.
- `tests/e2e_screen_trailer.rs` — integration tests for the dead-instance fallback path (CLI shell-out against a hand-crafted `.clicom/<pid>-<rand>/` dir).

**Modified:**
- `src/clicom_engine/mod.rs` — `pub mod status_trailer;`
- `src/clicom_engine/rhai_host.rs` — `HostContext.status` field; new `clicom_status_trailer()` host fn; sibling overloads for `screen_text`, `screen_last_after`, `screen_last_after_re`; updated test helpers.
- `src/clicom_cli/cmd_start.rs` — pass `Arc::clone(&ch.status)` into `HostContext`.
- `src/clicom_cli/quickops.rs` — `no_status: bool` parameter on `screen`, `screen_after`, `screen_after_re`; dead-instance fallback before falling through to `run_with`.
- `src/clicom_cli/cmd_mcp.rs` — `no_status` field in three tool schemas; parse + forward.
- `src/bin/clicom.rs` — `#[arg(long)] no_status: bool` on `Cmd::Screen`, `Cmd::ScreenAfter`, `Cmd::ScreenAfterRe`; forward to `quickops::*`.

Each task ends with a single commit.

---

## Task 1: `status_trailer` module — format function + unit tests

**Files:**
- Create: `src/clicom_engine/status_trailer.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Create the module skeleton with the failing test**

Create `src/clicom_engine/status_trailer.rs`:

```rust
//! Status trailer line for screen-query tools.
//!
//! Format: `[clicom: state=<word>  last_activity=<rfc3339-Z>  visible_rows=<n>]`
//! Spec: `docs/superpowers/specs/2026-05-07-screen-status-trailer-design.md`.

use chrono::{DateTime, Utc};
use std::fmt;

use crate::clicom_engine::meta::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailerState {
    Idle,
    Active,
    Exited,
    Died,
}

impl fmt::Display for TrailerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let w = match self {
            TrailerState::Idle => "idle",
            TrailerState::Active => "active",
            TrailerState::Exited => "exited",
            TrailerState::Died => "died",
        };
        f.write_str(w)
    }
}

impl From<State> for TrailerState {
    fn from(s: State) -> Self {
        match s {
            State::Idle => TrailerState::Idle,
            State::Busy => TrailerState::Active,
            State::Exited => TrailerState::Exited,
            State::Died => TrailerState::Died,
        }
    }
}

pub fn format(state: TrailerState, last_activity: DateTime<Utc>, visible_rows: u16) -> String {
    let ts = last_activity.format("%Y-%m-%dT%H:%M:%SZ");
    format!("[clicom: state={state}  last_activity={ts}  visible_rows={visible_rows}]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 7, 1, 34, 12).unwrap()
    }

    #[test]
    fn display_emits_lowercase_words() {
        assert_eq!(TrailerState::Idle.to_string(), "idle");
        assert_eq!(TrailerState::Active.to_string(), "active");
        assert_eq!(TrailerState::Exited.to_string(), "exited");
        assert_eq!(TrailerState::Died.to_string(), "died");
    }

    #[test]
    fn from_state_maps_busy_to_active() {
        assert_eq!(TrailerState::from(State::Idle), TrailerState::Idle);
        assert_eq!(TrailerState::from(State::Busy), TrailerState::Active);
        assert_eq!(TrailerState::from(State::Exited), TrailerState::Exited);
        assert_eq!(TrailerState::from(State::Died), TrailerState::Died);
    }

    #[test]
    fn format_matches_spec_example() {
        let s = format(TrailerState::Idle, fixed_ts(), 40);
        assert_eq!(
            s,
            "[clicom: state=idle  last_activity=2026-05-07T01:34:12Z  visible_rows=40]"
        );
    }

    #[test]
    fn format_double_space_separators() {
        let s = format(TrailerState::Active, fixed_ts(), 24);
        // exactly two spaces between fields, none trailing
        assert!(s.contains("=active  last_activity="));
        assert!(s.contains("Z  visible_rows=24"));
        assert!(s.ends_with("]"));
        assert!(!s.ends_with(" ]"));
    }
}
```

- [ ] **Step 2: Add the module to `clicom_engine`**

Edit `src/clicom_engine/mod.rs`. After the existing `pub mod console_mode;` line, add:

```rust
pub mod status_trailer;
```

- [ ] **Step 3: Run the new tests and confirm they pass**

Run: `cargo test --lib status_trailer`
Expected: `4 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/status_trailer.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): status_trailer::format + TrailerState enum"
```

---

## Task 2: Plumb `HostContext.status`

No new behavior — just makes the live `Status` reachable from Rhai host fns. Any test that constructs a `HostContext` must be updated to pass a status mutex.

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs:114-126` (struct), `:755-785` (test helpers)
- Modify: `src/clicom_cli/cmd_start.rs:127-133` (HostContext construction)

- [ ] **Step 1: Add the field to `HostContext`**

Edit `src/clicom_engine/rhai_host.rs`. The struct is at lines 114-126. Add the `status` field after `screen`:

```rust
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
```

- [ ] **Step 2: Update test helpers to construct `status`**

Still in `src/clicom_engine/rhai_host.rs`, find `make_ctx_with_cwd` (around line 759). Add `status` to the `HostContext { ... }` literal:

```rust
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
```

Apply the same `status: Arc::new(std::sync::Mutex::new(crate::clicom_engine::meta::Status::initial_busy())),` line to `make_ctx_with_rx` (around line 773-784).

- [ ] **Step 3: Pass `ch.status` from `cmd_start::run`**

Edit `src/clicom_cli/cmd_start.rs`. The `HostContext` literal lives in the run() body around line 127-133 (the block starting `let ctx = std::sync::Arc::new(rhai_host::HostContext { ... });`). Add `status: Arc::clone(&ch.status),` immediately after `screen:`:

```rust
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
```

- [ ] **Step 4: Build & run all tests to confirm nothing regressed**

Run: `cargo build`
Expected: clean build.

Run: `cargo test`
Expected: all existing tests still pass; no new tests introduced this task.

- [ ] **Step 5: Commit**

```bash
git add src/clicom_engine/rhai_host.rs src/clicom_cli/cmd_start.rs
git commit -m "feat(engine): plumb Status through HostContext"
```

---

## Task 3: `clicom_status_trailer()` Rhai host fn

Exposes the live trailer string to scripts. Also lets us validate end-to-end before touching the screen overloads.

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs` (add registration + tests)

- [ ] **Step 1: Write the failing test**

In `src/clicom_engine/rhai_host.rs` test module (after the existing screen tests, around the end of the test module), add:

```rust
#[test]
fn clicom_status_trailer_returns_formatted_line() {
    let screen = Arc::new(ScreenBuffer::new(40, 80));
    let ctx = make_ctx(Arc::clone(&screen));
    let mut e = build_engine();
    register_host_fns(&mut e, ctx);
    let v = run_script(&e, "clicom_status_trailer()").unwrap();
    let s: String = v.into_string().unwrap();
    // Default test status is initial_busy() → state=active, visible_rows=40
    assert!(s.starts_with("[clicom: state=active  last_activity="), "got: {s}");
    assert!(s.ends_with("  visible_rows=40]"), "got: {s}");
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cargo test --lib clicom_status_trailer_returns_formatted_line`
Expected: FAIL — "Function not found: clicom_status_trailer ()" (Rhai runtime error inside `run_script`).

- [ ] **Step 3: Add a small helper + register the fn**

Still in `src/clicom_engine/rhai_host.rs`. Above `register_host_fns` (around line 142), add:

```rust
fn build_status_trailer(ctx: &HostContext) -> String {
    let (state, last_activity) = {
        let s = ctx.status.lock().expect("status mutex poisoned");
        (crate::clicom_engine::status_trailer::TrailerState::from(s.state), s.last_activity)
    };
    let (rows, _cols) = ctx.screen.visible_dims();
    crate::clicom_engine::status_trailer::format(state, last_activity, rows)
}
```

Inside `register_host_fns`, after the existing `screen_text` registration (around line 173), add:

```rust
// clicom_status_trailer — returns the trailer line (no leading newline).
let c = Arc::clone(&ctx);
engine.register_fn("clicom_status_trailer", move || -> String {
    build_status_trailer(&c)
});
```

- [ ] **Step 4: Run the new test to confirm it passes**

Run: `cargo test --lib clicom_status_trailer_returns_formatted_line`
Expected: PASS.

- [ ] **Step 5: Run the full test suite for regression confidence**

Run: `cargo test`
Expected: all previously passing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): clicom_status_trailer() host fn"
```

---

## Task 4: `screen_text` / `screen_last_after` / `screen_last_after_re` overloads (default off)

Adds 1-arg / 2-arg sibling overloads. **No-arg form delegates with `prepend_status=false`** so existing scripts are unchanged.

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Write the failing tests**

Add four tests at the bottom of the test module in `src/clicom_engine/rhai_host.rs`:

```rust
#[test]
fn screen_text_no_arg_is_raw() {
    let screen = Arc::new(ScreenBuffer::new(5, 80));
    screen.advance_bytes(b"hi");
    let ctx = make_ctx(Arc::clone(&screen));
    let mut e = build_engine();
    register_host_fns(&mut e, ctx);
    let v = run_script(&e, "screen_text()").unwrap();
    let s: String = v.into_string().unwrap();
    assert!(!s.contains("[clicom:"), "no-arg form must NOT prepend status; got: {s:?}");
}

#[test]
fn screen_text_true_appends_trailer() {
    let screen = Arc::new(ScreenBuffer::new(5, 80));
    screen.advance_bytes(b"hi");
    let ctx = make_ctx(Arc::clone(&screen));
    let mut e = build_engine();
    register_host_fns(&mut e, ctx);
    let v = run_script(&e, "screen_text(true)").unwrap();
    let s: String = v.into_string().unwrap();
    assert!(s.contains("\n[clicom: state="), "1-arg true must append trailer on its own line; got: {s:?}");
}

#[test]
fn screen_last_after_two_arg_true_appends_trailer() {
    let screen = Arc::new(ScreenBuffer::new(5, 80));
    screen.advance_bytes(b"prefix-MARK-suffix");
    let ctx = make_ctx(Arc::clone(&screen));
    let mut e = build_engine();
    register_host_fns(&mut e, ctx);
    let v = run_script(&e, "screen_last_after(\"MARK\", true)").unwrap();
    let s: String = v.into_string().unwrap();
    assert!(s.starts_with("-suffix"), "transform must run before trailer; got: {s:?}");
    assert!(s.contains("\n[clicom: state="), "trailer must be appended; got: {s:?}");
}

#[test]
fn screen_last_after_re_two_arg_true_appends_trailer() {
    let screen = Arc::new(ScreenBuffer::new(5, 80));
    screen.advance_bytes(b"abc-123-tail");
    let ctx = make_ctx(Arc::clone(&screen));
    let mut e = build_engine();
    register_host_fns(&mut e, ctx);
    let v = run_script(&e, "screen_last_after_re(\"\\\\d+\", true)").unwrap();
    let s: String = v.into_string().unwrap();
    assert!(s.starts_with("-tail"), "regex transform must run before trailer; got: {s:?}");
    assert!(s.contains("\n[clicom: state="), "trailer must be appended; got: {s:?}");
}
```

- [ ] **Step 2: Run the new tests to confirm they fail**

Run: `cargo test --lib screen_text_true_appends_trailer screen_last_after_two_arg screen_last_after_re_two_arg`
Expected: each FAILS with "Function not found" or wrong-arity error from Rhai (the 2-arg overloads don't exist yet). The `screen_text_no_arg_is_raw` test should PASS already (current behavior is raw — confirms our baseline).

- [ ] **Step 3: Refactor existing fns + add overloads**

Replace the existing `screen_text`, `screen_last_after`, and `screen_last_after_re` registrations in `register_host_fns` (currently around lines 171-215) with this block:

```rust
    // screen_text() / screen_text(prepend_status: bool)
    {
        let c = Arc::clone(&ctx);
        engine.register_fn("screen_text", move || -> String { c.screen.to_plain_text() });
        let c = Arc::clone(&ctx);
        engine.register_fn("screen_text", move |prepend_status: bool| -> String {
            let text = c.screen.to_plain_text();
            if prepend_status {
                format!("{text}\n{}", build_status_trailer(&c))
            } else {
                text
            }
        });
    }

    // screen_save (unchanged — keep existing block here)
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_save", move |path: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        let body = c.screen.to_plain_text();
        let resolved = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&resolved, body.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("fs: {e}").into(), rhai::Position::NONE)))?;
        Ok(body.as_bytes().len() as i64)
    });

    // screen_last_after(marker) / screen_last_after(marker, prepend_status)
    {
        let c = Arc::clone(&ctx);
        engine.register_fn("screen_last_after", move |marker: &str| -> String {
            let lifetime = c.screen.lifetime_text();
            match lifetime.rfind(marker) {
                Some(idx) => lifetime[idx + marker.len()..].to_string(),
                None => String::new(),
            }
        });
        let c = Arc::clone(&ctx);
        engine.register_fn("screen_last_after", move |marker: &str, prepend_status: bool| -> String {
            let lifetime = c.screen.lifetime_text();
            let text = match lifetime.rfind(marker) {
                Some(idx) => lifetime[idx + marker.len()..].to_string(),
                None => String::new(),
            };
            if prepend_status {
                format!("{text}\n{}", build_status_trailer(&c))
            } else {
                text
            }
        });
    }

    // screen_save_last_after (unchanged — keep existing block here)
    let c = Arc::clone(&ctx);
    engine.register_fn("screen_save_last_after", move |path: &str, marker: &str| -> Result<i64, Box<rhai::EvalAltResult>> {
        let lifetime = c.screen.lifetime_text();
        let body = match lifetime.rfind(marker) { Some(i) => lifetime[i + marker.len()..].to_string(), None => String::new() };
        let resolved = resolve_path(&c.instance_cwd, path);
        crate::clicom_engine::fs_atomic::write(&resolved, body.as_bytes())
            .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("fs: {e}").into(), rhai::Position::NONE)))?;
        Ok(body.as_bytes().len() as i64)
    });

    // screen_last_after_re(pattern) / screen_last_after_re(pattern, prepend_status)
    {
        let c = Arc::clone(&ctx);
        engine.register_fn("screen_last_after_re", move |pattern: &str| -> Result<String, Box<rhai::EvalAltResult>> {
            let re = regex::Regex::new(pattern)
                .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("regex compile: {e}").into(), rhai::Position::NONE)))?;
            let lifetime = c.screen.lifetime_text();
            let mut last_end: Option<usize> = None;
            for m in re.find_iter(&lifetime) { last_end = Some(m.end()); }
            Ok(last_end.map(|i| lifetime[i..].to_string()).unwrap_or_default())
        });
        let c = Arc::clone(&ctx);
        engine.register_fn("screen_last_after_re", move |pattern: &str, prepend_status: bool| -> Result<String, Box<rhai::EvalAltResult>> {
            let re = regex::Regex::new(pattern)
                .map_err(|e| Box::new(rhai::EvalAltResult::ErrorRuntime(format!("regex compile: {e}").into(), rhai::Position::NONE)))?;
            let lifetime = c.screen.lifetime_text();
            let mut last_end: Option<usize> = None;
            for m in re.find_iter(&lifetime) { last_end = Some(m.end()); }
            let text = last_end.map(|i| lifetime[i..].to_string()).unwrap_or_default();
            if prepend_status {
                Ok(format!("{text}\n{}", build_status_trailer(&c)))
            } else {
                Ok(text)
            }
        });
    }
```

(The `screen_save` and `screen_save_last_after` registrations are *unchanged*. Just keep them in place between the new overload blocks. The existing `screen_save_last_after_re` registration also stays as-is below this — don't delete it.)

- [ ] **Step 4: Run the new tests + full suite**

Run: `cargo test`
Expected: all tests pass — the four new tests plus everything pre-existing.

- [ ] **Step 5: Commit**

```bash
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): screen_text/last_after/last_after_re prepend_status overloads"
```

---

## Task 5: CLI `--no-status` flag + `quickops` `no_status` parameter

Quick commands flip to **default trailer ON**. `--no-status` reverts to raw output.

**Files:**
- Modify: `src/clicom_cli/quickops.rs`
- Modify: `src/bin/clicom.rs`

- [ ] **Step 1: Update `quickops` signatures + Rhai source builders**

Replace the `screen`, `screen_after`, `screen_after_re` functions in `src/clicom_cli/quickops.rs` (currently lines 32-42) with:

```rust
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
```

- [ ] **Step 2: Add `no_status` to the CLI parser + dispatchers**

In `src/bin/clicom.rs`, edit the three `Cmd` variants (currently around lines 51-69):

```rust
    /// Print the wrapped agent's current visible screen.
    Screen {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] no_status: bool,
    },
    /// Print everything after the last occurrence of <marker>.
    ScreenAfter {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] no_status: bool,
        marker: String,
    },
    /// Print everything after the last regex match of <pattern>.
    ScreenAfterRe {
        #[arg(long)] partial: Option<String>,
        #[arg(long)] no_status: bool,
        pattern: String,
    },
```

Then update the corresponding dispatch arms (currently around lines 133-138):

```rust
        Cmd::Screen { partial, no_status } =>
            clicom::clicom_cli::quickops::screen(&cwd, partial.as_deref(), no_status)?,
        Cmd::ScreenAfter { partial, no_status, marker } =>
            clicom::clicom_cli::quickops::screen_after(&cwd, partial.as_deref(), &marker, no_status)?,
        Cmd::ScreenAfterRe { partial, no_status, pattern } =>
            clicom::clicom_cli::quickops::screen_after_re(&cwd, partial.as_deref(), &pattern, no_status)?,
```

- [ ] **Step 3: Build to confirm everything compiles**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all tests pass. (`quickops`'s own unit tests in the file are unaffected.)

- [ ] **Step 5: Commit**

```bash
git add src/clicom_cli/quickops.rs src/bin/clicom.rs
git commit -m "feat(cli): --no-status on screen/screen-after/screen-after-re (default trailer on)"
```

---

## Task 6: MCP `no_status` arg on `clicom_screen{,_after,_after_re}`

**Files:**
- Modify: `src/clicom_cli/cmd_mcp.rs`

- [ ] **Step 1: Locate the three tool schemas + dispatch arms**

Read `src/clicom_cli/cmd_mcp.rs` lines 130-180 (schemas — `clicom_screen`, `clicom_screen_after`, `clicom_screen_after_re`) and lines 395-425 (dispatch arms).

- [ ] **Step 2: Add `no_status` to each schema**

For `clicom_screen` (currently around line 139), update the schema's `properties` JSON to include `no_status`. The schema already has `partial`; add `no_status` next to it. The relevant block becomes (showing only the `properties` change you need to make):

```rust
            "name": "clicom_screen",
            "description": "Print the wrapped agent's current visible screen. Trailer line `[clicom: …]` appended by default; pass no_status: true for raw output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "partial": { "type": "string", "description": "Disambiguate when multiple instances live in cwd." },
                    "no_status": { "type": "boolean", "description": "Suppress the [clicom: …] trailer.", "default": false }
                }
            }
```

Apply the same `no_status` property addition to `clicom_screen_after` (around line 147) and `clicom_screen_after_re` (around line 159). Keep their existing required `marker` / `pattern` properties intact.

- [ ] **Step 3: Parse `no_status` in each dispatch arm**

Update the three dispatch arms (around lines 400-422) to pass `no_status` to the underlying `quickops` function. Replace each arm with:

```rust
        "clicom_screen" => {
            let partial = args.get("partial").and_then(|v| v.as_str());
            let no_status = args.get("no_status").and_then(|v| v.as_bool()).unwrap_or(false);
            run_script(cwd, partial, format!("screen_text({})", if no_status { "" } else { "true" }))
        }
        "clicom_screen_after" => {
            let partial = args.get("partial").and_then(|v| v.as_str());
            let no_status = args.get("no_status").and_then(|v| v.as_bool()).unwrap_or(false);
            let marker = args.get("marker").and_then(|v| v.as_str()).unwrap_or("");
            let suffix = if no_status { String::new() } else { ", true".to_string() };
            run_script(cwd, partial, format!("screen_last_after({}{suffix})", crate::clicom_cli::quickops::rhai_str_lit(marker)))
        }
        "clicom_screen_after_re" => {
            let partial = args.get("partial").and_then(|v| v.as_str());
            let no_status = args.get("no_status").and_then(|v| v.as_bool()).unwrap_or(false);
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let suffix = if no_status { String::new() } else { ", true".to_string() };
            run_script(cwd, partial, format!("screen_last_after_re({}{suffix})", crate::clicom_cli::quickops::rhai_str_lit(pattern)))
        }
```

(If the existing arms use a different helper name than `run_script` — e.g. `run_script_for_tool` — preserve whatever the file already uses; only the *body* of each arm needs updating.)

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 5: Smoke-test the schemas (best-effort)**

Run: `cargo test --test e2e_mcp 2>/dev/null || cargo test mcp` (run whatever MCP-related tests already exist in `tests/`).
Expected: all existing MCP tests still pass. (We're not adding new MCP tests in this task — those follow in Task 7's e2e battery.)

- [ ] **Step 6: Commit**

```bash
git add src/clicom_cli/cmd_mcp.rs
git commit -m "feat(mcp): no_status arg on clicom_screen{,_after,_after_re}"
```

---

## Task 7: Dead-instance fallback for the three quickops + e2e tests

The high-value piece: when `discovery::list_instances` resolves the target to `Died` or `Exited`, we read `screen.txt` from disk and synthesize a CLI-side trailer.

**Files:**
- Modify: `src/clicom_cli/quickops.rs`
- Create: `tests/e2e_screen_trailer.rs`

- [ ] **Step 1: Write the failing e2e test**

Create `tests/e2e_screen_trailer.rs`:

```rust
//! Integration tests for the dead-instance fallback path of
//! `clicom screen` / `clicom screen-after` / `clicom screen-after-re`.

use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Manually craft a `.clicom/<pid>-<rand>/` directory whose PID is dead
/// (so `discovery::list_instances` flips its state to `Died`).
fn craft_dead_instance(td: &TempDir, screen_body: &str) -> std::path::PathBuf {
    // 4_000_000 is guaranteed not a real process on any supported platform.
    let dead_pid: u32 = 4_000_000;
    let dot_clicom = td.path().join(".clicom");
    let inst = dot_clicom.join(format!("{dead_pid}-deadbe"));
    fs::create_dir_all(inst.join("commands")).unwrap();

    // meta.json
    let meta = serde_json::json!({
        "schema": "clicom-meta/1",
        "pid": dead_pid,
        "name": "agent",
        "command": ["fake"],
        "cwd": td.path(),
        "started_at": "2026-05-07T01:00:00Z",
    });
    fs::write(inst.join("meta.json"), serde_json::to_vec_pretty(&meta).unwrap()).unwrap();

    // status.json — Busy + dead pid → discovery rewrites to Died on read.
    let status = serde_json::json!({
        "schema": "clicom-status/1",
        "state": "busy",
        "last_activity": "2026-05-07T01:34:12Z",
        "exit_code": null,
        "exited_at": null,
    });
    fs::write(inst.join("status.json"), serde_json::to_vec_pretty(&status).unwrap()).unwrap();

    // commands.lock + screen.txt
    fs::write(inst.join("commands.lock"), b"").unwrap();
    fs::write(inst.join("screen.txt"), screen_body.as_bytes()).unwrap();

    inst
}

#[test]
fn screen_dead_instance_appends_died_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "line one\nline two\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen"])
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("line one\nline two"), "screen body missing: {stdout:?}");
    assert!(stdout.contains("[clicom: state=died"), "trailer missing: {stdout:?}");
    assert!(stdout.contains("last_activity=2026-05-07T01:34:12Z"), "ts wrong: {stdout:?}");
    assert!(stdout.contains("visible_rows=2]"), "rows wrong: {stdout:?}");
}

#[test]
fn screen_dead_instance_no_status_omits_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "only line\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen", "--no-status"])
        .output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("only line"));
    assert!(!stdout.contains("[clicom:"), "trailer should be suppressed: {stdout:?}");
}

#[test]
fn screen_after_dead_instance_applies_marker_then_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "before-MARK-after\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen-after", "MARK"])
        .output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first_line = stdout.lines().next().unwrap_or("");
    assert!(first_line.starts_with("-after"), "marker transform missing: {stdout:?}");
    assert!(stdout.contains("[clicom: state=died"), "trailer missing: {stdout:?}");
}

#[test]
fn screen_after_re_dead_instance_applies_regex_then_trailer() {
    let td = TempDir::new().unwrap();
    craft_dead_instance(&td, "abc-12345-tail\n");

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["screen-after-re", r"\d+"])
        .output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first_line = stdout.lines().next().unwrap_or("");
    assert!(first_line.starts_with("-tail"), "regex transform missing: {stdout:?}");
    assert!(stdout.contains("[clicom: state=died"), "trailer missing: {stdout:?}");
}
```

- [ ] **Step 2: Run the new tests to confirm they fail**

Run: `cargo test --test e2e_screen_trailer`
Expected: all four FAIL — currently `clicom screen` against a dead instance exits 2 with "no live wrapped agent in …".

- [ ] **Step 3: Add the fallback helper + branch into each quickop**

Edit `src/clicom_cli/quickops.rs`. Add new imports at the top of the file:

```rust
use crate::clicom_cli::discovery::{filter_by_partial, list_instances};
use crate::clicom_engine::{layout, meta::State, status_trailer::{self, TrailerState}};
```

Add this private helper above the existing `screen` function:

```rust
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
```

Now update each quickop to call the fallback first. Replace `screen`, `screen_after`, `screen_after_re` (the bodies you wrote in Task 5) with:

```rust
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
```

- [ ] **Step 4: Run the e2e tests to confirm they pass**

Run: `cargo test --test e2e_screen_trailer`
Expected: all four PASS.

- [ ] **Step 5: Run the full test suite for regression**

Run: `cargo test`
Expected: every test passes (this catches accidental breakage of the existing live-path tests).

- [ ] **Step 6: Commit**

```bash
git add src/clicom_cli/quickops.rs tests/e2e_screen_trailer.rs
git commit -m "feat(cli): dead-instance fallback for screen/screen-after/screen-after-re"
```

---

## Self-review (post-plan)

1. **Spec coverage:**
   - Spec §2 In-bullets:
     - "Three new Rhai overloads" → Task 4 ✓
     - "`clicom_status_trailer()`" → Task 3 ✓
     - "`HostContext` gains `Arc<Mutex<Status>>`" → Task 2 ✓
     - "Quick commands default trailer ON; `--no-status`" → Task 5 ✓
     - "MCP `no_status: bool`" → Task 6 ✓
     - "CLI-side dead-instance fallback" → Task 7 ✓
   - Spec §2 Out-bullets: explicitly excluded — confirmed not in any task.
   - Spec §3 trailer format: verified by `format_matches_spec_example` test in Task 1.

2. **Placeholder scan:** clean. Every step has either complete code or an exact command + expected output.

3. **Type consistency:**
   - `HostContext.status: Arc<std::sync::Mutex<Status>>` — used identically in Task 2 (definition), Task 2 step 2 (test helpers), Task 3 (`build_status_trailer`).
   - `try_dead_instance_fallback`'s `transform: FnOnce(&str) -> String` — consistent across all three call sites in Task 7 step 3.
   - `TrailerState::from(State)` — defined in Task 1, used in Task 3 + Task 7.

4. **No new placeholders, no contradictions.**

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-07-screen-status-trailer.md`. Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
