---
name: plan-sanity-check
description: Use when a fresh plan from the planner is ready and needs sanity-checking before the developer agent is dispatched on it. Adversarial review of the plan's "open questions resolved here" section — for each decision, name the alternative, verify the rejection is sound at realistic scale and data, mock UX decisions visually before approval. Output: list of revision requests, or "plan sound, dispatch".
---

# Plan Sanity-Check

## When you invoke this skill

The planner agent has written a plan and surfaced an "open questions resolved here, sanity-check before dev1 starts" section. You are about to dispatch the developer agent. **Do this review first.** Every minute spent revising plan text saves an order of magnitude of code-rewrite time later.

## The temptation to skip

The plan reads coherently. Each decision sounds reasonable. The temptation is to say *"all decisions look fine, dispatch"*.

**This is the failure mode.** Coherent ≠ correct. Confidence ≠ evidence. Past observation: plans went through 3 revisions because the user pushed back on UX details where the supervisor's first response had been *"all reasonable, no pushback"*. Cost of revising plan text in the same planner session: a single thinking burst. Cost of catching the same problem after dev has built it: a fix round + tests + re-review.

## Inputs

- The plan text in full
- Any prior plans in the project (for context on assumptions about file layouts, type names, conventions)
- The user's verbatim intent for this project (from the round-state ledger)
- Existing implemented code that the plan claims to build on (ground truth)

## The adversarial review

For each decision in "open questions resolved here":

### 1. Name the alternative

What was the other reasonable option? If the plan doesn't explicitly state the alternative, write it down yourself. Then judge: is the rejection of that alternative sound?

- **At scale:** does the chosen option still work with 100× the data the plan envisions? 10× users? Realistic worst-case load?
- **At realistic widths / sizes / inputs:** not the demo case — the messy real one.
- **Under concurrent access** (if relevant): does the chosen option hold when two writers race?

If you can't articulate why the rejected option fails, the rejection isn't sound — request a revision that names the failure mode.

### 2. For UX / layout decisions: mock it up visually before approval

Prose like *"agents on left, chat on right"* hides width problems. Abstract reasoning rubber-stamps what you already wanted to conclude.

**Mockup discipline:**

1. **Mock the WHOLE shell**, not just the component under discussion. A proposal that lives inside a 360px chat panel that lives inside an editor shell with activity bar + side panel + main area looks fine in isolation — until you draw the surrounding context and see the chat ends up at 200px wide.
2. **Mock the rejected proposal**, not just your preferred solution. Side-by-side at the same scale, with the same fake data. The point of mockup-as-tool is to feel the problem viscerally. Drawing only your preferred answer confirms what you already concluded.
3. **Multiple viewports** — 1024 / 1280 / 1440 / mobile.
4. **Use chrome-devtools-mcp** to screenshot the mockup HTML. Look at the images yourself with the Read tool.

### 3. For protocol / data-shape decisions: trace one realistic call end-to-end

Pick one realistic call or data flow. Walk it through the proposed shape from entry to exit. Note every transformation, every assumption.

Example failure: a flat list "works because agent names carry hierarchy as a path string" — would NOT work if hierarchy were a separate `parentId` field. The plan may have assumed the path-string shape without stating it.

### 4. For "simple X instead of complex Y": confirm simple X handles real data

The plan says *"we'll do simple X instead of complex Y because the data is small / flat / regular"*.

- **Get the actual data shape.** Read a sample. Don't infer from the plan's description.
- **Run simple X against the sample.** Does it actually work? Or does it work only on the idealized shape the plan envisioned?
- If real data has cases simple X doesn't handle, request a revision: either complex Y, or explicit *"we'll accept this limitation because <reason>"* in the plan.

### 5. Two-step instructions: spell out both halves

If the plan instructs the developer to do step A then step B, and B references "the file" or "the thing", **the antecedent must be explicit**. Past failure: *"fill in the file column"* was read as *"fill with path to existing file"* because it didn't say *"the file you just wrote in step A"*. Plan stalled on a chicken-and-egg blocker.

Brief format that works: *"Save plan to `<path>`, **then** update README to `[<title>](<path>)`."* The "**then**" + explicit referent prevents the misread.

### 6. Pre-written plans need re-grounding

If this plan was written before the prior plan was implemented, **ground it in real code**:

- The actual implementation of the prior plan is ground truth.
- The pre-written draft is reference to consider, modify, or replace.
- File names, type names, conventions in the draft may diverge from what got built.

Request: *"Re-read `<path-to-prior-plan-implementation>` and update assumptions about <file layouts | type names | conventions> before dispatch."*

## Anti-patterns

| Anti-pattern | What it costs | Fix |
|---|---|---|
| Rubber-stamping *"all decisions look reasonable"* | Plan revision later costs 10× | Push back substantively on at least one decision; name the failure mode |
| Approving a layout from prose description | Cramped panels at realistic viewports | Mock + screenshot at 1024/1280/1440 |
| Mocking only the suggested layout | Confirms your bias; misses the rejected option's actual problem | Side-by-side mockup of both |
| Approving "simple X" against the plan's idealized data | Implementation works on demo, fails on real data | Get a real-data sample; run X against it mentally |
| Treating pre-written plan as gospel | Plan diverges from what prior code actually built | Re-ground in implemented code; mark draft as reference, not spec |
| Approving a two-step instruction with implicit antecedent | Dev stalls on chicken-and-egg | Spell out both halves; use **then** + explicit referent |

## Output

Return ONE of:

**Plan sound, dispatch.** — All decisions reviewed adversarially; no revision needed.

OR a revision-request list:

```
Revision requests for plan <N>:

1. Decision: <decision name>
   Issue: <failure mode at scale / data / concurrency / readability>
   Proposed revision: <specific text or structural change>

2. Decision: <decision name>
   Issue: ...
   ...
```

Send the list to the planner. Loop until the plan returns sound. Then dispatch the developer per `WRAPPED-AGENT-IO.md`.

## After the plan is sound

Record in the round-state ledger:

- Plan path
- Verbatim user intent
- Promised features list (extracted from the plan — each checkable item; you'll need this for `VALIDATE-PROMISED.md`)

Then proceed to phase 2 (dispatch dev).
