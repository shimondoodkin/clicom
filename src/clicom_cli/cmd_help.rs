//! `clicom help` — top-level + topic-specific help.

const QUICKSTART: &str = "\
clicom — file-based command channel for wrapped CLI agents

QUICK START

  In one terminal (wraps the agent and stays alive for its session):
      clicom start -- claude

  In another terminal in the same directory (drives the agent):
      clicom run \"type_text(\\\"hello\\n\\\")\"
      clicom run \"wait_idle(800); screen_text()\"
      clicom run \"type_keys(\\\"[Up][Enter]\\\")\"

  Inspect:
      clicom status            # list live + recent instances
      clicom help              # subcommand reference
      clicom help host-fns     # all Rhai host functions
      clicom help script       # Rhai language cheatsheet
";

pub fn quickstart() -> i32 {
    println!("{QUICKSTART}");
    0
}

const TOP_LEVEL: &str = "\
clicom — file-based command channel for wrapped CLI agents

USAGE:
    clicom <SUBCOMMAND> [args]

SUBCOMMANDS:
    start    Wrap a command in a PTY (or pipes) and stay alive for its lifetime
    status   List instances or show details for one
    run      Drop a Rhai script into the queue and wait for the result
    queue    Drop a Rhai script and exit immediately (asynchronous)
    clean    Delete result triples (.out / .err / .done) from an instance's commands/
    mcp      Start a stdio MCP server (use from Claude Code / other MCP clients)
    help     Show this help, or `clicom help <topic>` for details

TOPICS:
    host-fns   Reference of all Rhai host functions (§4)
    script     Pointers to Rhai language docs and a one-page tutorial
    layout     The .clicom/ on-disk layout (§3)
    start | status | run | queue | clean | mcp
        Long-form help for that subcommand
";

pub fn run(topic: Option<&str>) -> i32 {
    let body = match topic {
        None => TOP_LEVEL.to_string(),
        Some("host-fns") => host_fns_help(),
        Some("script")   => script_help(),
        Some("layout")   => layout_help(),
        Some("start")    => start_help(),
        Some("status")   => status_help(),
        Some("run")      => run_help(),
        Some("queue")    => queue_help(),
        Some("clean")    => clean_help(),
        Some("mcp")      => mcp_help(),
        Some(other) => {
            eprintln!("clicom help: unknown topic '{other}'");
            return 2;
        }
    };
    println!("{body}");
    0
}

fn host_fns_help() -> String {
    "Rhai host functions registered by the wrapper:\n\
     \n\
     PTY input:\n\
       type_text(s: String, translate=true) -> ()    // sends s; translate=true: \\n -> \\r (typed-Enter); false: raw passthrough\n\
       type_keys(spec: String) -> ()                  // bracketed shortcut keys: [Ctrl+C] [Up] [Enter] [F5] [Tab] etc; plain text outside brackets passes through\n\
     \n\
     Visible screen:\n\
       screen_text() -> String\n\
       screen_save(path: String) -> i64\n\
     \n\
     Scrollback range:\n\
       screen_tail_text(from: i64, to: i64) -> String\n\
       screen_tail_save(path: String, from: i64, to: i64) -> Map\n\
     \n\
     After-marker tail:\n\
       screen_last_after(marker: String) -> String\n\
       screen_save_last_after(path: String, marker: String) -> i64\n\
       screen_last_after_re(regex: String) -> String\n\
       screen_save_last_after_re(path: String, regex: String) -> i64\n\
     \n\
     Waits:\n\
       wait_idle(ms: i64)\n\
       wait_idle(ms: i64, timeout_ms: i64)\n\
       wait_ms(ms: i64)\n\
     \n\
     Status & control:\n\
       status() -> Map { state, last_activity, lifetime_lines, trimmed_below, visible_rows, visible_cols }\n\
       set_timeout(ms: i64) -> ()\n\
     \n\
     JSON:\n\
       parse_json(s: String) -> Dynamic\n\
       to_json(v: Dynamic)   -> String\n\
     \n\
     File I/O (paths relative to wrapper cwd):\n\
       read_file(path: String) -> String\n\
       write_file(path: String, content: String) -> i64\n\
       append_file(path: String, content: String) -> i64\n\
       delete_file(path: String) -> ()\n\
       mkdirp(path: String) -> ()\n\
     \n\
     Network:\n\
       fetch_url(url: String) -> Map { status, body }\n\
     \n\
     Shell (uses host's cmd /C or sh -c):\n\
       shell_execute(command: String) -> Map { exit_code, stdout, stderr }\n\
     \n\
     Diagnostic output (captured to <id>.log, surfaced to driver stderr):\n\
       print(...)   // built-in\n\
       debug(...)   // built-in\n".into()
}
fn script_help() -> String {
    let mut s = String::from(include_str!("../../docs/help-script.txt"));
    if !s.ends_with('\n') { s.push('\n'); }
    s.push_str("\nFull language reference: https://rhai.rs/book/\n");
    s
}
fn layout_help() -> String { "Layout under <cwd>/.clicom/<pid>-<rand6>/:\n  meta.json status.json screen.txt commands.lock commands/<id>.{rhai,out,err,done}\n".into() }
fn start_help()  -> String { "clicom start [--mouse] [--nopty] [--name <name>] -- <command> [args...]\n".into() }
fn status_help() -> String { "clicom status [<partial>]\n".into() }
fn run_help()    -> String { "clicom run [<partial>] (<inline> | -f <file> | -) [--wait | --force] [--timeout <ms>]\n".into() }
fn queue_help()  -> String { "clicom queue [<partial>] (<inline> | -f <file> | -)\n".into() }
fn clean_help()  -> String { "clicom clean [<partial>] [<id>]\n".into() }
fn mcp_help() -> String {
    "clicom mcp\n\
     \n\
     Starts a stdio MCP (Model Context Protocol) server. Configure your MCP client\n\
     (e.g. Claude Code) to launch this binary in your project directory; the server\n\
     exposes clicom's driver operations as MCP tools so the model can drive wrapped\n\
     agents directly via tool calls (no shell escaping).\n\
     \n\
     Tools exposed:\n\
       clicom_status, clicom_type, clicom_keys, clicom_screen,\n\
       clicom_screen_after, clicom_screen_after_re, clicom_wait_idle,\n\
       clicom_run, clicom_queue, clicom_clean\n".into()
}
