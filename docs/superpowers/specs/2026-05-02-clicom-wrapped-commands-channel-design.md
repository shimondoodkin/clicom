# `clicom` — File-Based Command Channel for Wrapped Agents — Design Spec

**Status:** Draft 1
**Date:** 2026-05-02
**Related (in the sibling `../cliagentchat/` repo, the `inboxmcp` project):**
- `../cliagentchat/docs/superpowers/specs/2026-04-29-inboxmcp-design.md` §4 (wrapper), §3.2 (instances)
- `../cliagentchat/docs/superpowers/specs/2026-05-02-participant-status-design.md` (status.txt / pid.txt — separate signal, not reused here)

**Working-directory convention for this spec:** all unqualified relative paths (`./`, `src/...`, `Cargo.toml`, `tests/...`) refer to **this `clicom/` repo** (the one containing this spec). Anything that lives in the `inboxmcp` project is written explicitly as `../cliagentchat/...`.

## 1. Purpose

Introduce **`clicom`** — a small, self-contained CLI tool that wraps an arbitrary command in a PTY, exposes its screen and stdin through a per-cwd `.clicom/` directory, and accepts Rhai scripts as a control language for driving it.

`clicom` is its own binary in its own Cargo crate — **this directory** (`./`), a sibling of `../cliagentchat/` (the `inboxmcp` project). At the start of implementation, a small set of files (PTY, screen, idle, forwarding, atomic-write helper, pid-alive helper) is copied from `../cliagentchat/` into this crate's tree; the two projects then evolve independently. `clicom` does not depend on `inboxmcp` and inherits none of its heavier deps (no MCP, no inbox, no contexts, no GUI). Users who want a lightweight "wrap + observe + script" tool for any CLI agent reach for `clicom`. Users who want full participant/context/MCP integration continue to reach for `inboxmcp`.

The motivating use cases:

1. **A second Claude Code instance** drives the wrapped agent through a primitive both sides understand without MCP wiring.
2. **Shell scripts and pipelines** drive an agent without a JSON-RPC client.
3. **Self-driving agents** — the wrapped agent itself runs `clicom run "..."` from its own shell to script its environment.

A small Rhai script per call (one drop file = one script) gives a real control language without paying for a per-verb wire-format vocabulary. There is no separate command grammar; everything that can be done is exposed as Rhai host functions (§4).

## 2. Scope

**In:**
- New Cargo project — **this directory** (`./`), a sibling of `../cliagentchat/` (the `inboxmcp` project) — independent build, lean dep tree. Single binary `clicom`. Its only subcommands are `start`, `status`, `run`, `queue`, `clean`, `help` (§5). No convenience-per-host-fn subcommands.
- `clicom start [--mouse] [--nopty] -- <command> [args...]` wraps `<command>` in a PTY (or stdin/stdout pipes if `--nopty`), creates `<cwd>/.clicom/<pid>-<rand6>/`, and stays alive for the wrapped child's lifetime.
- The on-disk protocol under `<cwd>/.clicom/<pid>-<rand6>/`: `meta.json`, `status.json`, `screen.txt`, `commands.lock`, `commands/<id>.rhai`, `commands/<id>.done`.
- The wire format inside `.clicom/.../commands/` is **Rhai script** (one script per `<id>.rhai` file). The wrapper embeds the Rhai engine and exposes a small set of host functions (§4). There is no separate verb language.
- Scrollback support in `ScreenBuffer`: baseline 10_000 lines, hard cap 20_000 lines, trim oldest 10_000 when cap is reached.
- Retention policy for dead instance directories: keep 10 most recent dead, prune older.
- `.gitignore` ergonomics: `clicom start` appends `.clicom/` to `<cwd>/.gitignore` on first start, idempotent.
- Source-level reuse: PTY, screen buffer, idle detection, atomic-write, and pid-alive helpers are **copied once** from `../cliagentchat/` (the `inboxmcp` project) into `./src/clicom_engine/`. The two projects evolve independently after that. Convergence onto a shared library is deferred to a phase-2 spec (§10).

**Out:**
- **Phase 2 — MCP server.** A future spec will add `clicom mcp` that exposes the same host-fn surface as MCP tools, for clients that prefer JSON-RPC over the file protocol. Not in this spec.
- No change to existing `inboxmcp wrap`, MCP tools, inbox, contexts, or the `participants/<nick>/status/` layout. `clicom` is parallel; nothing is migrated.
- No remote / network access. The protocol is strictly local-FS.
- No streaming `<id>.done` updates — each script produces exactly one terminal `<id>.done`.
- No alternate runtimes (Lua, JS, Python). The extension `<id>.rhai` reserves the right to add `<id>.lua` etc. later, but this spec covers Rhai only.
- No security / authentication. The protocol is local-FS only and inherits OS-level permissions. Anything that can drop a file in `.clicom/<pid>-<rand6>/commands/` can run arbitrary code in the wrapper's Rhai sandbox (see §7 for the threat model).

## 3. On-disk layout

```
<cwd>/.clicom/
  <pid>-<rand6>/
    meta.json
    status.json
    screen.txt
    commands.lock
    commands/
      <id>.rhai
      <id>.done
  <pid2>-<rand6_2>/        # might be a corpse — flagged via status.json
  ...
```

`<pid>` is the wrapper process PID (decimal). `<rand6>` is a 6-character lowercase hex token chosen at wrapper start. Example: `12345-a3f9c2`. The dir name is fixed for the wrapper's lifetime; on exit it is **not** moved or renamed — `status.json` is rewritten with the exit info instead.

Why random suffix instead of a timestamp: it gives `clicom status <partial>` a stable, easily-typed handle. Users can match by `12345` (the pid) or by `a3f9` (a memorable prefix of the rand) without having to recall a timestamp. Sort order across instances comes from `meta.json:started_at`, not the dir name.

### 3.1 `meta.json` — written once at wrapper start, then immutable

```json
{
  "schema": "clicom-meta/1",
  "pid": 12345,
  "name": "alice",
  "command": ["claude", "code"],
  "cwd": "C:\\Users\\user\\Documents\\projects\\clicom",
  "started_at": "2026-05-02T14:15:30Z"
}
```

`name` is a friendly label for the instance. `clicom start --name <name>` sets it explicitly; otherwise it defaults to the basename of the first argument after `--`. `cwd` is the absolute path the wrapper was started in, so a driver in the same cwd can sanity-check the match.

### 3.2 `status.json` — rewritten on transitions

```json
{
  "schema": "clicom-status/1",
  "state": "idle",
  "last_activity": "2026-05-02T14:37:12Z",
  "exit_code": null,
  "exited_at": null
}
```

`state` ∈ `"idle" | "busy" | "exited" | "died"`. Written atomically (`*.tmp` + rename) via the `fs_atomic` helper copied from `inboxmcp` (§9).

- `idle` / `busy` — wrapper is alive; same semantic as `inboxmcp`'s `idle.flag` (no screen activity for the configured idle window).
- `exited` — wrapper wrote this itself on clean shutdown. `exit_code` and `exited_at` are populated.
- `died` — driver-side detection. When the driver reads a `status.json` with `state ∈ {idle, busy}` but `meta.json:pid` is no longer alive, it rewrites `status.json` with `state = "died"`, `exited_at = <now>`, `exit_code = null`. No background sweeper.

### 3.3 `screen.txt` — atomic-replace snapshot

Plain-text projection of the wrapped agent's visible screen (vt100 `Screen::contents()` — same projection that `inboxmcp` writes to `participants/<nick>/status/screen.txt`). Written by the wrapper on each idle transition, at most once per 250 ms during sustained activity, and **once more on child exit** before flipping `status.json` to `exited`. So after the wrapper has returned, `screen.txt` is guaranteed to reflect the last frame the agent produced. Atomic via `.tmp` + rename. Scrollback is delivered on demand via the `screen_tail_*` host fns (§4.3) and is *not* persisted to disk on exit.

### 3.4 `commands.lock` — writers' serialization lock

