//! `clicom mcp` — stdio MCP server. Exposes clicom's driver operations as tools
//! so MCP clients (Claude Code, etc.) can drive wrapped agents directly via tool calls.
//!
//! Protocol: JSON-RPC 2.0 over line-delimited stdio. Spec: https://spec.modelcontextprotocol.io/

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::clicom_cli::cmd_clean;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "clicom";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(cwd: &Path) -> Result<i32> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut line = String::new();

    eprintln!("[clicom mcp] starting in {}", cwd.display());

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break; // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(
                    out,
                    r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32700,"message":"parse error: {e}"}}}}"#
                );
                out.flush().ok();
                continue;
            }
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Notifications (no id) — handle and continue without responding.
        if id.is_none() {
            // e.g. notifications/initialized
            continue;
        }

        let response = handle(method, params, cwd);
        let envelope = match response {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, msg)) => json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}),
        };
        writeln!(out, "{envelope}")?;
        out.flush()?;
    }

    Ok(0)
}

fn handle(
    method: &str,
    params: Value,
    cwd: &Path,
) -> std::result::Result<Value, (i32, String)> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
        })),
        "tools/list" => Ok(json!({"tools": tool_definitions()})),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or((-32602, "missing 'name'".to_string()))?;
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or(json!({}));
            tool_call(name, args, cwd)
        }
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

fn tool_definitions() -> Vec<Value> {
    let partial_field = json!({
        "type": "string",
        "description": "Optional partial dir-name match if multiple instances exist."
    });
    vec![
        json!({
            "name": "clicom_status",
            "description": "List clicom instances in the current cwd.",
            "inputSchema": {
                "type": "object",
                "properties": {"partial": partial_field}
            }
        }),
        json!({
            "name": "clicom_type",
            "description": "Type text into the wrapped agent. Default appends Enter; set no_enter=true to skip; set raw=true to disable \\n\u{2192}\\r translation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to type."},
                    "partial": partial_field,
                    "no_enter": {"type": "boolean", "default": false},
                    "raw": {"type": "boolean", "default": false}
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "clicom_keys",
            "description": "Send a keyboard chord spec like '[Ctrl+C]' or '[Up][Up][Enter]' or 'hi[Tab]bye'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "spec": {"type": "string"},
                    "partial": partial_field
                },
                "required": ["spec"]
            }
        }),
        json!({
            "name": "clicom_screen",
            "description": "Print the wrapped agent's current visible screen. A `[clicom: …]` status trailer is appended by default; pass no_status: true for raw output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "partial": partial_field,
                    "no_status": {"type": "boolean", "default": false, "description": "Suppress the [clicom: …] trailer."}
                }
            }
        }),
        json!({
            "name": "clicom_screen_after",
            "description": "Return everything after the last occurrence of <marker>. A `[clicom: …]` status trailer is appended by default; pass no_status: true for raw output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "marker": {"type": "string"},
                    "partial": partial_field,
                    "no_status": {"type": "boolean", "default": false, "description": "Suppress the [clicom: …] trailer."}
                },
                "required": ["marker"]
            }
        }),
        json!({
            "name": "clicom_screen_after_re",
            "description": "Return everything after the last regex match of <pattern>. A `[clicom: …]` status trailer is appended by default; pass no_status: true for raw output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "partial": partial_field,
                    "no_status": {"type": "boolean", "default": false, "description": "Suppress the [clicom: …] trailer."}
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "clicom_wait_idle",
            "description": "Wait until the wrapped agent has been idle for <ms> ms.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ms": {"type": "integer", "default": 800},
                    "timeout_ms": {"type": "integer", "default": 60000},
                    "partial": partial_field
                }
            }
        }),
        json!({
            "name": "clicom_run",
            "description": "Run a Rhai script synchronously and return its result. For complex compositions; prefer the per-op tools above for one-shots.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string"},
                    "partial": partial_field
                },
                "required": ["source"]
            }
        }),
        json!({
            "name": "clicom_queue",
            "description": "Drop a Rhai script and return its <id> immediately (asynchronous).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string"},
                    "partial": partial_field
                },
                "required": ["source"]
            }
        }),
        json!({
            "name": "clicom_clean",
            "description": "Delete result triples (.out/.err/.done/.log) for an instance. Optionally restrict to a specific <id>.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "partial": partial_field,
                    "id": {"type": "string"}
                }
            }
        }),
        json!({
            "name": "clicom_whoami",
            "description": "Identify which clicom-wrapped instance this MCP server is running inside (walks parent-PID chain). Returns dir_name, path, wrapper_pid, name, state — or an error if not running inside a wrapper.",
            "inputSchema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "clicom_exec_detached",
            "description": "Spawn a command as a detached process. On Windows the child gets its own console window (CREATE_NEW_CONSOLE). On Unix the child inherits stdio. Returns the spawned pid. Useful for launching wrapped agents in a fresh terminal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cmd": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Argv for the command, e.g. [\"clicom\", \"start\", \"--\", \"claude\"]"
                    }
                },
                "required": ["cmd"]
            }
        }),
    ]
}

