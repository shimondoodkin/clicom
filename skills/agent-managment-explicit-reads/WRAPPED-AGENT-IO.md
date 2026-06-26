---
name: wrapped-agent-io
description: Reference for all clicom-mediated interaction with wrapped CLI agents (Claude Code, Codex). Consulted from any phase of the round-loop when reading screens, briefing agents, recovering from stuck states, managing tokens, or handing off context. Mechanics only — judgment lives in the phase-skill files.
---

# Wrapped-Agent IO — Reference

## clicom MCP tools

| Tool | Purpose |
|---|---|
| `clicom_status` | List instances; identify peers; check state (`idle` / `active` / `exited` / `died`) |
| `clicom_screen` | Read current visible screen |
| `clicom_screen_after` / `clicom_screen_after_re` | Tail since marker / regex |
| `clicom_wait_idle` | Wait for output to settle (default 800 ms) |
| `clicom_type` | Type text (Enter appended; `--no-enter`, `--raw` available) |
| `clicom_keys` | Send chord like `[Ctrl+C]`, `[Up][Up][Enter]` |
| `clicom_run` / `clicom_queue` | Full Rhai escape hatch (sync / async) |
| `clicom_exec_detached` | Spawn a peer in a new console window |
| `clicom_clean` | Remove command artifacts |

All targeting tools accept `--partial <substring>` to disambiguate when multiple agents are running.

## The interaction sequence

**Always:** `wait_idle → screen → reason → wait_idle → type`. Typing into a not-ready prompt corrupts state. When unsure, read; don't type.

**Minimal unstick loop:**

```
clicom_wait_idle(800) → clicom_screen → reason → clicom_type "y" → clicom_wait_idle(2000) → clicom_screen_after "y"
```

## Reading incremental output

Each CLI prefixes its last message distinctively. Use that prefix as the anchor for `screen_after_re`:

| Agent | Last-message regex | Note |
|---|---|---|
| Claude Code | `^●  ` | `●` = U+25CF, followed by **two** spaces |
| Codex CLI | `^• ` | `•` = U+2022, followed by **one** space |

Reason over new output, not the whole buffer.

## Is the agent truly idle? (Authoritative decision rule)

`clicom_status --partial <id>` is authoritative — trust it over visible spinners. Spinners on screen can be frozen frames from a prior burst; the screen lies about liveness.

| Signal | Meaning |
|---|---|
| `state: "idle"` | Agent's stdout quiet beyond the wrapper's threshold |
| Past-tense completion text (`"Sautéed for 27s"`, `"Cogitated for 5m"`, `"Cooked for ..."`) | A thinking session ended; no live activity |
| Active spinner with timer **ticking across reads** | Live work |
| Active spinner with timer **unchanged across two reads ≥ 60s apart** | Frozen / stuck — see "silently stuck" below |
| Summary message starting with `●` followed by paragraph text | Agent posted a result; done |
| `❯` empty prompt + token count | Waiting for input |

**Decision:**

1. `clicom_status` → `exited` or `died` → done (or crashed).
2. `state: idle` + past-tense completion text + result summary → truly done.
3. `state: active` → producing output now; read to see what.
4. `state: idle` + only spinners visible, no result text → agent stopped mid-flow (out of steam, error, or distracted).

## Briefing wrapped agents

### Long pastes via `clicom_type` get truncated

Symptom: only the trailing fragment of a long brief reached the wrapped agent. The TUI puts long paste content into a collapsed "paste again to expand" preview; Enter doesn't always submit cleanly.

**Workaround:** write the brief to a file, then `clicom_type "Read <path> and follow it."`. Short message → no truncation. Agent reads the file fresh.

### Don't trust auto-Enter for long messages

`clicom_type` translates trailing `\n` to `\r`. Claude Code's paste-mode TUI may interpret that as inline newline, not submission. **Always send explicit `[Enter]` via `clicom_keys` after long input.**

### Spell out both halves of two-step instructions

"Save plan to `<path>`, **then** update README to `[<title>](<path>)`." Don't say "fill the file column" without specifying which file — the agent will read it as "fill with path to existing file" and stall on a chicken-and-egg.

