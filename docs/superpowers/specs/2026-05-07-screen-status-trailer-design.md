# `clicom` — Screen Status Trailer — Design Spec

**Status:** Draft 1
**Date:** 2026-05-07
**Related:**
- [2026-05-02-clicom-wrapped-commands-channel-design.md](./2026-05-02-clicom-wrapped-commands-channel-design.md) — base design (host functions §4, status fields §3.5, discovery / lazy died-detection §3.7.1, §5.3)

**Working-directory convention:** all unqualified relative paths (`src/...`, `tests/...`) refer to this `clicom/` repo.

## 1. Purpose

Append a one-line **status trailer** to the output of read-only screen-query tools (`clicom screen`, `clicom screen-after`, `clicom screen-after-re`, and their MCP equivalents) so a calling supervisor (typically another LLM agent) can see the wrapped agent's lifecycle state, last activity timestamp, and visible-row count *in the same response* as the screen text. No round-trip to `clicom status` required.

This eliminates the most common source of supervisor-side bugs: mistaking a stale spinner frame in the rendered screen for live activity. With `state=idle` printed right after the buffer, that misread becomes impossible by construction.

A secondary goal: when the wrapped agent has crashed or exited, the same tools fall back to the persisted `screen.txt` snapshot and a CLI-side trailer reporting `state=died` or `state=exited`. The supervisor learns the agent's gone *and* sees its final visible state in one call.

## 2. Scope

**In:**
- Three new Rhai overloads exposed from the wrapper, each with a positional `prepend_status: bool` parameter. **Default is `false` via the no-arg form** — so `clicom run -- 'screen_text()'` and any other Rhai-script caller stays raw / unchanged. Opt in by passing `true`:
  - `screen_text()` / `screen_text(bool)`
  - `screen_last_after(marker)` / `screen_last_after(marker, bool)`
  - `screen_last_after_re(pattern)` / `screen_last_after_re(pattern, bool)`
- One new Rhai host fn `clicom_status_trailer()` returning the trailer string (without leading newline), exposed for power-user composition (`screen_text() + "\n" + clicom_status_trailer()`).
- `HostContext` gains an `Arc<Mutex<Status>>` field so the trailer can read live state.
- Quick commands (`clicom screen`, `clicom screen-after`, `clicom screen-after-re`) **default trailer ON**: they emit Rhai source with the bool set to `true`.
- CLI flag `--no-status` on those three quick commands flips the bool back to `false`.
- MCP tools (`clicom_screen`, `clicom_screen_after`, `clicom_screen_after_re`) **default trailer ON**: same translation.
- MCP arg `no_status: bool` (default `false`) on the three tools, flips trailer off.
- **CLI-side dead-instance fallback** for the three quick / MCP screen tools: when discovery resolves the target instance to `state ∈ {Died, Exited}`, the CLI/MCP path skips the script-drop and instead reads `screen.txt` + `status.json` from disk, applies the marker/regex transform if any, builds the trailer CLI-side (honoring `no_status`), prints, and exits 0.

