//! Status trailer line for screen-query tools.
//!
//! Format: `[clicom: state=<word>  last_activity=<rfc3339-Z>  visible_rows=<n>]`
//! Spec: `docs/superpowers/specs/2026-05-07-screen-status-trailer-design.md`.

use chrono::{DateTime, Utc};
use std::fmt;

use crate::clicom_engine::meta::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailerState {
    Idle,
    Active,
    Exited,
    Died,
}

impl fmt::Display for TrailerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let w = match self {
            TrailerState::Idle => "idle",
            TrailerState::Active => "active",
            TrailerState::Exited => "exited",
            TrailerState::Died => "died",
        };
        f.write_str(w)
    }
}

impl From<State> for TrailerState {
    fn from(s: State) -> Self {
        match s {
            State::Idle => TrailerState::Idle,
            State::Busy => TrailerState::Active,
            State::Exited => TrailerState::Exited,
            State::Died => TrailerState::Died,
        }
    }
}

pub fn format(state: TrailerState, last_activity: DateTime<Utc>, visible_rows: u16) -> String {
    let ts = last_activity.format("%Y-%m-%dT%H:%M:%SZ");
    format!("[clicom: state={state}  last_activity={ts}  visible_rows={visible_rows}]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 7, 1, 34, 12).unwrap()
    }

    #[test]
    fn display_emits_lowercase_words() {
        assert_eq!(TrailerState::Idle.to_string(), "idle");
        assert_eq!(TrailerState::Active.to_string(), "active");
        assert_eq!(TrailerState::Exited.to_string(), "exited");
        assert_eq!(TrailerState::Died.to_string(), "died");
    }

    #[test]
    fn from_state_maps_busy_to_active() {
        assert_eq!(TrailerState::from(State::Idle), TrailerState::Idle);
        assert_eq!(TrailerState::from(State::Busy), TrailerState::Active);
        assert_eq!(TrailerState::from(State::Exited), TrailerState::Exited);
        assert_eq!(TrailerState::from(State::Died), TrailerState::Died);
    }

    #[test]
    fn format_matches_spec_example() {
        let s = format(TrailerState::Idle, fixed_ts(), 40);
        assert_eq!(
            s,
            "[clicom: state=idle  last_activity=2026-05-07T01:34:12Z  visible_rows=40]"
        );
    }

    #[test]
    fn format_double_space_separators() {
        let s = format(TrailerState::Active, fixed_ts(), 24);
        assert!(s.contains("=active  last_activity="));
        assert!(s.contains("Z  visible_rows=24"));
        assert!(s.ends_with("]"));
        assert!(!s.ends_with(" ]"));
    }
}
