//! pid-alive check via sysinfo.

use sysinfo::{Pid, System};

/// Returns true if a process with this PID currently exists.
pub fn pid_is_alive(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_processes();
    sys.process(Pid::from_u32(pid)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_pid_is_alive() {
        let me = std::process::id();
        assert!(pid_is_alive(me));
    }

    #[test]
    fn obviously_dead_pid_is_not_alive() {
        // PID 0 / max-u32 should not match a real process on supported platforms.
        // On Windows, PID 0 is the System Idle Process and is always alive;
        // use a high PID that is virtually guaranteed to be unallocated instead.
        #[cfg(not(windows))]
        assert!(!pid_is_alive(0));
        assert!(!pid_is_alive(u32::MAX));
        // Additional high-value PID unlikely to be allocated on any platform.
        assert!(!pid_is_alive(4_000_000));
    }
}