**Out:**
- No trailer on `clicom run` (generic Rhai eval — power users compose with `clicom_status_trailer()` themselves).
- No trailer on `clicom queue`, `clicom type`, `clicom keys`, `clicom wait-idle` (these don't return screen text).
- No new fields beyond `state`, `last_activity`, `visible_rows`. `lifetime_lines`, `tokens_seen`, etc. are explicitly deferred — adding them later is non-breaking because the trailer format is `key=value` pairs.
- No API for parsing the trailer (callers are expected to grep / regex if they need to extract fields).

## 3. Trailer Format

Single line, prepended by `\n`, **no trailing newline**:

```
\n[clicom: state=<word>  last_activity=<rfc3339-Z>  visible_rows=<n>]
```

- Fields separated by **two spaces** (visual aid, easier to scan than single-space).
- `state` ∈ `{idle, active, exited, died}`. Mapping:
  - `State::Idle` → `idle`
  - `State::Busy` → `active` (supervisor-facing word, picked over internal "busy")
  - `State::Exited` → `exited`
  - `State::Died` → `died` (only emitted by the CLI-side fallback path; never by the Rhai-side path because a live wrapper can't see itself as died)
- `last_activity` formatted as `%Y-%m-%dT%H:%M:%SZ` (RFC3339 with `Z` suffix, no microseconds).
- `visible_rows` is the height of the active screen buffer, e.g. `40`. Derived from `ScreenBuffer::rows()` for the live path; from line-count of the on-disk `screen.txt` for the dead-instance fallback path.

Example:
```
[clicom: state=idle  last_activity=2026-05-07T01:34:12Z  visible_rows=40]
```

## 4. Architecture

Two execution paths, one shared format helper.

### 4.1 Live path (Rhai-side)

The live path runs inside the wrapper process when a script is dispatched through `commands/`.

- `HostContext` (in `src/clicom_engine/rhai_host.rs`) gains `pub status: Arc<Mutex<crate::clicom_engine::meta::Status>>`. Wired up at construction time in `cmd_start::run` from `ch.status` (already an `Arc<Mutex<Status>>`).
- A new module `src/clicom_engine/status_trailer.rs` exposes a single function:
  ```rust
  pub fn format(state: TrailerState, last_activity: chrono::DateTime<chrono::Utc>, visible_rows: u16) -> String;
  ```
  where `TrailerState` is a small enum with variants `Idle, Active, Exited, Died` and a `Display` impl emitting the lowercase word. Returns the bracket-wrapped line *without* the leading newline (caller decides separator).
- The wrapper's mapping `meta::State → TrailerState`:
  - `Idle → Idle`, `Busy → Active`, `Exited → Exited`, `Died → Died`. `Died` is not expected from the live path (a running wrapper can't see itself dead), but we map it through silently anyway — defensively cheaper than panicking on what is fundamentally a UX-feature code path.
- New host fn `clicom_status_trailer()` registers as zero-arg, returns `format(...)` using:
  - state from `HostContext.status.lock()` reading the current `Status.state`,
  - last_activity from the same lock,
  - visible_rows from `HostContext.screen.visible_dims().0`.
- Each existing host fn gets a sibling overload that takes the bool. Both the no-arg and 1-arg form share the same closure body via a small helper. When `prepend_status=true`, returns `format!("{text}\n{trailer}")`. When `false`, returns `text` unchanged. **The no-arg form is implemented as `register_fn(name, move || inner(false))`** — script callers default to raw text. Opt-in is `screen_text(true)` or composition with `clicom_status_trailer()`.

### 4.2 Dead-instance fallback (CLI-side)

The fallback path runs in the CLI/MCP process before any Rhai script is dispatched.

Touched files: `src/clicom_cli/quickops.rs`, `src/clicom_cli/cmd_mcp.rs`.

Each of `quickops::screen`, `quickops::screen_after`, `quickops::screen_after_re` gets a new wrapper with this shape:

```text
1. let inst = discovery::list_instances(cwd) |> filter_by_partial(partial)
   (this triggers lazy died-detection — Idle/Busy + dead pid → Died).
2. if inst is exactly 1 and inst.state ∈ {Died, Exited}:
     read screen.txt from inst.dir (anyhow-bail if missing)
     apply transform: identity / rfind(marker) suffix / regex find_iter last-end suffix
     trailer = status_trailer::format(inst.state -> Died|Exited, inst.status.last_activity, line_count(screen.txt))
     print "{transformed}\n{trailer}" if !no_status else "{transformed}"
     return Ok(0)
3. else (0 / >1 matches, or single live match):
     fall through to existing run_with(...) path.
```

The `partial.is_none() && len > 1` ambiguity case keeps its current "ambiguous match" message and exit 2.
The "no matches" case keeps its current "no live wrapped agent" message and exit 2.

Note: this fallback only checks the *resolved* `state`, not the wrapper's PID directly. We rely on `discovery::list_instances` having already done the pid-aliveness check and rewritten the on-disk status (its existing job per the base spec §5.3).

### 4.3 MCP wiring

`src/clicom_cli/cmd_mcp.rs` changes:
- Each of the three tool schemas gains `no_status: { type: "boolean", default: false }` in its `inputSchema.properties`.
- Each tool's dispatch arm reads `args.get("no_status").and_then(Value::as_bool).unwrap_or(false)` and forwards it as a new positional to the corresponding `quickops::*` function.

### 4.4 CLI wiring

`src/bin/clicom.rs`:
- `Cmd::Screen`, `Cmd::ScreenAfter`, `Cmd::ScreenAfterRe` each gain `#[arg(long)] no_status: bool`.
- Dispatch arms forward `no_status` to `quickops::*`.

`src/clicom_cli/quickops.rs` (existing functions get a `no_status: bool` parameter):
- `screen(cwd, partial, no_status)` runs the fallback first; on fall-through, builds Rhai source as `"screen_text()"` if `no_status` else `"screen_text(true)"` — quick path defaults trailer on.
- `screen_after(cwd, partial, marker, no_status)` analogous: `screen_last_after(<lit>)` vs `screen_last_after(<lit>, true)`.
- `screen_after_re(cwd, partial, pattern, no_status)` analogous: `screen_last_after_re(<lit>)` vs `screen_last_after_re(<lit>, true)`.

## 5. Data Flow

### Live, --no-status (raw, existing screen_text behavior)
```
clicom screen --no-status  →  quickops::screen(no_status=true)  →  run "screen_text()"
                                                                  →  wrapper Rhai (no-arg overload, prepend_status=false)
                                                                  →  raw text
                                                                  →  print
```

### Live, default (trailer on)
```
clicom screen  →  quickops::screen(no_status=false)  →  run "screen_text(true)"
                                                      →  wrapper Rhai (1-arg overload, prepend_status=true)
                                                      →  text + "\n" + trailer
                                                      →  print
```

### Script caller (`clicom run -- 'screen_text()'`) — unchanged
```
clicom run -- 'screen_text()'  →  wrapper Rhai (no-arg overload, prepend_status=false)
                               →  raw text  →  print
```

### Dead instance, default
```
clicom screen  →  discovery (lazy died-detection)  →  state == Died|Exited
              →  read screen.txt + status.json from disk
              →  CLI builds trailer via status_trailer::format(...)
              →  print "{disk_text}\n{trailer}"  →  exit 0
```

### Dead instance, --no-status
```
clicom screen --no-status  →  same as above but skip trailer
                           →  print "{disk_text}"  →  exit 0
```

## 6. Testing

Unit:
- `status_trailer::format` produces the exact `[clicom: state=… last_activity=… visible_rows=…]` form for each state variant. Use a fixed-instant `DateTime<Utc>` to make the test deterministic.
- `TrailerState` `Display` impl emits the four lowercase words.

Integration (Rhai-side, in `rhai_host.rs` test module):
- `screen_text()` (no arg) returns plain text identical to current behavior (default `prepend_status=false`).
- `screen_text(true)` returns `<text>\n[clicom: …]`.
- `screen_text(false)` explicitly equals the no-arg form.
- `screen_last_after("X")` and `screen_last_after("X", true)` likewise.
- `screen_last_after_re("X")` and the 2-arg form likewise.
- `clicom_status_trailer()` standalone returns just the trailer line (no leading newline).

Integration (CLI-side fallback, e2e tests under `tests/`):
- `tests/e2e_screen_trailer.rs` (new): manually craft a dead-instance directory (PID 4_000_000, status.state=Busy, screen.txt with known content), run `clicom screen` against it, assert stdout = `<screen.txt content>\n[clicom: state=died  last_activity=…  visible_rows=…]`. Also assert exit 0.
- Same with `state=Exited` (exited normally, exit_code=Some(0)) → trailer says `state=exited`.
- Same with `--no-status` flag → trailer absent.

## 7. Edge cases

- **Empty `screen.txt`** in fallback: trailer still emitted; the "text" portion before the trailer is empty (so output starts with `\n[clicom: …]`).
- **Missing `screen.txt`** in fallback (dir partially corrupted): bail with a clear error; do not synthesize an empty screen silently.
- **Multiple matching dead instances** (e.g. two crashed agents in the same cwd, partial matches both): keep current "ambiguous match" message + exit 2. The fallback only runs when discovery resolves to exactly one instance.
- **`screen_text()` called from inside a wrapping script that re-applies its own trailer**: doubles up. Acceptable; the user opted in. Power users use `screen_text(false)` to avoid it.
- **`HostContext.status` lock contention**: status is updated by the snapshot writer thread. Trailer reads the lock briefly. No new lock ordering concern because no host fn currently holds another lock while reaching for `status`.

## 8. Backward compatibility

- **Rhai script callers** (`clicom run -- 'screen_text()'`, custom `.rhai` scripts dropped via the queue): unchanged. The no-arg overloads default `prepend_status=false`, so `screen_text()`, `screen_last_after("X")`, and `screen_last_after_re("P")` produce the same raw output they always did. Opt in by passing `true` or composing with `clicom_status_trailer()`.
- **Quick CLI commands** (`clicom screen`, `clicom screen-after`, `clicom screen-after-re`): now print `<text>\n<trailer>` by default. **Behaviour change** for users who scripted around exact stdout — mitigation: pass `--no-status` for raw output, or update the script to strip the trailing `[clicom: …]` line.
- **MCP tools** (`clicom_screen*`): same as quick commands — return value now includes a trailing `[clicom: …]` line by default. Mitigation: `no_status: true` returns raw text.
- Existing in-repo tests that assert exact quick-command stdout need updating to either pass `--no-status` (preserve old assertion) or assert on the new default form. The Rhai-side tests for `screen_text()` etc. are unaffected.

No deprecation period; project is pre-1.0 and the supervisor-facing benefit on the quick-command / MCP side is the whole point.

## 9. Implementation order

Suggested commit boundaries (each independently buildable + testable):

1. **`status_trailer` module** — new file, format function, unit tests. No wiring yet.
2. **HostContext gains `status`** — plumb `ch.status` through, no behavior change yet (just availability).
3. **Rhai overloads** — register the bool variants for the three screen fns and add `clicom_status_trailer()`. Update Rhai-side tests.
4. **CLI/MCP `--no-status` wiring** — pass-through to quickops; quickops emits the right Rhai source. Update e2e tests for live path.
5. **Dead-instance fallback** — quickops gets the discovery-first branch, MCP gets the same. New e2e tests.

Each step ships a working binary; no half-states.
