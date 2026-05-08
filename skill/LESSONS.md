# Lessons learned — managing multi-plan development with clicom

Concrete observations from a session driving two wrapped Claude Code agents (`dev1`, `dev2`) through a multi-plan project (vibe-site-editor: 7 plans, plan 1 developed, plans 2-4 written-then-corrected, plan 2 development underway). Pair with `SKILL.md`.

---

## Planning vs. executing discipline

### Do **not** write all plans before developing any of them

We wrote Plans 2, 3, and 4 ahead of any implementation beyond Plan 1. The user corrected this: *"oh its bad need to develop in sequence and then do plan"*. Reasons it was bad:

- Each later plan made assumptions about file layouts, type names, conventions that the prior plan would supposedly produce.
- Without ground-truth code, those assumptions can't be validated. The plans look good in isolation but may diverge from reality.
- Token cost: dev2 burned ~280k tokens writing 3 plans that may need rewriting against actual code.

**Rule:** alternate write → develop → write → develop. With two devs, pipeline (dev1 develops Plan N while dev2 writes Plan N+1) — but never let writing get more than one plan ahead of development.

### A "written" plan is reference, not gospel

When pre-written plans need to be executed against later, ground them in real code:
- The actual implementation of the prior plan is ground truth.
- The pre-written draft is reference to consider, modify, or replace.

Save this kind of cross-wakeup intent to **project memory**, not to a /loop prompt — loop prompts get truncated/lost across long sessions, memory is durable.

### Observed token-vs-performance curve (Claude Code)

Empirically with this user's setup, output stays **correct** much longer than it stays **fast**:

| Tokens | Observed effect |
|---|---|
| 0 – ~550k | normal speed, correct |
| ~550k – ~850k | noticeably slower, still correct |
| ~850k+ | performance degrades (speed and quality) |

So the per-role thresholds below aren't "correctness walls" — they're **start-new-task** thresholds, chosen to keep agents in the fast band. Mid-task, push higher; only reset when you're picking up something new.

### Two agent roles, two token strategies

**Knowledge-keeper agents** — accumulated context *is* the value (design decisions, multi-plan reasoning, project history). Run them long, **up to ~900k**. Don't kill prematurely.

**Executor agents** — the artifact is the value (code, tests, deliverable). Once produced, accumulated context is disposable. Two thresholds, not one:
- **Mid-plan: let it complete**, even if it crosses 250k. Interrupting to /clear loses in-flight state. Better to finish at 280-320k than to abandon.
- **Between plans: don't start new work at ~200k or higher.** `/clear` first if it's at or above 200k when ready for the next plan.

Fresh executor per plan is cleaner than carrying forward — but only at the seam, never mid-plan.

Empirical data from this session:
- dev2 (knowledge-keeper, plan-writer): 50k → 193k → 282k → 331k across 3 plans. Stayed coherent. Cost per plan: 50-150k.
- dev1 (executor): 207k after Plan 1 alone (subagent-driven, ~19 min). Cleared before Plan 2.

When in doubt, ask: *"Could a fresh agent reproduce this work given a brief + project files?"* Yes → executor. No → knowledge-keeper.

### Reset trick: handoff file → fresh agent (better than `/compact`)

When a knowledge-keeper approaches its ceiling and you must reset:

1. Tell it: *"Write a handoff file at `<path>` with your accumulated experience, choices and why, important context."*
2. Verify the handoff file is substantive.
3. `/clear` (or spawn fresh).
4. Tell the new agent: *"Read `<path>` and follow it."*

Works better than `/compact` because the agent self-curates what matters, in its own language, rather than relying on automated summarization that drops critical nuance. Treat `/compact` as last resort.

For executors: no handoff needed between plans. The plan file itself is the handoff. Brief = "read README + plan + execute" → start fresh every time.

#### The supervisor can self-handoff via `clicom_queue`

The supervisor (project_lead) is itself a wrapped clicom agent. When *its own* token count climbs into the slow band, the same handoff dance works — but it has to be self-triggered, because after `/clear` you (current you) are gone and only the queued script and the handoff file survive to bootstrap the next you.

