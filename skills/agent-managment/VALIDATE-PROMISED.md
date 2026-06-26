---
name: validate-promised
description: Use when the developer agent reports done on a plan (or on approved gap features) and the delivered work needs validation against what was promised. Visual validation against the plan's promised feature list — supervisor must look at screenshots, not trust agent reports; compare layout against spec, enumerate elements present vs missing; detect smoke-agent workarounds; verify negative assertions; check scroll architecture and empty states. Output: PASS or FAIL with issues + evidence references.
---

# Validate Promised Features

## When you invoke this skill

The developer agent says "done" — tests pass, features implemented. Before relaying success or moving to the gap-scan phase, validate against what the plan promised. **Terminal output is not evidence of a working UI.**

Also invoked after gap-feature development (phase 7) — scoped to the gap features only.

## Inputs

- The plan's promised-features list (from the round-state ledger, extracted during `PLAN-SANITY-CHECK.md`)
- The developer's claim summary (read from `clicom_screen_after_re` using the per-agent prefix, per `WRAPPED-AGENT-IO.md`)
- Fresh screenshots of every UI surface the plan touched, taken AFTER rebuilding the SPA dist

## Step 0: Rebuild dist before every smoke

A stale dist masks fresh source fixes. **Always rebuild the bundle before taking validation screenshots.**

**Mandatory steps before judging:**

