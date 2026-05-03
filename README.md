# clicom

File-based command channel for wrapped CLI agents. Wraps any command in a PTY,
exposes its screen as a file, and accepts Rhai scripts to drive it.

## Build

    cargo build --release

The binary lands at `target/release/clicom` (Linux/macOS) or `target/release/clicom.exe` (Windows).

## Install

Copy the binary somewhere on your `$PATH`. The conventional spot for per-user binaries is `~/.local/bin/`:

**Linux / macOS:**

    mkdir -p ~/.local/bin
    cp target/release/clicom ~/.local/bin/
    # Make sure ~/.local/bin is on PATH; if not, add to ~/.bashrc or ~/.zshrc:
    #   export PATH="$HOME/.local/bin:$PATH"

**Windows (PowerShell):**

    New-Item -ItemType Directory -Force -Path "$HOME\.local\bin" | Out-Null
    Copy-Item target\release\clicom.exe "$HOME\.local\bin\"
    # Add ~/.local/bin to PATH if not already present:
    #   [Environment]::SetEnvironmentVariable("Path", "$env:Path;$HOME\.local\bin", "User")
    # (open a new terminal for the PATH change to take effect)

Verify:

    clicom help

## Use

    # In one shell:
    clicom start -- claude code

    # In another shell:
    clicom run "type_text(\"hello\n\"); wait_idle(800); screen_text()"

See `clicom help host-fns` for the full Rhai surface.

## Spec

`docs/superpowers/specs/2026-05-02-clicom-wrapped-commands-channel-design.md`.
