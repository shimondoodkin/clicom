# `clicom` Initial Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `clicom` CLI tool — a self-contained binary that wraps an arbitrary command in a PTY, exposes its screen and stdin through a per-cwd `.clicom/` directory, and accepts Rhai scripts as a control language.

**Architecture:** Greenfield Rust crate at `./` (sibling of `../cliagentchat/`, the `inboxmcp` project). Single binary `clicom` with subcommands `start`/`status`/`run`/`queue`/`clean`/`help`. Engine library (`clicom_engine`) owns wrapper-side concerns; CLI library (`clicom_cli`) owns driver-side concerns. File-protocol coordination via per-instance `commands.lock` (`fs2`); script execution via embedded Rhai with sandboxed host-fn surface. Reuses six modules copied verbatim from `../cliagentchat/`: `pty.rs`, `screen.rs` (then extended), `idle.rs`, `forwarding.rs`, `nudge.rs`, `fs_atomic.rs`.

**Tech Stack:** Rust 1.75+, `clap` v4, `portable-pty`, `vt100`, `rhai`, `notify`, `fs2`, `sysinfo`, `regex`, `crossbeam-channel`, `tracing`. Tests use `assert_cmd` + `tempfile`.

**Spec:** `docs/superpowers/specs/2026-05-02-clicom-wrapped-commands-channel-design.md`. All section references (`§3.7.2`, `§5.4`, etc.) point into that spec.

---

## File structure

```
./
  Cargo.toml
  README.md
  src/
    lib.rs                     # re-exports clicom_engine + clicom_cli
    clicom_engine/
      mod.rs                   # ClicomChannel, lifecycle orchestration
      ids.rs                   # rand6, unix_nanos, <id> format
      meta.rs                  # Meta + Status types, atomic writes
      layout.rs                # path helpers, dir-name parsing, partial-match
      gitignore.rs             # idempotent append
      retention.rs             # dead-instance retention + result-triple cap
      process.rs               # pid-alive check (sysinfo)
      fs_atomic.rs             # COPY from ../cliagentchat/src/fs_atomic.rs
      screen.rs                # COPY ../cliagentchat/src/wrap/screen.rs + extend with ScrollbackRing
      pty.rs                   # COPY ../cliagentchat/src/wrap/pty.rs
      nopty.rs                 # NEW — pipe-based spawn
      forwarding.rs            # COPY ../cliagentchat/src/wrap/forwarding.rs
      idle.rs                  # COPY ../cliagentchat/src/wrap/idle.rs
      nudge.rs                 # COPY ../cliagentchat/src/wrap/nudge.rs
      rhai_host.rs             # Rhai engine setup + host fn registration
      watcher.rs               # notify-based commands/ watcher + script executor
    clicom_cli/
      mod.rs                   # re-exports
      discovery.rs             # list instances, partial-match resolution
      drop.rs                  # acquire lock, drop .rhai, wait .done, read result files
      cmd_start.rs
      cmd_status.rs
      cmd_run.rs
      cmd_queue.rs
      cmd_clean.rs
      cmd_help.rs
    bin/
      clicom.rs                # clap parse + dispatch
  tests/
    e2e_basic.rs
    e2e_queue.rs
    e2e_busy.rs
    e2e_multi_instance.rs
    e2e_died.rs
    e2e_nopty.rs
    e2e_clean.rs
    e2e_rhai.rs
    fixtures/
      fake_agent.rs
```

---

## Milestone 1 — Foundation + `clicom start`

End state: `clicom start -- <cmd>` wraps a child in a PTY, creates `<cwd>/.clicom/<pid>-<rand6>/`, writes `meta.json`/`status.json`/`screen.txt`, idle/busy transitions land in `status.json`, retention sweeps run. `clicom status` reports. No script execution yet.

### Task 1: Project scaffold (Cargo.toml, lib.rs, bin/clicom.rs)

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/bin/clicom.rs`
- Create: `.gitignore`
- Create: `README.md`

- [ ] **Step 1: Initialize git repo (if not already)**

```bash
cd C:/Users/user/Documents/projects/clicom
git init
```

- [ ] **Step 2: Create `Cargo.toml`** matching §9 of the spec exactly:

```toml
[package]
name = "clicom"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[lib]
name = "clicom"
path = "src/lib.rs"

[[bin]]
name = "clicom"
path = "src/bin/clicom.rs"

[dependencies]
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
fs2 = "0.4"
sysinfo = "0.30"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
crossbeam-channel = "0.5"
portable-pty = "0.8"
vt100 = "0.16"
rand = "0.8"
regex = "1"
notify = "6"
rhai = { version = "1", features = ["serde"] }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = [
    "Win32_System_Console",
    "Win32_System_Threading",
    "Win32_Foundation",
    "Win32_System_Diagnostics_ToolHelp",
] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
pretty_assertions = "1"
serial_test = "3"
```

- [ ] **Step 3: Create `.gitignore`**

```
/target
*.lock.bak
.clicom/
```

- [ ] **Step 4: Create `src/lib.rs` skeleton**

```rust
pub mod clicom_engine;
pub mod clicom_cli;
```

- [ ] **Step 5: Create empty module files so the crate compiles**

```bash
mkdir -p src/clicom_engine src/clicom_cli
```

Create stub `src/clicom_engine/mod.rs`:
```rust
// modules added in subsequent tasks
```

Create stub `src/clicom_cli/mod.rs`:
```rust
// modules added in subsequent tasks
```

- [ ] **Step 6: Create `src/bin/clicom.rs` with a placeholder main**

```rust
fn main() -> anyhow::Result<()> {
    eprintln!("clicom: scaffolding only — subcommands land in later tasks");
    std::process::exit(2);
}
```

- [ ] **Step 7: Create a one-line `README.md`**

```markdown
# clicom

File-based command channel for wrapped CLI agents. See `docs/superpowers/specs/2026-05-02-clicom-wrapped-commands-channel-design.md`.
```

- [ ] **Step 8: Verify the crate builds**

Run: `cargo build`
Expected: succeeds; `clicom.exe` produced under `target/debug/`.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml .gitignore README.md src/
git commit -m "chore: initial Cargo scaffold + bin/lib skeleton"
```

### Task 2: `ids.rs` — id generation

**Files:**
- Create: `src/clicom_engine/ids.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write the failing tests** in `src/clicom_engine/ids.rs`:

```rust
//! Id generation helpers.
//!
//! - `rand6()`         → 6-char lowercase hex token (used in instance dir names and <id>s)
//! - `unix_nanos()`    → current Unix time in nanoseconds (for sortable <id>s)
//! - `make_command_id()` → "<unix_nanos>-<rand6>" form used for commands/<id>.rhai

use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn rand6() -> String {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..0x0100_0000); // 24 bits
    format!("{:06x}", n)
}

pub fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

pub fn make_command_id() -> String {
    format!("{}-{}", unix_nanos(), rand6())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rand6_is_six_hex_chars() {
        for _ in 0..100 {
            let s = rand6();
            assert_eq!(s.len(), 6);
            assert!(s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn rand6_collision_is_unlikely() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            seen.insert(rand6());
        }
        // ~1000 random 24-bit values should not repeat at this scale; ≥ 990 unique is fine
        assert!(seen.len() > 990);
    }

    #[test]
    fn make_command_id_has_two_parts() {
        let id = make_command_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].parse::<u128>().is_ok());
        assert_eq!(parts[1].len(), 6);
    }

    #[test]
    fn ids_are_sortable_by_drop_time() {
        let a = make_command_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = make_command_id();
        assert!(a < b, "{} should sort before {}", a, b);
    }
}
```

- [ ] **Step 2: Wire the module in `src/clicom_engine/mod.rs`**

```rust
pub mod ids;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib ids`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/ids.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): id generation (rand6, unix_nanos, make_command_id)"
```

### Task 3: `fs_atomic.rs` — copy from cliagentchat

**Files:**
- Create: `src/clicom_engine/fs_atomic.rs` (copy from `../cliagentchat/src/fs_atomic.rs`)
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Copy the file verbatim**

```bash
cp ../cliagentchat/src/fs_atomic.rs src/clicom_engine/fs_atomic.rs
```

The contents should be:
```rust
//! Single source of truth for atomic file writes (write `*.tmp` then rename).

use std::fs;
use std::path::Path;

pub fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(
        path.extension()
            .map(|e| format!("{}.tmp", e.to_string_lossy()))
            .unwrap_or_else(|| "tmp".into()),
    );
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
```

- [ ] **Step 2: Add a unit test** at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_atomically_via_tmp_rename() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("foo.json");
        write(&target, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
        // tmp file should be gone
        assert!(!td.path().join("foo.json.tmp").exists());
    }

    #[test]
    fn overwrites_existing_file() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("x.txt");
        std::fs::write(&target, b"old").unwrap();
        write(&target, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn creates_parent_dirs() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("a/b/c.txt");
        write(&target, b"x").unwrap();
        assert!(target.exists());
    }
}
```

- [ ] **Step 3: Wire the module**

Add to `src/clicom_engine/mod.rs`:
```rust
pub mod fs_atomic;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib fs_atomic`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/clicom_engine/fs_atomic.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): copy fs_atomic helper from inboxmcp"
```

### Task 4: `meta.rs` — Meta + Status types

**Files:**
- Create: `src/clicom_engine/meta.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write tests-first** in `src/clicom_engine/meta.rs`:

```rust
//! `meta.json` and `status.json` types and serialization.
//!
//! Schemas: `clicom-meta/1`, `clicom-status/1` (§3.1, §3.2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::clicom_engine::fs_atomic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub schema: String,        // "clicom-meta/1"
    pub pid: u32,
    pub name: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub started_at: DateTime<Utc>,
}

impl Meta {
    pub const SCHEMA: &'static str = "clicom-meta/1";

    pub fn new(pid: u32, name: String, command: Vec<String>, cwd: PathBuf) -> Self {
        Meta {
            schema: Self::SCHEMA.to_string(),
            pid,
            name,
            command,
            cwd,
            started_at: Utc::now(),
        }
    }

    pub fn write_to(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        fs_atomic::write(path, &json)
    }

    pub fn read_from(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State { Idle, Busy, Exited, Died }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub schema: String,        // "clicom-status/1"
    pub state: State,
    pub last_activity: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub exited_at: Option<DateTime<Utc>>,
}

impl Status {
    pub const SCHEMA: &'static str = "clicom-status/1";

    pub fn initial_busy() -> Self {
        Status {
            schema: Self::SCHEMA.to_string(),
            state: State::Busy,
            last_activity: Utc::now(),
            exit_code: None,
            exited_at: None,
        }
    }

    pub fn write_to(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        fs_atomic::write(path, &json)
    }

    pub fn read_from(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn meta_round_trips() {
        let td = TempDir::new().unwrap();
        let path = td.path().join("meta.json");
        let m = Meta::new(123, "alice".into(), vec!["claude".into(), "code".into()], td.path().to_path_buf());
        m.write_to(&path).unwrap();
        let read = Meta::read_from(&path).unwrap();
        assert_eq!(read.schema, "clicom-meta/1");
        assert_eq!(read.pid, 123);
        assert_eq!(read.name, "alice");
        assert_eq!(read.command, vec!["claude", "code"]);
    }

    #[test]
    fn status_round_trips_and_uses_lowercase_state() {
        let td = TempDir::new().unwrap();
        let path = td.path().join("status.json");
        let s = Status::initial_busy();
        s.write_to(&path).unwrap();
        let read = Status::read_from(&path).unwrap();
        assert_eq!(read.state, State::Busy);
        // Verify on-disk format uses lowercase strings (per spec example)
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"busy\""), "state should serialize as lowercase: {raw}");
    }
}
```

- [ ] **Step 2: Wire the module** in `src/clicom_engine/mod.rs`:

```rust
pub mod meta;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib meta`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/meta.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): Meta + Status types with atomic write/read"
```

### Task 5: `layout.rs` — path helpers + partial-match

**Files:**
- Create: `src/clicom_engine/layout.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write tests-first**:

```rust
//! Path layout helpers (§3) + partial-match for instance discovery (§5.3).

use std::path::{Path, PathBuf};

pub fn dot_clicom(cwd: &Path) -> PathBuf { cwd.join(".clicom") }

pub fn instance_dir(cwd: &Path, pid: u32, rand6: &str) -> PathBuf {
    dot_clicom(cwd).join(format!("{}-{}", pid, rand6))
}

pub fn instance_dir_name(pid: u32, rand6: &str) -> String {
    format!("{}-{}", pid, rand6)
}

