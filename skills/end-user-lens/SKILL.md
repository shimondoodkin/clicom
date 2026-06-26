---
name: end-user-lens
description: Use when about to ship user-facing copy (errors, prompts, labels, help, docs), pick a default that affects what the user sees, finalize a flow with user steps, name a command/flag/field/button, or commit any change the end user will see. Spawns a fast subagent that adopts the actual end-user persona (with no project context) and reports what's confusing, missing, or unclear. The agent is not the audience; this is how you remember.
---

# End-User Lens

A short transient subagent invocation that judges user-facing work from the audience's actual position — no project context, no insider knowledge, just what they'd see.

## Why this exists

The agent has been steeping in project context. The end user has not. Things that feel obvious to the agent ("of course `--strict` means strict mode") are opaque to a fresh user. The end-user lens forces a fresh-eyes pass: *given only what the user sees, can they actually do the thing?*

## When to invoke

Proactively, not on demand. Triggers:

- Writing user-facing copy: error messages, prompts, labels, help text, docs, marketing
- Picking a default that affects what the user sees or does
- Finalizing a flow that requires more than one user step
- Naming things — commands, flags, fields, buttons, menu items
- About to declare "done" on anything user-visible

## How to invoke

Spawn a general-purpose subagent with this prompt:

```
You are [specific audience: e.g. a CLI user new to this tool / a non-technical end user / a junior dev / a tiling contractor on a job site].
You don't have prior context on this project.
You see/use: [exact thing — paste copy verbatim, or describe the flow step by step]

Do you understand what this is and what to do? What confuses you? What would you expect that's missing?
Answer in 3 lines.
```

**The audience description matters more than people think.** "User new to this tool" is too generic and produces generic feedback. Describe the actual audience: their role, their context (mobile? terminal? in a hurry? under stress?), what they were trying to do when they hit this surface, how technical they are, and what they care about.

## For external / rendered surfaces

If the work has any rendered or external surface — web page, desktop UI, mobile app, API response, generated image — the persona must actually *see* or *exercise* it, not read about it. Description tells you nothing about whether the user can use it. Tell the persona which tool to use:

- `claude-in-chrome` — web pages, browser console, network
- `screenmcp` — desktop apps, native windows
- `android-emulator-skill` — Android apps
- `WebFetch` — HTTP endpoints, JSON
- `imagemage` — visual asset judgment
- ...or any other installed tool that exposes the surface

The persona inherits your MCPs and skills (see Core mechanic #10 in the parent skill); it can call them directly. If the right tool for the surface isn't installed, you can't run this lens against it — surface the gap to the user instead of guessing from text.

## Acting on the verdict

**Relay verbatim.** If the persona said "I have no idea what this button does", record that, not "the button label could be clearer". Concrete confusion is actionable; softened critique gets rationalized away.

**Treat each "what's missing" with weight.** End users tell you what they expected and didn't find — that's the most valuable signal in the response, often more than what they found confusing. Don't dismiss it as user error.

## When NOT to invoke

- Internal tooling no end user will ever see
- Backend work behind a stable API the user won't notice
- Already user-tested copy or flows where iteration is constrained
- When the audience is genuinely ambiguous — pin down audience first; a generic persona produces generic feedback

## Pair with

- **`pm-lens`** for "is this even the right thing to build?". The PM lens catches scope and direction; this lens catches usability. Run both in parallel when the decision is high-stakes — they catch different failures.
