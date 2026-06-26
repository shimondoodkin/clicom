---
name: pm-lens
description: Use when about to make a decision with multiple valid answers — API/UX/CLI shape, adding a feature or flag the original ask didn't explicitly request, picking a default, declaring "done" on user-visible work, or wondering whether a planned change has drifted from what the user actually wanted. Spawns a fast PM-perspective subagent that returns a 3-line verdict (ship | redirect | halt) judged against the user's intent — not their literal phrasing.
---

# PM Lens

A short transient subagent invocation that pressure-tests an in-flight decision against the user's intent before committing.

## Why this exists

Agents drift. They over-engineer, add unrequested features, optimize for themselves rather than the user, and miss things the user clearly wanted. The verbatim ask is the defensive anchor against that drift; the goal is the user (and their end users) being happy with the result. The PM lens is a 30-second sanity check that asks: *given what the user actually wants, is this the right thing to build?*

## When to invoke

Proactively, not on demand. Triggers:

- About to make an API/UX/CLI shape decision with multiple valid answers
- Adding a feature, flag, or option that's either unrequested (scope creep) or missing something the user clearly wanted (scope miss)
- Writing user-facing copy: errors, prompts, labels, help, docs
- Picking defaults — the most user-affecting choice
- A flow now requires more than 2 user steps
- About to declare "done" on anything user-visible

## How to invoke

Spawn a general-purpose subagent with this prompt:

```
You are the product manager for [audience].
Original ask (verbatim, as a reference anchor): [paste user's exact phrasing]
What the user actually seems to want (intent): [your read of intent + who the end users are + what would make them happy]
About to: [specific decision]
Relevant context: [code/UI/state — paste, don't summarize]

Does this serve the user's intent and the end users' happiness, or is it drift / over-engineering / scope creep / scope miss?
Answer in 3 lines: (1) verdict ship | redirect | halt, (2) one-line reason, (3) one-line redirect if needed.
```

The two separate fields — verbatim ask and your read of intent — are deliberate. The verbatim text anchors the persona against your own bias when summarizing intent; your intent read forces you to commit to a position the persona can verify or push back on.

## Acting on the verdict

**Relay verbatim.** Use the persona's actual language when communicating the verdict onward (or applying it yourself) — don't soften, paraphrase, or hedge. Under pressure, agents (including yourself) rationalize gentle critique away; concrete direct language survives.

**If the verdict is `redirect`**, do the redirect. Don't engage with the tangent that prompted the lens in the first place — re-anchor on what the user originally asked for.

**If the verdict is `halt`**, surface the question to the user. This is the case where the decision is genuinely theirs to make (irreversible direction, scope expansion, money/legal/security/privacy).

## When NOT to invoke

- Pure mechanical work with no user-facing decision (running tests, internal refactors invisible to the user)
- Decisions the user already explicitly made
- Trivial defaults where any reasonable choice is fine
- When you don't have enough context to articulate the user's intent — gather it first; the lens is useless without it

## Pair with

- **`end-user-lens`** for the complementary "will the user understand this?" question. Run both in parallel when stakes warrant — they catch different failures (PM lens catches *what to build*, end-user lens catches *whether they can use it*).