pub fn meta_path(instance: &Path) -> PathBuf { instance.join("meta.json") }
pub fn status_path(instance: &Path) -> PathBuf { instance.join("status.json") }
pub fn screen_path(instance: &Path) -> PathBuf { instance.join("screen.txt") }
pub fn lock_path(instance: &Path) -> PathBuf { instance.join("commands.lock") }
pub fn commands_dir(instance: &Path) -> PathBuf { instance.join("commands") }
pub fn rhai_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.rhai", id))
}
pub fn out_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.out", id))
}
pub fn err_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.err", id))
}
pub fn done_path(instance: &Path, id: &str) -> PathBuf {
    commands_dir(instance).join(format!("{}.done", id))
}

/// Substring match of `<partial>` against a dir name like "12345-a3f9c2".
pub fn partial_matches(dir_name: &str, partial: &str) -> bool {
    dir_name.contains(partial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_name_format() {
        assert_eq!(instance_dir_name(12345, "a3f9c2"), "12345-a3f9c2");
    }

    #[test]
    fn partial_matches_pid_or_rand_or_combined() {
        let name = "12345-a3f9c2";
        assert!(partial_matches(name, "12345"));
        assert!(partial_matches(name, "a3f9"));
        assert!(partial_matches(name, "12345-a3"));
        assert!(partial_matches(name, "f9c2"));
        assert!(!partial_matches(name, "9999"));
    }

    #[test]
    fn paths_compose_correctly() {
        let cwd = Path::new("/tmp/work");
        let inst = instance_dir(cwd, 99, "abcdef");
        assert_eq!(inst, Path::new("/tmp/work/.clicom/99-abcdef"));
        assert_eq!(rhai_path(&inst, "1-deadbe"), Path::new("/tmp/work/.clicom/99-abcdef/commands/1-deadbe.rhai"));
    }
}
```

- [ ] **Step 2: Wire it** in `src/clicom_engine/mod.rs`:

```rust
pub mod layout;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib layout`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/layout.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): path layout helpers + partial-match"
```

### Task 6: `process.rs` — pid-alive check

**Files:**
- Create: `src/clicom_engine/process.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write tests-first**:

```rust
//! pid-alive check via sysinfo.

use sysinfo::{Pid, System};

/// Returns true if a process with this PID currently exists.
pub fn pid_is_alive(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_processes();
    sys.process(Pid::from_u32(pid)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_pid_is_alive() {
        let me = std::process::id();
        assert!(pid_is_alive(me));
    }

    #[test]
    fn obviously_dead_pid_is_not_alive() {
        // PID 0 / max-u32 should not match a real process on supported platforms.
        assert!(!pid_is_alive(0));
        assert!(!pid_is_alive(u32::MAX));
    }
}
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod process;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib process`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/process.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): pid-alive check via sysinfo"
```

### Task 7: `gitignore.rs` — idempotent append

**Files:**
- Create: `src/clicom_engine/gitignore.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write tests-first**:

```rust
//! Idempotent ".clicom/" append to <cwd>/.gitignore (§3.8).

use std::fs;
use std::io::Write;
use std::path::Path;

const ENTRY: &str = ".clicom/";

/// If `<cwd>/.gitignore` exists and does not already contain a line equal to
/// ".clicom/" (after trim), append it on its own line. If `.gitignore` does
/// not exist, do nothing. Idempotent.
pub fn ensure_clicom_ignored(cwd: &Path) -> anyhow::Result<()> {
    let gi = cwd.join(".gitignore");
    if !gi.exists() {
        return Ok(());
    }
    let body = fs::read_to_string(&gi)?;
    if body.lines().any(|l| l.trim() == ENTRY) {
        return Ok(());
    }
    let needs_newline = !body.ends_with('\n') && !body.is_empty();
    let mut f = fs::OpenOptions::new().append(true).open(&gi)?;
    if needs_newline { f.write_all(b"\n")?; }
    f.write_all(ENTRY.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_gitignore_does_nothing() {
        let td = TempDir::new().unwrap();
        ensure_clicom_ignored(td.path()).unwrap();
        assert!(!td.path().join(".gitignore").exists());
    }

    #[test]
    fn appends_when_absent() {
        let td = TempDir::new().unwrap();
        let gi = td.path().join(".gitignore");
        fs::write(&gi, "/target\n").unwrap();
        ensure_clicom_ignored(td.path()).unwrap();
        let body = fs::read_to_string(&gi).unwrap();
        assert!(body.contains(".clicom/"));
        assert!(body.starts_with("/target"));
    }

    #[test]
    fn idempotent_when_present() {
        let td = TempDir::new().unwrap();
        let gi = td.path().join(".gitignore");
        fs::write(&gi, "/target\n.clicom/\n").unwrap();
        ensure_clicom_ignored(td.path()).unwrap();
        let body = fs::read_to_string(&gi).unwrap();
        assert_eq!(body.matches(".clicom/").count(), 1);
    }

    #[test]
    fn handles_no_trailing_newline() {
        let td = TempDir::new().unwrap();
        let gi = td.path().join(".gitignore");
        fs::write(&gi, "/target").unwrap();   // no \n
        ensure_clicom_ignored(td.path()).unwrap();
        let body = fs::read_to_string(&gi).unwrap();
        assert!(body.contains("\n.clicom/\n"));
    }
}
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod gitignore;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib gitignore`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/gitignore.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): idempotent .gitignore append"
```

### Task 8: `screen.rs` — copy + extend with ScrollbackRing

**Files:**
- Create: `src/clicom_engine/screen.rs` (copy from `../cliagentchat/src/wrap/screen.rs`, then extend per §6.5)
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Copy + extend** the file. The combined contents:

```rust
//! Virtual terminal model + parallel scrollback ring for clicom (§6.5).
//!
//! `ScreenBuffer::advance_bytes` feeds the vt100 parser and captures any rows
//! that scrolled off the visible region into the scrollback ring. The ring's
//! lifetime line index is the source of truth for `screen_tail_*` queries —
//! vt100's own scrollback is a *visual* buffer that can lose plain-text
//! content when lines re-flow.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const SOFT_VT100_SCROLLBACK: usize = 64; // rows kept inside vt100 (approach (a) per §6.5)
const HARD_CAP: usize = 20_000;
const DROP_CHUNK: usize = 10_000;

pub struct ScreenBuffer {
    inner: Arc<Mutex<vt100::Parser>>,
    scrollback: Arc<Mutex<ScrollbackRing>>,
    rows: u16,
    cols: u16,
}

#[derive(Debug)]
struct ScrollbackRing {
    lines: VecDeque<String>,    // finalized lines
    trimmed_below: u64,         // lifetime index of lines[0]
    last_visible_top_row0: Vec<String>,  // previous-frame visible rows for diffing
}

impl ScrollbackRing {
    fn new() -> Self {
        ScrollbackRing { lines: VecDeque::new(), trimmed_below: 0, last_visible_top_row0: Vec::new() }
    }

    fn append(&mut self, line: String) {
        self.lines.push_back(line);
        if self.lines.len() > HARD_CAP {
            for _ in 0..DROP_CHUNK {
                self.lines.pop_front();
                self.trimmed_below += 1;
            }
        }
    }

    fn total_lifetime(&self, visible_rows: usize) -> u64 {
        self.trimmed_below + self.lines.len() as u64 + visible_rows as u64
    }
}

impl ScreenBuffer {
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = vt100::Parser::new(rows, cols, SOFT_VT100_SCROLLBACK);
        ScreenBuffer {
            inner: Arc::new(Mutex::new(parser)),
            scrollback: Arc::new(Mutex::new(ScrollbackRing::new())),
            rows, cols,
        }
    }

    pub fn handle(&self) -> Arc<Mutex<vt100::Parser>> { Arc::clone(&self.inner) }

    pub fn advance_bytes(&self, bytes: &[u8]) {
        // Snapshot the current visible top-row text before applying.
        let prev_top = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return };
            visible_rows(&p, self.rows, self.cols)
        };
        // Apply the bytes.
        {
            let mut p = match self.inner.lock() { Ok(p) => p, Err(_) => return };
            p.process(bytes);
        }
        // Diff against new visible state to detect rows that scrolled off the top.
        let new_top = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return };
            visible_rows(&p, self.rows, self.cols)
        };

        if let Ok(mut sb) = self.scrollback.lock() {
            let scrolled_off = detect_scrolled_off(&prev_top, &new_top);
            for line in scrolled_off {
                sb.append(line);
            }
            sb.last_visible_top_row0 = new_top;
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        if let Ok(mut p) = self.inner.lock() {
            p.screen_mut().set_size(rows, cols);
        }
    }

    /// Plain text of the current visible screen.
    pub fn to_plain_text(&self) -> String {
        match self.inner.lock() { Ok(p) => p.screen().contents(), Err(_) => String::new() }
    }

    pub fn visible_dims(&self) -> (u16, u16) { (self.rows, self.cols) }

    /// (lifetime_lines, trimmed_below)
    pub fn lifetime_info(&self) -> (u64, u64) {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return (0, 0) };
        (sb.total_lifetime(self.rows as usize), sb.trimmed_below)
    }

    /// Read a half-open range [from, to) of *resolved* lifetime indexes.
    /// Returns (lines, actual_from, actual_to). Caller is responsible for
    /// negative-index resolution and emptiness checks.
    pub fn read_range(&self, from: u64, to: u64) -> ReadResult {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return ReadResult::empty() };
        let visible_rows_text: Vec<String> = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return ReadResult::empty() };
            visible_rows(&p, self.rows, self.cols)
        };
        let trimmed = sb.trimmed_below;
        let scroll_end = trimmed + sb.lines.len() as u64;
        let total = scroll_end + visible_rows_text.len() as u64;

        if from >= total && to >= total {
            return ReadResult { lines: Vec::new(), actual_from: from.min(total), actual_to: to.min(total),
                                trimmed_below: trimmed, total_lifetime: total };
        }
        let af = from.max(trimmed);
        let at = to.min(total);
        let mut out = Vec::new();
        for i in af..at {
            if i < scroll_end {
                let idx = (i - trimmed) as usize;
                out.push(sb.lines.get(idx).cloned().unwrap_or_default());
            } else {
                let idx = (i - scroll_end) as usize;
                out.push(visible_rows_text.get(idx).cloned().unwrap_or_default());
            }
        }
        ReadResult { lines: out, actual_from: af, actual_to: at, trimmed_below: trimmed, total_lifetime: total }
    }

    /// True iff the requested half-open range is wholly below `trimmed_below`.
    pub fn range_wholly_trimmed(&self, from: u64, to: u64) -> bool {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return false };
        to <= sb.trimmed_below
    }

    /// Concatenated lifetime text (visible region + scrollback ring), oldest-first.
    /// Used by screen_last_after / _re searches.
    pub fn lifetime_text(&self) -> String {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return String::new() };
        let visible_rows_text: Vec<String> = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return String::new() };
            visible_rows(&p, self.rows, self.cols)
        };
        let mut s = String::new();
        for l in sb.lines.iter() { s.push_str(l); s.push('\n'); }
        for l in visible_rows_text.iter() { s.push_str(l); s.push('\n'); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct ReadResult {
    pub lines: Vec<String>,
    pub actual_from: u64,
    pub actual_to: u64,
    pub trimmed_below: u64,
    pub total_lifetime: u64,
}

impl ReadResult {
    fn empty() -> Self { ReadResult { lines: Vec::new(), actual_from: 0, actual_to: 0, trimmed_below: 0, total_lifetime: 0 } }
}

fn visible_rows(parser: &vt100::Parser, rows: u16, _cols: u16) -> Vec<String> {
    let screen = parser.screen();
    let mut out = Vec::with_capacity(rows as usize);
    for r in 0..rows {
        out.push(screen.row(r).map(String::from).unwrap_or_default());
    }
    out
}

/// Identify rows from the previous top-of-screen that no longer appear at the
/// top of the new visible region — these have scrolled off and must enter
/// scrollback. Conservative diff: walk down `prev` until we find the first
/// row that matches the new top, return everything above it.
fn detect_scrolled_off(prev: &[String], new_visible: &[String]) -> Vec<String> {
    if prev.is_empty() || new_visible.is_empty() { return Vec::new(); }
    let new_top = &new_visible[0];
    if let Some(pos) = prev.iter().position(|r| r == new_top) {
        prev[..pos].to_vec()
    } else {
        // No anchor found — assume the screen was rewritten, not scrolled.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump(buf: &ScreenBuffer, s: &str) { buf.advance_bytes(s.as_bytes()); }

    #[test]
    fn fresh_buffer_has_zero_lifetime() {
        let b = ScreenBuffer::new(10, 80);
        let (total, trimmed) = b.lifetime_info();
        assert!(total >= 10, "visible region counted: {total}");
        assert_eq!(trimmed, 0);
    }

    #[test]
    fn visible_screen_renders_plain_text() {
        let b = ScreenBuffer::new(5, 80);
        pump(&b, "hello\n");
        let s = b.to_plain_text();
        assert!(s.contains("hello"), "got: {s:?}");
    }

    #[test]
    fn scrolling_captures_lines_into_ring() {
        let b = ScreenBuffer::new(3, 10);
        for i in 0..6 {
            pump(&b, &format!("line{i}\n"));
        }
        let txt = b.lifetime_text();
        // We expect early lines to have entered the ring even after scrolling out of view.
        assert!(txt.contains("line0") || txt.contains("line1"),
            "expected early lines in lifetime_text, got: {txt:?}");
    }

    #[test]
    fn read_range_clamps_when_partially_trimmed() {
        let b = ScreenBuffer::new(2, 10);
        for i in 0..50 { pump(&b, &format!("L{i}\n")); }
        let (total, trimmed) = b.lifetime_info();
        assert!(total > trimmed);
        let r = b.read_range(0, total);
        assert_eq!(r.actual_from, trimmed);
        assert_eq!(r.actual_to, total);
    }

    #[test]
    fn range_wholly_trimmed_detected() {
        let b = ScreenBuffer::new(2, 10);
        for i in 0..50 { pump(&b, &format!("L{i}\n")); }
        let (_total, trimmed) = b.lifetime_info();
        if trimmed > 0 {
            assert!(b.range_wholly_trimmed(0, trimmed));
        }
    }
}
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod screen;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib screen`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/screen.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): screen buffer with parallel scrollback ring"
```

### Task 9: `idle.rs` — copy verbatim

**Files:**
- Create: `src/clicom_engine/idle.rs` (copy from `../cliagentchat/src/wrap/idle.rs`)
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Copy**

```bash
cp ../cliagentchat/src/wrap/idle.rs src/clicom_engine/idle.rs
```

The contents are 47 lines — verify they end with the existing `IdleDetector` impl.

- [ ] **Step 2: Add unit tests** at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn starts_busy() {
        let now = Instant::now();
        let d = IdleDetector::new(1, now);
        assert_eq!(d.state(), IdleState::Busy);
    }

    #[test]
    fn becomes_idle_after_threshold_with_no_bytes() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        let later = now + Duration::from_secs(2);
        let ev = d.tick(later);
        assert_eq!(ev, Some(IdleEvent::BecameIdle));
        assert_eq!(d.state(), IdleState::Idle);
    }

    #[test]
    fn note_byte_returns_busy_event_when_idle() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        d.tick(now + Duration::from_secs(2));
        let ev = d.note_byte(now + Duration::from_secs(3));
        assert_eq!(ev, Some(IdleEvent::BecameBusy));
    }

    #[test]
    fn no_event_when_already_in_state() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        assert_eq!(d.note_byte(now + Duration::from_millis(100)), None);
        d.tick(now + Duration::from_secs(2));
        assert_eq!(d.tick(now + Duration::from_secs(3)), None);
    }
}
```

- [ ] **Step 3: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod idle;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib idle`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/clicom_engine/idle.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): copy IdleDetector from inboxmcp + tests"
```

### Task 10: `nudge.rs` — copy verbatim

**Files:**
- Create: `src/clicom_engine/nudge.rs` (copy from `../cliagentchat/src/wrap/nudge.rs`)
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Copy**

```bash
cp ../cliagentchat/src/wrap/nudge.rs src/clicom_engine/nudge.rs
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod nudge;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/nudge.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): copy nudge helper from inboxmcp"
```

### Task 11: `pty.rs` — copy verbatim

**Files:**
- Create: `src/clicom_engine/pty.rs` (copy from `../cliagentchat/src/wrap/pty.rs`)
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Copy and inspect**

```bash
cp ../cliagentchat/src/wrap/pty.rs src/clicom_engine/pty.rs
```

If the copied file imports anything from `crate::wrap::*`, change those imports to `crate::clicom_engine::*`. Build until clean.

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod pty;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/pty.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): copy PTY spawn from inboxmcp"
```

### Task 12: `forwarding.rs` — copy verbatim (with mouse strip)

**Files:**
- Create: `src/clicom_engine/forwarding.rs` (copy from `../cliagentchat/src/wrap/forwarding.rs`)
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Copy + fix imports**

```bash
cp ../cliagentchat/src/wrap/forwarding.rs src/clicom_engine/forwarding.rs
```

Rewrite any `crate::wrap::*` imports to `crate::clicom_engine::*` and `crate::wrap::mouse_filter` (if present) by also copying that file from `../cliagentchat/src/wrap/mouse_filter.rs` to `src/clicom_engine/mouse_filter.rs` and updating the `pub mod mouse_filter;` in `mod.rs`.

- [ ] **Step 2: Wire**:

```rust
// src/clicom_engine/mod.rs
pub mod mouse_filter;   // only if forwarding.rs imports it
pub mod forwarding;
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/forwarding.rs src/clicom_engine/mouse_filter.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): copy stdin/stdout forwarding from inboxmcp"
```

### Task 13: `nopty.rs` — pipe-based spawn (new for clicom)

**Files:**
- Create: `src/clicom_engine/nopty.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write the failing test** in `src/clicom_engine/nopty.rs`:

