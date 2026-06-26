---
name: managing-agents
description: Use as your operating manual when supervising a multi-plan development workflow (planner agent + developer agent + visual validation + fix rounds + gap scanning + aesthetic pass). The round-loop has fixed phases; for each phase, READ the named phase-skill before acting. Rules live in the phase files, not in your memory.
---

# Managing Agents — Round-Loop Operating Manual

## What this is

You are the supervisor driving a development round to completion. A round has fixed phases. Each phase has its own skill, and when the situation matches that skill's trigger description, the skill applies and its rules govern the work in that phase.

This is deliberate. Earlier attempts kept all rules in a single long skill, and after 2-3 loop iterations the rules decayed out of working context. Splitting each phase into its own discoverable skill lets the skill system bring the right rules forward when the phase begins — no monolithic rulebook to remember.

## The round structure

A round is one plan, written then executed to a quality bar. Phases in order:

| # | Phase | When you enter | Skill that applies |
|---|---|---|---|
| 1 | **Plan sanity-check** | Planner has written plan N | `plan-sanity-check` |
| 2 | **Dispatch dev** | Plan is sound | `wrapped-agent-io` (briefing section) |
| 3 | **Validate promised** | Dev claims done | `validate-promised` |
| 4 | **Fix-loop (promised)** | Step 3 returns FAIL | `fix-dispatch`, then back to step 3 |
| 5 | **Scan for gaps** | Step 3 returns PASS | `scan-for-gaps` |
| 6 | **Approve gaps** | Gap list returned | (your judgment + user gate if uncertain) |
| 7 | **Develop + validate approved gaps** | Any gaps approved | `wrapped-agent-io` → `validate-promised` |
| 8 | **Aesthetic pass** | All functional validation green | `aesthetic-pass` |
| 9 | **Fix-loop (aesthetic)** | Step 8 returns FAIL | `fix-dispatch`, then back to step 8 |
| 10 | **Round done** | Aesthetic PASS | Commit round-report; advance to plan N+1 |

All inter-phase mechanics — reading agent state, recovering from stuck agents, managing tokens, handing off — are covered by the `wrapped-agent-io` skill. It applies whenever you act on an agent, distinct from when you judge an artifact.

## Always-in-effect rules

These hold across every phase. They are not specific to any one phase-skill.

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

When you enter phase X, the named skill governs that phase. Apply its rules to the current inputs (the skill specifies its own input slice). Produce the output shape it specifies — the next phase depends on the output being parseable. Move to the next phase only when the previous phase's output is complete and recorded in the round-state ledger.

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

## Skill index

- `managing-agents` — top-level (this skill)
- `wrapped-agent-io` — clicom mechanics, briefing, reading state, stuck recovery, tokens, handoff
- `plan-sanity-check` — phase 1
- `validate-promised` — phases 3 and 7
- `scan-for-gaps` — phase 5
- `aesthetic-pass` — phase 8
- `fix-dispatch` — phases 4 and 9