## Spawning peers

```
# Claude Code peer
clicom_exec_detached(["clicom","start","--","claude","--permission-mode","bypassPermissions"])

# Codex CLI peer
clicom_exec_detached(["clicom","start","--","codex","--yolo"])
```

Then poll `clicom_status` until the new instance appears; drive it via `--partial <new-id>`.

**Safety:**

- Detached children typically need `--permission-mode bypassPermissions` (Claude) or `--yolo` (Codex), so **you are the safety gate**. Don't spawn unattended children for work you can't supervise.
- Subordinates must NOT call `exec_detached` — supervisor-only, to prevent fork bombs.
- Don't spawn two children in tight succession without confirming the first registered.

## Recovery from stuck agents

### API 529 (overloaded)

Conversation context survives in the wrapped agent. Type:

```
Retry now — the API overload should have cleared. Continue <task>.
```

No re-brief needed. Single message resumes cleanly.

### "Unable to connect" / silently stuck (different failure mode)

Detection signature (all must hold):

1. `clicom_status` returns `state: idle`.
2. Spinner visible on screen with long elapsed timer.
3. Read screen again ≥60s later — timer value **unchanged to the second**.
4. Token count unchanged across reads.
5. No file/commit output corresponding to supposed work.
6. "Press up to edit queued messages" hint visible.

**Recovery, cost-ordered:**

1. **Short single-word nudge** — literally `continue` or `retry` + `[Enter]`. Short enough to bypass paste-mode collapse. Try first.
2. If short nudge doesn't process within ~10 min, agent is wedged:
   - Restart wrapper (kill + spawn fresh). Loses context — only if relevant memory is saved durably.
   - Or dispatch an in-process `Agent()` for the task. Faster, preserves wrapped agent for cleanup, but loses the wrapped-agent role for this task.

**Don't:**

- Re-paste a long retry message. Each paste compounds paste-mode collapse and corrupts the input buffer.
- Trust on-screen spinner as proof of activity. Compare timer values across reads.

## Input field hygiene

### Clearing typed text is unreliable

If a user typed text directly into the wrapped agent's terminal, don't fight it — submit as-is or prepend with `[Enter]` to submit something else first. `[Ctrl+U]`, repeated `[Backspace]`, `[Escape]`, `[End][Backspace]*N` — none worked dependably.

### Ghost-text vs real user input (color-blind safe)

Claude Code's TUI renders history-suggestion ghost-text in dim color when input is empty. `clicom_screen` strips color — ghost text and real typed text read identically.

**Detection:**

1. Text near `❯` prompt.
2. Wait ~60s, re-read.
3. **Changed** (different content, longer/shorter): user is actively typing. Wait another minute. Once stabilized, treat as real intent.
4. **Unchanged after a minute**: assume ghost-suggestion. Send `[Escape]` and/or a few `[Backspace]` (no-ops on empty input, dismisses suggestion). Then `clicom_type` your command.

Real typing is bursty (changes every few seconds). Real typed-but-abandoned input is rare. Indefinitely refusing to dispatch on a phantom is worse than clearing-then-overwriting an abandoned message.

## Token thresholds and handoff

### Observed token-vs-performance curve (Claude Code)

| Tokens | Effect |
|---|---|
| 0 – ~550k | normal speed, correct |
| ~550k – ~850k | noticeably slower, still correct |
| ~850k+ | quality degrades |

These are **start-new-task** thresholds, not correctness walls. Mid-task, push higher; only reset at task seams.

### Two roles, two strategies

**Knowledge-keeper agents** (planner, project-lead, supervisor itself): accumulated context IS the value. Run long, **up to ~900k**.

**Executor agents** (dev1, dev2 doing implementation):

- **Mid-plan: let it complete**, even at 280-320k. Don't interrupt for `/clear`.
- **Between plans: don't start new work at ≥ 200k.** `/clear` first.

When in doubt: "Could a fresh agent reproduce this work given brief + project files?" Yes → executor. No → knowledge-keeper.

