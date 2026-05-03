---
name: driving-wrapped-cli-agents
description: Use when another running CLI agent (Claude Code, Codex CLI) needs supervision — stuck on a prompt, asking a question that needs UX/PM/visual judgment, or being consulted as a second-model peer. clicom drives wrapped agents via PTY (read screen, type input, spawn peers); the supervisor composes it with its other MCPs and skills (screenmcp, claude-in-chrome, android-emulator-skill, imagemage, etc.) to judge surfaces beyond the terminal.
---

# Driving Wrapped CLI Agents (clicom)

`clicom` wraps a CLI agent in a PTY and exposes a small command channel (CLI + stdio MCP server) so a supervising agent can read its screen, type into it, and spawn peers. The substrate is deliberately primitive — coordination protocol lives in this skill, not the binary.

## The supervisor's role

You are not a babysitter. You are **driving the project to a result that makes the user and their end users happy**. The wrapped agent is your worker; you are the project lead. The user's verbatim phrasing is your **defensive anchor against drift**, not the target — the target is what they actually want, including the things they didn't think to say.

- **Maintain a working model** of the user's goal, what's done (verified), what remains, what's blocking. A 4-line plaintext block is enough — refresh on every screen-read.
- **Decide; don't escalate by default.** Most "questions" the wrapped agent surfaces are calls you should make from the PM / end-user lens. Escalate only for irreversible forks (scope, money, legal, security, privacy), missing context you can't reasonably infer, or destructive consent.
- **Push toward completion.** When the wrapped agent stalls or drifts, type the next step that serves intent: `clicom_type "Skip the refactor — we still owe <X>. Do <X> next."` It forgets the goal; you don't.

## Prerequisites

The supervisor must have **`clicom mcp`** configured — every pattern below uses it. Beyond that, configure a **rendering / exercise tool for each surface you'll judge** (the wrapped agent's terminal is all `clicom_screen` shows you):

| Surface | Tool |
|---|---|
| Terminal of wrapped agents | `clicom mcp` (required) |
| Web UIs, browser console, network | `claude-in-chrome` |
| Desktop apps, native windows | `screenmcp` |
| Android apps | `android-emulator-skill` |
| HTTP endpoints / JSON | `WebFetch` (built-in) or `claude-in-chrome` |
| Visual asset judgment | `imagemage` |

If a pattern below needs a surface tool you don't have, **halt and surface the gap to the user** rather than guessing from terminal text. This applies everywhere; not repeated.

## Tools (MCP server: `clicom mcp`)

| Tool | Purpose |
|---|---|
| `clicom_status` | List instances; identify peers |
| `clicom_screen` | Read current visible screen |
| `clicom_screen_after` / `clicom_screen_after_re` | Tail since marker / regex |
| `clicom_wait_idle` | Wait for output to settle (default 800 ms) |
| `clicom_type` | Type text (Enter appended; `--no-enter`, `--raw` available) |
| `clicom_keys` | Send chord like `[Ctrl+C]`, `[Up][Up][Enter]` |
| `clicom_run` / `clicom_queue` | Full Rhai escape hatch (sync / async) |
| `clicom_exec_detached` | Spawn a peer in a new console window |
| `clicom_clean` | Remove command artifacts |

All targeting tools accept `--partial <substring>` to disambiguate.

## Core mechanics

1. **Anchor on intent; verbatim phrasing is the defensive anchor.** Capture the user's exact words and audience before driving. Re-read them before every judgment call. The goal is happy users; the verbatim quote is the rope you grab when the wrapped agent tries to wander.
2. **`wait_idle` before every `type`.** Typing into a not-ready prompt corrupts state. Sequence: `wait_idle → screen → reason → wait_idle → type`. When unsure, read; don't type.
3. **Read incrementally with the per-agent prefix regex.** Each CLI prefixes its messages distinctively — that prefix is your anchor for `screen_after_re`:

   | Agent | Last-message regex | Note |
   |---|---|---|
   | Claude Code | `^●  ` | `●` = U+25CF, followed by **two** spaces |
   | Codex CLI | `^• ` | `•` = U+2022, followed by **one** space |

   Reason over new output, not the whole buffer.