Empty file, created by the wrapper at startup, never deleted during the wrapper's lifetime. Writers (drivers) take an `fs2` exclusive lock on it. The wrapper does **not** lock this file. The lock's only job is to serialize *callers* so each `clicom` invocation behaves synchronously and concurrent callers naturally queue.

Lock-hold scope by caller mode:

- `clicom run` (default) and `clicom run --wait` — held for the whole call: drop → wait for own `.done` → read result files → delete result files → release. Subsequent writers block on lock acquisition until this caller's script has finished and its outputs have been consumed.
- `clicom run --force` — held briefly during the drop, released *before* waiting for `.done`, then **re-acquired** briefly for the read+delete phase once `.done` lands. Other writers can interleave during the wait, but the result files are read+deleted under the lock so they cannot race with `clicom clean`.
- `clicom queue` — held briefly during the drop, released immediately after. The wrapper executes the script asynchronously; the caller does not wait.
- `clicom clean` — held for the whole sweep (enumerate → unlink). Coordinates clean against `run` so a clean cannot wipe an in-flight run's `.out` between the wrapper's write and the run's read.

The lock is per-instance-dir: each `<pid>-<rand6>/` has its own `commands.lock`, so different live wrappers in the same cwd serialize independently.

If a writer exits abnormally while holding the lock, the OS releases the file lock when the handle drops (this is the standard `fs2` / `flock` / Windows `LockFileEx` behavior); subsequent writers proceed without manual recovery.

Rationale for "writers only": if the wrapper also held the lock while processing, a slow script (e.g. one calling `wait_idle(1500, 30000)`) would prevent the next caller from even *dropping* its `.rhai`. With writers-only, the next caller blocks on lock acquisition until the previous caller releases — i.e., until its `.done` lands — which is exactly the right semantic.

### 3.5 `commands/<id>.rhai` — one Rhai script per file

`<id>` = `<unix_nanos>-<rand6>`, e.g. `1714689432123456789-a3f9c2`. Sortable by drop time, unique under concurrent writers.

Atomic creation: writer writes `commands/<id>.rhai.tmp` then renames to `<id>.rhai`. The wrapper only ever opens fully-named `*.rhai` files. The wrapper uses the `notify` crate (declared in §9) to wake on file arrival and falls back to a 250 ms poll on platforms where `notify` does not deliver create events for renames.

File contents — UTF-8 Rhai source, multi-line OK. Examples:

```rhai
// Single-step: type something
type_text("hello\n");
```

```rhai
// Multi-step atomic flow: type, wait for the agent to settle, capture output
type_text("ls\n");
wait_idle(800);
let out = screen_last_after("ls\n");
screen_save_last_after("ls.out", "ls\n");
out                          // last expression → result body of <id>.done
```

```rhai
// Per-script timeout, optional. Default is the wrapper-config timeout (§6).
set_timeout(120000);          // 2 minutes
type_text("npm test\n");
wait_idle(2000, 90000);
screen_save("test.out");
```

The script's *last expression* is the result. Plain Rhai semantics — a trailing `;` makes the result `()` (unit), no `;` makes the value the result. The result is JSON-encoded into `<id>.out` on success; `<id>.done` itself only carries the `OK` / `ERR <code>` marker (§3.6).

### 3.6 Result files — wrapper writes `.out`, optionally `.err`, then `.done`

Each script produces up to three sibling files in `commands/`. Each is written atomically (`.tmp` + rename). Order of writes is significant: outputs first, completion marker last. Watchers should key on `<id>.done` as the "ready" signal — when it appears, the other files are guaranteed already in place.

**`<id>.out`** — always present after completion.

The script's return value, JSON-encoded, on a single line. `()` (unit) → `null`. Strings keep newlines via `\n` JSON escapes, so the file is always exactly one line.

```json
"the captured screen tail content with \nembedded newlines"
```

```json
{"actual_from": 12100, "actual_to": 12200, "bytes": 4823}
```

**`<id>.err`** — present only on failure.

Two lines. First line: short code. Subsequent line(s): human-readable detail.

```
runtime
expected number, got string at line 3
```

`<short_code>` ∈ `parse | runtime | timeout | host_fn | fs | range | internal`.

- `parse` — Rhai source failed to parse.
- `runtime` — Rhai threw a runtime error (uncaught script exception, division by zero, type mismatch, etc.). Detail is the Rhai error message.
- `timeout` — wall-clock timeout (§6) or operations-budget exhausted.
- `host_fn` — a host function raised. Detail starts with the host-fn name, e.g. `wait_idle: timeout after 60000ms`.
- `fs` — filesystem error from a save-form host fn.
- `range` — `screen_tail_*` resolved a wholly-trimmed range (see §4).
- `internal` — caught panic; the wrapper logs the full backtrace, the `.err` carries only a short message.

**`<id>.done`** — completion marker, written last.

Single line: `OK` on success, `ERR <short_code>` on failure. Existence of this file is the signal that processing is complete and `.out`/`.err` are ready to read.

```
OK
```

```
ERR runtime
```

**Lifecycle:**

The wrapper writes outputs in this order:
1. Write `<id>.out`.
2. If failed, write `<id>.err`.
3. Write `<id>.done`. (Reading processes use this as the readiness barrier.)
4. Delete `<id>.rhai`.

`clicom run` reads then deletes its own result files (`<id>.out`, `<id>.done`, and `<id>.err` if present). `clicom queue` doesn't read them — the eventual caller does, via `clicom clean <id>` (§5.1) or by reading the files directly. Orphan result triples (queued scripts whose caller never came back) are bounded by the count-based eviction in §3.7.2: the wrapper keeps at most the **10 most recent** result triples per `commands/` dir.

### 3.7 Retention

Two independent retention rules.

#### 3.7.1 Dead-instance retention

Live instances are never pruned. On wrapper start (and once per hour while running), every wrapper sweeps `<cwd>/.clicom/`:

1. For each subdir whose `meta.json:pid` is alive and equal to its own → skip (live, owned by another wrapper or this one).
2. For each subdir whose `meta.json:pid` is alive but different from this wrapper's pid → skip (peer instance).
3. For each subdir whose `meta.json:pid` is dead and `status.json:state ∈ {idle, busy}` → rewrite `status.json` with `state = "died"`. Counts as dead from here.
4. Among all dead subdirs, sort by `meta.json:started_at` descending, keep the first 10, recursively remove the rest.

Drivers do step 3 lazily when they read `status.json`; the wrapper's sweep is a safety net. Step 4 is the wrapper's job exclusively — drivers never delete other instances' directories.

The wrapper does **not** sweep result files at shutdown; live-time eviction (§3.7.2) is the only mechanism for bounding result-file count, and dead-instance retention eventually removes the whole dir anyway.

#### 3.7.2 Result-triple cap (per live `commands/` dir)

After writing each new `<id>.done`, the wrapper enumerates result triples in its own `commands/` dir, sorted by `<id>` (lexicographic = drop-time order). If more than **10** triples exist, the oldest excess triples are deleted (each triple = `<id>.out`, `<id>.err` if present, `<id>.done`). `<id>.rhai` files (queued, not yet processed) are never touched by this rule.

This cap is purely count-based; there is no time-based component. It runs only on the wrapper-owned dir, never on peers' dirs. It is the only mechanism that prevents orphaned `clicom queue` results from accumulating during a long-running wrapper.

Manual cleanup is also available via the `clicom clean` subcommand (§5.1), which lets a user remove a specific triple by `<id>` or wipe all triples in an instance's `commands/` dir.

### 3.8 `.gitignore`

On wrapper start, if `<cwd>/.gitignore` exists and does not already contain a line equal to `.clicom/` (after trim), append it on its own line. If `.gitignore` does not exist, do not create it (avoid surprising the user). Idempotent.

## 4. Host functions

The wrapper registers the following Rhai host functions. All save-form host functions write atomically (`*.tmp` + rename); paths are resolved relative to the wrapper's cwd, absolute paths used as-is. All host functions block the script until they complete; the script itself blocks `<id>.done` from being written until it returns.