fn rhai_str_lit(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| String::from("\"\""))
}

fn run_script(
    cwd: &Path,
    partial: Option<String>,
    source: String,
) -> std::result::Result<Value, (i32, String)> {
    use crate::clicom_engine::{layout, meta::State};
    use std::time::{Duration, Instant};

    let candidates: Vec<_> = crate::clicom_cli::discovery::filter_by_partial(
        crate::clicom_cli::discovery::list_instances(cwd),
        partial.as_deref(),
    )
    .into_iter()
    .filter(|i| matches!(i.status.state, State::Idle | State::Busy))
    .collect();

    let inst = match candidates.len() {
        0 => {
            return Err((
                -32000,
                format!("no live wrapped agent in {}", cwd.display()),
            ))
        }
        1 => candidates[0].dir.clone(),
        _ => {
            return Err((
                -32000,
                format!("ambiguous match: {} candidates", candidates.len()),
            ))
        }
    };

    let _guard = crate::clicom_cli::drop::acquire_lock(&inst)
        .map_err(|e| (-32000, format!("lock: {e}")))?;
    let id = crate::clicom_cli::drop::drop_rhai(&inst, &source)
        .map_err(|e| (-32000, format!("drop: {e}")))?;

    let done = layout::done_path(&inst, &id);
    let deadline = Instant::now() + Duration::from_millis(60_000);
    while !done.exists() {
        if Instant::now() >= deadline {
            return Err((-32000, "script execution timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let body = std::fs::read_to_string(&done)
        .map_err(|e| (-32000, format!("read done: {e}")))?;
    let out_text =
        std::fs::read_to_string(layout::out_path(&inst, &id)).unwrap_or_default();
    let err_text =
        std::fs::read_to_string(layout::err_path(&inst, &id)).unwrap_or_default();
    let log_text =
        std::fs::read_to_string(layout::log_path(&inst, &id)).unwrap_or_default();

    let _ = std::fs::remove_file(layout::out_path(&inst, &id));
    let _ = std::fs::remove_file(layout::err_path(&inst, &id));
    let _ = std::fs::remove_file(layout::log_path(&inst, &id));
    let _ = std::fs::remove_file(&done);

    if body.trim_start().starts_with("OK") {
        let mut content = Vec::new();
        if !log_text.trim().is_empty() {
            content.push(json!({"type":"text","text":format!("[log]\n{log_text}")}));
        }
        let val: Value =
            serde_json::from_str(out_text.trim()).unwrap_or(Value::Null);
        let display = match &val {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other)
                .unwrap_or_else(|_| out_text.clone()),
        };
        if !display.is_empty() {
            content.push(json!({"type":"text","text":display}));
        }
        if content.is_empty() {
            content.push(json!({"type":"text","text":"(ok)"}));
        }
        Ok(json!({"content": content, "isError": false}))
    } else {
        Ok(json!({
            "content": [{"type":"text","text": err_text.trim_end()}],
            "isError": true
        }))
    }
}

fn s_arg<'a>(args: &'a Value, key: &str) -> std::result::Result<&'a str, (i32, String)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or((-32602, format!("missing string '{key}'")))
}

fn s_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn b_opt(args: &Value, key: &str) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn i_opt(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

/// Wrap a successful dead-instance fallback response into the MCP envelope
/// (mirrors run_script's OK branch).
fn dead_response_envelope(text: String) -> Value {
    let display = if text.is_empty() { "(ok)".to_string() } else { text };
    json!({
        "content": [{"type":"text","text": display}],
        "isError": false
    })
}

fn tool_call(
    name: &str,
    args: Value,
    cwd: &Path,
) -> std::result::Result<Value, (i32, String)> {
    match name {
        "clicom_status" => {
            let partial = s_opt(&args, "partial");
            let items = crate::clicom_cli::discovery::filter_by_partial(
                crate::clicom_cli::discovery::list_instances(cwd),
                partial.as_deref(),
            );
            let arr: Vec<Value> = items
                .into_iter()
                .map(|i| {
                    json!({
                        "dir_name": i.dir_name,
                        "name": i.meta.name,
                        "pid": i.meta.pid,
                        "state": format!("{:?}", i.status.state).to_lowercase(),
                        "started_at": i.meta.started_at.to_rfc3339(),
                    })
                })
                .collect();
            Ok(json!({
                "content": [{"type":"text","text": serde_json::to_string_pretty(&arr).unwrap_or_default()}],
                "isError": false
            }))
        }
        "clicom_type" => {
            let text = s_arg(&args, "text")?;
            let partial = s_opt(&args, "partial");
            let translate = !b_opt(&args, "raw");
            let no_enter = b_opt(&args, "no_enter");
            let body = if no_enter || text.ends_with('\n') {
                text.to_string()
            } else {
                format!("{}\n", text)
            };
            run_script(
                cwd,
                partial,
                format!("type_text({}, {})", rhai_str_lit(&body), translate),
            )
        }
        "clicom_keys" => {
            let spec = s_arg(&args, "spec")?;
            let partial = s_opt(&args, "partial");
            run_script(cwd, partial, format!("type_keys({})", rhai_str_lit(spec)))
        }
        "clicom_screen" => {
            let partial = s_opt(&args, "partial");
            let no_status = b_opt(&args, "no_status");
            match crate::clicom_cli::quickops::try_dead_instance_response(
                cwd,
                partial.as_deref(),
                no_status,
                |s| s.to_string(),
            ) {
                Ok(Some(text)) => return Ok(dead_response_envelope(text)),
                Ok(None) => {}
                Err(e) => return Err((-32000, format!("dead-fallback: {e}"))),
            }
            let src = if no_status { "screen_text()" } else { "screen_text(true)" };
            run_script(cwd, partial, src.to_string())
        }
        "clicom_screen_after" => {
            let marker = s_arg(&args, "marker")?;
            let partial = s_opt(&args, "partial");
            let no_status = b_opt(&args, "no_status");
            let m = marker.to_string();
            match crate::clicom_cli::quickops::try_dead_instance_response(
                cwd,
                partial.as_deref(),
                no_status,
                |s| match s.rfind(&m) {
                    Some(idx) => s[idx + m.len()..].to_string(),
                    None => String::new(),
                },
            ) {
                Ok(Some(text)) => return Ok(dead_response_envelope(text)),
                Ok(None) => {}
                Err(e) => return Err((-32000, format!("dead-fallback: {e}"))),
            }
            let src = if no_status {
                format!("screen_last_after({})", crate::clicom_cli::quickops::rhai_str_lit(marker))
            } else {
                format!("screen_last_after({}, true)", crate::clicom_cli::quickops::rhai_str_lit(marker))
            };
            run_script(cwd, partial, src)
        }
        "clicom_screen_after_re" => {
            let pattern = s_arg(&args, "pattern")?;
            let partial = s_opt(&args, "partial");
            let no_status = b_opt(&args, "no_status");
            // Compile up-front so the dead-fallback errors on bad patterns the
            // same way the live path does (matches quickops::screen_after_re).
            let re_compiled = regex::Regex::new(pattern)
                .map_err(|e| (-32000, format!("regex compile: {e}")))?;
            match crate::clicom_cli::quickops::try_dead_instance_response(
                cwd,
                partial.as_deref(),
                no_status,
                move |s| {
                    let mut last_end: Option<usize> = None;
                    for m in re_compiled.find_iter(s) { last_end = Some(m.end()); }
                    last_end.map(|i| s[i..].to_string()).unwrap_or_default()
                },
            ) {
                Ok(Some(text)) => return Ok(dead_response_envelope(text)),
                Ok(None) => {}
                Err(e) => return Err((-32000, format!("dead-fallback: {e}"))),
            }
            let src = if no_status {
                format!("screen_last_after_re({})", crate::clicom_cli::quickops::rhai_str_lit(pattern))
            } else {
                format!("screen_last_after_re({}, true)", crate::clicom_cli::quickops::rhai_str_lit(pattern))
            };
            run_script(cwd, partial, src)
        }
        "clicom_wait_idle" => {
            let ms = i_opt(&args, "ms", 800);
            let to = i_opt(&args, "timeout_ms", 60_000);
            let partial = s_opt(&args, "partial");
            run_script(
                cwd,
                partial,
                format!("wait_idle({ms}, {to})"),
            )
        }
        "clicom_run" => {
            let source = s_arg(&args, "source")?.to_string();
            let partial = s_opt(&args, "partial");
            run_script(cwd, partial, source)
        }
        "clicom_queue" => {
            let source = s_arg(&args, "source")?.to_string();
            let partial = s_opt(&args, "partial");
            use crate::clicom_engine::meta::State;
            let candidates: Vec<_> = crate::clicom_cli::discovery::filter_by_partial(
                crate::clicom_cli::discovery::list_instances(cwd),
                partial.as_deref(),
            )
            .into_iter()
            .filter(|i| matches!(i.status.state, State::Idle | State::Busy))
            .collect();
            if candidates.len() != 1 {
                return Err((
                    -32000,
                    format!("expected 1 instance, found {}", candidates.len()),
                ));
            }
            let inst = candidates[0].dir.clone();
            let _guard = crate::clicom_cli::drop::acquire_lock(&inst)
                .map_err(|e| (-32000, format!("lock: {e}")))?;
            let id = crate::clicom_cli::drop::drop_rhai(&inst, &source)
                .map_err(|e| (-32000, format!("drop: {e}")))?;
            Ok(json!({
                "content": [{"type":"text","text": id}],
                "isError": false
            }))
        }
        "clicom_clean" => {
            let partial = s_opt(&args, "partial");
            let id = s_opt(&args, "id");
            cmd_clean::run(cwd, partial.as_deref(), id.as_deref())
                .map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({
                "content": [{"type":"text","text":"cleaned"}],
                "isError": false
            }))
        }
        "clicom_whoami" => {
            match crate::clicom_cli::cmd_whoami::resolve_self(cwd, std::process::id()) {
                Some(me) => {
                    let v = json!({
                        "dir_name": me.dir_name,
                        "path": me.dir.display().to_string(),
                        "wrapper_pid": me.meta.pid,
                        "name": me.meta.name,
                        "state": format!("{:?}", me.status.state).to_lowercase(),
                        "started_at": me.meta.started_at.to_rfc3339(),
                    });
                    Ok(json!({
                        "content": [{"type":"text","text": serde_json::to_string_pretty(&v).unwrap_or_default()}],
                        "isError": false
                    }))
                }
                None => Ok(json!({
                    "content": [{"type":"text","text": format!("not running inside a clicom-wrapped process in {}", cwd.display())}],
                    "isError": true
                })),
            }
        }
        "clicom_exec_detached" => {
            let cmd_arr = args.get("cmd").and_then(|v| v.as_array())
                .ok_or((-32602, "missing array 'cmd'".to_string()))?;
            let argv: Vec<String> = cmd_arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if argv.is_empty() {
                return Err((-32602, "empty 'cmd' array".to_string()));
            }
            let pid = crate::clicom_cli::cmd_exec_detached::spawn_detached(&argv)
                .map_err(|e| (-32000, e.to_string()))?;
            Ok(json!({
                "content": [{"type":"text","text": pid.to_string()}],
                "isError": false
            }))
        }
        _ => Err((-32602, format!("unknown tool: {name}"))),
    }
}
