# clicom — Manual Smoke Recipe

1. `cd <some empty dir>`
2. `clicom start -- claude code`
3. In another shell, same dir:
   - `clicom status` — see one live instance
   - `clicom run "screen_text()"` — should print Claude's banner
   - `clicom run "type_text(\"what is 2+2?\n\")"` — types into Claude
   - `clicom run "wait_idle(2000); screen_last_after(\"2+2\")"` — pulls the answer
4. Exit Claude. Run `clicom status` — instance now `exited`.
