# clicom

File-based command channel for wrapped CLI agents. Wraps any command in a PTY,
exposes its screen as a file, and accepts Rhai scripts to drive it.

## Build

    cargo build --release
    cp target/release/clicom.exe ~/.local/bin/

## Use

    # In one shell:
    clicom start -- claude code

    # In another shell:
    clicom run "type_text(\"hello\n\"); wait_idle(800); screen_text()"

See `clicom help host-fns` for the full Rhai surface.

## Spec

`docs/superpowers/specs/2026-05-02-clicom-wrapped-commands-channel-design.md`.
