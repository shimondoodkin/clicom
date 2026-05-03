//! `clicom status` — read-only inspection (§5.1 status section).

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::discovery;

pub fn run(cwd: &Path, partial: Option<&str>) -> Result<i32> {
    let mut items = discovery::list_instances(cwd);
    items = discovery::filter_by_partial(items, partial);

    if items.is_empty() {
        eprintln!("no clicom instances in {}", cwd.display());
        return Ok(2);
    }

    // Sort: live first (idle/busy), then dead (exited/died), each by started_at desc.
    items.sort_by(|a, b| {
        use crate::clicom_engine::meta::State;
        let live = |s: State| matches!(s, State::Idle | State::Busy);
        live(b.status.state).cmp(&live(a.status.state))
            .then(b.meta.started_at.cmp(&a.meta.started_at))
    });

    if items.len() == 1 && partial.is_some() {
        // Detail view: dump full meta + status JSON.
        println!("{}", serde_json::to_string_pretty(&items[0].meta)?);
        println!("{}", serde_json::to_string_pretty(&items[0].status)?);
    } else {
        // Row view.
        for it in &items {
            let exit = it.status.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
            println!(
                "{:24}  {:7}  {:16}  {}  {}  {}",
                it.dir_name,
                format!("{:?}", it.status.state).to_lowercase(),
                it.meta.name,
                it.meta.started_at,
                it.status.last_activity,
                exit,
            );
        }
    }
    Ok(0)
}