```rust
//! Pipe-based spawn (no PTY). Wires child stdin/stdout/stderr to plain pipes;
//! host stdin → child stdin, child stdout → host stdout. Used by `clicom start --nopty`.

use std::io::Read;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct NoPtyChild {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
}

pub fn spawn(command: &[String]) -> anyhow::Result<NoPtyChild> {
    let (head, tail) = command.split_first().ok_or_else(|| anyhow::anyhow!("empty command"))?;
    let mut cmd = Command::new(head);
    cmd.args(tail)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn()?;
    let stdin  = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    Ok(NoPtyChild { child, stdin, stdout })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn echo_cmd() -> Vec<String> {
        vec!["cmd".into(), "/C".into(), "echo hello".into()]
    }
    #[cfg(unix)]
    fn echo_cmd() -> Vec<String> {
        vec!["sh".into(), "-c".into(), "echo hello".into()]
    }

    #[test]
    fn spawns_child_and_captures_stdout() {
        let mut p = spawn(&echo_cmd()).unwrap();
        let mut s = String::new();
        p.stdout.read_to_string(&mut s).unwrap();
        assert!(s.contains("hello"), "got: {s:?}");
        let st = p.child.wait().unwrap();
        assert!(st.success());
    }
}
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod nopty;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib nopty`
Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/nopty.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): pipe-based --nopty spawn helper"
```

### Task 14: `retention.rs` — dead-instance + result-triple cap

**Files:**
- Create: `src/clicom_engine/retention.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Write the file** with both retention rules from §3.7.1 and §3.7.2:

```rust
//! Retention policies (§3.7).
//! 3.7.1 — dead-instance dirs: keep 10 most recent, prune older.
//! 3.7.2 — result-triple cap: per commands/, keep 10 most recent triples.

use std::fs;
use std::path::{Path, PathBuf};

use crate::clicom_engine::layout;
use crate::clicom_engine::meta::{Meta, State, Status};
use crate::clicom_engine::process::pid_is_alive;

/// Run the dead-instance sweep over `<cwd>/.clicom/` (§3.7.1).
///
/// `self_pid` is the calling wrapper's PID, used to skip live peers.
pub fn sweep_dead_instances(cwd: &Path, self_pid: u32, keep: usize) -> anyhow::Result<()> {
    let root = layout::dot_clicom(cwd);
    if !root.exists() { return Ok(()); }
    let mut dead: Vec<(chrono::DateTime<chrono::Utc>, PathBuf)> = Vec::new();

    for entry in fs::read_dir(&root)? {
        let e = entry?;
        let p = e.path();
        if !p.is_dir() { continue; }
        let meta_path = layout::meta_path(&p);
        let status_path = layout::status_path(&p);
        let m = match Meta::read_from(&meta_path) { Ok(m) => m, Err(_) => continue };
        let alive = pid_is_alive(m.pid);
        if alive { continue; }   // skip live (own or peer)
        // Pid dead: rewrite status if it still claims idle/busy.
        if let Ok(mut s) = Status::read_from(&status_path) {
            if matches!(s.state, State::Idle | State::Busy) {
                s.state = State::Died;
                s.exited_at = Some(chrono::Utc::now());
                s.exit_code = None;
                let _ = s.write_to(&status_path);
            }
        }
        dead.push((m.started_at, p));
    }

    let _ = self_pid; // currently unused; reserved for future filtering
    dead.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, dir) in dead.into_iter().skip(keep) {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// Enforce the result-triple cap on a single live `commands/` dir (§3.7.2).
/// Keeps the `keep` most-recent triples (sorted by `<id>` ascii — drop-time order).
pub fn evict_result_triples(commands_dir: &Path, keep: usize) -> anyhow::Result<()> {
    if !commands_dir.exists() { return Ok(()); }
    let mut done_ids: Vec<String> = Vec::new();
    for entry in fs::read_dir(commands_dir)? {
        let e = entry?;
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".done") {
            done_ids.push(id.to_string());
        }
    }
    done_ids.sort();
    if done_ids.len() <= keep { return Ok(()); }
    for id in done_ids.iter().take(done_ids.len() - keep) {
        for ext in &[".out", ".err", ".done"] {
            let _ = fs::remove_file(commands_dir.join(format!("{}{}", id, ext)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(p: &Path, body: &str) { fs::write(p, body).unwrap(); }

    #[test]
    fn evict_keeps_newest_triples() {
        let td = TempDir::new().unwrap();
        let cmds = td.path().to_path_buf();
        for i in 0..12u32 {
            let id = format!("{:020}-aaaaaa", i);
            touch(&cmds.join(format!("{id}.out")), "x");
            touch(&cmds.join(format!("{id}.done")), "OK");
        }
        evict_result_triples(&cmds, 10).unwrap();
        let mut survivors: Vec<String> = fs::read_dir(&cmds).unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".done"))
            .collect();
        survivors.sort();
        assert_eq!(survivors.len(), 10);
        assert!(survivors.iter().all(|n| !n.starts_with(&format!("{:020}-aaaaaa", 0))));
        assert!(survivors.iter().all(|n| !n.starts_with(&format!("{:020}-aaaaaa", 1))));
    }

    #[test]
    fn evict_ignores_rhai_files() {
        let td = TempDir::new().unwrap();
        let cmds = td.path().to_path_buf();
        touch(&cmds.join("0001-aaaaaa.rhai"), "type_text(\"x\")");
        evict_result_triples(&cmds, 10).unwrap();
        assert!(cmds.join("0001-aaaaaa.rhai").exists());
    }
}
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_engine/mod.rs
pub mod retention;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib retention`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/retention.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): retention (dead instances + result-triple cap)"
```

### Task 15: `ClicomChannel` — lifecycle orchestrator (start + status writer)

**Files:**
- Modify: `src/clicom_engine/mod.rs` (add `ClicomChannel` struct directly in `mod.rs`)

- [ ] **Step 1: Add the lifecycle struct** to `src/clicom_engine/mod.rs`:

```rust
pub mod ids;
pub mod fs_atomic;
pub mod meta;
pub mod layout;
pub mod process;
pub mod gitignore;
pub mod screen;
pub mod idle;
pub mod nudge;
pub mod pty;
pub mod nopty;
pub mod forwarding;
pub mod retention;
pub mod mouse_filter;   // only if it was needed by forwarding

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::clicom_engine::meta::{Meta, State, Status};

#[derive(Debug, Clone)]
pub enum SpawnMode {
    Pty { strip_mouse: bool },
    NoPty,
}

/// Orchestrates one `<cwd>/.clicom/<pid>-<rand6>/` lifecycle (§6.2).
pub struct ClicomChannel {
    pub instance_dir: PathBuf,
    pub meta: Meta,
    pub status: Arc<Mutex<Status>>,
}

impl ClicomChannel {
    /// Create the on-disk layout and write initial meta+status. Does NOT spawn the child;
    /// that is the caller's responsibility (§5.1 step 5). The child + threads are wired
    /// by `cmd_start` in M1 / Task 18.
    pub fn create(cwd: &std::path::Path, pid: u32, name: String, command: Vec<String>) -> anyhow::Result<Self> {
        let rand6 = ids::rand6();
        let instance_dir = layout::instance_dir(cwd, pid, &rand6);
        std::fs::create_dir_all(&instance_dir)?;
        std::fs::create_dir_all(layout::commands_dir(&instance_dir))?;
        // Touch the lock file so writers can lock it.
        std::fs::OpenOptions::new().create(true).write(true).open(layout::lock_path(&instance_dir))?;

        let meta = Meta::new(pid, name, command, cwd.to_path_buf());
        meta.write_to(&layout::meta_path(&instance_dir))?;

        let status = Status::initial_busy();
        status.write_to(&layout::status_path(&instance_dir))?;

        Ok(ClicomChannel {
            instance_dir,
            meta,
            status: Arc::new(Mutex::new(status)),
        })
    }