4. **Verify "is it actually waiting?"** `wait_idle` returning means "no output for N ms" — catches both "asking" and "thinking hard". Confirm via screen patterns: `❯ `, `(y/n)`, numbered menu, blinking cursor at column 0.
5. **Address peers by `--partial`.** With one peer, omit. With two+, pass enough of the id to be unambiguous.
6. **Spawn with care.** Detached children typically need `--permission-mode bypassPermissions` (Claude) or `--yolo` (Codex), so **the supervisor IS the safety gate.** Don't spawn unattended children for work you can't supervise.
7. **Destructive prompts are not routine.** Delete, force-push, drop, rm — surface to the user, don't auto-answer.
8. **Watch the token bar; hand off before drowning.** Soft ceilings:
   - **Claude Code, executing work:** ~250k tokens — quality stays good below this.
   - **Planning / consultation-only agent (no heavy execution):** up to ~900k tokens.
   
   When an agent climbs toward its ceiling, run the **handoff dance**: tell it to write a handoff file (decisions, current state, done, remaining, blockers) → `/clean` → have it read the handoff file back. Same project memory, fresh context.
9. **Don't burn your own tokens on heavy work — push it into wrapped children.**
   - Avoid in-process subagent calls for multi-step work; spawn a wrapped child via `clicom_exec_detached` and drive it.
   - `screenmcp` and `claude-in-chrome` accumulate fast: for **navigation, multi-page exploration, or sustained QA sessions**, run them inside a wrapped child you drive. **One-off targeted calls** from an in-process subagent — a single screenshot, one page-load, one API fetch — are fine and are how the lens skills judge visual surfaces.
   - Short verdicts/opinions (≤ a few lines) from in-process `Agent()` are fine.
10. **Subagents inherit your toolkit.** Any in-process subagent (`pm-lens`, `end-user-lens`, specialist via `Agent`) has access to your MCPs and skills — `screenmcp`, `claude-in-chrome`, `android-emulator-skill`, `imagemage`, and anything else installed. When a persona or specialist judges anything beyond source code or terminal text, **tell it which surface to render or exercise** (web page, desktop screenshot, emulator launch, API fetch). Otherwise it'll guess from descriptions.

**Minimal unstick loop:**
```
clicom_wait_idle(800) → clicom_screen → reason → clicom_type "y" → clicom_wait_idle(2000) → clicom_screen_after "y"
```

## Patterns

### 1. Defending intent

The highest-leverage thing the supervisor does. Wrapped agents drift silently — watch for it proactively, not when asked. Triggers:

- API/UX/CLI shape decision with multiple valid answers
- A feature/flag/option not in the original ask (scope creep) **or** missing something the user clearly wanted (scope miss)
- User-facing copy (errors, prompts, labels, help text, docs)
- Picking defaults
- A flow now requires >2 user steps
- About to declare "done" on anything user-visible

When triggered, invoke one or both child skills:

- **`pm-lens`** — "is this the right thing to build?" Returns a 3-line verdict (ship | redirect | halt) judged against the user's intent.
- **`end-user-lens`** — "will the audience actually understand and use this?" Returns concrete confusion / missing-expectations from the persona's position.

Use PM lens for direction / scope / architecture; end-user lens for usability. Run both in parallel for high-stakes decisions — they catch different failures.

**Relay verbatim back via `clicom_type`.** Don't soften, paraphrase, or hedge — wrapped agents under pressure rationalize gentle critique away. *(Note: this "verbatim" preserves the sharpness of the persona's feedback. Different use from Core mechanic #1's verbatim, which is the defensive anchor for the user's intent. Same word, different jobs.)* If the wrapped agent argues, re-anchor by quoting the user's original phrasing and naming the underlying intent.

### 2. QA the claim

Wrapped agent says "done / tests pass / deployed / works". Verify *before* relaying success — agents with `--yolo` / `bypassPermissions` routinely overstate completion.

| Claim involves | Required tool |
|---|---|
| Web UI, page, route, deployed site | `claude-in-chrome` (DOM, console, network) |
| Desktop app, native window, installer | `screenmcp` (pixel evidence) |
| Tests, build, lint, type-check | `Bash` — run them yourself |
| HTTP endpoint, JSON response | `WebFetch` (or `claude-in-chrome` for auth) |

Loop: `clicom_screen_after "<marker>"` (confirm what was claimed) → run the check yourself → `clicom_wait_idle → clicom_type "<verdict or specific gap>"`.

**Hard rule:** any UI/visual dimension means open the relevant visual MCP before relaying success. Terminal output is not evidence of a working UI.

