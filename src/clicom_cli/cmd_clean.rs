//! `clicom clean` — delete result triples (.out / .err / .done) under lock.

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::{discovery, drop as drop_h};
use crate::clicom_engine::layout;

pub fn run(cwd: &Path, partial: Option<&str>, id: Option<&str>) -> Result<i32> {
    // §5.3: state filter widened — clean works on any state.
    let candidates = discovery::filter_by_partial(discovery::list_instances(cwd), partial);
    let inst = match candidates.len() {
        0 => { eprintln!("no clicom instance in {}", cwd.display()); return Ok(2); }
        1 => &candidates[0].dir,
        _ => {
            eprintln!("ambiguous match — candidates:");
            for i in &candidates { eprintln!("  {}", i.dir_name); }
            return Ok(2);
        }
    };

    let _guard = drop_h::acquire_lock(inst)?;
    let cmds = layout::commands_dir(inst);

    if let Some(id) = id {
        for ext in &["out", "err", "done"] {
            let _ = std::fs::remove_file(cmds.join(format!("{id}.{ext}")));
        }
    } else {
        // Sweep mode — only triples whose .done exists.
        let mut done_ids: Vec<String> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&cmds) {
            for e in rd.flatten() {
                if let Some(name) = e.file_name().to_str() {
                    if let Some(id) = name.strip_suffix(".done") {
                        done_ids.push(id.to_string());
                    }
                }
            }
        }
        for id in &done_ids {
            for ext in &["out", "err", "done"] {
                let _ = std::fs::remove_file(cmds.join(format!("{id}.{ext}")));
            }
        }
    }
    Ok(0)
}
