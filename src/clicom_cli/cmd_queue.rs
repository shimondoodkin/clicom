//! `clicom queue` — drop the script, exit immediately (asynchronous).

use anyhow::Result;
use std::path::Path;

use crate::clicom_cli::{discovery, drop as drop_h};
use crate::clicom_engine::meta::State;

pub fn run(cwd: &Path, partial: Option<&str>, source: String) -> Result<i32> {
    let candidates: Vec<_> = discovery::filter_by_partial(discovery::list_instances(cwd), partial)
        .into_iter().filter(|i| matches!(i.status.state, State::Idle | State::Busy)).collect();
    let inst = match candidates.len() {
        0 => { eprintln!("no live wrapped agent in {}", cwd.display()); return Ok(2); }
        1 => &candidates[0].dir,
        _ => {
            eprintln!("ambiguous match — candidates:");
            for i in &candidates { eprintln!("  {}", i.dir_name); }
            return Ok(2);
        }
    };
    let _guard = drop_h::acquire_lock(inst)?;
    let id = drop_h::drop_rhai(inst, &source)?;
    std::mem::drop(_guard);  // explicit
    println!("{id}");
    Ok(0)
}
