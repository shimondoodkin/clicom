---
name: script-driven-rounds
description: Use when supervisor work has settled into a repeating loop (write plan → develop → validate → fix → gap-scan → fix → aesthetic → done) and the wrapping agent is forgetting rules between iterations. Encode the loop as a script with bounded, stateless LLM judgment nodes; rules live in script logic + prompt templates, not in agent memory.
---

# Script-Driven Rounds

Companion to `GENERAL-TASK.md`. That skill covers free-form supervision — judging novel situations, defending intent, recovering from stuck agents. **This skill covers what to do when the supervision becomes repetitive.**

## The failure this fixes

The supervisor is running a stable, repeating workflow per plan:

1. planner writes plan N
2. dev develops plan N
3. visual validation of features promised in the plan
4. if problems → fix-round → re-validate (loop)
5. scan for needed features that the plan missed → approve some → develop them
6. validate the approved gap-features
7. aesthetic critique → fix-round → re-check (loop)
8. round done, advance to plan N+1

After 2–3 rounds, the supervisor's working context has filled with screen-reads, dev transcripts, validation output, fix dispatches. Rules from `GENERAL-TASK.md` and `LESSONS.md` get pushed out or auto-summarized. The supervisor starts:

- skipping steps (forgot to run aesthetic pass)
- rubber-stamping verdicts (smoke agent says PASS, didn't open screenshot)
- inconsistent lens use (PM-lens on plan 3 but not plan 4)
- relying on stale state ("the dev said it's done last round, probably still true")

These aren't bad-rule problems. The rules are sound. They're storage problems — rules in long-lived agent context decay.

## The shift

Don't run this loop as a free-form supervisor. Decompose it:

- **A script** owns control flow, retries, transitions, state.
- **Bounded LLM judgment nodes** — each call is stateless: `(rules + state slice) → verdict`.

What was "the agent must remember to do X" becomes either:

- **Script logic** — `if !verdict.pass: dispatch_fix()`. Forgetting is impossible because the agent isn't deciding.
- **Prompt template** for one node — e.g. `validate-promised.md` describes how to judge promised features, injected as the system prompt of one fresh call. The agent never had to "remember" it across rounds.

No long-lived orchestrator. Each judgment node = one cold invocation with a small input.

## Round-loop, scripted

Pseudocode (translate to Rhai for `clicom_queue`, TS, Python, whatever your harness supports):

```pseudocode
fn run_round(plan_path, dev_id) {
    let state = init_state(plan_path, dev_id);   // see "state" below

    // 1. Develop
    dispatch_dev(state.plan, dev_id);
    wait_until_dev_idle(dev_id);
    state.delivered = snapshot_delivered(dev_id);

    // 2. Validate promised features (fix-loop)
    let verdict = judge("validate-promised", state);
    while !verdict.pass {
        if state.fix_count.validate >= MAX_FIX { escalate_to_user(verdict); return; }
        dispatch_fix(dev_id, verdict.issues);
        wait_until_dev_idle(dev_id);
        state.fix_count.validate += 1;
        verdict = judge("validate-promised", state);
    }

    // 3. Gap scan → approval gate → develop approved → re-validate
    let gaps = judge("scan-for-gaps", state);      // returns list, NOT yes/no
    let approved = approve_gaps(gaps);             // human gate, or rules-based
    for gap in approved {
        dispatch_dev_gap(dev_id, gap);
        wait_until_dev_idle(dev_id);
        validate_loop(state.with_scope(gap), MAX_FIX);
    }

    // 4. Aesthetic pass (fix-loop)
    let aesth = judge("aesthetic-pass", state);
    while !aesth.pass {
        if state.fix_count.aesthetic >= MAX_FIX { escalate_to_user(aesth); return; }
        dispatch_fix(dev_id, aesth.issues);
        wait_until_dev_idle(dev_id);
        state.fix_count.aesthetic += 1;
        aesth = judge("aesthetic-pass", state);
    }

    finalize(state);  // commit verdicts, write round-report, advance
}
```

`dispatch_*` and `wait_until_*` are thin wrappers over clicom (`clicom_type`, `clicom_wait_idle`, `clicom_status`, `clicom_screen_after_re`). `judge(node_name, state)` is the only LLM invocation — see below.

## State the script carries (durable, file-backed)

Persist per round in `.round/<plan_id>/state.json` (or equivalent). The script reads/writes; LLM nodes consume slices.

| Field | Purpose |
|---|---|
| `plan.path`, `plan.content` | Source of truth for promised features |
| `plan.promised` | Extracted list of deliverables (one entry per checkable item) |
| `dev.id` | wrapped agent identifier for clicom |
| `delivered.screenshots[]` | post-dev visual evidence per surface |
| `verdict.history[]` | every node's verdict in order, with timestamps |
| `fix_count.{validate,aesthetic}` | retry counters; gates `MAX_FIX` |
| `gaps.proposed[]`, `gaps.approved[]` | output of scan-for-gaps + the approval decisions |
| `round.status` | `developing | validating | gap-scan | aesthetic | done | escalated` |

Round state IS the memory. The supervisor never has to "remember" anything across nodes — it re-reads the file.

## Judgment nodes

One prompt template per node, one stateless invocation per call. Bounded input, bounded output (JSON).

| Node template | Input slice | Output |
|---|---|---|
| `prompts/validate-promised.md` | `plan.promised`, `delivered.screenshots`, dev's claim summary | `{pass: bool, issues: [{description, evidence_ref}]}` |
| `prompts/scan-for-gaps.md` | `plan`, `delivered.screenshots`, persona context | `{gaps: [{description, rationale, severity}]}` — never yes/no, always a list |
| `prompts/aesthetic-pass.md` | `delivered.screenshots` only | `{pass: bool, issues: [{location, smell, severity}]}` |
| `prompts/approve-gaps.md` (optional, if not human) | `gaps.proposed`, `plan`, `scope_budget` | `{approved: [...], deferred: [...], reasons}` |

**How to invoke each node, in this environment:**

- **Preferred:** spawn a fresh wrapped Claude Code via `clicom_exec_detached`, brief it with one line — `"Read prompts/<node>.md and .round/<plan_id>/state.json. Write your verdict to .round/<plan_id>/verdicts/<node>-<n>.json. Exit when done."`. Wait for the verdict file. Kill the wrapper.
- **Acceptable for short judgments** (≤ a few hundred tokens in + out, no MCP needed): in-process `Agent()` call with the prompt-template content + state slice as the brief. Cheaper, no spawn overhead. Use for aesthetic-pass and approve-gaps.
- **Never:** call back into the supervising agent's own context to judge. That's what we're trying to escape.

**Per-node prompt template rules** (write these into each `prompts/<node>.md`):

- State the rules inline. No "see LESSONS.md" — the node has no memory of LESSONS.md.
- Specify the output schema explicitly with one example. Parseable, not prose.
- Bound the input by reference, not by paste — `state.json` carries the slice the node needs.
- For visual nodes: tell the node which screenshot tool to use and what to compare against (cf. `LESSONS.md` line 222 — "look at screenshots yourself").

## Splitting `GENERAL-TASK.md` / `LESSONS.md` rules

Audit every rule. Each one goes into exactly one of three buckets.

| Goes into | Examples from existing skills |
|---|---|
| **Script logic** (enforced by code) | `wait_idle` before every `type` (wrap `dispatch_*` with it); if validation fails 3× → escalate (`MAX_FIX` counter); destructive prompts halt (script regex on screen → user gate); rebuild SPA dist before smoke (script step before `dispatch_dev` for UI plans); ghost-text detection (compare two screen-reads ≥60s apart in code) |
| **Per-node prompt** (rules in template, injected fresh) | "enumerate missing affordances, not just present ones" → `validate-promised.md`; "agents describe what's there, not what's absent — list spec-described elements with present/missing" → `validate-promised.md`; aesthetic smell list (line 332–340 of LESSONS) → `aesthetic-pass.md`; "don't modify state to make a check pass — fail-and-report" → every validation node |
| **Stays in `GENERAL-TASK.md`** (free-form supervisor judgment) | Defending intent on novel scope; handling stuck-after-API-error states; spawn-and-drive ad-hoc peers; QA the claim for one-off non-recurring work; recovery from `MCP server failed` |

**Audit heuristic:** can the rule be expressed as `if X then Y`? → script logic. Does it tell the LLM how to judge a specific kind of artifact? → prompt template. Is it about reading a novel situation? → stays free-form.

Rules in script + prompts are **enforced** — they fire every time. Rules in prose are **aspirational** — the agent may forget.

## What survives, what doesn't

This pattern only works if the script + prompts + state file outlive any single agent context.

- **Script lives in code** (`scripts/round.rhai` or `scripts/round.ts`). Versioned in git. `/clear` and handoffs are irrelevant to it.
- **Prompt templates live as files** in `prompts/`. Versioned. Read fresh per node invocation.
- **State lives on disk** per round, in `.round/<plan_id>/`. Replayable, inspectable.

When the supervisor's context is cleared (per `LESSONS.md` line 70-115 self-handoff dance), the script and prompts and state are unaffected. The fresh supervisor reads the handoff and resumes the script — it doesn't need to reconstruct the rules.

## When to apply this skill

- The supervision workflow has been repeated 2+ times and is stable in shape.
- You've caught yourself or the supervisor forgetting a rule mid-round.
- You can name each step concretely and write its node template in one sitting.

## When NOT to apply (stay free-form per `GENERAL-TASK.md`)

- One-off work, exploration, novel architecture.
- The "shape" of a round is still being discovered — a script encodes the wrong shape and resists change.
- Fewer than 2 rounds completed — don't script before you've felt the pattern.
- The repeating part is small (1–2 steps) and free-form judgment dominates the round.

## Tradeoff

The script encodes today's understanding. Adding a step ("also run a11y pass") = editing code, not telling an LLM. Adding a node template = writing a new prompt file. This is the cost of escape from drift.

Mitigations:

- Keep the script short — < 200 lines. Pseudocode-like, not a framework.
- Keep templates short — one rule-set per node, ≤ 100 lines.
- Treat both as living config. When the round shape changes meaningfully, throw out the script and rewrite. Cheaper than retrofitting.
- Version state-file schema; bump when nodes change inputs.

## Anti-patterns specific to this pattern

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| Long-lived "orchestrator" agent that calls the script | Defeats the purpose — the orchestrator still drifts | Script runs in a non-LLM runtime (Rhai, TS, Python). LLMs only enter at judgment nodes. |
| Judgment node reads `LESSONS.md` itself | Bloats the cold context, slow, costly | Distill the relevant rules into the node's prompt template once. |
| `scan-for-gaps` returns `{found: bool}` | Open-ended judgment forced into yes/no loses signal | Return a list; let the script (or human) gate which become work. |
| State spread across multiple files with no schema | Nodes can't reliably consume slices | Single `state.json` per round, documented schema. |
| Skipping the fix-count cap | Infinite fix loops on a fundamentally broken plan | `MAX_FIX` per phase; escalate to user verbatim with verdict history. |
| Reusing a wrapped agent across judgment nodes | Context accumulates → same forgetting problem reappears | Fresh wrapper per node, killed on exit. Or in-process `Agent()` for short ones. |
