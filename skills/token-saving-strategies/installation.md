# Installation

Setup commands for each tool. See [SKILL.md](SKILL.md) for when to use each.

## semble — Semantic Code Search

Requires [uv](https://docs.astral.sh/uv/getting-started/installation/).

### MCP (top-level agent)

```bash
# Claude Code
claude mcp add semble -s user -- uvx --from "semble[mcp]" semble
```

```toml
# Codex — ~/.codex/config.toml
[mcp_servers.semble]
command = "uvx"
args = ["--from", "semble[mcp]", "semble"]
```

```json
// OpenCode — ~/.opencode/config.json
{
  "mcp": {
    "semble": {
      "type": "local",
      "command": ["uvx", "--from", "semble[mcp]", "semble"]
    }
  }
}
```

```json
// Cursor — ~/.cursor/mcp.json (or .cursor/mcp.json in project)
{
  "mcpServers": {
    "semble": {
      "command": "uvx",
      "args": ["--from", "semble[mcp]", "semble"]
    }
  }
}
```

### CLI (for sub-agents — required, since sub-agents can't call MCP)

```bash
pip install semble        # or: uv add semble
semble init               # writes .claude/agents/semble-search.md
```

Then append a "Code Search" snippet to `AGENTS.md` / `CLAUDE.md`:

```markdown
## Code Search

Use `semble search` to find code by describing what it does or naming a symbol/identifier, instead of grep:

​```bash
semble search "authentication flow" ./my-project
semble search "save_pretrained" ./my-project
semble search "save model to disk" ./my-project --top-k 10
​```

Use `semble find-related` to discover code similar to a known location:

​```bash
semble find-related src/auth.py 42 ./my-project
​```

`path` defaults to the current directory; git URLs are accepted.
If `semble` is not on `$PATH`, use `uvx --from "semble[mcp]" semble` in its place.

## Workflow

1. Start with `semble search` to find relevant chunks.
2. Inspect full files only when the chunk is not enough context.
3. Optionally use `semble find-related` with a promising result's `file_path` and `line`.
4. Use grep only for exhaustive literal matches or quick string confirmation.
```

### Updating

```bash
pip install --upgrade semble
uv add semble --upgrade
uv cache clean semble        # for MCP users (restart MCP client after)
```

## ast-grep — Structural Code Search

CLI only; no MCP. Pick any installer:

```bash
npm install --global @ast-grep/cli
pip install ast-grep-cli
brew install ast-grep
cargo install ast-grep --locked
cargo binstall ast-grep
scoop install main/ast-grep
sudo port install ast-grep   # MacPorts
nix-shell -p ast-grep
mise use -g ast-grep
```

## rtk — Shell Output Compressor

```bash
brew install rtk                                                           # macOS / Linux
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
cargo install --git https://github.com/rtk-ai/rtk
```

Pre-built binaries: https://github.com/rtk-ai/rtk/releases

### Hook setup (transparent rewrite)

```bash
rtk init -g                     # Claude Code / Copilot (default)
rtk init -g --gemini            # Gemini CLI
rtk init -g --codex             # Codex
rtk init -g --agent cursor      # Cursor
rtk init --agent windsurf       # Windsurf
rtk init --agent cline          # Cline / Roo Code
rtk init --agent kilocode       # Kilo Code
rtk init --agent antigravity    # Google Antigravity
```

Restart the agent. Bash calls like `git status` are auto-rewritten to `rtk git status`.

### Verify

```bash
rtk --version
rtk gain                        # cumulative token savings
```

### Windows

For the full hook system, prefer **WSL**. Native Windows works for ad-hoc `rtk <command>` from PowerShell / Windows Terminal, but the hook integration is WSL-first.

### Scope limit

The hook only fires on **Bash tool calls**. Claude Code's built-in `Read`, `Grep`, `Glob` bypass it. To get rtk filtering for those workflows, drive shell commands explicitly: `rtk read`, `rtk grep`, `rtk find`.

## qmd — Local Notes/Docs Search

```bash
npm install -g @tobilu/qmd     # or: bun install -g @tobilu/qmd
# Or run directly: npx @tobilu/qmd … / bunx @tobilu/qmd …
```

### Claude Code plugin (recommended)

```bash
claude plugin marketplace add tobi/qmd
claude plugin install qmd@qmd
```

### Manual MCP

```json
// ~/.claude/settings.json or Claude Desktop config
{
  "mcpServers": {
    "qmd": { "command": "qmd", "args": ["mcp"] }
  }
}
```

### Initial indexing

```bash
qmd collection add ~/notes --name notes
qmd collection add ~/Documents/meetings --name meetings
qmd context add qmd://notes "Personal notes and ideas"
qmd context add qmd://meetings "Meeting transcripts"
qmd embed                              # generate embeddings
```

### Long-running HTTP server (avoids reloading models per session)

```bash
qmd mcp --http --daemon                # background, PID at ~/.cache/qmd/mcp.pid
qmd mcp stop                           # stop via PID file
qmd status                             # confirm running
```

Then point any MCP client at `http://localhost:8181/mcp`.