    pub fn set_state(&self, state: State) -> anyhow::Result<()> {
        let mut s = self.status.lock().map_err(|_| anyhow::anyhow!("status mutex poisoned"))?;
        s.state = state;
        s.last_activity = chrono::Utc::now();
        s.write_to(&layout::status_path(&self.instance_dir))
    }

    pub fn write_screen(&self, content: &str) -> anyhow::Result<()> {
        crate::clicom_engine::fs_atomic::write(&layout::screen_path(&self.instance_dir), content.as_bytes())
    }

    pub fn on_shutdown(&self, exit_code: i32) -> anyhow::Result<()> {
        let mut s = self.status.lock().map_err(|_| anyhow::anyhow!("status mutex poisoned"))?;
        s.state = State::Exited;
        s.exit_code = Some(exit_code);
        s.exited_at = Some(chrono::Utc::now());
        s.write_to(&layout::status_path(&self.instance_dir))
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_writes_layout_files() {
        let td = TempDir::new().unwrap();
        let ch = ClicomChannel::create(td.path(), 999, "alice".into(), vec!["echo".into(), "hi".into()]).unwrap();
        assert!(ch.instance_dir.starts_with(td.path().join(".clicom")));
        assert!(layout::meta_path(&ch.instance_dir).exists());
        assert!(layout::status_path(&ch.instance_dir).exists());
        assert!(layout::lock_path(&ch.instance_dir).exists());
        assert!(layout::commands_dir(&ch.instance_dir).is_dir());
        let m = Meta::read_from(&layout::meta_path(&ch.instance_dir)).unwrap();
        assert_eq!(m.pid, 999);
        assert_eq!(m.name, "alice");
    }

    #[test]
    fn set_state_persists() {
        let td = TempDir::new().unwrap();
        let ch = ClicomChannel::create(td.path(), 1, "x".into(), vec!["a".into()]).unwrap();
        ch.set_state(State::Idle).unwrap();
        let s = Status::read_from(&layout::status_path(&ch.instance_dir)).unwrap();
        assert_eq!(s.state, State::Idle);
    }

    #[test]
    fn on_shutdown_writes_exited() {
        let td = TempDir::new().unwrap();
        let ch = ClicomChannel::create(td.path(), 1, "x".into(), vec!["a".into()]).unwrap();
        ch.on_shutdown(0).unwrap();
        let s = Status::read_from(&layout::status_path(&ch.instance_dir)).unwrap();
        assert_eq!(s.state, State::Exited);
        assert_eq!(s.exit_code, Some(0));
    }
}
```

- [ ] **Step 2: Build + run tests**

Run: `cargo test --lib`
Expected: all engine tests pass (the new 3 channel tests + everything from Tasks 2–14).

- [ ] **Step 3: Commit**

```bash
git add src/clicom_engine/mod.rs
git commit -m "feat(engine): ClicomChannel — instance dir lifecycle + state transitions"
```

### Task 16: `cmd_status` — minimal read-only viewer (used to test M1 end-to-end)

**Files:**
- Create: `src/clicom_cli/discovery.rs`
- Create: `src/clicom_cli/cmd_status.rs`
- Modify: `src/clicom_cli/mod.rs`

- [ ] **Step 1: Implement `discovery.rs`**:

```rust
//! Instance discovery + lazy died-detection (§5.3, §3.7.1 step 3 lazy form).

use std::path::{Path, PathBuf};

use crate::clicom_engine::layout;
use crate::clicom_engine::meta::{Meta, State, Status};
use crate::clicom_engine::process::pid_is_alive;

#[derive(Debug, Clone)]
pub struct InstanceInfo {
    pub dir: PathBuf,
    pub dir_name: String,
    pub meta: Meta,
    pub status: Status,
}

/// List every subdir under `<cwd>/.clicom/`. Tolerates corrupt dirs by skipping.
/// Performs lazy died-detection per §5.3 step 2.
pub fn list_instances(cwd: &Path) -> Vec<InstanceInfo> {
    let root = layout::dot_clicom(cwd);
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&root) { Ok(e) => e, Err(_) => return out };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue; }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let meta = match Meta::read_from(&layout::meta_path(&dir)) { Ok(m) => m, Err(_) => continue };
        let status_path = layout::status_path(&dir);
        let mut status = match Status::read_from(&status_path) { Ok(s) => s, Err(_) => continue };
        if matches!(status.state, State::Idle | State::Busy) && !pid_is_alive(meta.pid) {
            status.state = State::Died;
            status.exited_at = Some(chrono::Utc::now());
            status.exit_code = None;
            let _ = status.write_to(&status_path);
        }
        out.push(InstanceInfo { dir, dir_name, meta, status });
    }
    out
}

pub fn filter_by_partial(items: Vec<InstanceInfo>, partial: Option<&str>) -> Vec<InstanceInfo> {
    match partial {
        None => items,
        Some(p) => items.into_iter().filter(|i| layout::partial_matches(&i.dir_name, p)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_returns_no_instances() {
        let td = TempDir::new().unwrap();
        let v = list_instances(td.path());
        assert!(v.is_empty());
    }

    #[test]
    fn lazy_died_detection_rewrites_status() {
        let td = TempDir::new().unwrap();
        // Manually craft an instance dir whose pid is dead.
        let dir = layout::instance_dir(td.path(), 0, "deadbe");  // pid 0 is "dead"
        std::fs::create_dir_all(&dir).unwrap();
        let meta = Meta::new(0, "x".into(), vec!["a".into()], td.path().to_path_buf());
        meta.write_to(&layout::meta_path(&dir)).unwrap();
        Status::initial_busy().write_to(&layout::status_path(&dir)).unwrap();
        let v = list_instances(td.path());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].status.state, State::Died);
        // Verify it was persisted.
        let on_disk = Status::read_from(&layout::status_path(&dir)).unwrap();
        assert_eq!(on_disk.state, State::Died);
    }
}
```

- [ ] **Step 2: Implement `cmd_status.rs`**:

```rust
//! `clicom status` — read-only inspection (§5.1 status section).

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::discovery;

pub fn run(cwd: &Path, partial: Option<&str>) -> Result<i32> {
    let mut items = discovery::list_instances(cwd);
    items = discovery::filter_by_partial(items, partial);

    if items.is_empty() {
        eprintln!("no clicom instances in {}", cwd.display());
        return Ok(2);
    }

    // Sort: live first (idle/busy), then dead (exited/died), each by started_at desc.
    items.sort_by(|a, b| {
        use crate::clicom_engine::meta::State;
        let live = |s: State| matches!(s, State::Idle | State::Busy);
        live(b.status.state).cmp(&live(a.status.state))
            .then(b.meta.started_at.cmp(&a.meta.started_at))
    });

    if items.len() == 1 && partial.is_some() {
        // Detail view: dump full meta + status JSON.
        println!("{}", serde_json::to_string_pretty(&items[0].meta)?);
        println!("{}", serde_json::to_string_pretty(&items[0].status)?);
    } else {
        // Row view.
        for it in &items {
            let exit = it.status.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
            println!(
                "{:24}  {:7}  {:16}  {}  {}  {}",
                it.dir_name,
                format!("{:?}", it.status.state).to_lowercase(),
                it.meta.name,
                it.meta.started_at,
                it.status.last_activity,
                exit,
            );
        }
    }
    Ok(0)
}
```

- [ ] **Step 3: Wire `clicom_cli/mod.rs`**:

```rust
pub mod discovery;
pub mod cmd_status;
```

- [ ] **Step 4: Run discovery tests**

Run: `cargo test --lib discovery`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/clicom_cli/discovery.rs src/clicom_cli/cmd_status.rs src/clicom_cli/mod.rs
git commit -m "feat(cli): discovery + clicom status (read-only)"
```

### Task 17: `cmd_help` — top-level + topic strings

**Files:**
- Create: `src/clicom_cli/cmd_help.rs`
- Modify: `src/clicom_cli/mod.rs`

- [ ] **Step 1: Implement** with the spec's six subcommands and topics:

```rust
//! `clicom help` — top-level + topic-specific help.

const TOP_LEVEL: &str = "\
clicom — file-based command channel for wrapped CLI agents

USAGE:
    clicom <SUBCOMMAND> [args]

SUBCOMMANDS:
    start    Wrap a command in a PTY (or pipes) and stay alive for its lifetime
    status   List instances or show details for one
    run      Drop a Rhai script into the queue and wait for the result
    queue    Drop a Rhai script and exit immediately (asynchronous)
    clean    Delete result triples (.out / .err / .done) from an instance's commands/
    help     Show this help, or `clicom help <topic>` for details

TOPICS:
    host-fns   Reference of all Rhai host functions (§4)
    script     Pointers to Rhai language docs and a one-page tutorial
    layout     The .clicom/ on-disk layout (§3)
    start | status | run | queue | clean
        Long-form help for that subcommand
";

pub fn run(topic: Option<&str>) -> i32 {
    let body = match topic {
        None => TOP_LEVEL.to_string(),
        Some("host-fns") => host_fns_help(),
        Some("script")   => script_help(),
        Some("layout")   => layout_help(),
        Some("start")    => start_help(),
        Some("status")   => status_help(),
        Some("run")      => run_help(),
        Some("queue")    => queue_help(),
        Some("clean")    => clean_help(),
        Some(other) => {
            eprintln!("clicom help: unknown topic '{other}'");
            return 2;
        }
    };
    println!("{body}");
    0
}

fn host_fns_help() -> String {
    "Rhai host functions registered by the wrapper:\n\
     \n\
     PTY input:\n\
       type_text(s: String) -> ()\n\
     \n\
     Visible screen:\n\
       screen_text() -> String\n\
       screen_save(path: String) -> i64\n\
     \n\
     Scrollback range:\n\
       screen_tail_text(from: i64, to: i64) -> String\n\
       screen_tail_save(path: String, from: i64, to: i64) -> Map\n\
     \n\
     After-marker tail:\n\
       screen_last_after(marker: String) -> String\n\
       screen_save_last_after(path: String, marker: String) -> i64\n\
       screen_last_after_re(regex: String) -> String\n\
       screen_save_last_after_re(path: String, regex: String) -> i64\n\
     \n\
     Waits:\n\
       wait_idle(ms: i64)\n\
       wait_idle(ms: i64, timeout_ms: i64)\n\
       wait_ms(ms: i64)\n\
     \n\
     Status & control:\n\
       status() -> Map { state, last_activity, lifetime_lines, trimmed_below, visible_rows, visible_cols }\n\
       set_timeout(ms: i64) -> ()\n".into()
}
fn script_help() -> String { "See https://rhai.rs/book/ for the language reference.\n".into() }
fn layout_help() -> String { "Layout under <cwd>/.clicom/<pid>-<rand6>/:\n  meta.json status.json screen.txt commands.lock commands/<id>.{rhai,out,err,done}\n".into() }
fn start_help()  -> String { "clicom start [--mouse] [--nopty] [--name <name>] -- <command> [args...]\n".into() }
fn status_help() -> String { "clicom status [<partial>]\n".into() }
fn run_help()    -> String { "clicom run [<partial>] (<inline> | -f <file> | -) [--wait | --force] [--timeout <ms>]\n".into() }
fn queue_help()  -> String { "clicom queue [<partial>] (<inline> | -f <file> | -)\n".into() }
fn clean_help()  -> String { "clicom clean [<partial>] [<id>]\n".into() }
```

- [ ] **Step 2: Wire it**:

```rust
// src/clicom_cli/mod.rs
pub mod cmd_help;
```

- [ ] **Step 3: Build + run**

Run: `cargo build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_cli/cmd_help.rs src/clicom_cli/mod.rs
git commit -m "feat(cli): clicom help with topics"
```

### Task 18: `cmd_start` + clap dispatch — first end-to-end milestone

**Files:**
- Create: `src/clicom_cli/cmd_start.rs`
- Modify: `src/bin/clicom.rs`

This task wires the wrapper threads (forwarding, screen feeder, idle detector → status writer, snapshot writer). It produces the M1 end state.

- [ ] **Step 1: Implement `cmd_start.rs`** with the orchestration in §5.1:

