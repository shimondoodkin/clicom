---
name: managing-agents
description: Use as your operating manual when supervising a multi-plan development workflow (planner agent + developer agent + visual validation + fix rounds + gap scanning + aesthetic pass). The round-loop has fixed phases; for each phase, READ the named phase-skill before acting. Rules live in the phase files, not in your memory.
---

# Managing Agents — Round-Loop Operating Manual

## What this is

You are the supervisor driving a development round to completion. A round has fixed phases. **For each phase, read the named skill file before acting in that phase.** The rules for judging that phase live in the file — you do not need to remember them across phases or across rounds.

This is deliberate. Earlier attempts kept all rules in a single long skill; after 2-3 loop iterations, the rules decayed out of the supervisor's working context and steps got skipped. The split-by-phase structure resets the rule context every phase.

## The round structure

A round is one plan, written then executed to a quality bar. Phases in order:

| # | Phase | When you enter | Read this skill before acting |
|---|---|---|---|
| 1 | **Plan sanity-check** | Planner has written plan N | `PLAN-SANITY-CHECK.md` |
| 2 | **Dispatch dev** | Plan is sound | `WRAPPED-AGENT-IO.md` (briefing section) |
| 3 | **Validate promised** | Dev claims done | `VALIDATE-PROMISED.md` |
| 4 | **Fix-loop (promised)** | Step 3 returns FAIL | `FIX-DISPATCH.md`, then back to step 3 |
| 5 | **Scan for gaps** | Step 3 returns PASS | `SCAN-FOR-GAPS.md` |
| 6 | **Approve gaps** | Gap list returned | (your judgment + user gate if uncertain) |
| 7 | **Develop + validate approved gaps** | Any gaps approved | `WRAPPED-AGENT-IO.md` → `VALIDATE-PROMISED.md` |
| 8 | **Aesthetic pass** | All functional validation green | `AESTHETIC-PASS.md` |
| 9 | **Fix-loop (aesthetic)** | Step 8 returns FAIL | `FIX-DISPATCH.md`, then back to step 8 |
| 10 | **Round done** | Aesthetic PASS | Commit round-report; advance to plan N+1 |

All inter-phase mechanics — reading agent state, recovering from stuck agents, managing tokens, handing off — live in `WRAPPED-AGENT-IO.md`. Consult it when acting on an agent, not when judging an artifact.

## Always-in-effect rules

These hold across every phase. Re-read this section if you find yourself reasoning about agent management without re-reading.

1. **You decide; you don't escalate by default.** Wrapped-agent questions about API/UX/copy/defaults are judgment calls you should make. Escalate only for irreversible forks (scope, money, legal, security, privacy), missing context you can't infer, or destructive consent.
2. **The user's intent is the anchor.** Capture verbatim phrasing at the start of the round. Re-read it before each judgment phase. If a wrapped agent argues a judgment, quote the user's phrasing back.
3. **Terminal output is not evidence of a working UI.** Every "done" claim involving a visual surface must be checked with a visual MCP (`claude-in-chrome` / `chrome-devtools-mcp` / `screenmcp`). Never relay PASS based on terminal text.
4. **Fresh evidence per validation.** Rebuild the SPA dist before every visual validation. A stale dist masks fresh source fixes.
5. **Look at screenshots yourself.** Smoke-agent reports describe what's there, not what's absent. Supervisor eyes on the post-action screenshot is non-negotiable.
6. **Sequence: functional → aesthetic.** Don't polish a wireframe that doesn't work yet.

## Fix-loop escalation caps

After each fix-loop iteration, you decide whether to continue. Hard caps:

- **Validate-promised fix-loop:** 3 failed attempts → escalate to user with verdict history verbatim. Don't keep dispatching.
- **Aesthetic fix-loop:** 2 failed attempts → escalate. (Aesthetic fixes are smaller; if they aren't landing, the brief is wrong, not the dev.)
- **Gap-development per gap:** 2 failed attempts on one gap → drop the gap from approved, log the reason.

Escalation format: paste the user the verdict history verbatim (no paraphrase). Let them decide: retry with different brief, abandon the issue, manual fix, or drop the plan.

## Phase invocation discipline

When entering phase X:

1. **Pause.** Read `<PHASE-NAME>.md` in full. Do not rely on memory of last round's pass.
2. Apply its rules to the current inputs only. The skill specifies its own input slice and output shape.
3. Produce output in the shape the skill specifies. Don't skip the output shape — the next phase depends on it being parseable.
4. Move to the next phase only when the previous phase's output is complete and recorded in the round-state ledger.

## Round-state ledger

Maintain a short plaintext ledger. Refresh on every phase transition.

```
Round: plan-<N>
Phase: <current>
Dev: <wrapped-agent-id> @ <token-count>
Validate-promised fix-count: <n>/3
Aesthetic fix-count: <n>/2
Gaps approved: <count>; developed: <count>
Last verdict: <one line summary | none>
User intent (verbatim): <quote>
```

Keep it ≤ 8 lines. It's your re-anchoring tool when a phase takes long enough that you need to reload context.

## When NOT in a round (free-form supervision)

This skill set is for the repeating round-loop only. For situations not on the phase map — bootstrapping a project, recovering from cross-round drift, ad-hoc Q&A from a dev outside a phase, novel architecture decisions — see the archived skills at `../agent-managment-initial/GENERAL-TASK.md` and `LESSONS.md`. They are reference material, not the operating manual.

If you find yourself in a phase but the phase-skill doesn't cover the situation, that's a signal: either you're not actually in that phase, or the situation is novel and belongs in free-form territory. Decide before acting.

## File index

- `SKILL.md` — this file (top-level)
- `WRAPPED-AGENT-IO.md` — clicom mechanics, briefing, reading state, stuck recovery, tokens, handoff
- `PLAN-SANITY-CHECK.md` — phase 1
- `VALIDATE-PROMISED.md` — phases 3 and 7
- `SCAN-FOR-GAPS.md` — phase 5
- `AESTHETIC-PASS.md` — phase 8
- `FIX-DISPATCH.md` — phases 4 and 9
