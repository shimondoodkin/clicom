//! `clicom help` — top-level + topic-specific help.

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
    help     Show this help, or `clicom help <topic>` for details

TOPICS:
    host-fns   Reference of all Rhai host functions (§4)
    script     Pointers to Rhai language docs and a one-page tutorial
    layout     The .clicom/ on-disk layout (§3)
    start | status | run | queue | clean
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
       type_text(s: String) -> ()\n\
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
       set_timeout(ms: i64) -> ()\n".into()
}
fn script_help() -> String { "See https://rhai.rs/book/ for the language reference.\n".into() }
fn layout_help() -> String { "Layout under <cwd>/.clicom/<pid>-<rand6>/:\n  meta.json status.json screen.txt commands.lock commands/<id>.{rhai,out,err,done}\n".into() }
fn start_help()  -> String { "clicom start [--mouse] [--nopty] [--name <name>] -- <command> [args...]\n".into() }
fn status_help() -> String { "clicom status [<partial>]\n".into() }
fn run_help()    -> String { "clicom run [<partial>] (<inline> | -f <file> | -) [--wait | --force] [--timeout <ms>]\n".into() }
fn queue_help()  -> String { "clicom queue [<partial>] (<inline> | -f <file> | -)\n".into() }
fn clean_help()  -> String { "clicom clean [<partial>] [<id>]\n".into() }