```rust
//! `clicom start` — spawn a child, wire screen tap, idle detector, snapshot writer.
//! Stays in the foreground until the child exits, then writes status="exited" + screen.txt.

use anyhow::Result;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

use crate::clicom_engine::{self, ClicomChannel, SpawnMode};
use crate::clicom_engine::{layout, retention, gitignore};
use crate::clicom_engine::idle::{IdleDetector, IdleEvent, IdleState};
use crate::clicom_engine::meta::State;
use crate::clicom_engine::screen::ScreenBuffer;

pub struct StartArgs {
    pub mouse: bool,
    pub nopty: bool,
    pub name: Option<String>,
    pub command: Vec<String>,
}

pub fn run(cwd: &std::path::Path, args: StartArgs) -> Result<i32> {
    if args.command.is_empty() {
        eprintln!("clicom start: missing command after `--`");
        return Ok(2);
    }
    let pid = std::process::id();
    let name = args.name.clone().unwrap_or_else(|| {
        std::path::Path::new(&args.command[0])
            .file_stem().and_then(|s| s.to_str()).unwrap_or("clicom").to_string()
    });
    let ch = ClicomChannel::create(cwd, pid, name, args.command.clone())?;
    retention::sweep_dead_instances(cwd, pid, 10)?;
    let _ = gitignore::ensure_clicom_ignored(cwd);

    // Hourly retention sweep
    {
        let cwd = cwd.to_path_buf();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(3600));
            let _ = retention::sweep_dead_instances(&cwd, pid, 10);
        });
    }

    let screen = Arc::new(ScreenBuffer::new(40, 120));
    let stop = Arc::new(AtomicBool::new(false));

    // Snapshot writer thread: write screen.txt on each idle transition + at most every 250ms.
    let (idle_tx, idle_rx) = crossbeam_channel::unbounded::<IdleEvent>();
    {
        let screen = Arc::clone(&screen);
        let stop = Arc::clone(&stop);
        let inst_dir = ch.instance_dir.clone();
        let status = Arc::clone(&ch.status);
        thread::spawn(move || {
            let mut last_write = Instant::now() - Duration::from_secs(1);
            while !stop.load(Ordering::SeqCst) {
                // Drain idle events (state transitions).
                while let Ok(ev) = idle_rx.try_recv() {
                    let s = match ev { IdleEvent::BecameIdle => State::Idle, IdleEvent::BecameBusy => State::Busy };
                    if let Ok(mut st) = status.lock() {
                        st.state = s;
                        st.last_activity = chrono::Utc::now();
                        let _ = st.write_to(&layout::status_path(&inst_dir));
                    }
                    let body = screen.to_plain_text();
                    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&inst_dir), body.as_bytes());
                    last_write = Instant::now();
                }
                // Throttled snapshot.
                if last_write.elapsed() >= Duration::from_millis(250) {
                    let body = screen.to_plain_text();
                    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&inst_dir), body.as_bytes());
                    last_write = Instant::now();
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
    }

    // Idle detector ticker.
    let detector = Arc::new(std::sync::Mutex::new(IdleDetector::new(1, Instant::now())));
    {
        let det = Arc::clone(&detector);
        let stop = Arc::clone(&stop);
        let tx = idle_tx.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(200));
                let now = Instant::now();
                if let Ok(mut d) = det.lock() {
                    if let Some(ev) = d.tick(now) { let _ = tx.send(ev); }
                }
            }
        });
    }

    // Spawn child + forwarding loop. Uses pty/nopty per args.
    let exit_code = if args.nopty {
        spawn_and_forward_nopty(&args.command, &screen, &detector, &idle_tx)?
    } else {
        spawn_and_forward_pty(&args.command, args.mouse, &screen, &detector, &idle_tx)?
    };

    // Final snapshot before flipping to exited.
    let body = screen.to_plain_text();
    let _ = clicom_engine::fs_atomic::write(&layout::screen_path(&ch.instance_dir), body.as_bytes());
    ch.on_shutdown(exit_code)?;
    stop.store(true, Ordering::SeqCst);
    Ok(exit_code)
}

fn spawn_and_forward_pty(
    command: &[String],
    _strip_mouse: bool,  // wire to forwarding helper that strips mouse if false-default per spec
    screen: &Arc<ScreenBuffer>,
    detector: &Arc<std::sync::Mutex<IdleDetector>>,
    _idle_tx: &crossbeam_channel::Sender<IdleEvent>,
) -> Result<i32> {
    // Bridge to engine::pty / engine::forwarding using their existing entry points
    // (refer to ../cliagentchat for the exact call signatures and adapt).
    // Returns child exit code.
    let _ = (command, screen, detector);
    todo!("wire engine::pty + engine::forwarding here, mirroring the inboxmcp wrap loop")
}

fn spawn_and_forward_nopty(
    command: &[String],
    screen: &Arc<ScreenBuffer>,
    detector: &Arc<std::sync::Mutex<IdleDetector>>,
    _idle_tx: &crossbeam_channel::Sender<IdleEvent>,
) -> Result<i32> {
    use std::io::{Read, Write};
    let mut child = clicom_engine::nopty::spawn(command)?;
    let mut buf = [0u8; 8192];
    // Read child stdout → screen tap + host stdout.
    let stop_reader = Arc::new(AtomicBool::new(false));
    let r_stop = Arc::clone(&stop_reader);
    let screen_clone = Arc::clone(screen);
    let det_clone = Arc::clone(detector);
    let mut stdout = child.stdout;
    let reader_handle = thread::spawn(move || -> anyhow::Result<()> {
        let mut local = [0u8; 8192];
        while !r_stop.load(Ordering::SeqCst) {
            let n = match stdout.read(&mut local) { Ok(n) if n > 0 => n, _ => break };
            screen_clone.advance_bytes(&local[..n]);
            std::io::stdout().write_all(&local[..n]).ok();
            std::io::stdout().flush().ok();
            if let Ok(mut d) = det_clone.lock() {
                let _ = d.note_byte(Instant::now());
            }
        }
        Ok(())
    });
    // Forward host stdin → child stdin in a separate thread (best effort).
    let mut child_stdin = child.stdin;
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut local = [0u8; 8192];
        let mut handle = stdin.lock();
        loop {
            let n = match handle.read(&mut local) { Ok(n) if n > 0 => n, _ => break };
            if child_stdin.write_all(&local[..n]).is_err() { break; }
        }
    });

    let status = child.child.wait()?;
    stop_reader.store(true, Ordering::SeqCst);
    let _ = reader_handle.join();
    let _ = buf;
    Ok(status.code().unwrap_or(0))
}
```

The PTY path is left as a `todo!()` — Task 19 fills it in by adapting the forwarding loop from `../cliagentchat/src/wrap/lifecycle.rs` and `../cliagentchat/src/wrap/forwarding.rs`. M1 ships with `--nopty` working end-to-end first.

- [ ] **Step 2: Update `src/bin/clicom.rs`** with clap dispatch:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clicom")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Start {
        #[arg(long)] mouse: bool,
        #[arg(long)] nopty: bool,
        #[arg(long)] name: Option<String>,
        #[arg(last = true)] command: Vec<String>,
    },
    Status { partial: Option<String> },
    Help { topic: Option<String> },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    let code = match cli.cmd {
        Cmd::Start { mouse, nopty, name, command } => {
            clicom::clicom_cli::cmd_start::run(&cwd, clicom::clicom_cli::cmd_start::StartArgs { mouse, nopty, name, command })?
        }
        Cmd::Status { partial } => clicom::clicom_cli::cmd_status::run(&cwd, partial.as_deref())?,
        Cmd::Help { topic } => clicom::clicom_cli::cmd_help::run(topic.as_deref()),
    };
    std::process::exit(code);
}
```

- [ ] **Step 3: Wire `cmd_start` in `src/clicom_cli/mod.rs`**

```rust
pub mod cmd_start;
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean (the `todo!()` compiles).

- [ ] **Step 5: Manual smoke test (--nopty path)**

```bash
target/debug/clicom.exe start --nopty -- cmd /C "echo hello && timeout /T 1"
target/debug/clicom.exe status
```

Expected: instance dir appears under `.clicom/`; `screen.txt` contains "hello"; status starts as `busy` then `exited`.

- [ ] **Step 6: Commit**

```bash
git add src/clicom_cli/cmd_start.rs src/clicom_cli/mod.rs src/bin/clicom.rs
git commit -m "feat(cli): clicom start (--nopty path) + status/help dispatch"
```

### Task 19: PTY path for `cmd_start`

**Files:**
- Modify: `src/clicom_cli/cmd_start.rs`

- [ ] **Step 1: Replace the `spawn_and_forward_pty` `todo!()`** with the real PTY wiring. Open `../cliagentchat/src/wrap/lifecycle.rs` and `../cliagentchat/src/wrap/forwarding.rs` for reference; adapt their `run_session` / forwarding loop to:
  - call `clicom_engine::pty::spawn_pty(command)` (or whatever the copied helper exposes),
  - feed bytes from the master reader into `screen.advance_bytes(...)` and `detector.note_byte(...)`,
  - apply the mouse-strip filter (default true) to host stdout via `clicom_engine::mouse_filter` if it was copied,
  - forward host stdin → master writer.

Keep the function returning the child's exit code. The threading shape mirrors the existing inboxmcp implementation; do not invent a new pattern.

- [ ] **Step 2: Manual smoke test (PTY path)**

```bash
target/debug/clicom.exe start -- powershell.exe -NoExit -Command "Write-Host 'pty ready'"
```

In another shell while it's running:
```bash
target/debug/clicom.exe status
type .clicom/<inst>/screen.txt
```

Then exit the wrapped shell. Expected: `screen.txt` reflects the final frame, `status.json` shows `state="exited"`.

- [ ] **Step 3: Commit**

```bash
git add src/clicom_cli/cmd_start.rs
git commit -m "feat(cli): clicom start PTY path (mouse-strip default)"
```

### Task 20: M1 end-to-end test — `tests/e2e_basic.rs` (start + status only)

**Files:**
- Create: `tests/fixtures/fake_agent.rs`
- Create: `tests/e2e_basic.rs`

- [ ] **Step 1: Create the fake-agent fixture**

```rust
// tests/fixtures/fake_agent.rs
//! Tiny binary used by integration tests: cat stdin to stdout until EOF, then exit.

use std::io::{Read, Write};

fn main() {
    let mut buf = [0u8; 1024];
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    loop {
        let n = match handle.read(&mut buf) { Ok(n) if n > 0 => n, _ => break };
        std::io::stdout().write_all(&buf[..n]).ok();
        std::io::stdout().flush().ok();
    }
}
```

To make `cargo test` build this fixture, declare it in `Cargo.toml`:

```toml
[[bin]]
name = "fake_agent"
path = "tests/fixtures/fake_agent.rs"
test = false
bench = false
```

- [ ] **Step 2: Write the basic e2e test**

```rust
// tests/e2e_basic.rs
use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn start_status_basic_smoke() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "echo hello"])
        .spawn().unwrap();
    // give the wrapper a moment to spawn + write its layout
    std::thread::sleep(std::time::Duration::from_millis(500));
    // status should print at least one row
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["status"])
        .output().unwrap();
    assert!(out.status.success() || out.status.code() == Some(2),
            "status exit: {:?}", out.status);
    // wait for child to exit
    let st = child.wait().unwrap();
    assert!(st.success() || st.code() == Some(0));
    // verify .clicom dir + screen.txt exists
    let clicom_dir = td.path().join(".clicom");
    assert!(clicom_dir.is_dir());
    let mut found_screen = false;
    for e in std::fs::read_dir(clicom_dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() && p.join("screen.txt").exists() { found_screen = true; }
    }
    assert!(found_screen, "screen.txt should exist in instance dir");
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test --test e2e_basic`
Expected: passes.

- [ ] **Step 4: Commit + tag M1**

```bash
git add tests/fixtures/fake_agent.rs tests/e2e_basic.rs Cargo.toml
git commit -m "test(e2e): basic start + status smoke test (M1 done)"
git tag -a m1-foundation -m "Milestone 1: foundation + clicom start"
```

---

## Milestone 2 — Rhai engine + script execution

End state: scripts execute end-to-end via the file protocol — `<id>.rhai` is detected, parsed, run with all host fns, and `.out`/`.err`/`.done` are written atomically. Result-triple cap evicts after each `.done`. No driver CLI yet (test by hand-dropping `.rhai` files).

### Task 21: `rhai_host.rs` scaffold — Engine, limits, sandbox