**Trigger:** token count above ~550k (top of the fast band per the speed curve table) with non-trivial work still ahead. Don't wait for the wall — once you're slow, the handoff itself runs slow.

**The dance — in this order, no skipping:**

1. **Write the handoff file** to a known path (`.project-lead-handoff.md` at project root). Capture: project layout, plan status, active dev IDs + state, scheduled crons, conventions you've established, what to do next, briefing patterns that work, loose ends. The fresh you reads ONLY this file — if it isn't in the file, it's lost.
2. **Update LESSONS.md if you discovered new technique** — do this BEFORE queueing the script. Once /clear fires, the meta-knowledge you'd want to save is gone.
3. **Take a `clicom_screen` of yourself** — check the `❯` input line for typed-but-not-submitted text. Pending input would prepend or interleave with the script's `/clear` and submit garbage like `some_user_text/clear`. Decide:
   - `❯` is empty → safe to /clear directly.
   - `❯` has user-typed text → don't fight it (clearing input is unreliable, see below). Either submit it first (if it makes sense to act on it) or wait for the user. Once `❯` is empty, proceed.
   - `❯` has script-typed leftovers from a prior incomplete dance → press `[Enter]` to flush, or wait for clicom timeout, then proceed.
4. **Queue a Rhai script via `clicom_queue` targeting your own dir** that runs autonomously after your current turn ends. The script must do everything — you won't exist to nudge it:

```
wait_idle(2000, 60000);
// optional: clear input if you saw stale text in step 3 and decided to flush rather than submit
// type_keys("[Enter]");  // submit any leftover, then proceed
type_text("/clear");
type_keys("[Enter]");
wait_idle(2000, 30000);
type_text("Read C:\\path\\to\\.project-lead-handoff.md and follow it.");
type_keys("[Enter]");
```

`clicom_queue` is fire-and-forget — you queue and continue. The leading `wait_idle` makes the script wait until your current turn ends. Then `/clear` + Enter lands, conversation resets, the read-handoff message arrives, fresh project_lead picks it up cold.

