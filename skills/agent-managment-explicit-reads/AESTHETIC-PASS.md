---
name: aesthetic-pass
description: Read this skill after all functional validation passes (validate-promised + any approved gap features). Aesthetic critique — compare screenshots against production tools (Linear, Vercel, Figma, Stripe), check the smell list, identify wireframe-y patterns. Output: PASS or FAIL with location-specific issues. Don't bundle these with bug fixes.
---

# Aesthetic Pass

## When you invoke this skill

ALL functional validation has passed:

- `VALIDATE-PROMISED.md` → PASS
- `SCAN-FOR-GAPS.md` → all approved gaps implemented and re-validated PASS

**Sequence matters.** Polishing a wireframe that doesn't work yet is wasted effort. Apply this lens once the spec is met but before "round done".

## The honest question

Look at the screenshot and ask: *"Does this look finished, or like a wireframe?"*

Compare against production tools: **Linear, Vercel, Figma, Stripe**. Open one of them at a comparable surface in `chrome-devtools-mcp` and screenshot. Look side-by-side at the same viewport. If your surface looks visibly less polished, list why.

## The smell list

Each smell is a yellow flag, not necessarily a fail. Multiple smells on one screen → wireframe-y → FAIL.

| Smell | What it looks like | Why it reads wireframe |
|---|---|---|
| **Vast unanchored empty regions** | A panel with one button in the top-left and 80% empty space | User has no idea what should fill it; needs a placeholder, condensed layout, or proper empty state |
| **Same-weight headers everywhere** | All section titles same size + weight | No visual hierarchy; everything competes for attention |
| **Inconsistent icon sizes** | 16px icons next to 24px icons in the same context | Reads as un-designed; needs a single icon scale per context |
| **Zero accent color, only neutrals** | Grayscale UI with no brand color or status color | Looks unfinished; production tools have 1-2 accent colors |
| **Outlined-only active states** | A "selected" tab differs from inactive only by a 1px border | Easy to miss; use fill or background tint for active |
| **Floating placeholder text in giant empty panel** | "No items yet" floating at the top-left of a 600px-tall empty box | Needs centering or proper empty-state composition |
| **Icons without labels or tooltips** | Bare icon row, user must hover and guess | Guessing UI; either label or add tooltips |
| **No depth at all** | Everything flat, no borders, no shadows, no contrast | Reads as wireframe; needs subtle depth |
| **Buttons clipped or overflowing** | Action buttons cut off by panel edges | Layout math wrong; common with fixed-width panels |
| **Whitespace rhythm broken** | Random gaps: 4px here, 11px there, 17px elsewhere | Production tools use 4 / 8 / 16 / 24px rhythm consistently |

## What production tools have (the positive list)

When in doubt, check against these patterns:

- **1-2 accent colors** for primary actions, status, and emphasis.
- **Weight-based hierarchy:** titles bold, sections medium, body regular — clear scale.
- **4 / 8 / 16 / 24 px whitespace rhythm**, not random.
- **Subtle depth:** 1px borders, 1-2px shadows, or background tint to separate regions.
- **Single icon set** at consistent size per context.
- **Filled active states:** selected tab has a background tint, not just a border.
- **Status bars / footers** with subtle context info (current branch, save state, last-updated).
- **Deliberate use of negative space:** empty regions feel intentional, not abandoned.
- **Consistent button shapes:** same corner radius, padding, font weight per category.

## Process

1. Open the validated surface screenshots with the Read tool.
2. Open Linear / Vercel / Figma / Stripe at a comparable surface in `chrome-devtools-mcp`. Screenshot.
3. Look at them side-by-side.
4. Walk through the smell list. Mark each smell present.
5. For each smell present, locate it (which panel, which element).

## Anti-patterns

| Anti-pattern | Cost | Fix |
|---|---|---|
| Running aesthetic pass before functional validation is green | Polish on broken UI; rework when bugs are fixed | Strict sequence: functional → aesthetic |
| Bundling aesthetic issues with bug-fix briefs | Dev confuses correctness work with quality work; both suffer | Separate dispatches: bug-fix round, then aesthetic round |
| Comparing only against your own prior work | Bar drifts down over time | Compare against external production tools every pass |
| Judging from descriptions only (*"it looks fine"*) | Misses what's actually rendered | Always open the screenshot with the Read tool |
| Listing smells without locations | Dev can't act on it | Specify panel + element for each smell |
| Suggesting wholesale redesigns when one smell triggered | Scope explodes, dev overwhelmed | One smell → one concrete fix proposal |

## Output

```
Verdict: PASS | FAIL

Aesthetic issues (omit if PASS):
1. [<severity: blocker | high | medium | low>] Location: <panel / element / route>
   Smell: <from smell list, or named pattern>
   Comparison: <production tool reference if relevant>
   Proposed fix: <concrete suggestion — color, weight, spacing, layout change>

2. ...
```

- **PASS** → round done. Record in round-state ledger; commit; advance to plan N+1.
- **FAIL** → `FIX-DISPATCH.md` with these issues. Re-enter this skill after fix is reported done. Hard cap: 2 aesthetic fix iterations per round (per `SKILL.md`). If you hit the cap, escalate — the brief is wrong, not the dev.