### 4.1 PTY input

```rhai
type_text(s: String)            -> ()
```

Inject `s` into the wrapped PTY's stdin via the nudge channel exposed by `clicom_engine/nudge.rs` (§6.1). Returns when bytes have been *written* to the PTY (not when the agent has finished reacting — use `wait_idle()` for that). Throws `host_fn` if the nudge channel is closed (wrapper shutting down).

### 4.2 Visible screen (current frame)

```rhai
screen_text()                   -> String
screen_save(path: String)       -> i64    // bytes written
```

`screen_text()` returns the current visible-screen plain-text projection (vt100 `Screen::contents()`). `screen_save(path)` writes the same content to `path` and returns the byte count.

These force a *fresh* projection at the moment the host fn runs, distinct from the always-on `screen.txt` (§3.3) which is updated on the wrapper's snapshot cadence. Use `screen_text()` after `wait_idle()` for the post-quiesce frame.

### 4.3 Scrollback range

```rhai
screen_tail_text(from: i64, to: i64)             -> String
screen_tail_save(path: String, from: i64, to: i64)
                                                  -> Map { actual_from, actual_to,
                                                           total_lifetime, trimmed_below,
                                                           bytes }
```

`from` and `to` are integer line indexes into the lifetime buffer (see §6.3 for the line-index model):

- **Non-negative:** absolute lifetime index (line 0 is the first line ever produced by the wrapped agent in this session).
- **Negative:** counts from the end. `-1` = most recent line.

The slice is `[from, to)` (half-open) after resolving negatives. If `from > to` after resolution → throw `host_fn` with detail `bad range`. If both `from` and `to` resolve below `trimmed_below` (the oldest line still in scrollback) → throw `range` with detail `requested below trim watermark`.

Otherwise the request is **clamped silently** to the available range. The caller learns the actual range from the return value (`screen_tail_text` returns just the content; if you need the metadata, use `screen_tail_save` or call `status()` first).

`screen_tail_save` writes the file with a header line then content:

```
# requested: 12000..12200  actual: 12100..12200  total_lifetime: 12200  trimmed_below: 12100
<line 12100>
<line 12101>
...
<line 12199>
```

`screen_tail_text` returns the body only (no header) — scripts that need the metadata should call `screen_tail_save` to a temp path or pull from `status()`.

### 4.4 "After marker" — tail since reference point

```rhai
screen_last_after(marker: String)                       -> String
screen_save_last_after(path: String, marker: String)    -> i64

screen_last_after_re(regex: String)                     -> String
screen_save_last_after_re(path: String, regex: String)  -> i64
```

Search the lifetime scrollback (including the visible region) for the **last** occurrence of `marker` (literal text) or `regex` (Rust-regex syntax). Return everything after the match (exclusive of the match itself), as a plain-text string. The save-forms write the same content and return byte count.

If the marker is not found, returns `""` / `0` bytes (no throw). Scripts decide what to do.

Regex flavor: `regex::Regex` from the `regex` crate (declared in §9). Multi-line off by default (use `(?m)` to enable). Unicode on. Compile errors throw `host_fn` with detail starting `regex compile:`.

Use case — capture the output of a command:

```rhai
type_text("ls\n");
wait_idle(800);
let out = screen_last_after("ls\n");           // everything since the command
```

### 4.5 Waits

```rhai
wait_idle(ms: i64)                          // default timeout 60_000
wait_idle(ms: i64, timeout_ms: i64)
wait_ms(ms: i64)
```

`wait_idle(ms[, timeout_ms])` blocks until the wrapper has reported `idle` continuously for `ms` milliseconds. Throws `host_fn` with detail `wait_idle: timeout after <timeout>ms` on timeout. The idle signal is provided by `clicom_engine/idle.rs` (§6.1).

`wait_ms(ms)` sleeps for exactly `ms` milliseconds. Internal cap of 600_000 (10 min); larger values throw `host_fn` with detail `wait_ms: cap exceeded`.

### 4.6 Status & control

```rhai
status()                       -> Map { state, last_activity,
                                        lifetime_lines, trimmed_below,
                                        visible_rows, visible_cols }
set_timeout(ms: i64)           -> ()
```

`status()` returns wrapper state at the moment of the call.

`set_timeout(ms)` overrides the per-script wall-clock timeout for the current script (default is the wrapper-config timeout, see §6). Should be called before any long-running host fn. Calling `set_timeout` more than once in one script replaces the previous value. The cap is 3_600_000 (1 hour); larger values throw `host_fn` with detail `set_timeout: cap exceeded`.

## 5. The `clicom` binary

`clicom` does two things: it **wraps** a command (`clicom start`) and it **drives** a wrapped command (`clicom run`). The same binary serves both roles. Read-only inspection (`clicom status`), manual cleanup (`clicom clean`), and self-documentation (`clicom help`) round out the surface. There are no per-host-fn convenience subcommands — `clicom run "<rhai>"` is the way.

### 5.1 Subcommands

```
clicom start [--mouse] [--nopty] [--name <name>] -- <command> [args...]

clicom status [<partial>]

clicom run   [<partial>] <inline>     [--wait | --force] [--timeout <ms>]
clicom run   [<partial>] -f <file>    [--wait | --force] [--timeout <ms>]
clicom run   [<partial>] -            [--wait | --force] [--timeout <ms>]   # stdin

clicom queue [<partial>] <inline>
clicom queue [<partial>] -f <file>
clicom queue [<partial>] -

clicom clean [<partial>]                                         # all triples
clicom clean [<partial>] <id>                                    # one triple

clicom help [<topic>]
```

The optional `<partial>` is a substring matched against `<pid>-<rand6>` to pick which instance to target. Disambiguation rules are in §5.3. If the substring would be ambiguous with the script source, callers can disambiguate by using `-f <file>` or `-` (so the only positional is the partial), or by quoting carefully so the parser sees two distinct positionals.

#### `clicom start`

Wrap `<command>` (everything after `--`) and stay alive for the child's lifetime.