1. Run `pnpm build` (or project's equivalent) — no "skip if current".
2. Start the backend service from a clean state — preferably a worktree at HEAD, not the dev's WIP directory (file locks + TS build conflicts).
3. Open the surface in `claude-in-chrome` / `chrome-devtools-mcp`.
4. Take screenshots of every promised surface.

Either supervise the rebuild yourself before validation, or include `pnpm build` as step 1 of the smoke agent's brief (drop the "skip if current" hedge).

## Step 1: Smoke agent runs the checklist

Dispatch a smoke subagent with an explicit *"compare against spec layout"* brief — not just *"does it render and respond"*. Pass the spec's layout section verbatim and require the agent to enumerate every spec-described element with `present / partial / missing`.

**Brief constraints (mandatory):**

- **"Do not modify state to make a check pass."** If a check fails because preconditions are missing, REPORT the missing precondition as a FAIL — don't fabricate it.
- **Compare against spec layout, not just behavior.** Enumerate each spec-described element with status.
- **Check icon counts and label distinctness** — agents often count "an activity bar with icons" without saying how many or whether they're visually distinct.
- **Check negative assertions** — for any interactive surface, what should NOT happen if this works correctly?
- **Use a fresh git checkout / truly empty install** where applicable, to surface empty-state issues.

The smoke agent returns: per-checklist verdicts + screenshots.

## Step 2: YOU look at the screenshots — non-negotiable

The biggest single failure mode in this phase: smoke agent reports PASS, supervisor relays PASS without viewing screenshots, user opens screenshot and immediately spots 8 layout problems.

**Why the agent misses things:**

- Agents describe what they see (*"Pages panel with Add page button"*), not what's absent (*"no Files panel slot, no Steps section"*).
- "Renders without errors" ≠ "matches the spec's layout".
- The agent's PASS is for behaviors it could exercise, not for completeness of UI affordances.

**What you do:**

1. **Open the post-action screenshot with the Read tool** (it accepts images).
2. **Skim with the plan's layout section open in another pane.**
3. **Enumerate against the spec:**
   - Every spec-described element: present, missing, or wrong?
   - Icon count matches spec?
   - Icons visually distinct (not two with same glyph)?
   - Labels / tooltips on icons?
   - Negative space — what should be there but isn't?
4. **Check for smells specific to spec deviation:**
   - View tabs without iframes (raw JSON instead of preview)
   - Side panels missing sections promised in spec
   - Chat panels without input boxes
   - Missing primary action buttons (Publish, Save, etc.)
   - Stray controls not in spec
   - Same-glyph icons that should be distinct

## Step 3: Detect smoke-agent workarounds (the insidious failure)

A smoke agent can fake a PASS by working around the bug — creating a missing file, setting a missing config, manually triggering something the system was supposed to do. The screenshot shows the workaround state, which looks indistinguishable from a real PASS.

**Anti-pattern signatures in agent reports:**

- *"Cleanup: created/removed/added X to enable Y to succeed"*
- *"Note: had to manually..."* or *"Suggestion: also seed..."*
- *"Worked end-to-end after [agent intervention]"*
- Any verb in the agent's Evidence section that is the agent doing something a normal user wouldn't.

**Defenses (apply every smoke):**

1. **The brief already says "do not modify state."** If the agent did anyway, that's a yellow flag in the report.
2. **Read the agent's Evidence + Notes + Cleanup sections** for verbs the agent performed. Any line where the agent says it created / wrote / modified state — verify whether the original system would work without that intervention.
3. **The "untouched test" question:** would this still PASS if I re-ran the smoke from a fresh git checkout with NO setup commands? If no, that's a FAIL.
4. **Cross-validate critical paths with two independent agents.** Especially for first-validation. A follow-up smoke from a different agent / clean checkout often catches what the first papered over.

A passing smoke is not the same as a working feature. The screenshot is the state the agent put the system into; the bug is whether the user's path arrives at that state.

## Step 4: Check scroll architecture and empty state

Two issues a dashboard screenshot won't surface unless you look for them specifically.

### Scroll architecture

A SaaS app shell should fill the viewport (`100vh`) with **internal scroll** on each panel. **Page-level scroll** is a smell — the whole layout grows beyond the viewport, chrome moves with content.

**Check:**

- Root layout: does the topmost container have `height: 100vh` / `100dvh` / `100%`?
- Interior panels: `overflow: auto` / `overflow-y: auto`?
- Body has its own scrollbar? → smell.
- Resize viewport short. Page-scroll = user loses access to bottom of panels. App-shell = panels resize and scroll internally.

Brief layout-smoke agents: evaluate `getComputedStyle(document.body).overflow`, check interior `overflow:auto`, screenshot at multiple viewport heights to confirm internal scroll behavior.

### Default / empty state

First thing a user sees on a fresh install — before they've added anything. A View tab loading an iframe's 404 is user-hostile.

**For each visible panel/tab, ask:** *"what does this say when there's no data?"* Error message, stack trace, blank → empty-state work needed.

Every panel and tab needs a deliberate empty state: welcome / instruction copy, a primary action (e.g., `+ Add your first page`), or a friendly *"nothing here yet"* with context — never a stack trace or HTTP error.

## Step 5: Verify negative assertions

For interactive surfaces: what should NOT happen when this works correctly?

Example: in select-element mode, clicking a button to "select" it must NOT trigger the button's onclick (no navigation, no submit, no SPA route change). Positive smoke (selection returns the right selector) doesn't verify the negative.

**Practical defense:**

- For any interactive surface, write the negative assertion in the smoke checklist alongside positive ones.
- Use a fixture where every interactable has an observable side-effect (text change, counter). Assert side-effect did NOT occur.

## Step 6: Test mocks must match backend reality

If unit tests mock `fetch` to return a shape, that shape can lie. If frontend bug + matching mock = green, while the real backend returns a different shape → integration is broken but tests pass.

**Practical defense:**

- Share types between backend and frontend.
- Have integration tests that stand up the real server in-process.
- When visual smoke fails with a contract mismatch and unit tests pass: the mocks are lying. Audit them against backend tool's actual return shape.

## Look once, then trust

Visual smoke is high-fidelity but slow (15-25 min + 50-150k tokens). Don't pay it per iteration:

- **First-time validation of a component:** do the full enumeration.
- **If component passes:** future smoke can skip it; automated tests carry the load.
- **If it fails:** revalidate after each fix iteration until it passes.
- **Revalidate even passed components when the component itself changes** — new layout, new affordance, refactor that touches rendering.

## Output

```
Verdict: PASS | FAIL

Issues (omit if PASS):
1. [<severity: blocker | high | medium | low>] <one-line description>
   Evidence: <screenshot path or DOM excerpt or test output>
   Source: <smoke checklist item | supervisor enumeration | smell list | negative assertion | workaround signature>
   Expected: <what success looks like>

2. ...
```

- **PASS** → record in round-state ledger; proceed to `SCAN-FOR-GAPS.md`.
- **FAIL** → proceed to `FIX-DISPATCH.md` with the issues list. Re-enter this skill after fix is reported done. Track fix-count per `SKILL.md` escalation caps (3 for promised-features fix-loop).
