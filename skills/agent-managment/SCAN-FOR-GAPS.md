---
name: scan-for-gaps
description: Use after promised features have passed validation, to find affordances the plan missed before declaring the round done. Convenience lens — walk through 2-3 realistic user tasks mentally, surface missing affordances the plan didn't enumerate, return a list (not yes/no) for approval. Catches things the plan didn't think to ask for that the user clearly wants.
---

# Scan For Gaps

## When you invoke this skill

`VALIDATE-PROMISED.md` returned PASS. The features the plan promised are delivered. **Now find what the plan missed.**

Functional verification (unit tests + smoke) confirms things WORK. It doesn't confirm things are GOOD or COMPLETE. The user's true intent often includes affordances the plan didn't enumerate — they're "obvious" from the user's perspective and so didn't make it into the plan.

**Be proactive.** Don't wait for the user to enumerate each missing affordance. Walk through tasks yourself.

## The lens: imagined-task walkthrough

Pick **2-3 realistic user tasks** for the surface the plan delivered. Perform each task mentally, step by step, as a real user would. Note every friction.

A "realistic" task is:

- Something a user would do in the first week of using the feature.
- Specific — not *"use the editor"*, but *"preview my homepage at tablet size and verify it fits"*.
- Has a clear success state.

**Walkthrough format per task:**

```
Task: <one-line description>
Steps:
  1. <user action> → <what happens, what they see>
  2. <user action> → <what happens, what they see>
  ...
  N. <success state reached, OR friction blocks reaching it>
Frictions encountered:
  - <missing affordance, awkward step, surprise>
  - ...
```

**Example** (preview-surface plan):

```
Task: preview my homepage at tablet size and check it fits.
Steps:
  1. Click View tab → iframe loads at default desktop width.
  2. Look for a tablet preset → no viewport selector visible. Friction.
  3. Manually resize browser? → still showing 1280px iframe in a smaller area.
  4. ... never reach success state.
Frictions:
  - No viewport-size selector (mobile / tablet / desktop / fit).
  - No auto-fit when viewport changes.
  - User can't accomplish the task with the delivered UI.
```

This walkthrough surfaces what the plan missed: a zoom selector with auto-fit option.

## What kinds of gaps to look for

- **Missing affordances for obvious tasks** — zoom, refresh, navigate-back, search, filter.
- **Friction in the golden path** — too many clicks, redundant confirmations, no feedback.
- **Missing recovery paths** — what if the iframe fails to load? what if the URL is wrong? what if the file doesn't exist?
- **Visible "what next?"** — after completing one task, does the UI suggest the next thing?
- **Visible "where am I?"** — breadcrumbs, current selection highlighted, status of in-progress work.
- **State you can't tell from looking** — is this saved? is this published? what version? when was this last refreshed?

## What NOT to surface here

This is the convenience lens, not the aesthetic lens, not the bug lens.

- **Aesthetic issues** → `AESTHETIC-PASS.md` (later phase).
- **Bugs in delivered features** → `VALIDATE-PROMISED.md` (already passed; if you find one now, loop back to that phase).
- **Speculation about future features the user didn't hint at.** Stay close to the delivered surface — the gap is the missing affordance, not the missing feature.

## Output: a list, never yes/no

Returning `{found: bool}` loses signal. Always return a list (which may be empty). Each item is a proposal for the approval gate to decide on.

```
Gaps proposed for plan <N>:

1. <gap one-liner>
   Rationale: <why a user would want this; which task it unblocks>
   Severity: blocker | high | medium | low
   Effort estimate: small | medium | large
   Proposed work: <one-line description of what implementing it looks like>

2. ...

(If no gaps: return empty list with one-line note "imagined tasks completed without friction".)
```

The approval gate (supervisor or user, depending on round-state ledger authorization) decides which to develop. Severity + effort lets the gate weigh quickly.

## Approval gate guidance

When deciding which gaps to approve for development:

- **Blockers** that prevent a realistic task from completing → almost always approve.
- **High severity + small/medium effort** → usually approve; high value-per-cost.
- **Medium severity + large effort** → defer to user; not obvious worth.
- **Low severity** → log but defer unless trivial.

If the user has standing authorization (round-state ledger says *"drive autonomously"* or similar), you can approve blockers + high-severity-small-effort yourself. Otherwise surface the list to the user with your recommendation.

**Always log dropped gaps in the round-state ledger.** Don't silently discard — the user may want to revisit.

## Anti-patterns

| Anti-pattern | Cost | Fix |
|---|---|---|
| Waiting for the user to enumerate missing affordances | Round drags; user does the supervisor's job | Walk through tasks proactively; surface gaps before the user does |
| Returning yes/no instead of list | Loses signal; approval gate can't weigh | Always a list; empty list is fine |
| Suggesting features the user never hinted at | Scope creep | Stay close to delivered surface and walkthrough frictions |
| Mixing aesthetic complaints into the gap list | Confuses the dev when the gap-development brief lands | Aesthetic goes in `AESTHETIC-PASS.md`; this list is functional affordances only |
| Walkthrough on idealized data only | Misses the realistic-friction problem | Use realistic data: long names, real URLs, edge cases |
| Approving every proposed gap | Scope creep at the supervisor's hand | Apply the severity/effort rule; defer or drop when not obviously worth it |

## After gaps are listed

1. Apply the approval gate. Record approved + deferred + dropped in round-state ledger.
2. For approved gaps: dispatch the developer per `WRAPPED-AGENT-IO.md` briefing patterns.
3. After dev reports done: re-enter `VALIDATE-PROMISED.md` scoped to the gap features only.
4. Once all approved gaps validate PASS, proceed to `AESTHETIC-PASS.md`.

Per-gap fix-loop cap: 2 attempts (per `SKILL.md`). If a single gap fails twice, drop it from approved and log the reason.