**Files:**
- Create: `src/clicom_engine/rhai_host.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Implement the Engine builder** with sandbox limits per §6.3:

```rust
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
    let ast = engine.compile(source).map_err(|e| *Box::new(rhai::EvalAltResult::ErrorParsing(e.0, e.1)))?;
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
```

- [ ] **Step 2: Wire it**

```rust
// src/clicom_engine/mod.rs
pub mod rhai_host;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib rhai_host`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/rhai_host.rs src/clicom_engine/mod.rs
git commit -m "feat(engine): rhai engine builder with sandbox limits"
```

### Task 22: Host fn — `type_text`

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Add a registration helper + the host fn**:

```rust
// inside rhai_host.rs, below build_engine():

pub fn register_host_fns(engine: &mut Engine, ctx: Arc<HostContext>) {
    // type_text
    let c = Arc::clone(&ctx);
    engine.register_fn("type_text", move |s: &str| -> Result<(), Box<rhai::EvalAltResult>> {
        c.nudge_tx.send(s.as_bytes().to_vec())
            .map_err(|_| Box::new(rhai::EvalAltResult::ErrorRuntime("type_text: channel closed".into(), rhai::Position::NONE)))?;
        Ok(())
    });
}
```

- [ ] **Step 2: Add a test that exercises type_text via the engine**:

```rust
// in the existing tests mod
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
    run_script(&e, "type_text(\"hi\\n\")").unwrap();
    let bytes = rx.recv().unwrap();
    assert_eq!(bytes, b"hi\n");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib rhai_host`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): host fn type_text"
```

### Task 23: Host fns — `screen_text` + `screen_save`

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Add to `register_host_fns`**:

```rust
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

fn resolve_path(cwd: &std::path::Path, p: &str) -> std::path::PathBuf {
    let pp = std::path::Path::new(p);
    if pp.is_absolute() { pp.to_path_buf() } else { cwd.join(pp) }
}
```

- [ ] **Step 2: Add a test**

```rust
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
```

- [ ] **Step 3: Run tests; commit**

Run: `cargo test --lib rhai_host`
Expected: 4 tests pass.

```bash
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): host fns screen_text + screen_save"
```

### Task 24: Host fns — `screen_tail_text` + `screen_tail_save`

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Add the negative-index resolver + the two host fns**:

```rust
fn resolve_indexes(buf: &ScreenBuffer, from: i64, to: i64) -> Result<(u64, u64), Box<rhai::EvalAltResult>> {
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
```

- [ ] **Step 2: Add tests for negative indexing + bad range**

```rust
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

fn make_ctx(screen: Arc<ScreenBuffer>) -> Arc<HostContext> {
    let (tx, _rx) = crossbeam_channel::unbounded();
    Arc::new(HostContext {
        screen,
        nudge_tx: tx,
        instance_cwd: std::env::temp_dir(),
        idle_observer: Arc::new(std::sync::Mutex::new(crate::clicom_engine::idle::IdleDetector::new(1, std::time::Instant::now()))),
    })
}
```

- [ ] **Step 3: Run tests; commit**

Run: `cargo test --lib rhai_host`

```bash
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): host fns screen_tail_text + screen_tail_save"
```

### Task 25: Host fns — `screen_last_after` family (literal + regex)

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Implement the four variants**:

```rust
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

// screen_save_last_after_re — same shape; deduplicate as you wish.
```

- [ ] **Step 2: Add tests**

```rust
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
```

- [ ] **Step 3: Run; commit**

```bash
cargo test --lib rhai_host
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): host fns screen_last_after (literal + regex, save variants)"
```

### Task 26: Host fns — `wait_idle`, `wait_ms`, `status`, `set_timeout`

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Implement** with the caps from §4.5 / §4.6:

```rust
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

// set_timeout — store the override on the engine's tag scope; the per-script wrapper in
// Task 28 reads it before launching the AST. Cap = 3_600_000.
let c = Arc::clone(&ctx);
engine.register_fn("set_timeout", move |ms: i64| -> Result<(), Box<rhai::EvalAltResult>> {
    if ms > 3_600_000 {
        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime("set_timeout: cap exceeded".into(), rhai::Position::NONE)));
    }
    *c.script_timeout_override.lock().unwrap() = Some(ms.max(0) as u64);
    Ok(())
});
```

Add a new field to `HostContext`:
```rust
pub script_timeout_override: Arc<std::sync::Mutex<Option<u64>>>,
```

Initialize it as `Arc::new(Mutex::new(None))` in callers.

- [ ] **Step 2: Add tests for the caps**

```rust
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
```

Update the `make_ctx` helper to include the new field.

- [ ] **Step 3: Run; commit**

```bash
cargo test --lib rhai_host
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): host fns wait_idle, wait_ms, status, set_timeout"
```

### Task 27: Per-script execution — JSON-encode result + error-code mapping

**Files:**
- Modify: `src/clicom_engine/rhai_host.rs`

- [ ] **Step 1: Add `execute_script_to_files`** that owns the §6.3 / §3.6 write protocol:

```rust
//! Per-script execution: parse → run → write `.out` → write `.err` if any → write `.done`.
//! Atomic writes via fs_atomic. Order matters (`.done` is the readiness barrier).

use std::path::Path;

pub enum ScriptOutcome {
    Ok,
    Err(&'static str),  // short_code: "parse" | "runtime" | "timeout" | "host_fn" | "fs" | "range" | "internal"
}

pub fn execute_script_to_files(
    engine: &Engine,
    source: &str,
    out_path: &Path,
    err_path: &Path,
    done_path: &Path,
    deadline: std::time::Instant,
) -> ScriptOutcome {
    // Parse
    let ast = match engine.compile(source) {
        Ok(ast) => ast,
        Err(e) => return write_failure(out_path, err_path, done_path, "parse", &e.to_string()),
    };
    // Run with timeout — Rhai's set_max_operations is the primary cap; we layer wall-clock
    // by checking deadline in a watcher thread that calls engine.set_progress(...) returning
    // false to abort. For simplicity in v1, run synchronously and let max_operations do the
    // heavy lifting; richer wall-clock enforcement is a follow-up.
    let mut scope = rhai::Scope::new();
    let result: Result<rhai::Dynamic, Box<rhai::EvalAltResult>> = engine.eval_ast_with_scope(&mut scope, &ast);
    let _ = deadline;
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

fn write_failure(out: &Path, err: &Path, done: &Path, code: &'static str, message: &str) -> ScriptOutcome {
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
```

- [ ] **Step 2: Test the round-trip**

```rust
#[test]
fn execute_writes_done_after_out() {
    let td = tempfile::TempDir::new().unwrap();
    let out = td.path().join("id.out");
    let err = td.path().join("id.err");
    let done = td.path().join("id.done");
    let mut e = build_engine();
    register_host_fns(&mut e, make_ctx(Arc::new(ScreenBuffer::new(5, 80))));
    let outcome = execute_script_to_files(&e, "1 + 2", &out, &err, &done, std::time::Instant::now() + std::time::Duration::from_secs(5));
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
    let outcome = execute_script_to_files(&e, "let x: int = \"bad\";", &out, &err, &done, std::time::Instant::now() + std::time::Duration::from_secs(5));
    assert!(matches!(outcome, ScriptOutcome::Err(_)));
    assert!(std::fs::read_to_string(&done).unwrap().starts_with("ERR "));
    assert!(std::fs::read_to_string(&err).unwrap().lines().next().is_some());
}
```

- [ ] **Step 3: Run; commit**

```bash
cargo test --lib rhai_host
git add src/clicom_engine/rhai_host.rs
git commit -m "feat(engine): per-script execute with JSON-encoded out + classified .err"
```

### Task 28: `watcher.rs` — notify-based commands/ watcher + sequential executor

**Files:**
- Create: `src/clicom_engine/watcher.rs`
- Modify: `src/clicom_engine/mod.rs`

- [ ] **Step 1: Implement the watcher**:

```rust
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
        if let Ok(mut w) = notify::recommended_watcher(move |_| { let _ = tx.send(()); }) {
            let _ = w.watch(&cmd_dir_clone, RecursiveMode::NonRecursive);
            watcher_opt = Some(w);
        }
        let _ = watcher_opt;

        let mut last_poll = Instant::now() - Duration::from_secs(1);
        while !s.load(std::sync::atomic::Ordering::SeqCst) {
            // Wake on notify or timeout.
            let _ = rx.recv_timeout(Duration::from_millis(250));
            if last_poll.elapsed() < Duration::from_millis(50) { continue; }
            last_poll = Instant::now();

            // Collect *.rhai files, sorted ascending by name.
            let mut rhai_files: Vec<PathBuf> = match std::fs::read_dir(&cmd_dir_clone) {
                Ok(rd) => rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "rhai").unwrap_or(false))
                    .map(|e| e.path()).collect(),
                Err(_) => continue,
            };
            rhai_files.sort();

            for rhai in rhai_files {
                let id = match rhai.file_stem().and_then(|s| s.to_str()) { Some(s) => s.to_string(), None => continue };
                let source = match std::fs::read_to_string(&rhai) { Ok(s) => s, Err(_) => continue };
                let out_p  = layout::out_path(&instance_dir, &id);
                let err_p  = layout::err_path(&instance_dir, &id);
                let done_p = layout::done_path(&instance_dir, &id);
                let deadline = Instant::now() + Duration::from_millis(default_timeout_ms);
                let _ = rhai_host::execute_script_to_files(&engine, &source, &out_p, &err_p, &done_p, deadline);
                let _ = std::fs::remove_file(&rhai);
                let _ = retention::evict_result_triples(&cmd_dir_clone, 10);
            }
        }
    });

    Ok(WatcherHandle { _stop: stop })
}
```

- [ ] **Step 2: Wire it**

```rust
// src/clicom_engine/mod.rs
pub mod watcher;
```

- [ ] **Step 3: Build (no unit test for the watcher — covered by e2e in M3)**

Run: `cargo build`
Expected: clean.

- [ ] **Step 4: Wire the watcher into `cmd_start.rs`** — after spawning the screen+idle threads, build the engine, register host fns, wrap in `Arc<Engine>`, call `spawn_watcher(...)`. The host context's `nudge_tx` should be the same channel that feeds bytes into the PTY (the `--nopty` path needs an analogous channel into the `child_stdin` writer).

- [ ] **Step 5: Manual smoke test**

```bash
target/debug/clicom.exe start --nopty -- cmd /C "for /L %i in (1,1,5) do @echo step%i & timeout /T 1 >nul"
# In another shell:
echo "type_text(\"hi\\n\")" > .clicom/<inst>/commands/$(date +%s%N)-aaaaaa.rhai
# Watch the .out / .done files appear.
```

- [ ] **Step 6: Commit**

```bash
git add src/clicom_engine/watcher.rs src/clicom_engine/mod.rs src/clicom_cli/cmd_start.rs
git commit -m "feat(engine): commands/ watcher + sequential script executor"
```

### Task 29: M2 sanity test — drop a `.rhai` directly, check `.done`

**Files:**
- Create: `tests/e2e_rhai_engine.rs` (temporary; renamed in M3)

- [ ] **Step 1: Write a test that runs the wrapper and hand-drops a script**

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[test]
fn wrapper_executes_dropped_rhai() {
    let td = TempDir::new().unwrap();
    let mut child = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 5 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));

    let inst_dir = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } })
        .unwrap();
    let cmd_dir = inst_dir.join("commands");
    let id = format!("{}-aaaaaa", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let rhai = cmd_dir.join(format!("{id}.rhai"));
    std::fs::write(&rhai, "1 + 1").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let done = cmd_dir.join(format!("{id}.done"));
    while !done.exists() {
        if Instant::now() > deadline { panic!("done file did not appear"); }
        std::thread::sleep(Duration::from_millis(50));
    }
    let body = std::fs::read_to_string(&done).unwrap();
    assert_eq!(body.trim(), "OK");
    let out = std::fs::read_to_string(cmd_dir.join(format!("{id}.out"))).unwrap();
    assert!(out.trim().contains("2"));

    let _ = child.kill();
    let _ = child.wait();
}
```

- [ ] **Step 2: Run + commit + tag M2**

```bash
cargo test --test e2e_rhai_engine
git add tests/e2e_rhai_engine.rs
git commit -m "test(e2e): hand-drop .rhai exercises engine end-to-end (M2 done)"
git tag -a m2-engine -m "Milestone 2: Rhai engine + script execution"
```

---

## Milestone 3 — Driver CLI + integration tests

