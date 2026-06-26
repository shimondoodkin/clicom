---
name: fix-dispatch
description: Use when a validation phase (validate-promised or aesthetic-pass) returns FAIL with issues and the developer agent needs to be briefed for a fix round. Decide whether to bundle small issues or dispatch immediately, brief using the file-not-paste pattern, and re-enter the validation phase after fix is reported done.
---

# Fix Dispatch

## When you invoke this skill

A judgment phase returned FAIL:

- `VALIDATE-PROMISED.md` → FAIL with functional issues, OR
- `AESTHETIC-PASS.md` → FAIL with aesthetic issues, OR
- A gap-development validation → FAIL.

You have a list of issues and need to dispatch a fix round to the developer agent.

## Step 1: The bundling decision

Do you dispatch THIS issue alone, or queue it with others?

### Bundle (queue, don't dispatch yet)

- **Small issues within the same category** (layout, convenience polish, copy tweaks).
- **3+ items haven't accumulated yet** — wait until they do, or until a natural seam.
- **Within the same validation phase** — bundling cross-phase issues (validate-promised + aesthetic) confuses the dev.

Each dispatch has overhead — read brief, build context, edit, test, commit, summarize. 5 separate single-fix dispatches = 5× the overhead. Bundling amortizes.

### Dispatch immediately (don't bundle)

- **Regression in just-shipped work** — failure mode is different; fix now to surface the regression while context is fresh.
- **Blocker** — anything stopping other work (build broken, test infra broken).
- **Critical user-facing bug** — silent data loss, security, broken auth.
- **Natural seam reached** — between major rounds, or 3+ accumulated items in a category.

### Decision rule

If unsure: queue and wait for the 3rd item or a natural seam. The cost of waiting on a small UI tweak is low; the cost of dispatching 5 mini-fixes is high.

**When queueing**, record the item in the round-state ledger under "queued fixes" so you don't lose it.

## Step 2: Write the brief to a file

**Do NOT type the brief directly via `clicom_type`.** Claude Code's TUI collapses long pastes into "paste again to expand" previews; Enter doesn't submit cleanly. Symptom: only the trailing fragment reached the dev.

**Workaround that works:**

1. Write the brief to a file at the project root, e.g. `.fix-round-<phase>-<N>.md`.
2. `clicom_type "Read <path> and follow it."` — short message, no truncation.
3. Send explicit `[Enter]` via `clicom_keys "[Enter]"`. Don't rely on auto-Enter from the trailing `\n`.

See `WRAPPED-AGENT-IO.md` for the underlying mechanics.

## Step 3: Brief format

Tight and actionable. Each issue gets a section.

```
# Fix round <N> — <validate-promised | aesthetic | gap-validation>

## Context

Plan: <plan path>
Round state: <one line — e.g., "validate-promised iteration 2/3">
Validation evidence: <path to screenshots, smoke report, etc.>

## Issues to fix

### Issue 1: <one-line description>

Severity: <blocker | high | medium | low>
Location: <file / component / route>
Evidence: <screenshot path or test output excerpt>
Expected outcome: <what success looks like, concretely>
Constraint: <if any — e.g., "don't break <other feature>", "match Linear's <pattern>">

### Issue 2: ...

## After fixing

Run: <test command(s) that should pass>
Then: report done with a summary of what changed per issue.
```

## Step 4: Anti-patterns in the brief itself

| Anti-pattern | What goes wrong | Fix |
|---|---|---|
| *"Make it look better"* | Dev guesses at aesthetic intent | Specify the smell, the location, and the production-tool reference |
| Mixing functional + aesthetic in one brief | Dev confuses correctness work with quality work | Separate briefs per category |
| No expected outcome | Dev declares done at a different state than the supervisor wanted | Each issue gets a concrete *"what success looks like"* line |
| No evidence link | Dev can't see what the supervisor saw | Always link the screenshot / test output that surfaced the issue |
| *"Don't break anything"* without specifics | Dev guesses at the regression surface | Name the specific features that must still work |
| Issues listed without severity | Dev can't prioritize when something has to give | Always tag severity |

## Step 5: After dispatch — wait and re-validate

1. `clicom_wait_idle` on the dev with a generous timeout. Use `clicom_status` (not visible spinners) to confirm idle truly means done. See `WRAPPED-AGENT-IO.md` for the decision rule.
2. Read the dev's claim summary via `clicom_screen_after_re` using the per-agent prefix.
3. Re-enter the validation phase that returned FAIL — `VALIDATE-PROMISED.md` or `AESTHETIC-PASS.md`. **Re-run only the failing validation, not all phases.**
4. Increment the fix-count for that phase in the round-state ledger.
5. Check the escalation cap (from `SKILL.md`): 3 for validate-promised, 2 for aesthetic, 2 per gap. If exceeded, escalate to the user with verdict history verbatim.

## Step 6: Look-once rule for re-validation

After a fix, re-validate ONLY the components affected by the fix. Components that already passed don't need full re-enumeration unless the fix touched them. (See `VALIDATE-PROMISED.md` *"Look once, then trust"*.)

If you can't tell from the dev's diff what was touched, ask the dev to summarize affected components in their done-report. Don't guess.

## Step 7: When the cap is hit

You've dispatched the max fix-rounds and the validation still fails.

**Don't:**

- Dispatch a 4th attempt anyway.
- Paraphrase the verdict history to make it sound less bad.
- Drop the issue silently.

**Do:**

- Paste the user the verdict history verbatim, in order, with timestamps if available.
- Name what you've tried (briefs, evidence, dev's claims each round).
- Propose options the user can pick from:
  - Retry with a different brief framing
  - Manual fix by user
  - Abandon the issue (and what that means downstream)
  - Drop the plan and rewrite

Then wait for direction. The cap exists because past this point, more fix-rounds usually mean the brief is wrong or the plan was unsound — neither solved by dev retries.

## Anti-patterns observed

| Anti-pattern | Cost paid | Fix |
|---|---|---|
| Hot-fix single tiny issue immediately | 5× overhead across a round | Bundle ≥3 small fixes per category |
| Bundling regressions with small fixes | Regression delays + dev confusion | Regressions dispatch immediately; small fixes queue |
| Long-paste brief via `clicom_type` | Dev sees only the trailing fragment | File + *"Read <path> and follow it."* |
| Auto-Enter only on long input | Submit may not land in paste-mode TUI | Explicit `[Enter]` via `clicom_keys` after long input |
| Re-running ALL validations after a fix | Token waste, slow rounds | Re-run only the failing phase |
| Skipping the escalation cap | Infinite fix loops on a broken plan | Hard cap from `SKILL.md`; escalate verbatim |
| Dispatching without an expected-outcome line | Dev declares done at the wrong state | Every issue gets *"what success looks like"* |