- Default: spawn `<command>` in a PTY using `portable-pty`. Mouse-tracking enable/disable sequences are stripped from the host stdout (matching `inboxmcp wrap`'s `strip_mouse=true` default), so click-drag selection works in the host terminal.
- `--mouse`: don't strip mouse tracking. Hand them through to the host terminal.
- `--nopty`: do not allocate a PTY. Wire the child's stdin/stdout/stderr to plain pipes; host stdin is forwarded to child stdin, child stdout is forwarded to host stdout. Useful for non-interactive children. The screen-buffer projection still tracks bytes for `screen_text()`/scrollback, but vt100 control sequences from non-PTY children are usually absent so the projection is just the byte stream as text.
- `--name <name>`: friendly label written into `meta.json:name`. Defaults to `argv[0]`'s basename.

On startup `clicom start`:

1. Generates `<rand6>` and computes the instance dir `<cwd>/.clicom/<pid>-<rand6>/`.
2. Creates the dir, writes `meta.json`, initial `status.json` (`busy`), creates `commands.lock` and `commands/`.
3. Runs the retention sweep (§3.7).
4. Appends `.clicom/` to `<cwd>/.gitignore` if missing (§3.8).
5. Spawns the child, starts the screen-buffer feeder, idle detector, snapshot writer, and command-watcher threads.
6. Forwards host stdin → child, child stdout → host (with the screen tap), reusing the same forwarding logic as `inboxmcp wrap` (`../cliagentchat/src/wrap/forwarding.rs`, copied into `./src/clicom_engine/forwarding.rs`).
7. On child exit: writes `status.json` with `state="exited"`, `exit_code`, `exited_at`. Returns the child's exit code.

#### `clicom status`

Read-only — does not go through the queue.

- `clicom status` (no arg): print one row per `<cwd>/.clicom/*/` subdir. Columns: `<pid>-<rand6>`, `state`, `name`, `started_at`, `last_activity`, `exit_code` (if exited/died). Live instances first, then dead, sorted by `started_at` desc.
- `clicom status <partial>`: filter to subdirs whose dir name contains `<partial>` as a substring. If exactly one matches, print its full `status.json` + `meta.json`. If multiple match, print the row form. If zero match, exit 2.

`<partial>` matches anywhere in the dir name — typical inputs are the pid (`12345`), a prefix of the rand suffix (`a3f9`), or a chunk of the combined form (`12345-a3`).

#### `clicom run`

Drop a Rhai script into the matched instance's queue, wait for `<id>.done`, print the result.

- Script source: positional inline (last positional arg) | `-f <file>` | `-` (stdin until EOF).
- `<partial>` (optional positional): substring match against `<pid>-<rand6>` (§5.3).
- `--timeout <ms>`: **single combined wall-clock budget** for the whole `clicom run` invocation, covering (under `--wait`) the queue-empty wait plus (in all modes) the wait for our own `<id>.done`. Default 600_000 (matches the wrapper's default per-script timeout). If the script calls `set_timeout(N)` for a longer per-script budget, pass a matching or larger driver `--timeout`.

Busy semantics — three modes, mutually exclusive:

- **Default** (no flag): after acquiring `commands.lock`, scan `commands/`. If any `*.rhai` files are present (orphans from a crashed driver, or queued scripts dropped by `clicom queue`), release lock and exit `5` with stderr `busy: <N> pending script(s)`. Note: a competing `clicom run` (default or `--wait`) does *not* trigger the busy fail — it holds the lock through its own `.done`, so this caller blocks on lock acquisition until the previous run finishes, then sees an empty queue. The fail-fast only fires against orphans or `clicom queue` traffic.
- **`--wait`**: while holding `commands.lock`, wait for `commands/` to be empty (notify watcher + 250 ms poll, capped by the remaining `--timeout` budget). Then drop our `.rhai` and wait for our `.done`. The lock is held for the entire wait, so other writers (run *or* queue) block behind us.
- **`--force`**: acquire lock briefly, drop our `.rhai` regardless of queue state, release lock, then wait for our `.done` outside the lock. Other writers can interleave.

In all three modes, `clicom run` is **synchronous**: it doesn't return until its own `.done` lands or the combined `--timeout` budget expires (exit 4).

#### `clicom queue`

Drop a Rhai script and exit immediately. Asynchronous fire-and-forget.

- Script source: positional inline | `-f <file>` | `-` (stdin until EOF). `<partial>` optional positional, same as `run`.
- Acquires `commands.lock`, writes `<id>.rhai`, releases lock, prints `<id>` to stdout, exits 0.
- The wrapper executes the script when it reaches the head of the queue, then writes `<id>.out` (and `<id>.err` if it failed) and finally `<id>.done`.
- The caller is responsible for polling/watching `<id>.done`, reading `<id>.out` / `<id>.err`, and deleting them (or running `clicom clean <id>`). Result triples whose caller never returns are bounded — at most the **10 most recent** triples are kept per `commands/` dir (§3.7.2); older ones are evicted by the wrapper after each new `.done`.
- `clicom queue` does not check whether the queue is busy — that's the whole point. Use `clicom run` if you need synchrony or busy-failure semantics.

#### `clicom clean`

Manual cleanup of result triples (`<id>.out`, `<id>.err`, `<id>.done`) in a target instance's `commands/` dir. Never touches `<id>.rhai` files (those are queued scripts the wrapper still needs to process); never touches another instance's dir.

- `clicom clean [<partial>]` — for the matched instance, delete result triples in `commands/`. **Sweep mode only touches triples whose `<id>.done` exists** — this prevents clean from racing with the wrapper mid-write (e.g. wrapper has written `.out` but not yet `.done`, and a queue-consumer is about to wait on `.done`).
- `clicom clean [<partial>] <id>` — delete the single triple matching `<id>` (no `.done`-existence requirement; this is the explicit "I know the script is done, drop my files" call). The typical `clicom queue` consumer flow is: poll for `<id>.done`, read `<id>.out` (and `<id>.err`), then `clicom clean <id>`.
- `<partial>` is the same instance-selector as `run`/`queue`/`status` (§5.3). Zero or multiple matches → exit 2.
- Exits 0 on success (including when the `<id>` triple does not exist — clean is idempotent), 2 on driver-side problems.

`clicom clean` does not require the wrapper to be live — it works on `state ∈ {idle, busy, exited, died}` so users can still tidy up after a wrapper has shut down (until dead-instance retention removes the whole dir).

**Lock coordination:** `clicom clean` acquires `commands.lock` for the duration of the sweep (§3.4). This blocks against in-flight `clicom run` callers (default / `--wait` hold the lock through their read+delete; `--force` re-acquires it for its read+delete) so clean cannot wipe a run's `.out` between the wrapper writing it and the run reading it. Against `clicom queue`, the lock is contended only briefly during the queue-caller's drop. Against an exited or died wrapper, no one else holds the lock, so clean proceeds immediately.

#### `clicom help`

Prints help. Two levels:

- `clicom help` — top-level: the six subcommands above with their flags.
- `clicom help <topic>`:
  - `clicom help host-fns` — full reference of all Rhai host functions (§4): name, signature, what it does, examples. This is the canonical user-facing list of "what you can do inside `clicom run`."
  - `clicom help script` — pointers to Rhai language docs + a one-page tutorial covering let-bindings, if/else, while/for, try/catch, and the most common host-fn idioms (type-then-wait-then-capture, polling-with-screen-text).
  - `clicom help start` / `clicom help run` / `clicom help status` / `clicom help clean` — long-form help for that subcommand.
  - `clicom help layout` — the `.clicom/` on-disk layout (§3) for users who want to inspect or write tooling against it.

Keeping help comprehensive matters because there are no convenience subcommands; `help host-fns` is how users learn what the script language can do.

### 5.2 Output handling

`clicom run` prints the script's return value:

- String → printed to stdout verbatim, no quoting.
- Anything else (map, array, int, bool) → pretty-printed JSON to stdout.
- `()` / null → no stdout output.

stderr carries human-readable status (errors, instance discovery messages).

### 5.3 Instance discovery

`clicom run`, `clicom queue`, `clicom clean`, and `clicom status <partial>` scan `<cwd>/.clicom/` (its own cwd, not arbitrary). For each subdir:

1. Read `meta.json`; tolerate missing/corrupt by skipping.
2. Read `status.json`; tolerate missing by skipping. If `state ∈ {idle, busy}` but `meta.json:pid` is dead, rewrite `status.json` as `died` (lazy detection — see §3.7.1 step 3). For `clicom run`/`queue`, this subdir is no longer a candidate. For `clicom status` and `clicom clean`, it's still listed/eligible (with the now-corrected `died` state).
3. Otherwise, the subdir's state is whatever `status.json` says.

For `clicom run` and `clicom queue`:

- Apply `<partial>` substring filter on dir name if the optional positional was given.
- Filter to `state ∈ {idle, busy}`.
- Zero candidates → exit 2, message `no live wrapped agent in <cwd>` (or `no match for <partial>`).
- One candidate → use it.
- Multiple candidates → exit 2, list candidates with their `<pid>-<rand6>`.

For `clicom clean`:

- Apply `<partial>` substring filter if given.
- Do **not** filter on state — clean works against `state ∈ {idle, busy, exited, died}` so callers can tidy up after a wrapper has stopped.
- Zero / multiple → exit 2 (same messages as above).

### 5.4 Driver flow

#### Common drop sequence

1. Resolve script source: positional inline / `-f <file>` / `-` (stdin).
2. Resolve target instance per §5.3.
3. Acquire `fs2::FileExt::lock_exclusive` on the instance's `commands.lock` (blocking).
4. Generate `<id>` = `<unix_nanos>-<rand6>`.
5. Write `commands/<id>.rhai.tmp`, then rename to `<id>.rhai`.

#### `clicom run` flow (after the common drop)

Behavior depends on the busy mode:

- **Default**: between steps 3 and 4 above, scan `commands/` for any existing `*.rhai` files. If found → release lock, exit 5 with `busy: <N> pending script(s)`.
- **`--wait`**: between steps 3 and 4, wait until `commands/` is empty (notify watcher + 250 ms poll), capped by the remaining `--timeout` budget. Continue with the drop while still holding the lock.
- **`--force`**: skip the busy check entirely.

`--timeout` is a **single combined budget** for the whole `clicom run` invocation — under `--wait` it covers both the queue-empty wait and the subsequent wait for our own `.done`. If the budget runs out at any point, exit 4.

After the drop:

6. Wait for `<id>.done` to appear. For `--force`, the lock is *released* before waiting (so the next caller can proceed). For default and `--wait`, the lock is held until `.done` lands.
7. **For `--force` only**: re-acquire `commands.lock` (blocking) so the read+delete phase below cannot race with `clicom clean`. Default and `--wait` already hold the lock, so this is a no-op for them.
8. Read `<id>.done` (single-line marker: `OK` or `ERR <code>`).
9. If `OK`: read `<id>.out` (always present), print per §5.2.
10. If `ERR <code>`: read `<id>.err` (always present on error), print to stderr.
11. Delete `<id>.done`, `<id>.out`, and `<id>.err` (whichever exist).
12. Release lock.
13. Exit zero on `OK`, non-zero per §5.5.

#### `clicom queue` flow (after the common drop)

6. Release lock.
7. Print `<id>` to stdout. Exit 0.

The wrapper will eventually write `<id>.out` (always), `<id>.err` (if failed), and `<id>.done` (last). The caller's responsibility from here on: poll/watch `<id>.done`, read outputs, then delete the files (or run `clicom clean <id>`).

For v1, `clicom queue` callers retrieve results via plain shell (`cat .clicom/*/commands/<id>.out`). A `clicom check <id>` retrieve-and-delete helper is listed in §10 as a follow-up.

#### `clicom clean` flow

This command does not write `<id>.rhai`, but it **does** take `commands.lock` to coordinate with in-flight `clicom run` callers (§3.4).

1. Resolve target instance per §5.3 (state filter widened to include `exited` / `died`).
2. Acquire `fs2::FileExt::lock_exclusive` on the instance's `commands.lock` (blocking; see §7 for the long-wait failure mode).
3. Determine target file set:
   - **No `<id>` argument (sweep mode)**: enumerate `commands/`. For each `<id>` such that `<id>.done` exists, include `<id>.out`, `<id>.err`, `<id>.done` in the target set. *Triples without `<id>.done` are skipped* — they are either mid-write by the wrapper or freshly-arrived results whose consumer has not yet seen `.done`.
   - **`<id>` argument**: target set is exactly `<id>.out`, `<id>.err`, `<id>.done` (whichever exist). No `.done`-existence requirement; the explicit `<id>` form is the caller asserting "this script is done, drop my files."
4. Delete each target file. Missing files are not an error (clean is idempotent).
5. Release lock.
6. Exit 0.

### 5.5 Exit codes

- `0` — `OK` (success). For `clicom clean`, also returned when the target file did not exist (idempotent).
- `1` — internal / unexpected.
- `2` — driver-side problem: bad CLI args, no live agent, ambiguous instance, no match for `<partial>`, malformed `<id>.done`.
- `3` — wrapper-reported `ERR` (any `<short_code>` from §3.6). Code is repeated on stderr.
- `4` — driver-side timeout: `--timeout` budget exhausted (`clicom run` only).
- `5` — `clicom run` (default mode) refused: queue not empty (orphan `*.rhai` or `clicom queue` traffic).

## 6. Wrapper-side internals

### 6.1 Crate structure

`clicom` is a **separate Cargo project** in **this directory** (`./`), a sibling of `../cliagentchat/` (the `inboxmcp` project). It does not share `Cargo.toml` with `inboxmcp`. This keeps `clicom`'s dependency tree lean and isolates its build from `inboxmcp`'s GUI/MCP/contexts deps.

Source-level overlap with `inboxmcp`'s `../cliagentchat/src/wrap/*` is handled by **one-time copying** at the start of implementation: `pty.rs`, `screen.rs`, `idle.rs`, `forwarding.rs`, `fs_atomic.rs`, and the pid-alive helper are copied into this project. The two projects then evolve independently. Convergence onto a shared library (Cargo workspace or extracted `clicom_engine` crate) is deferred to a phase-2 spec when the duplication has actually bitten.

Source layout for this project (paths are relative to `./`, i.e. this `clicom/` repo):

```
./
  Cargo.toml
  Cargo.lock
  README.md
  src/
    lib.rs                     # crate root, re-exports clicom_engine + clicom_cli
    clicom_engine/             # wrapper-side library
      mod.rs                   # ClicomChannel, lifecycle orchestration
      layout.rs                # path helpers, dir-name parsing, partial-match
      meta.rs                  # meta.json + status.json types & atomic write
      ids.rs                   # rand6, unix_nanos, <id> format
      gitignore.rs             # idempotent append helper
      retention.rs             # dead-instance retention sweep
      process.rs               # pid-alive check (sysinfo)
      fs_atomic.rs             # atomic file write helper
                               #  (copied from ../cliagentchat/src/fs_atomic.rs)
      screen.rs                # ScreenBuffer with scrollback ring (copied + extended
                               #  from ../cliagentchat/src/wrap/screen.rs)
      pty.rs                   # PTY spawn
                               #  (copied from ../cliagentchat/src/wrap/pty.rs)
      nopty.rs                 # pipe-based spawn (no PTY, new for clicom)
      forwarding.rs            # bidirectional byte forwarding
                               #  (copied from ../cliagentchat/src/wrap/forwarding.rs)
      idle.rs                  # idle detector
                               #  (copied from ../cliagentchat/src/wrap/idle.rs)
      nudge.rs                 # PTY-input nudge channel
                               #  (copied from ../cliagentchat/src/wrap/nudge.rs)
      rhai_host.rs             # Rhai Engine setup + host fn registration
      watcher.rs               # notify-based commands/ watcher + script executor
                               # + result-triple cap eviction (§3.7.2)
    clicom_cli/                # driver-side CLI module
      mod.rs                   # re-exports
      discovery.rs             # list instances, partial-match resolution
      drop.rs                  # acquire lock, drop .rhai, wait .done, read result files
      cmd_start.rs             # `clicom start` impl
      cmd_status.rs            # `clicom status` impl
      cmd_run.rs               # `clicom run` impl (default + --wait + --force)
      cmd_queue.rs             # `clicom queue` impl
      cmd_clean.rs             # `clicom clean` impl
      cmd_help.rs              # `clicom help` impl + help text strings
    bin/
      clicom.rs                # binary entry: clap parse + dispatch
  tests/
    e2e_basic.rs               # start + run round-trip, screen_text/screen_save
    e2e_queue.rs               # queue + result files + result-triple cap
    e2e_busy.rs                # run busy modes (default / --wait / --force)
    e2e_multi_instance.rs      # multi-instance + partial match
    e2e_died.rs                # died detection + dead-instance retention
    e2e_nopty.rs               # --nopty + final-screen-on-exit
    e2e_clean.rs               # `clicom clean` (all triples, single id, idempotent)
    e2e_rhai.rs                # ops cap, set_timeout, sandbox (eval disabled)
    fixtures/
      fake_agent.rs            # tiny test binary that cat's stdin to stdout
```

This project is independent: `cargo build --release` (run from this `./` directory) produces `target/release/clicom.exe` without touching `../cliagentchat/`. The `inboxmcp` project's existing `../cliagentchat/src/wrap/*` is untouched.

### 6.2 `clicom_engine::ClicomChannel`

Owns one `<cwd>/.clicom/<pid>-<rand6>/` lifecycle.

- `ClicomChannel::start(cwd, pid, name, command, mode) -> Self` — generates `<rand6>`, creates the dir, writes `meta.json`, writes initial `status.json` (`busy`), creates `commands.lock` and `commands/`, runs the gitignore append, runs the retention sweep, builds the Rhai `Engine` with host fns registered, and starts the watcher thread. `mode` ∈ `{ Pty { strip_mouse: bool }, NoPty }`.
- A background thread: `notify` watcher on `commands/`, drains `*.rhai` files in oldest-first order, executes each script (per §6.3 flow), writes `<id>.done`.
- `set_state(state)` — writes `status.json` atomically.
- `update_screen()` — writes `screen.txt` atomically. Called from the snapshot cadence inside the engine, not from outside.
- `on_shutdown(exit_code)` — writes `status.json` with `state = "exited"`, `exit_code`, `exited_at`. Does **not** remove the directory.

The Rhai host fns registered on the engine close over an `Arc<ScreenBuffer>`, the nudge-channel `Sender`, and the idle-state observer. They run on the watcher thread's script-executor; long-running ones (`wait_idle`, `wait_ms`) park that thread, which is fine because scripts execute strictly serially.

### 6.3 Per-script processing flow

1. Read `<id>.rhai` source.
2. Compile to Rhai AST. On compile error → write `<id>.out` (`null`), write `<id>.err` (`parse\n<msg>`), write `<id>.done` (`ERR parse`), delete `<id>.rhai`, continue.
3. Acquire a fresh `Scope` and run the AST against the shared `Engine` (host fns pre-registered at startup). Wall-clock timeout = the value passed to `set_timeout(N)` if the script called it before any host fn, else the engine-config default (600_000 ms by default; env var `CLICOM_SCRIPT_TIMEOUT_MS`). Engine limits enforced (see below).
4. On script return, write outputs in this order (each atomic via `.tmp` + rename):
   - **Success:** `<id>.out` = JSON-encoded return value. Then `<id>.done` = `OK`.
   - **Failure:** `<id>.out` = `null`. `<id>.err` = `<code>\n<message>`. Then `<id>.done` = `ERR <code>`.

   Codes match §3.6: `parse | runtime | timeout | host_fn | fs | range | internal`.
5. Delete `<id>.rhai`. Independent of success/failure.

The strict write order (`.out` first, then `.err` if needed, then `.done` last) ensures any reader watching for `<id>.done` finds the other files already in place.

Engine limits, registered once at startup:

- `set_max_operations(10_000_000)` — coarse runaway protection. Scripts can legitimately loop while polling `screen_text()`, so this is generous. Tunable via `CLICOM_MAX_OPS`.
- `set_max_call_levels(64)`.
- `set_max_string_size(4 * 1024 * 1024)` — accommodates `screen_tail_text(0, -1)` over a full 20K-line scrollback.
- `set_max_array_size(10_000)`.
- `set_max_map_size(10_000)`.
- `disable_symbol("eval")` — no dynamic code-from-strings.
- No module resolver — scripts cannot `import "..."` files.

Sandbox: Rhai itself does not expose FS, network, or process. The host functions exposed in §4 are the *entire* surface. There is no `read_file`, `write_file`, or shell-execute host fn. Save-form host fns are the only way for scripts to write files, and they only write the path the caller passes (no implicit dirs).

Panics inside any host fn are caught (`std::panic::catch_unwind`), logged with backtrace, and surface as `ERR internal\n<short msg>`.

### 6.4 Concurrency

The watcher thread processes scripts **sequentially** — one at a time, in oldest-`<id>`-first order. Sequential processing matches the writer-side lock semantics (only one script in flight ever) and removes any need for inter-script coordination. The throughput cost is negligible: drivers serialize on `commands.lock` anyway, so two `.rhai` files only ever coexist briefly in the rare case of a crashed driver.

A single `Engine` instance is shared across all scripts in this wrapper's lifetime. Each script gets a fresh `Scope`, so there is no state leakage between scripts (no globals, no `let g` carries over). If callers need state across scripts, they save it via `screen_save_last_after` etc. and re-read on the next call.

### 6.5 Scrollback in `clicom_engine::screen`

`clicom_engine::screen::ScreenBuffer` is a generalization of `inboxmcp`'s `../cliagentchat/src/wrap/screen.rs`. The key change from that implementation: a parallel scrollback ring is added so the engine can serve `screen_tail_*` and `screen_last_after*` from a stable, indexable lifetime buffer (vt100's own scrollback is a *visual* buffer and does not preserve line content as plain text once lines scroll out).

```rust
pub struct ScreenBuffer {
    inner: Arc<Mutex<vt100::Parser>>,
    scrollback: Arc<Mutex<ScrollbackRing>>,
}

struct ScrollbackRing {
    lines: VecDeque<String>,   // finalized lines (no in-progress current-row)
    trimmed_below: u64,        // lifetime index of lines[0]
    hard_cap: usize,           // 20_000
    drop_chunk: usize,         // 10_000
}
```

`advance_bytes` continues to feed the vt100 parser. In addition, after each call, the buffer captures any rows that scrolled off the top of the visible region during this call and appends them to `lines`. When `lines.len() > hard_cap`, drop the oldest `drop_chunk` and bump `trimmed_below` accordingly.

The exact mechanism for detecting scrolled-off rows is an implementation detail of `clicom_engine::screen` and is locked by the implementation plan, not this spec. Two viable approaches: (a) configure the vt100 parser with a small detection scrollback (e.g. 64 rows), then on each advance read the bottom-most rows of vt100's scrollback that are newly populated and append their plain text; or (b) keep a copy of the previous visible-rows snapshot, diff against the current snapshot after each advance, and append the rows that fell off the top. The spec's contract is the line-index semantics — lifetime indexes are monotonic, never reused, and `trimmed_below` is the index of `lines[0]`. The vt100 parser's scrollback parameter (passed to `Parser::new(rows, cols, scrollback)`) is sized to whatever the chosen capture approach requires; it is *not* the source of truth for `screen_tail_*` queries.

The lifetime line index of the *current* visible top row is `trimmed_below + lines.len()`. The `screen_tail_*` host fns resolve indexes by combining `lines` (absolute indexes `[trimmed_below, trimmed_below + lines.len())`) with the visible region (indexes `[trimmed_below + lines.len(), trimmed_below + lines.len() + visible_rows)`). The `screen_last_after*` host fns search across the same combined range, last-occurrence-first.

Negative indexes in the request are resolved against `total_lifetime = trimmed_below + lines.len() + visible_rows`. After resolution and clamping, lines are read from `lines` and/or the parser's visible rows.

Edge cases:

- **Resize**: on resize, the scrollback ring is unaffected. Only the visible rows count changes.
- **Restart of the wrapped command**: not in scope. The wrapper's lifetime equals the wrapped-agent process's lifetime; a restart is a new wrapper, new pid, new `<pid>-<rand6>` dir, line indexes restart at 0.

### 6.6 Snapshot cadence

`screen.txt` is rewritten atomically on each idle transition (busy → idle and idle → busy) and at most once per 250 ms during sustained activity. Same cadence as `inboxmcp`'s existing snapshot path (see `../cliagentchat/src/wrap/snapshot.rs` for reference); the engine implements it independently. `clicom`-started wrappers do not write to `participants/<nick>/status/` — that's `inboxmcp`-specific.

**On child exit:** the wrapper writes one final snapshot of `screen.txt` *before* flipping `status.json` to `state="exited"`. This guarantees that after the wrapper returns, `screen.txt` reflects the last frame the agent produced and `status.json:exit_code` reflects the child's exit. The pair (`screen.txt`, `status.json`) is the post-mortem record of the run.

The full lifetime scrollback is *not* written to disk on exit — it remains only in the running wrapper's memory. If forensic capture of scrollback is needed, the caller can issue a final `clicom run "screen_tail_save(\"final.txt\", 0, -1)"` before the wrapped agent exits.

## 7. Failure modes

| Failure | Behavior |
|---|---|
| `clicom run` driver dies after dropping `.rhai`, before reading result files | Wrapper still executes and writes `.out`/`.err`/`.done`. The files linger like a queued result — they're cleaned by retention or by a later caller. The lock is released by OS on driver exit. |
| Wrapper dies mid-script (between reading `.rhai` and writing `.done`) | Driver hits its `--timeout`, exits 4. Next driver invocation marks the instance `died` lazily. The unprocessed `.rhai` lingers; the next live wrapper in this cwd ignores other instances' command dirs entirely. |
| `clicom queue` script never gets read | Result files (`.out` / `.err` / `.done`) accumulate in `commands/`, but bounded by the result-triple cap (§3.7.2): only the 10 most recent triples are kept. Dead-instance retention (§3.7.1) eventually removes the whole dir. |
| `clicom run` (default) issued while orphan `*.rhai` or `clicom queue` traffic is pending | Exits 5 with stderr `busy: <N> pending script(s)`. No `.rhai` is dropped. A competing `clicom run` does not trigger this — it holds `commands.lock` through its own `.done`, so a second `run` blocks on lock acquisition until the first finishes (then sees an empty queue and proceeds). |
| `clicom run --wait` issued, queue never empties (e.g. continuous `queue` traffic) | Driver hits `--timeout`, exits 4. The lock is released; subsequent writers proceed. |
| `clicom clean` blocked by a long-running `clicom run` | Clean acquires `commands.lock` (§3.4) for the sweep. A run holding the lock through its `.done`+read+delete makes clean wait. There is no per-clean timeout; long-blocking runs delay cleans indefinitely. Workaround: target the specific stale `<id>` once the offending run finishes, or wait for the wrapper to exit (then no one holds the lock). |
| Partial pid match ambiguous | `clicom run`/`queue`/`status` exits 2, prints all matching dirs to stderr. The user disambiguates with a longer prefix. |
| Two writers collide on the same `<id>` | Statistically impossible (`unix_nanos + 6 hex = ~6.4 × 10^25` namespace) but if it happens, atomic `rename` will succeed for both because the second writer creates a *different* `<id>.rhai.tmp` first, then renames over its own target. There is no cross-writer overlap because each writer generates its own unique `<id>`. |
| Lock file deleted out from under live writers | Writers re-create on next invocation; in-flight lock holders see EBADF and exit 1. Don't delete the lock file. |
| `.gitignore` is read-only / permission-denied | Logged at `WARN`, ignored. Wrapper continues. |
| `commands/` becomes huge (many orphan `.rhai` from a misbehaving driver) | Wrapper processes oldest-first, no upper bound. Each new `clicom start` creates a *new* `<pid>-<rand6>` dir, so orphans stay confined to the dir of the wrapper that received them. Once that wrapper exits, dead-instance retention (§3.7.1) eventually removes the whole dir. |
| Scrollback memory pressure | Hard cap 20_000 lines × ~200 bytes/line typical = ~4 MB worst case per wrapper. Acceptable. |
| Rhai script infinite loop | Caught by `max_operations` cap → `ERR runtime`. Or by wall-clock timeout → `ERR timeout`. Either way the script-executor thread returns and the wrapper resumes processing the next script. |
| Rhai script panics (e.g. unwrap on a None) | Caught by `catch_unwind`, surfaces as `ERR internal`. The engine itself is not corrupted because each script runs against a fresh `Scope`. |
| Malicious script tries to escape the sandbox | Rhai has no FS / network / shell access. Host fns are the only effects, and they only do what they're documented to do. Worst case: a malicious script types arbitrary input into the wrapped agent's stdin or writes to a file path the script passed to a save-form host fn. **This is the threat model declared in §2: anything that can drop a file in `commands/` can already drive the agent — that's the protocol's contract, not a leak.** |

## 8. Test plan

**Unit (`src/clicom_engine/`):**
- Each host fn called directly (bypassing Rhai) with valid and invalid args; verify return shapes and error codes.
- Scrollback range resolution: positive, negative, mixed, fully-trimmed (`ERR range`), partially-trimmed (clamp).
- "After marker" search: literal text last-occurrence, regex last-occurrence, marker-not-found returns empty, regex compile error → `ERR host_fn`.
- Dead-instance retention (§3.7.1): 12 dead dirs → 10 kept, 2 removed; live dir untouched.
- Result-triple cap (§3.7.2): write 12 triples directly to `commands/`, invoke the eviction routine, verify the 2 oldest are removed and the 10 newest remain; verify `<id>.rhai` files are never touched.
- Lazy died-detection: kill the pid, observe state transition on read.
- `.gitignore` append idempotency: existing line, missing file, permission denied.
- Engine operations cap: a script with `loop {}` → `ERR runtime` within bounded time.
- Engine wall-clock timeout: a script that calls `wait_ms(700_000)` (above cap) → `ERR host_fn`. A script that `set_timeout(500); wait_ms(2000)` → `ERR timeout`.
- Dir-name partial match: substrings of pid, rand, and combined form all resolve correctly; ambiguous matches list candidates.

**Integration tests** use `assert_cmd` + `tempfile`. File names match the §6.1 layout (`tests/e2e_*.rs`).

**`tests/e2e_basic.rs` — start + run round-trip:**
- `clicom start -- <fake_agent>` (a tiny Rust binary that `cat`s stdin to stdout) → instance dir is created with the right files.
- `clicom run "type_text(\"hi\\n\")"` → bytes appear on the agent's stdout via `screen.txt`.
- `clicom run "screen_text()"` → stdout is the current visible screen.
- `clicom run "screen_save(\"out.txt\")"` → `out.txt` matches the screen.
- `clicom run "screen_last_after(\"marker\")"` returns the post-marker tail; marker-not-found returns empty.
- `clicom run "screen_tail_text(0, -1)"` covers the full range.
- `clicom run "set_timeout(5000); wait_idle(500, 4000)"` exits 0 after the agent quiesces.
- Trivial inline composition: `clicom run "type_text(\"hi\\n\"); wait_idle(500); screen_text()"` — exit 0, stdout is the screen content.
- Driver timeout: kill the wrapper while a `.rhai` is in flight, expect exit 4.
- `clicom status` lists all instances; `clicom status <pid>` filters to one.

**`tests/e2e_queue.rs` — queue + result files + result-triple cap:**
- `clicom queue` returns immediately, prints `<id>` to stdout. After the wrapper completes the script, `<id>.out` / `<id>.done` are present.
- `clicom queue` followed by reading the result files manually: caller confirms the workflow without `clicom check`.
- Result file ordering: `<id>.done` only appears after `<id>.out` (and `<id>.err` if applicable) — verified by polling the directory at high frequency during a slow script.
- Result-triple cap (§3.7.2): drop 12 `clicom queue` scripts in sequence; after the wrapper finishes the last one, only 10 triples remain in `commands/`, and the 2 oldest have been evicted.

**`tests/e2e_busy.rs` — run busy modes:**
- `clicom run` (default) with another `clicom queue` script in flight: start a 2-second-blocking script via `queue`, then run `clicom run "type_text(\"hi\")"` immediately → exit 5 with `busy: 1 pending`.
- `clicom run` (default) with another `clicom run` (default) in flight: second caller blocks on `commands.lock` until the first's `.done` lands, then succeeds (exit 0). Confirms competing `run` does *not* trigger busy fail.
- `clicom run --wait`: same setup as the queue case, but `clicom run --wait "type_text(\"hi\")"` blocks ~2 s then succeeds.
- `clicom run --force`: same setup, but `clicom run --force "type_text(\"hi\")"` queues and waits for own `.done`; second-script result returned correctly even though the first was still running when we dropped.
- Two concurrent `clicom run` calls serialize correctly via `commands.lock` (verified by `<id>` ordering in `<id>.done` mtimes).
- `--timeout` is a combined budget: `clicom run --wait --timeout 1500` against a 2-s-blocking queued script → exit 4 (budget exhausted during the queue-empty wait).

**`tests/e2e_multi_instance.rs` — multi-instance + partial match:**
- Two wrappers in the same cwd. `clicom run "<script>"` (no partial) → exit 2 with both candidates listed. `clicom run <partial> "<script>"` resolves correctly.
- Partial matches: substrings of pid, rand suffix, and combined `<pid>-<rand>` form all resolve to the right instance.

**`tests/e2e_died.rs` — died detection + dead-instance retention:**
- Kill a wrapper, re-run `clicom status` → state transitions to `died` lazily.
- Retention sweep: 12 dead dirs → 10 kept, 2 removed; live dir untouched.

**`tests/e2e_nopty.rs` — `--nopty` + final-screen-on-exit + mouse:**
- `clicom start --nopty -- echo hello` → child runs without PTY, `screen.txt` captures the output, instance exits cleanly with `state="exited"`, `exit_code=0`.
- `clicom start --mouse` → mouse-tracking sequences pass through to host stdout (verified by feeding a known mouse sequence and reading host stdout).
- Final-screen on exit: `clicom start -- <fake_agent>` that prints "GOODBYE" then exits → after the wrapper returns, `screen.txt` contains "GOODBYE" and `status.json` has `state="exited"`, `exit_code=0`.

**`tests/e2e_clean.rs` — `clicom clean`:**
- `clicom clean <id>` deletes the named triple's `.out` / `.err` / `.done`; `<id>.rhai` is untouched if present.
- `clicom clean` (no `<id>`) wipes triples in `commands/` *whose `.done` exists*; `<id>.rhai` files and triples lacking `.done` are untouched.
- Idempotency: `clicom clean <id>` against a non-existent `<id>` → exit 0.
- Works against `state="exited"` and `state="died"` instances (after killing the wrapper).
- **Sweep-mode skip rule**: pre-create a synthetic `<idA>.out` (no `.done`) alongside a complete `<idB>.out`+`<idB>.done`. Run `clicom clean` (no `<id>`). After: `<idA>.out` still exists; `<idB>` triple is gone.
- **Lock coordination with `clicom run` (default)**: start a 2-second-blocking `clicom run` in another shell, fire `clicom clean` immediately. Verify clean blocks until the run completes (clean's wall-clock duration ≥ run's), and that the run's stdout reflects its successful read of `.out` (i.e., clean did not wipe it).
- **Lock coordination with `clicom run --force`**: same setup with `--force`. Verify clean still blocks across the run's read+delete phase (which re-acquires the lock per §5.4 step 7).
- **No coordination needed against `clicom queue` (drop-only)**: queue's drop is brief; clean coexists with queue traffic without measurable blocking.

**`tests/e2e_rhai.rs` — Rhai sandbox + limits:**
- Multi-step atomic: a multi-line script that does `type_text → wait_idle → screen_save_last_after`; verify the captured file contains only the post-marker content. Multiple statements in one script run as one queue entry.
- Error propagation: a script that asks for a fully-trimmed range → exit 3, `ERR range` on stderr.
- Operations cap: a script with `loop {}` → exit 3, `ERR runtime`, wrapper still responsive to next script.
- Wall-clock timeout: `clicom run "set_timeout(500); wait_ms(2000)"` → exit 3, `ERR timeout`.
- Sandbox: `clicom run "eval(\"type_text(\\\"x\\\")\")"` → exit 3, `ERR parse` (eval disabled at compile time).

**Manual smoke recipe** (added to `docs/superpowers/plans/`):
- Steps to wrap `claude` itself with `clicom start -- claude code`, drive it from a second shell with `clicom run "..."`, observe round-trip via `screen.txt` and `clicom status`.

## 9. Migration / compatibility

- No on-disk migration. `.clicom/` is a new directory; existing `participants/<nick>/status/` and `instances/` (in the `inboxmcp` project) are untouched.
- **`clicom` is a separate Cargo project** in **this directory** (`./`), a sibling of `../cliagentchat/` (the `inboxmcp` project). The `inboxmcp` project's `../cliagentchat/Cargo.toml` and source tree are not modified by this spec.
- This project's `./Cargo.toml` declares its own dependencies:
  ```toml
  [package]
  name = "clicom"
  version = "0.1.0"
  edition = "2021"
  rust-version = "1.75"

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
- Files copied once at the start of implementation from `../cliagentchat/` (the `inboxmcp` project) into `./src/clicom_engine/`:
  - `../cliagentchat/src/wrap/pty.rs` → `./src/clicom_engine/pty.rs`
  - `../cliagentchat/src/wrap/screen.rs` → `./src/clicom_engine/screen.rs` (then extended with the scrollback ring)
  - `../cliagentchat/src/wrap/idle.rs` → `./src/clicom_engine/idle.rs`
  - `../cliagentchat/src/wrap/forwarding.rs` → `./src/clicom_engine/forwarding.rs`
  - `../cliagentchat/src/wrap/nudge.rs` → `./src/clicom_engine/nudge.rs`
  - `../cliagentchat/src/fs_atomic.rs` → `./src/clicom_engine/fs_atomic.rs`
  - The pid-alive logic from `../cliagentchat/src/instance.rs` → `./src/clicom_engine/process.rs` (new helper)
- After copying, the two projects are independent. Code drift is accepted; convergence onto a shared library is a phase-2 spec (§10).
- Build & deploy (run from this `./` directory):
  ```bash
  cargo build --release
  cp target/release/clicom.exe ~/.local/bin/
  ```
  `inboxmcp`'s deploy step in `../cliagentchat/CLAUDE.md` is unchanged; a parallel block for `clicom` is added to **this repo's** `./CLAUDE.md` as part of implementation.

## 10. Open items (defer to follow-up specs)

- **Phase 2 — `clicom mcp` MCP server.** Expose every Rhai host fn (§4) as an MCP tool so JSON-RPC clients can drive the wrapped agent without writing Rhai. The MCP server runs as a child process of `clicom start` (or as a long-running peer), shares the `clicom_engine::Engine` with the file-protocol watcher, and reuses the same lock semantics. Separate spec.
- **Phase 2 — `inboxmcp` adoption of `clicom_engine`.** In the sibling `../cliagentchat/` repo, replace `../cliagentchat/src/wrap/screen.rs`, `../cliagentchat/src/wrap/pty.rs`, `../cliagentchat/src/wrap/idle.rs`, etc. with thin shims onto `clicom_engine` (consumed from this `clicom/` crate) so `inboxmcp wrap` also emits `.clicom/<pid>-<rand6>/`. Lets `clicom run` drive `inboxmcp`-wrapped agents transparently. Separate spec to keep blast radius small.
- **Built-in Rhai helper library**: `screen_contains(re)`, `screen_grep(re)`, `with_timeout(ms, fn)`, etc. Distributed as a `.rhai` file the engine auto-imports, or registered as Rust host fns. Add as real scripts surface the need.
- **Alternate runtimes**: `<id>.lua` (mlua), `<id>.js` (boa or rquickjs). Reserved by file-extension dispatch in §3.5. Add only if a use case demands it.
- **`clicom watch` / live-status TUI**: a foreground command that re-renders `screen.txt` + `status.json` for a chosen instance, as a convenience for human observers. Easy follow-up; not blocking anything.
- **`clicom check <id>`**: a *retrieve-and-delete* helper for `clicom queue` users that combines reading `<id>.out` / `<id>.err` / `<id>.done` and then running `clicom clean <id>` in one call, with optional `--wait` to block until `<id>.done` lands. (`clicom clean <id>` already covers the delete half.) Eliminates the need for callers to know the on-disk path layout.
- **`clicom queue --tag <tag>`**: caller-supplied label embedded in the `<id>` (e.g. `<unix_nanos>-<rand6>-<tag>`) so multiple queued scripts are easier to track. Currently the caller has to remember the auto-generated `<id>`.
- Wrapper-side persistence between scripts: a small key-value store accessible from scripts (`get(k)`, `set(k, v)`). Currently scripts are stateless across calls.
- Streaming results: scripts that produce output progressively (`yield(...)`) writing to `<id>.done.partial` for the driver to tail. Currently single terminal `<id>.done`.