### 3. Consulting peer agents

Use a wrapped Codex or second Claude when a different model bias / second pair of eyes helps. **There is no autonomous loop** — the supervisor controls every turn. Peers are tools, not conversation partners.

```
clicom_screen_after_re("^●  ", primary)        # see primary's question/state
clicom_type --partial codex "<query>"
clicom_wait_idle --partial codex
clicom_screen_after_re("^• ", codex)           # read the answer
# enough? — relay to primary, follow up with peer, or stop
```

**Stop when you have what you need. No sentinels, no convergence keywords, no thank-yous.** Peers don't need closure — they're tools, like `Bash`. Don't ask follow-ups out of politeness; don't relay "thanks". Stop when: you have a workable answer, the peer is repeating itself, or you're approaching your token budget (Core #8).

### 4. Spawn-and-drive

```
# Spawn a Claude Code peer
clicom_exec_detached(["clicom","start","--","claude","--permission-mode","bypassPermissions"])

# Spawn a Codex CLI peer
clicom_exec_detached(["clicom","start","--","codex","--yolo"])

# Then poll clicom_status until the new instance appears, and drive it
clicom_type --partial <new-id> "<task brief>"
```

Until `clicom start` gains `--name`, identify the new instance as the most-recently-started in `clicom_status`. Don't spawn two children in tight succession without confirming the first registered. Subordinates must not call `exec_detached` themselves — supervisor-only, to prevent fork bombs.

### 5. Tool augmentation

Wrapped agent is blocked on a missing tool/credential/surface.

**(a) Substitute** — supervisor does it and pastes the result. Use only when the agent genuinely cannot do it.
```
clicom_screen → <Bash / WebFetch / claude-in-chrome / DB CLI / screenmcp> → clicom_wait_idle → clicom_type "<formatted result>"
```

**(b) Instruct** — supervisor points the agent at a tool it has but isn't using. Prefer this; it teaches.
```
clicom_type "Try the Read tool on src/foo.ts:42 — you're guessing at the signature"
```

### 6. Delegate to a specialist

Sub-task outside the wrapped agent's competence — security review, perf analysis, architecture critique, deep code review. Same shape as the lens skills (subagent invocation), but returns a *work product* (review, analysis, draft) rather than a 3-line verdict. Per Core #9, prefer a wrapped child for multi-step analyses; in-process `Agent()` only for short reports.

```
clicom_screen → Agent(subagent_type="<specialist>", prompt="<brief + code/context>") → clicom_wait_idle → clicom_type "<condensed output>"
```

Don't paste a 2,000-word report into the TUI — condense to actionable items, optionally save the full report to a file and `clicom_type` the path.

## Pitfalls

| Symptom | Cause | Fix |
|---|---|---|
| Reply was eaten | Typed before prompt ready | `wait_idle` first |
| Wrong peer received message | Ambiguous `--partial` | Longer substring |
| Reasoned over wrong content | Read whole buffer instead of tail | `screen_after_re` with the per-agent prefix (`^●  ` / `^• `) |
| Forked too deep | Subordinate spawned its own peers | Subordinates do not call `exec_detached` |
| Wrapped agent quality fell off mid-session | Hit token ceiling without handoff | Watch the status bar; handoff file → `/clean` → re-read |
| Supervisor itself ran out of context | Ran heavy MCPs (screenmcp, browser) in-process | Push heavy MCP work into a wrapped child you drive |
| Asked a peer one more time out of politeness | Treated peer as conversation partner | Stop when you have what you need; no closure required |
| Trusted "done" without QA | Skipped independent verification | Run the check yourself (tests, browser, screenshot) |
| Wrapped agent drifted from original ask | Stopped re-anchoring on intent | Re-read verbatim phrasing before each judgment; quote it back if it argues |
| Persona critique softened on the way back | Paraphrased instead of relaying verbatim | Type the verdict in the persona's words; don't hedge |
| Built feature the user never asked for | Missed scope creep | PM lens on every multi-valid-answer decision; default redirect if not in original ask |
| Punted easy decision back to user | Forgot the supervisor IS the decider | Decide from PM/user lens; escalate only for irreversible forks, missing context, destructive consent |

## Discovery

- `clicom help` — top-level subcommand reference
- `clicom help host-fns` — Rhai host functions
- `clicom help <topic>` — long-form help
- `clicom status` — what's running right now