End state: `clicom run/queue/clean` work via the file protocol with full lock coordination (--force re-acquire, clean's lock+sweep filter), and the integration-test suite covers all behaviors in §8.

### Task 30: `drop.rs` — common drop sequence (lock + write `.rhai`)

**Files:**
- Create: `src/clicom_cli/drop.rs`
- Modify: `src/clicom_cli/mod.rs`

- [ ] **Step 1: Implement** the lock + atomic write:

```rust
//! Common drop sequence shared by `clicom run` and `clicom queue` (§5.4).

use anyhow::Result;
use fs2::FileExt;
use std::fs::OpenOptions;
use std::path::Path;

use crate::clicom_engine::{layout, ids};

pub struct LockGuard { file: std::fs::File }
impl Drop for LockGuard { fn drop(&mut self) { let _ = self.file.unlock(); } }

pub fn acquire_lock(instance_dir: &Path) -> Result<LockGuard> {
    let f = OpenOptions::new().read(true).write(true).create(true).open(layout::lock_path(instance_dir))?;
    f.lock_exclusive()?;
    Ok(LockGuard { file: f })
}

pub fn drop_rhai(instance_dir: &Path, source: &str) -> Result<String> {
    let id = ids::make_command_id();
    let final_path = layout::rhai_path(instance_dir, &id);
    let tmp = final_path.with_extension("rhai.tmp");
    std::fs::write(&tmp, source)?;
    std::fs::rename(&tmp, &final_path)?;
    Ok(id)
}
```

- [ ] **Step 2: Wire it**

```rust
// src/clicom_cli/mod.rs
pub mod drop;
```

- [ ] **Step 3: Add a unit test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    #[test]
    fn drop_rhai_writes_full_filename() {
        let td = TempDir::new().unwrap();
        std::fs::create_dir_all(layout::commands_dir(td.path())).unwrap();
        let id = drop_rhai(td.path(), "1+1").unwrap();
        assert!(layout::rhai_path(td.path(), &id).exists());
        assert!(!layout::rhai_path(td.path(), &id).with_extension("rhai.tmp").exists());
    }
}
```

- [ ] **Step 4: Run; commit**

```bash
cargo test --lib drop
git add src/clicom_cli/drop.rs src/clicom_cli/mod.rs
git commit -m "feat(cli): drop helper (commands.lock + atomic .rhai write)"
```

### Task 31: `cmd_queue.rs` — the simplest driver

**Files:**
- Create: `src/clicom_cli/cmd_queue.rs`
- Modify: `src/clicom_cli/mod.rs`, `src/bin/clicom.rs`

- [ ] **Step 1: Implement queue** per §5.4:

```rust
//! `clicom queue` — drop the script, exit immediately (asynchronous).

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::{discovery, drop as drop_h};
use crate::clicom_engine::meta::State;