**Things that survive /clear:**
- Cron jobs (`CronCreate` jobs are session-level, not conversation-level)
- The handoff file on disk
- LESSONS.md and memory (auto-loaded into the new context)
- Wrapped child agents (dev1, dev2 — they're separate processes with their own contexts)
- The queue script itself (runs after /clear has landed)

**Things that DON'T survive /clear:**
- Conversation history (the whole point)
- `ScheduleWakeup` next-fire (re-arm after handoff if you were using dynamic mode — note `CronCreate` does survive)
- In-process subagent state (irrelevant since they're transient anyway)
- Any meta-knowledge not captured in handoff/LESSONS/memory

**When to do it:**
- Token count > ~550k and rising, with significant work still ahead
- About to hand off to next-day session and want continuity
- When `/compact` would drop too much nuance

**Why /compact is worse:** the agent self-curates the handoff file in its own language, ranking what matters. /compact relies on automated summarization that drops critical nuance and is one-shot — you can't review what got dropped.

### `/clear` is non-optional for executors between tasks

Claude Code itself surfaces *"new task? /clear to save 207.6k tokens"* as the universal hint. Don't ignore it for executors.

---

## Briefing wrapped agents reliably

### Long pastes via `clicom_type` get truncated by Claude Code's TUI

Symptom: only the trailing fragment of a long brief reached dev1; the rest was lost.

The TUI puts long paste content into a collapsed "paste again to expand" preview. Pressing Enter (whether via clicom_type's auto-append or explicit `[Enter]`) doesn't always submit cleanly.

**Workaround that worked:** write the brief to a file, then `clicom_type "Read <path> and follow it."`. Short message → no truncation. Agent reads the file fresh.

### Don't trust auto-Enter for long messages

`clicom_type` translates trailing `\n` to `\r`. For Claude Code's paste-mode TUI, that may be interpreted as an inline newline, not submission. **Always send an explicit `[Enter]` via `clicom_keys` after long input.**

### Clearing the input field is unreliable for *real* typed text

Tried, none worked dependably to clear text actually typed into Claude Code's input:
- `[Ctrl+U]`
- `[Backspace]*N`
- `[Escape]`
- `[End][Backspace]*N`

If user has actually typed text into the wrapped agent's terminal directly, **don't try to override it** — either submit it as-is (if it makes sense) or prepend with `[Enter]` to submit something else first.

**But:** if the screen shows persistent unchanged text near `❯` after a 60s wait, it is most often a **TUI history-suggestion / ghost-text** (faded autocomplete from prior commands), not real input. The supervisor's `clicom_screen` capture strips color, so you cannot distinguish dim suggestion text from real typed text — they read identically. In that case, `[Escape]` or a few `[Backspace]` keys are no-ops on the empty input but do dismiss the suggestion; safe to send before your command. See "User-typed direct input — distinguish from ghost-suggestion" below.

### Spell out both halves of a two-step instruction

When briefing dev2 to "write Plan 2 + update README to link it", I said "fill in the file column" without explicitly saying "the file you just wrote". dev2 read it as "fill column with path to existing file" and surfaced a chicken-and-egg blocker. **Brief format that worked second time:** "Save plan to `<path>`, *then* update README to `[<title>](<path>)`."

---

## Demo / verification environments

### Don't run the editor in the same dir where a dev is editing

dev1 is mid-Plan-2 in `vibe-editor/`, modifying source files. Trying to start the Plan 1 editor service from `vibe-editor/` causes:
- TypeScript build fails (dev1's WIP test references unimplemented types)
- File lock conflicts when service tries to read while dev1 writes

**Solution that worked:** `git worktree add ../vibe-editor-plan1-demo HEAD` (HEAD = the committed Plan 1 final state). Run the demo from the worktree, separate from dev1's WIP. Remove the worktree when done.

### `npx tsx` may not keep long-running services alive

`npx tsx src/cli/index.ts serve` started Fastify but the parent process exited immediately, killing the server. Same code via `node --import tsx -e "<inline>"` with an explicit `setInterval(()=>{}, 1000)` keep-alive worked.

Hypothesis: npx detaches in a way that loses the event-loop reference. Use `node --import tsx` for long-running services; reserve `npx tsx` for one-shot CLI invocations.

### Verify "done" claims with your own shell

The skill's "QA the claim" pattern paid off — dev1 said Plan 1 was done; project_lead independently ran `npm test` and saw `60/60 tests passed across 22 test files`. The discipline catches overstatement before propagating up to the user. **Never relay "done" without your own verification.**

### Unit tests are not sufficient for UI plans — visual smoke is mandatory

Caught the hard way on Plan 2: I ran `pnpm test` (63/63 pass, 27 files), declared Plan 2 verified, started moving toward Plan 3. The user pushed back: visual smoke test was required and skipped.

The parent skill is explicit: *"any UI/visual dimension means open the relevant visual MCP before relaying success. Terminal output is not evidence of a working UI."*

For any plan with a frontend / web surface / visual output:

1. Unit tests pass (necessary).
2. **Spawn a subagent** to run the actual stack: build frontend → install demo site → start backend → open in browser via `claude-in-chrome` / `chrome-devtools-mcp` / Playwright / `screenmcp`. Take screenshots. Click around.
3. Subagent reports verdict: PASS / ISSUES (list) / FAIL.
4. Both 1 and 2 must pass before flipping the plan to verified.

For sustained QA (multi-page nav, many screenshots), use a **wrapped clicom child** rather than in-process Agent — visual MCP traffic accumulates fast in the supervisor.

#### Always rebuild SPA dist before smoke

When a frontend has a build step (Vite, Webpack, etc.) and the backend serves the build output, **always rebuild the dist before each smoke test**. Stale dist will mask source fixes. A previous run's dist sitting on disk is a foot-gun — the source can be fixed but the smoke test runs against the old bundle and reports "still broken".

Got bitten on Plan 2 smoke #3: dev1 fixed the source, the smoke agent's brief said *"build (skip if dist already current)"*, agent skipped, smoke reported FAIL on a fix that was actually correct. Smoke #4 with forced rebuild → PASS on the same source.

**Rule for smoke briefs:** drop the "skip if current" hedge. Either supervise the rebuild yourself before dispatching, or include `pnpm build` (no skip) as step 1 of the agent's brief.

#### Look at the screenshots yourself — agent reports describe what's there, not what's missing

This is the biggest lesson from the Plan 2 verification session.

A smoke subagent reported `PASS` after exercising the dashboard — login, panels, tabs all "render". I relayed PASS without viewing the screenshots. The user opened the screenshot and immediately saw 8 layout problems:

- View tab had no `<iframe>` — just raw JSON text where the dev preview should be
- Activity bar had 3 icons instead of 4 (Files missing)
- Side panel only stacked Pages — Files and Steps sections weren't there
- Chat panel had Project/Page/Editor tabs but **no input box at all**
- No Publish button anywhere
- Stray "Pretty-print" checkbox not in spec
- No breakpoint selector
- Two activity bar icons rendered with the same flag glyph

None of these caused the agent's automated checks to fail. The agent verified *response to interactions* (click Pages → no crash; type path → file opens; select element → returns selector). It did not verify *the layout matches the spec's described UI*.

**Why the agent missed it:**
- Agents describe what they see ("Pages panel with Add page button"), not what's absent ("no Files panel slot, no Steps section in this view").
- "Renders without errors" ≠ "matches the spec's layout".
- The agent's PASS was for a checklist of behaviors it could exercise, not for completeness of UI affordances.

**What I should do next time:**
1. **Always open the post-login screenshot myself** with the Read tool (it accepts images). Skim with the spec's layout description in mind. Look for negative space — what should be there but isn't?
2. **Brief agents with explicit "compare against spec layout" checks**, not just "does it render and respond". Pass the spec's layout section verbatim and ask the agent to enumerate every spec-described element with present/missing.
3. **Check icon counts and label distinctness** — agents often count "an activity bar with icons" without saying how many or whether they're visually distinct.
4. **Negative assertions matter as much as positive ones** — for selectElement, the spec implied "clicking selects without triggering target's handler". Smoke verified the positive (selector returned), missed the negative (button onclick suppressed). The user foresaw this; smoke didn't.

**Practical workflow that worked:**
1. Smoke agent runs full SMOKE_TEST.md → returns PASS/FAIL per checklist item.
2. Supervisor reads the post-login screenshot with the spec's layout section open.
3. Supervisor enumerates layout issues against the spec.
4. Bundle behavioral fixes + layout issues in a single fix brief for the dev.

**Validate each component visually ONCE — when it passes — then trust the tests:**

Visual smoke is high-fidelity but slow (15-25 min + 50-150k tokens). It's the only way to catch layout drift and missing affordances, but **don't pay it per iteration**.

- **First-time validation**: open the screenshot, check against spec, enumerate.
- **If the component passes**: future smoke can skip it; automated tests carry the load.
- **If it fails**: revalidate after each fix iteration until it passes. The "look once" rule applies *post-pass*, not pre-pass.
- Re-validate even passed components when **the component itself changes** — new layout, new affordance, refactor that touches rendering.

Smoke #5 was the failing validation pass. Smoke #6 verified the specific fixes. Once smoke #6 passes for those changes, those components fall under "look once" — future smokes skip them unless modified again.

#### Detecting scroll-and-empty-state issues — what an agent doesn't see in a single dashboard screenshot

Two issues a dashboard screenshot won't surface unless you specifically look for them:

**Scroll architecture.** A SaaS app shell should fill the viewport (`100vh`) with **internal scroll** on each panel — sidebars, lists, main content all scroll within their bounds. The chrome (activity bar, header, tabs) stays put. **Page-level scroll** instead is a smell — the whole layout grows beyond the viewport, the user has to scroll the document to reach panels, the chrome moves with the content.

How to detect, given screenshots and DOM:
- Look at the root layout: does the topmost container have `height: 100vh` or equivalent (`100dvh`, `100%`)?
- Do interior panels have `overflow: auto` / `overflow-y: auto` so they scroll within their box?
- Or does the body have its own scrollbar? (Smell.)
- Test: "if I shrink the window short, does the user lose access to the bottom of any panel?" Page-scroll = yes. App-shell = no, panels resize and scroll internally.

Brief layout-smoke agents to **check the scroll architecture explicitly**, not just "did the panel render": evaluate `getComputedStyle(document.body).overflow`, check if any interior panel has `overflow:auto`, resize the viewport and screenshot at multiple heights to confirm internal scroll behavior.

**Default / empty state.** The first thing a user sees on a fresh install — before they've added anything, before any data flows in. The View tab loading the iframe's 404 ("Route GET:/ not found") is a user-hostile default. Every panel and tab needs a deliberate empty state:
- Welcome / instruction copy
- A primary action (e.g., "+ Add your first page")
- Or a friendly "nothing here yet" with context, not a stack trace or HTTP error

How to detect:
- Run the smoke against a **truly empty** install (fresh `vibe-editor install` with no extra files added). The smoke fixture I used had a button, which masked the empty-state question.
- For each visible panel/tab: ask "what does this say when there's no data?" Find an error message, a stack trace, or just blank → empty-state work needed.
- Check that any error visible during normal first-load is the user's responsibility (e.g., "Set up your dev server URL"), not internal noise (e.g., "Route GET:/ not found").

**Add to layout-smoke briefs:**
- "What's the default state of each panel before any user data?"
- "Does the layout use page-level scroll, or do panels scroll internally?"
- "Resize the window to 600px tall — does the user lose access to anything?"

#### After it works, run two more lenses: convenience + aesthetic

Functional verification (unit tests + smoke) confirms things WORK. It doesn't confirm things are GOOD. Two more lenses to run before declaring a UI plan done:

**Lens 1 — Convenience (imagined-task walkthrough).** Pick 2-3 realistic user tasks and mentally perform them step-by-step. Note every friction. Example: *"preview my homepage at tablet size and check it fits"* — Click View, click Tablet preset, iframe is 768px in a 1000px area, no auto-fit → friction → needs zoom selector with fit option. Repeat for "refresh preview", "navigate to a different route", "find yesterday's commit". Surface the missing affordances.

The user pointed out one specific example (zoom selector + auto-fit on viewport change) by walking through this themselves. **Do this proactively** — don't wait for the user to enumerate each missing affordance.

**Lens 2 — Aesthetic critique.** Look at the screenshot and ask honestly: *"Does this look finished, or like a wireframe?"* Compare against production tools (Linear, Vercel, Figma, Stripe). Smell list:
- ⚠️ Vast unanchored empty regions → needs placeholder or condensed layout
- ⚠️ Same-weight headers everywhere → needs hierarchy via weight/size
- ⚠️ Inconsistent icon sizes → design pass needed
- ⚠️ Zero accent color, only neutrals → looks unfinished / wireframe
- ⚠️ Outlined-only active states (easy to miss) → use fill/background
- ⚠️ Floating placeholder text in giant empty panel → needs centering or proper empty state
- ⚠️ Icons without labels or tooltips → guessing UI
- ⚠️ No depth at all (everything flat, no borders/shadows) → reads as wireframe

Production tools have: 1-2 accent colors, weight-based hierarchy, 4/8/16px whitespace rhythm, subtle depth, single icon set, filled active states, status bars, deliberate use of negative space.

**Sequence matters:** Lens 1 + 2 come AFTER functional verification, not before. Polishing a wireframe that doesn't work yet is wasted effort. Apply once spec is met but before "done" declaration.

**Don't bundle these with bug fixes** — separate rounds. Bug-fix briefs are about correctness; convenience and aesthetic rounds are about quality. Mixing them confuses the dev.

#### Bundle small fixes — don't hot-fix one-at-a-time

Within a single category (layout, convenience, polish), small fixes should accumulate and be dispatched as a single round, not 5 separate single-fix briefs.

Each dispatch has overhead — read brief, build context, edit, test, commit, summarize. 5 separate single-fix dispatches = 5× the overhead. Bundling amortizes it.

**Workflow:**
- When a small issue surfaces (button cutoff, missing tooltip, color tweak): add to a "queued small fixes" list, mention it's queued, don't dispatch.
- Dispatch when: 3+ items accumulated, OR a natural seam (between major rounds), OR user explicitly asks to send what's accumulated.
- Cross-theme bundles are fine for very small stuff.

**Don't bundle (dispatch immediately):**
- A reported regression in just-shipped work — failure mode is different, fix now
- Anything blocking other work (test infra broken, build broken)
- Critical user-facing bugs

The user explicitly corrected me when I almost dispatched a single L11 (commit button cutoff) hot-fix — bundle with round 4.

#### Test mocks must match backend reality, not frontend assumption

When unit tests mock `fetch` to return a shape, that shape can lie. In Plan 2, several mocks returned bare arrays where the real backend returned `{pages: [...]}`. Tests passed (frontend bug + matching mock = "green") while the actual integration was broken.

**Practical defense:**
- Share types between backend and frontend (via a shared package or generated declarations). The compiler catches the mismatch.
- Or: have a small set of integration tests that stand up the real server in-process (Plan 2 had `mcpRoundtrip.test.ts` — that one would have caught it if it'd exercised pages.list specifically).
- When the visual smoke fails with a contract mismatch and unit tests pass: the test mocks are lying. Audit them against the backend tool's actual return shape.

#### Negative assertions are part of UX behavior

The user foresaw an issue: in selectElement mode, clicking a button to "select" it must NOT trigger the button's own onclick (no navigation, no submit, no SPA route change). Smoke #5 verified the *positive* (selection returns the right selector) but didn't verify the *negative* (button's text didn't change to "Clicked!" after select-click).

**Practical defense:**
- For any interactive surface, ask: "what should NOT happen if this works correctly?" Add it to the smoke checklist.
- Use a fixture page where every interactable element has an observable side-effect (text change, counter, console marker). After exercising the feature, assert the side-effect did NOT occur.

#### Combine tooling for hybrid OS/browser flows

Some flows can't be tested with chrome-devtools-mcp alone — Chrome's `getDisplayMedia` "Share this tab?" dialog is an OS-level window outside DevTools' reach. Using `screenmcp` to drive the OS mouse to click "Allow" lets the smoke agent cover the persistent-stream pattern end-to-end.

When briefing a smoke agent: name the specific MCP for each surface (chrome-devtools-mcp for in-page interactions; screenmcp for OS dialogs and mouse-on-iframe; both can be loaded together). Don't make the agent guess.

For pure-backend plans (like Plan 1), unit tests + a few curl calls is sufficient — there's no rendered surface to judge.

---

## clicom mechanics

### `clicom status` "idle" ≠ task complete

"idle" = no screen changes for some threshold. A wrapped agent thinking deeply (`xhigh effort`) can pause output briefly between thoughts and register as idle. **Always read the screen** to disambiguate "task done" from "thinking pause" from "actually crashed".

### `wait_idle` timeout is a positive signal during execution

If `wait_idle` times out, the agent is producing output — that's normal during heavy work. Don't treat the timeout as a failure; just read the screen.

### How to tell if a wrapped agent is truly idle (and how I got it wrong)

**Authoritative signal:** `clicom_status --partial <id>` returns `state: "idle"` or `state: "exited"` or `state: "died"`. This is the wrapper's own assessment based on output activity. Trust it.

**Confusing signal: on-screen spinners frozen from prior bursts.** I made this mistake — saw `"Tinkering… 23s · still thinking with xhigh effort"` on dev1's screen and reported "actively working", when in fact `clicom_status` was returning `state: "idle"`. The user had to correct me ("dev1 is idle").

Why the screen lied: clicom_screen captures the visible terminal buffer. When an agent finishes a long burst of activity, the last drawn frame includes its in-progress spinner. The terminal doesn't get a "clear those spinners" frame on stop — they sit there until the next user input redraws. **Visual spinners are not reliable proof of activity.**

**The real read: combine `state` + past-tense completion markers + summary text.**

| Signal | Meaning |
|---|---|
| `state: "idle"` | Agent's stdout has been quiet for the wrapper's threshold |
| `"Sautéed for 27s"` / `"Cogitated for 5m 50s"` / `"Cooked for ..."` (past tense) | A thinking session **ended** — visible elapsed time, no live spinner |
| `"Tinkering… 23s"` / `"Vibing…"` / `"Musing… still thinking with xhigh effort"` | A spinner — could be live OR frozen from a prior burst |
| `❯` empty prompt + `tokens` count | Waiting for input |
| Summary message starting with `●` followed by paragraph text | Agent posted a result; it's done |
| Pending typed text after `❯` (visible chars before any `\n`) | User typed something but didn't press Enter; agent isn't yet processing it |

**Decision rule for "is dev1 actually doing work?":**

1. Call `clicom_status --partial <id>`. If `state: "exited"` or `"died"` — done (or crashed).
2. If `state: "idle"`: read the screen and look for past-tense completion markers + visible result. If present → truly done waiting for next input.
3. If `state: "active"`: agent is producing output now. Read the screen to see what.
4. If only spinners visible and no result text and `state: "idle"` → agent stopped mid-flow without committing/summarizing. Often indicates it ran out of steam, hit an error, or got distracted (like dev1 did during round 5 when it stopped mid-polish to read its own screenshots).

**Don't:** rely on visual spinners alone. They lie about liveness.

**Do:** trust `clicom_status` first; cross-check screen for past-tense completion text + result summary; flag uncommitted work.

### Recovery from API 529 is a single-message nudge

dev2 hit Anthropic 529 (overloaded), exhausted 10/10 retries, gave up after 3m 33s. Conversation context survives in the wrapped agent — typing `"Retry now — the API overload should have cleared. Continue <task>."` resumed cleanly. **No need to re-brief from scratch.**

### `MCP server failed · /mcp` after `/reload-plugins`

Reloading plugins can knock out clicom's own MCP-into-the-wrapped-agent path. Visible as an error chip on the wrapped agent's screen. Doesn't kill the wrapped agent — but if you spawned it expecting MCP access into something else, that path may need re-establishing.

---

## Wakeup cadence

### The 5-minute cache window matters less when the user wants visibility

`SKILL.md` and the `ScheduleWakeup` doc both recommend 1200-1800s for idle ticks (avoid the cache miss without amortization at 300-1199s). The user in this session preferred 10-min (600s) to stay closer to progress.

**Per-skill rule "user instructions always take precedence" applies.** When the user asks for a tighter cadence, accept the cache cost.

### Fixed cadence → cron, not repeated `ScheduleWakeup`

When the user says **"wake every N minutes"** (a fixed cadence, not "self-pace"):

- **Use `CronCreate`**, not `ScheduleWakeup`. Cron is one register-and-forget call; ScheduleWakeup is a per-firing arming that must be re-scheduled at the end of every iteration.
- Drift accumulates with ScheduleWakeup-as-fake-cron. If you forget to re-schedule once (e.g., user interrupts your tail of the response), the loop dies silently. With cron, the next fire is independent of whether you remembered to re-arm.
- The `/loop` skill already routes correctly when the input has a leading interval token (`/loop 10m <prompt>` → fixed-interval / cron) vs. when it doesn't (`/loop <prompt>` → dynamic / ScheduleWakeup). **Tell the user to include the interval token** if they want fixed cadence — or convert yourself once they specify cadence in chat.

**Mental model:**
- Cron = "fire every X, no matter what." Use when cadence is the primary requirement.
- ScheduleWakeup = "fire once after Y, I'll decide what's next when I get there." Use when next-fire timing depends on observed state.

### Reschedule with the same prompt verbatim — but update it when intent changes

The /loop's prompt is the next-firing instruction. When the user changes direction mid-flight ("write all plans" → "develop in sequence"), update the /loop prompt on the next ScheduleWakeup so the future loop firings have the new logic. **Don't rely on the model "remembering" — encode it in the prompt or memory.**

---

## Communication shape

### Status reports stay tight when they're tabular

Repeated status updates in this session worked best as a 4-row table (`Plan 1: done, verified · Plan 2: writing, dev2 at 286k · ...`) plus one line on the next wakeup. Prose status updates buried the actionable bit.

### Surface gates and decisions, not micromanagement

When briefing agents, explicit gates ("Do this BEFORE that"; "trigger only when X is true") matter more than play-by-play. The Plan 3/4 rewrite memory has a two-condition gate (Plan 2 developed AND Plan 2 verified) so future-self acts only when both are true.

### User-typed direct input — distinguish from ghost-suggestion (you can't see color)

The user types instructions directly into wrapped agents at first-person speed — observed multiple times ("option 2, write plan 2", "write Plan 5"). But Claude Code's TUI also renders **history-suggestion ghost-text** in a dim color when the input is empty — and `clicom_screen` strips color, so a real typed message and a faded suggestion read identically in the supervisor's view.

**The detection rule (color-blind safe):**

1. `clicom_screen` — check for text near the `❯` prompt.
2. If present, **wait ~60s and re-read**.
3. If text **changed** (different content, longer/shorter): the user is actively typing. Wait another minute. Once the text stabilizes (user stops between keystrokes), treat as real intent — read it, decide whether to submit, append, or supersede your action.
4. If text is **unchanged after a minute**: assume it's a ghost-suggestion (a real human paused mid-type for a full minute would either keep going or send Enter). Send `[Escape]` and/or a few `[Backspace]` keys (no-op on empty input, dismisses the suggestion if present), then `clicom_type` your own command and `clicom_keys "[Enter]"`.

**Concrete cases that taught this rule (post-/clear of supervisor):** dev1 showed `❯ check git log` and dev2 showed `❯ write plan 5` — both unchanged across multiple wakeups. They were history hints, not user intent. The earlier "wait then integrate" rule paralyzed the supervisor on phantoms.

**Why the unchanged → ghost heuristic is safe:** real user typing is bursty (changes every few seconds). Real typed-but-abandoned content is rare. Even when it does happen, clearing-then-overwriting an abandoned half-typed message is far better than indefinitely refusing to dispatch new work.

**Don't apply to:**
- Text actively changing across two reads (60s apart) — real user typing, do not clear.
- Empty `❯` — just type your command.
- Text the user explicitly told you to ignore or proceed past.

---

## Anti-patterns observed

| Anti-pattern | Cost paid | Fix |
|---|---|---|
| Wrote all plans first, then developed | ~280k tokens of pre-written plans now need rewriting against actual code | Develop in sequence; max one plan written ahead |
| Long-paste brief via `clicom_type` | dev1 saw only the last fragment of the brief — entire instruction lost | Write brief to a `.md` file, type a one-liner pointing at it |
| `npx tsx` for long-running serve | Background task exited with code 0 immediately, server unreachable | `node --import tsx -e "..."` with explicit keep-alive |
| Trusted dev's "done" claim w/o running tests | (Avoided this time, but the temptation is real after a confident summary) | Run `npm test` (or equivalent) yourself before relaying |
| Tried to clear typed input with `Ctrl+U` / Backspaces | Fragmented input persisted, risk of accidental submission later | If user typed first, work with their text; don't fight the TUI |
| Set 600s wakeup against skill's 1200s rec | (User explicitly asked) | Skill is default; user override wins |