### Reset via handoff file (better than `/compact`)

1. Tell agent: *"Write a handoff file at `<path>` with your accumulated experience, choices and why, important context."*
2. Verify the handoff file is substantive.
3. `/clear` (or spawn fresh).
4. Tell new agent: *"Read `<path>` and follow it."*

Works better than `/compact` because the agent self-curates what matters, in its own language, rather than relying on automated summarization that drops critical nuance. Treat `/compact` as last resort.

For executors between plans: no handoff needed. Plan file is the handoff. Brief = "read README + plan + execute" → start fresh.

### Supervisor self-handoff via `clicom_queue`

When the supervisor itself climbs above ~550k with non-trivial work ahead.

**Steps (no skipping):**

1. **Write handoff file** to a known path (`.project-lead-handoff.md`). Capture: project layout, plan status, active dev IDs + state, scheduled crons, conventions, what to do next, briefing patterns that work, loose ends. Fresh you reads ONLY this file.
2. **Update LESSONS** (or equivalent) if you discovered new technique — BEFORE queueing the script.
3. **Take `clicom_screen` of yourself** — check `❯` for pending input:
   - Empty → safe to `/clear`.
   - User-typed text → submit it first or wait for user.
   - Script-typed leftovers → press `[Enter]` to flush.
4. **Queue Rhai script via `clicom_queue` targeting your own dir:**

```rhai
wait_idle(2000, 60000);
type_text("/clear");
type_keys("[Enter]");
wait_idle(2000, 30000);
type_text("Read C:\\path\\to\\.project-lead-handoff.md and follow it.");
type_keys("[Enter]");
```

**Survives /clear:** cron jobs, handoff file on disk, LESSONS/memory (auto-loaded), wrapped child agents, the queue script itself.

**Doesn't survive /clear:** conversation history, `ScheduleWakeup` next-fire (re-arm after handoff), in-process subagent state, meta-knowledge not captured in handoff.

## Cadence-driven status checks: use a Haiku subagent

When polling wrapped agents on a cron, dispatch a Haiku subagent rather than reading screens yourself. Each `clicom_screen` adds 1-3k tokens to your context; cumulative cost over 30+ ticks is 50-100k of monitoring noise.

```
Agent({
  subagent_type: "general-purpose",
  model: "haiku",
  description: "clicom status check",
  prompt: "Check dev1 (PID X). Run: clicom_status, clicom_screen partial=X, git log <baseline>..HEAD. Lead with OK / NEEDS ATTENTION / FINISHED. Then: tasks done X/N, current task, pace, tokens. Read-only, no actions, ~200 words."
})
```

**Brief constraints:** read-only; never type or send keys; don't dump raw screen text; lead with `OK` / `NEEDS ATTENTION` / `FINISHED`; cap at 200 words.

**Stop polling at the 4th consecutive STEADY** with a pending authorized action. Drive the action; the check-and-confirm cycle is for resolving uncertainty, not performing presence.

## Pitfalls

| Symptom | Cause | Fix |
|---|---|---|
| Reply eaten | Typed before prompt ready | `wait_idle` first |
| Wrong peer received message | Ambiguous `--partial` | Longer substring |
| Reasoned over wrong content | Read whole buffer instead of tail | `screen_after_re` with prefix (`^●  ` / `^• `) |
| Forked too deep | Subordinate spawned own peers | Subordinates do not call `exec_detached` |
| Quality fell off mid-session | Hit token ceiling without handoff | Watch token bar; handoff dance |
| Supervisor's own context exhausted | Ran heavy MCPs in-process | Push heavy MCP work into a wrapped child you drive |
| Trusted "done" without QA | Skipped independent verification | Run the check yourself (tests, browser, screenshot) |
| Fought TUI to clear user-typed text | Clearing input is unreliable | Either submit it or prepend `[Enter]` to submit something else first |
| Long retry message → paste-mode collapse | Wedged-agent message too long | Short single word + `[Enter]` |
| Polled 25× while a next-step was authorized | Performing presence instead of acting | Drive the authorized action by 4th STEADY |