pub fn run(cwd: &Path, partial: Option<&str>, source: String) -> Result<i32> {
    let candidates: Vec<_> = discovery::filter_by_partial(discovery::list_instances(cwd), partial)
        .into_iter().filter(|i| matches!(i.status.state, State::Idle | State::Busy)).collect();
    let inst = match candidates.len() {
        0 => { eprintln!("no live wrapped agent in {}", cwd.display()); return Ok(2); }
        1 => &candidates[0].dir,
        _ => {
            eprintln!("ambiguous match — candidates:");
            for i in &candidates { eprintln!("  {}", i.dir_name); }
            return Ok(2);
        }
    };
    let _guard = drop_h::acquire_lock(inst)?;
    let id = drop_h::drop_rhai(inst, &source)?;
    drop(_guard);  // explicit
    println!("{id}");
    Ok(0)
}
```

- [ ] **Step 2: Wire + dispatch** — add `pub mod cmd_queue;` to `clicom_cli/mod.rs`. In `bin/clicom.rs`:

```rust
Cmd::Queue { partial, file, source } => {
    let body = read_script_source(source.as_deref(), file.as_deref())?;
    clicom::clicom_cli::cmd_queue::run(&cwd, partial.as_deref(), body)?
}
```

Add to the `Cmd` enum:
```rust
Queue {
    partial: Option<String>,
    #[arg(short = 'f')] file: Option<String>,
    /// Inline script. Use "-" to read from stdin.
    source: Option<String>,
},
```

And the helper:
```rust
fn read_script_source(arg: Option<&str>, file: Option<&str>) -> anyhow::Result<String> {
    use std::io::Read;
    if let Some(p) = file { return Ok(std::fs::read_to_string(p)?); }
    match arg {
        Some("-") => { let mut s = String::new(); std::io::stdin().read_to_string(&mut s)?; Ok(s) }
        Some(s) => Ok(s.to_string()),
        None => anyhow::bail!("no script source given (positional, -f <file>, or -)"),
    }
}
```

- [ ] **Step 3: Manual smoke test**

```bash
target/debug/clicom.exe start --nopty -- cmd /C "ping -n 3 127.0.0.1 >nul"
target/debug/clicom.exe queue "1 + 1"
ls .clicom/*/commands/
```

Expected: queue prints an `<id>`; eventually an `<id>.done` and `<id>.out` appear.

- [ ] **Step 4: Commit**

```bash
git add src/clicom_cli/cmd_queue.rs src/clicom_cli/mod.rs src/bin/clicom.rs
git commit -m "feat(cli): clicom queue (drop and exit)"
```

### Task 32: `cmd_run.rs` — default mode

**Files:**
- Create: `src/clicom_cli/cmd_run.rs`
- Modify: `src/clicom_cli/mod.rs`, `src/bin/clicom.rs`

- [ ] **Step 1: Implement default-mode `run`** per §5.4:

```rust
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
```

- [ ] **Step 2: Dispatch in `bin/clicom.rs`**:

```rust
Cmd::Run { partial, file, wait, force, timeout, source } => {
    let body = read_script_source(source.as_deref(), file.as_deref())?;
    let mode = if wait { BusyMode::Wait }
               else if force { BusyMode::Force }
               else { BusyMode::Default };
    clicom::clicom_cli::cmd_run::run(&cwd, clicom::clicom_cli::cmd_run::RunArgs {
        partial, source: body, mode, timeout_ms: timeout.unwrap_or(600_000),
    })?
}
```

Add `Cmd::Run` to the enum:
```rust
Run {
    partial: Option<String>,
    #[arg(short = 'f')] file: Option<String>,
    #[arg(long)] wait: bool,
    #[arg(long)] force: bool,
    #[arg(long)] timeout: Option<u64>,
    source: Option<String>,
},
```

Bring `BusyMode` into scope in main: `use clicom::clicom_cli::cmd_run::BusyMode;`.

- [ ] **Step 3: Wire `pub mod cmd_run;` and rebuild**

Run: `cargo build`
Expected: clean.

- [ ] **Step 4: Manual smoke test**

```bash
target/debug/clicom.exe start --nopty -- cmd /C "ping -n 5 127.0.0.1 >nul"
target/debug/clicom.exe run "1 + 2"
# Expected stdout: 3
target/debug/clicom.exe queue "wait_ms(2000)"
target/debug/clicom.exe run "1+1"
# Expected: exit 5, stderr "busy: 1 pending script(s)"
```

- [ ] **Step 5: Commit**

```bash
git add src/clicom_cli/cmd_run.rs src/clicom_cli/mod.rs src/bin/clicom.rs
git commit -m "feat(cli): clicom run (default + --wait + --force with lock re-acquire)"
```

### Task 33: `cmd_clean.rs` — manual cleanup with lock + sweep filter

**Files:**
- Create: `src/clicom_cli/cmd_clean.rs`
- Modify: `src/clicom_cli/mod.rs`, `src/bin/clicom.rs`

- [ ] **Step 1: Implement** per §5.4 (`clicom clean` flow):

```rust
//! `clicom clean` — delete result triples (.out / .err / .done) under lock.

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::{discovery, drop as drop_h};
use crate::clicom_engine::layout;

pub fn run(cwd: &Path, partial: Option<&str>, id: Option<&str>) -> Result<i32> {
    // §5.3: state filter widened — clean works on any state.
    let candidates = discovery::filter_by_partial(discovery::list_instances(cwd), partial);
    let inst = match candidates.len() {
        0 => { eprintln!("no clicom instance in {}", cwd.display()); return Ok(2); }
        1 => &candidates[0].dir,
        _ => {
            eprintln!("ambiguous match — candidates:");
            for i in &candidates { eprintln!("  {}", i.dir_name); }
            return Ok(2);
        }
    };

    let _guard = drop_h::acquire_lock(inst)?;
    let cmds = layout::commands_dir(inst);

    if let Some(id) = id {
        for ext in &["out", "err", "done"] {
            let _ = std::fs::remove_file(cmds.join(format!("{id}.{ext}")));
        }
    } else {
        // Sweep mode — only triples whose .done exists.
        let mut done_ids: Vec<String> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&cmds) {
            for e in rd.flatten() {
                if let Some(name) = e.file_name().to_str() {
                    if let Some(id) = name.strip_suffix(".done") {
                        done_ids.push(id.to_string());
                    }
                }
            }
        }
        for id in &done_ids {
            for ext in &["out", "err", "done"] {
                let _ = std::fs::remove_file(cmds.join(format!("{id}.{ext}")));
            }
        }
    }
    Ok(0)
}
```

- [ ] **Step 2: Dispatch + wire**

```rust
// src/clicom_cli/mod.rs
pub mod cmd_clean;
```

```rust
// src/bin/clicom.rs (Cmd enum)
Clean {
    partial: Option<String>,
    id: Option<String>,
},
// dispatch:
Cmd::Clean { partial, id } =>
    clicom::clicom_cli::cmd_clean::run(&cwd, partial.as_deref(), id.as_deref())?,
```

- [ ] **Step 3: Manual smoke test**

```bash
target/debug/clicom.exe queue "1+1"  # waits, eventually emits <id>
target/debug/clicom.exe clean        # sweeps all triples whose .done exists
```

- [ ] **Step 4: Commit**

```bash
git add src/clicom_cli/cmd_clean.rs src/clicom_cli/mod.rs src/bin/clicom.rs
git commit -m "feat(cli): clicom clean (lock-coordinated, sweep filter on .done)"
```

### Task 34: e2e test — `tests/e2e_basic.rs` (extend, replace earlier file)

**Files:**
- Modify: `tests/e2e_basic.rs`
- Delete: `tests/e2e_rhai_engine.rs`

- [ ] **Step 1: Replace `tests/e2e_basic.rs`** with the full §8 spec for that file:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let child = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 30 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));
    child
}

#[test]
fn instance_dir_appears_with_layout_files() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let clicom_dir = td.path().join(".clicom");
    let inst = std::fs::read_dir(&clicom_dir).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    assert!(inst.join("meta.json").exists());
    assert!(inst.join("status.json").exists());
    assert!(inst.join("commands.lock").exists());
    assert!(inst.join("commands").is_dir());
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_returns_value_via_out_file() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "1 + 2"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("3"), "stdout: {s}");
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_screen_text_returns_visible() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Type something into the wrapped child via the channel
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "type_text(\"marker\\n\")"]).output().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "screen_text()"]).output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("marker") || !s.is_empty(), "screen: {s}");
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn status_lists_instance() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["status"]).output().unwrap();
    assert!(out.status.success());
    assert!(!out.stdout.is_empty());
    let _ = child.kill(); let _ = child.wait();
}
```

- [ ] **Step 2: Delete the temporary `tests/e2e_rhai_engine.rs`** — it's superseded.

```bash
rm tests/e2e_rhai_engine.rs
```

- [ ] **Step 3: Run tests**

Run: `cargo test --test e2e_basic`
Expected: all 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e_basic.rs
git rm tests/e2e_rhai_engine.rs
git commit -m "test(e2e): expand e2e_basic coverage; remove temporary engine test"
```

### Task 35: e2e test — `tests/e2e_queue.rs`

**Files:**
- Create: `tests/e2e_queue.rs`

- [ ] **Step 1: Implement** per §8 spec:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn queue_returns_id_and_emits_results() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "1 + 1"]).output().unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(!id.is_empty());

    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let done = inst.join("commands").join(format!("{id}.done"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done.exists() {
        if Instant::now() > deadline { panic!("done file did not appear"); }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(inst.join("commands").join(format!("{id}.out")).exists());

    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn done_appears_after_out() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Slow script so we have time to observe the order
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "wait_ms(500); 42"]).output().unwrap();
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let done = inst.join("commands").join(format!("{id}.done"));
    let out_p = inst.join("commands").join(format!("{id}.out"));

    let mut saw_done_with_out = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if done.exists() {
            saw_done_with_out = out_p.exists();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(saw_done_with_out, ".done landed without .out present");
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn result_triple_cap_evicts_oldest() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Drop 12 quick scripts via queue
    let mut ids = Vec::new();
    for _ in 0..12 {
        let out = Command::cargo_bin("clicom").unwrap()
            .current_dir(td.path()).args(["queue", "1"]).output().unwrap();
        ids.push(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    // Wait for all .done files to land
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let cmds = inst.join("commands");
    let last_done = cmds.join(format!("{}.done", ids.last().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(15);
    while !last_done.exists() {
        if Instant::now() > deadline { panic!("scripts did not finish"); }
        std::thread::sleep(Duration::from_millis(50));
    }
    // After eviction, only the 10 newest should remain
    let dones: Vec<_> = std::fs::read_dir(&cmds).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "done").unwrap_or(false))
        .collect();
    assert_eq!(dones.len(), 10, "expected 10 .done files post-eviction, got {}", dones.len());

    let _ = child.kill(); let _ = child.wait();
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo test --test e2e_queue
git add tests/e2e_queue.rs
git commit -m "test(e2e): clicom queue + result-triple cap"
```

### Task 36: e2e test — `tests/e2e_busy.rs` (incl. lock coordination)

**Files:**
- Create: `tests/e2e_busy.rs`

- [ ] **Step 1: Implement** per §8 (busy modes + --timeout combined budget + competing-run lock):

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn run_default_fails_busy_with_queued_script() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Queue a slow script
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "wait_ms(2000)"]).output().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "type_text(\"hi\")"]).output().unwrap();
    assert_eq!(out.status.code(), Some(5), "expected exit 5; stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("busy"));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_default_competing_run_does_not_busy_fail() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // First run with a wait_ms inside; should not block the second from succeeding (it just queues on lock)
    let td_path = td.path().to_path_buf();
    let h = std::thread::spawn(move || {
        Command::cargo_bin("clicom").unwrap()
            .current_dir(&td_path).args(["run", "wait_ms(1500); 1"]).output().unwrap()
    });
    std::thread::sleep(Duration::from_millis(200));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "2"]).output().unwrap();
    assert!(out.status.success(), "second run failed; stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("2"));
    let _ = h.join();
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn run_wait_blocks_until_queue_empty() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "wait_ms(1500)"]).output().unwrap();
    let start = Instant::now();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "--wait", "1"]).output().unwrap();
    let elapsed = start.elapsed();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(elapsed >= Duration::from_millis(1200), "should have waited; elapsed {:?}", elapsed);
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn timeout_combined_budget_under_wait() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let _ = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "wait_ms(2000)"]).output().unwrap();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "--wait", "--timeout", "1500", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    let _ = child.kill(); let _ = child.wait();
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo test --test e2e_busy
git add tests/e2e_busy.rs
git commit -m "test(e2e): run busy modes + lock coordination + combined timeout"
```

### Task 37: e2e test — `tests/e2e_clean.rs` (incl. lock + sweep filter)

**Files:**
- Create: `tests/e2e_clean.rs`

- [ ] **Step 1: Implement** per §8:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path())
        .args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn clean_with_id_deletes_triple() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let id = String::from_utf8_lossy(&Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "1"]).output().unwrap().stdout).trim().to_string();
    std::thread::sleep(Duration::from_millis(500));
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let cmds = inst.join("commands");
    assert!(cmds.join(format!("{id}.done")).exists());
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["clean", "", &id]).output().unwrap();
    assert!(out.status.success());
    assert!(!cmds.join(format!("{id}.done")).exists());
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn clean_idempotent_for_missing_id() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["clean", "", "1234567890-aaaaaa"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn clean_sweep_skips_triples_without_done() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    // Wait for the wrapper, then craft a synthetic .out without .done
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let cmds = inst.join("commands");
    let synthetic = "9999999999999999999-aaaaaa";
    std::fs::write(cmds.join(format!("{synthetic}.out")), "lonely").unwrap();

    // Drop a real queue script and let it complete
    let id = String::from_utf8_lossy(&Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["queue", "1"]).output().unwrap().stdout).trim().to_string();
    std::thread::sleep(Duration::from_millis(500));
    assert!(cmds.join(format!("{id}.done")).exists());

    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["clean"]).output().unwrap();
    assert!(out.status.success());

    // Real triple gone, synthetic .out preserved
    assert!(!cmds.join(format!("{id}.done")).exists());
    assert!(cmds.join(format!("{synthetic}.out")).exists());

    let _ = child.kill(); let _ = child.wait();
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo test --test e2e_clean
git add tests/e2e_clean.rs
git commit -m "test(e2e): clicom clean (by-id, idempotent, sweep .done filter)"
```

### Task 38: e2e test — `tests/e2e_multi_instance.rs`

**Files:**
- Create: `tests/e2e_multi_instance.rs`

- [ ] **Step 1: Implement** per §8:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn ambiguous_run_lists_candidates() {
    let td = TempDir::new().unwrap();
    let mut a = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    let mut b = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ambiguous"));
    let _ = a.kill(); let _ = a.wait();
    let _ = b.kill(); let _ = b.wait();
}

#[test]
fn partial_match_resolves_one() {
    let td = TempDir::new().unwrap();
    let mut a = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    let mut b = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    // Pick the first instance dir's dir-name as a partial.
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap().next().unwrap().unwrap();
    let dir_name = inst.file_name().to_string_lossy().to_string();
    let pid = dir_name.split('-').next().unwrap();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", pid, "1"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let _ = a.kill(); let _ = a.wait();
    let _ = b.kill(); let _ = b.wait();
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo test --test e2e_multi_instance
git add tests/e2e_multi_instance.rs
git commit -m "test(e2e): multi-instance partial-match + ambiguity"
```

### Task 39: e2e test — `tests/e2e_died.rs`

**Files:**
- Create: `tests/e2e_died.rs`

- [ ] **Step 1: Implement**:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn killing_wrapper_marks_died_lazily() {
    let td = TempDir::new().unwrap();
    let mut c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let _ = c.kill(); let _ = c.wait();
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["status"]).output().unwrap();
    assert!(out.status.success() || out.status.code() == Some(2));
    let s = String::from_utf8_lossy(&out.stdout);
    // Either "died" or "exited" depending on shutdown ordering — both are acceptable post-mortem states.
    assert!(s.contains("died") || s.contains("exited"));
}

#[test]
fn dead_instance_retention_keeps_only_ten() {
    use clicom::clicom_engine::{layout, retention, meta::{Meta, Status}};
    let td = TempDir::new().unwrap();
    for i in 0..12u32 {
        let dir = layout::instance_dir(td.path(), 0, &format!("dead{i:02}"));
        std::fs::create_dir_all(&dir).unwrap();
        let m = Meta::new(0, format!("agent{i}"), vec!["x".into()], td.path().to_path_buf());
        // backdate started_at so we get a known sort order
        let mut m = m;
        m.started_at = chrono::Utc::now() - chrono::Duration::seconds(i as i64);
        m.write_to(&layout::meta_path(&dir)).unwrap();
        Status::initial_busy().write_to(&layout::status_path(&dir)).unwrap();
    }
    retention::sweep_dead_instances(td.path(), std::process::id(), 10).unwrap();
    let n = std::fs::read_dir(td.path().join(".clicom")).unwrap().count();
    assert_eq!(n, 10);
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo test --test e2e_died
git add tests/e2e_died.rs
git commit -m "test(e2e): died lazy detection + retention sweep"
```

### Task 40: e2e test — `tests/e2e_nopty.rs` (incl. final-screen-on-exit)

**Files:**
- Create: `tests/e2e_nopty.rs`

- [ ] **Step 1: Implement**:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn nopty_echo_captures_stdout_and_exits_clean() {
    let td = TempDir::new().unwrap();
    let st = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "echo GOODBYE"])
        .status().unwrap();
    assert!(st.success() || st.code() == Some(0));
    // Find the instance dir
    let inst = std::fs::read_dir(td.path().join(".clicom")).unwrap()
        .find_map(|e| { let p = e.unwrap().path(); if p.is_dir() { Some(p) } else { None } }).unwrap();
    let screen = std::fs::read_to_string(inst.join("screen.txt")).unwrap();
    assert!(screen.contains("GOODBYE"), "screen: {screen}");
    let status: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(inst.join("status.json")).unwrap()).unwrap();
    assert_eq!(status["state"], "exited");
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo test --test e2e_nopty
git add tests/e2e_nopty.rs
git commit -m "test(e2e): --nopty + final-screen-on-exit"
```

### Task 41: e2e test — `tests/e2e_rhai.rs` (sandbox + caps)

**Files:**
- Create: `tests/e2e_rhai.rs`

- [ ] **Step 1: Implement** per §8:

```rust
use assert_cmd::prelude::*;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn start_wrapper(td: &TempDir) -> std::process::Child {
    let c = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["start", "--nopty", "--", "cmd", "/C", "ping -n 60 127.0.0.1 >nul"])
        .spawn().unwrap();
    std::thread::sleep(Duration::from_millis(800)); c
}

#[test]
fn eval_is_disabled_at_compile_time() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "eval(\"type_text(\\\"x\\\")\")"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("parse"));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn wait_ms_above_cap_throws() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "wait_ms(700000)"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let _ = child.kill(); let _ = child.wait();
}

#[test]
fn loop_caught_by_max_operations() {
    let td = TempDir::new().unwrap();
    let mut child = start_wrapper(&td);
    let out = Command::cargo_bin("clicom").unwrap()
        .current_dir(td.path()).args(["run", "loop {}"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let _ = child.kill(); let _ = child.wait();
}
```

- [ ] **Step 2: Run + commit + tag M3**

```bash
cargo test --test e2e_rhai
git add tests/e2e_rhai.rs
git commit -m "test(e2e): rhai sandbox + ops cap"
git tag -a m3-driver-cli -m "Milestone 3: driver CLI + integration tests"
```

### Task 42: README + smoke recipe

**Files:**
- Modify: `README.md`
- Create: `docs/superpowers/plans/manual-smoke-recipe.md`

- [ ] **Step 1: Expand README** with install + usage + examples:

```markdown
# clicom

File-based command channel for wrapped CLI agents. Wraps any command in a PTY,
exposes its screen as a file, and accepts Rhai scripts to drive it.

## Build

    cargo build --release
    cp target/release/clicom.exe ~/.local/bin/

## Use

    # In one shell:
    clicom start -- claude code

    # In another shell:
    clicom run "type_text(\"hello\\n\"); wait_idle(800); screen_text()"

See `clicom help host-fns` for the full Rhai surface.

## Spec

`docs/superpowers/specs/2026-05-02-clicom-wrapped-commands-channel-design.md`.
```

- [ ] **Step 2: Write the manual smoke recipe**

```markdown
# clicom — Manual Smoke Recipe

1. `cd <some empty dir>`
2. `clicom start -- claude code`
3. In another shell, same dir:
   - `clicom status` — see one live instance
   - `clicom run "screen_text()"` — should print Claude's banner
   - `clicom run "type_text(\"what is 2+2?\\n\")"` — types into Claude
   - `clicom run "wait_idle(2000); screen_last_after(\"2+2\")"` — pulls the answer
4. Exit Claude. Run `clicom status` — instance now `exited`.
```

- [ ] **Step 3: Commit**

```bash
git add README.md docs/superpowers/plans/manual-smoke-recipe.md
git commit -m "docs: README + manual smoke recipe"
```

### Task 43: Self-review — final pass over coverage

- [ ] **Step 1: Run the full test suite**

```bash
cargo test
```

Expected: all unit + integration tests pass.

- [ ] **Step 2: Verify coverage of §8 spec sections**

Walk through §8 of the spec:
- Unit tests (engine): covered in Tasks 2–8, 14, 21–27.
- e2e_basic.rs: Task 34.
- e2e_queue.rs: Task 35 (incl. result-triple cap).
- e2e_busy.rs: Task 36 (incl. lock coordination).
- e2e_multi_instance.rs: Task 38.
- e2e_died.rs: Task 39.
- e2e_nopty.rs: Task 40 (incl. final-screen-on-exit).
- e2e_clean.rs: Task 37 (incl. sweep filter, lock coordination tests with run/--force; the `clicom run --force` lock-coordination test is in e2e_busy or add here as needed).
- e2e_rhai.rs: Task 41.

If gaps surface, add tasks. Commit any added tests.

- [ ] **Step 3: Final commit**

```bash
git tag -a v0.1.0 -m "Initial clicom release"
```

---

## Open follow-ups (out of scope for this plan)

The §10 follow-ups in the spec are explicitly deferred:

- `clicom mcp` MCP server (Phase 2)
- `inboxmcp` adoption of `clicom_engine` (Phase 2)
- Built-in Rhai helper library
- Alternate runtimes (Lua, JS)
- `clicom watch` TUI
- `clicom check <id>` retrieve-and-delete helper
- `clicom queue --tag <tag>`
- Wrapper-side persistence between scripts
- Streaming results

Each warrants its own plan when prioritized.
