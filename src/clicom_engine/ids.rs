//! Id generation helpers.
//!
//! - `rand6()`         → 6-char lowercase hex token (used in instance dir names and <id>s)
//! - `unix_nanos()`    → current Unix time in nanoseconds (for sortable <id>s)
//! - `make_command_id()` → "<unix_nanos>-<rand6>" form used for commands/<id>.rhai

use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn rand6() -> String {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..0x0100_0000); // 24 bits
    format!("{:06x}", n)
}

pub fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

pub fn make_command_id() -> String {
    format!("{}-{}", unix_nanos(), rand6())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rand6_is_six_hex_chars() {
        for _ in 0..100 {
            let s = rand6();
            assert_eq!(s.len(), 6);
            assert!(s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn rand6_collision_is_unlikely() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            seen.insert(rand6());
        }
        // ~1000 random 24-bit values should not repeat at this scale; ≥ 990 unique is fine
        assert!(seen.len() > 990);
    }

    #[test]
    fn make_command_id_has_two_parts() {
        let id = make_command_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].parse::<u128>().is_ok());
        assert_eq!(parts[1].len(), 6);
    }

    #[test]
    fn ids_are_sortable_by_drop_time() {
        let a = make_command_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = make_command_id();
        assert!(a < b, "{} should sort before {}", a, b);
    }
}
