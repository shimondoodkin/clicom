---
name: token-saving-strategies
description: Use when picking or configuring a code/knowledge search or shell-output tool to lower token usage in agentic workflows (Claude Code, Codex, Cursor) — covers semble (semantic code search), ast-grep (structural search/codemods), rtk (shell output compressor), and qmd (local notes/docs search). For install commands see installation.md.
---

# Token-Saving Strategies for Coding Agents

Four complementary tools cut tokens by attacking different bottlenecks. Pick by the bottleneck, not by preference.

## Pick by bottleneck

| Bottleneck                                      | Tool      | Category             |
| ------------------------------------------------ | --------- | -------------------- |
| Reading whole notes/docs to find one fact        | qmd       | Knowledge retrieval  |
| `grep` + `read` dumps full files                 | semble    | Semantic code search |
| `grep -A/-B` returns arbitrary line windows      | ast-grep  | Structural search    |
| Verbose CLI output (git, ls, tests, lint, …)     | rtk       | Shell output         |

For setup commands, see [installation.md](installation.md).

## Decision rule for code search

- Natural-language question about the codebase → **semble** (`semble search "..."`).
- Known syntactic pattern (specific call shape, refactor target) → **ast-grep** (`ast-grep -p '...' -l <lang>`).
- Exact literal string or regex → **ripgrep** (or `rtk grep`).
- Personal notes / project docs → **qmd**.
- Verbose Bash output → let the **rtk** hook handle it transparently.

## What each tool does (one-liner each)

- **semble** — chunks a repo, scores chunks with static Model2Vec embeddings + BM25, fuses with RRF, reranks with code-aware signals. ~250 ms index, ~1.5 ms query, ~98% fewer tokens than grep + read. CPU-only, fully local. Tools: `search`, `find_related`.
- **ast-grep** — tree-sitter–based AST search/lint/rewrite. Patterns look like the code you want to find; `$VAR` (capital after `$`) are wildcards. Deterministic — no ranking. Try patterns at https://ast-grep.github.io/playground.html before committing.
- **rtk** — Rust CLI proxy that filters/groups/dedupes/truncates command output before the agent sees it. Hooks into Bash tool calls (`git status` → `rtk git status` transparently). 60–90% per-command savings.
- **qmd** — on-device search (BM25 + vector + LLM rerank) over your markdown notes/docs/transcripts. Local via node-llama-cpp. Tree-structured "context" annotations help the LLM pick the right doc.

## Sub-agent caveat (Claude Code, Codex CLI)

Sub-agents **cannot call MCP tools** — MCP schemas are lazy-loaded only at the top-level agent. For sub-agents, fall back to the CLI:

- `semble init` writes `.claude/agents/semble-search.md` for a dedicated sub-agent.
- Append a "Code Search" snippet to `AGENTS.md` / `CLAUDE.md` directing all agents to `semble search` instead of grep.
- `ast-grep` is CLI-only anyway — no MCP, no caveat.

## Failure modes & mitigations

All four trade fidelity for tokens. The agent **can't see what was dropped**, so it can't ask for it — that's the meta-risk.

### rtk (highest risk — lossy by design)

| Failure mode                                                    | Mitigation                                                          |
| --------------------------------------------------------------- | ------------------------------------------------------------------- |
| `rtk git add` → `"ok"` hides what was actually staged           | Run `git status` after; verify before commit                        |
| Test compactors hide skipped tests, warnings, count drift       | For high-stakes runs, add `--verbose` or run raw                    |
| `rtk read -l aggressive` strips bodies → reasoning on sigs only | Default level for code you'll edit; aggressive only for scans       |
| Truncation/dedup collapses similar errors — one may differ      | If errors look templated, re-run raw to confirm                     |
| `Read`/`Grep`/`Glob` bypass the hook → inconsistent file views  | Pick one lens per task; built-in `Read` is always the raw escape    |

### semble (moderate — ranked, chunked)

| Failure mode                                                    | Mitigation                                                          |
| --------------------------------------------------------------- | ------------------------------------------------------------------- |
| Chunks hide caller context that changes semantics               | `Read` the full file before editing                                 |
| ~15% miss rate at top-10 (NDCG@10 = 0.854)                      | Empty-ish results ≠ absence; cross-check with ripgrep on a keyword  |
| Remote git URL cached for the session; branch switches invisible| Re-add the repo or restart the MCP session after branch changes     |

### ast-grep (lowest hallucination risk; real codemod risk)

| Failure mode                                                    | Mitigation                                                          |
| --------------------------------------------------------------- | ------------------------------------------------------------------- |
| Wrong pattern → silent zero results                             | Cross-check zero results with a ripgrep on the symbol name          |
| `--rewrite` over-matches and mass-edits incorrectly             | Preview without `-U`/`--update-all`; commit before applying         |
| Wrong language flag (e.g., `-l ts` on `.tsx`)                   | Use `-l tsx` for JSX; verify on one file before scaling             |

### qmd (moderate — ranked, chunked, manual reindex)

| Failure mode                                                    | Mitigation                                                          |
| --------------------------------------------------------------- | ------------------------------------------------------------------- |
| Hybrid + rerank can miss the right doc                          | Lower `--min-score` or use `qmd search` (lexical) as a sanity check |
| Stale index after content changes (no auto-watch)               | Re-run `qmd embed` after editing the indexed corpus                 |
| Snippet without surrounding doc structure can mislead           | `qmd get <path> --full` before drawing conclusions                  |

## Rule of thumb

- **Exploration / bulk reads** — token-savers are a clear win.
- **High-stakes edits** (security, migrations, anything irreversible) — bypass the lossy layers; `Read` the raw file, run the raw command, verify by hand.

## Combining them

A typical Claude Code setup (see [installation.md](installation.md) for commands):

1. `rtk init -g` once — covers all Bash-driven file/git/test/lint output.
2. `semble` MCP — semantic code search inside any repo.
3. `ast-grep` on `$PATH` — structural search/codemods (CLI only).
4. `qmd` MCP plugin — personal/project notes and docs.

Top-level agent gets MCP access to qmd + semble; sub-agents fall back to `semble` and `ast-grep` CLIs via `AGENTS.md` / `CLAUDE.md` instructions; everything else runs through the rtk hook.

## References

- semble — https://github.com/MinishLab/semble
- ast-grep — https://github.com/ast-grep/ast-grep
- rtk — https://github.com/rtk-ai/rtk
- qmd — https://github.com/tobi/qmd
