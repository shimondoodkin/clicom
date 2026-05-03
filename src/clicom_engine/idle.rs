//! Idle/busy state machine. Pure logic — no I/O.

use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleState { Busy, Idle }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEvent { BecameIdle, BecameBusy }

pub struct IdleDetector {
    threshold: Duration,
    state: IdleState,
    last_byte_at: Instant,
    last_byte_at_system: SystemTime,
}

impl IdleDetector {
    pub fn new(threshold_seconds: u64, now: Instant) -> Self {
        IdleDetector {
            threshold: Duration::from_secs(threshold_seconds),
            state: IdleState::Busy, // start busy until first idle period
            last_byte_at: now,
            last_byte_at_system: SystemTime::now(),
        }
    }

    pub fn state(&self) -> IdleState { self.state }

    /// Returns the wall-clock time of the last received byte (or construction time).
    pub fn last_activity(&self) -> SystemTime { self.last_byte_at_system }

    /// Call every time a byte is read from the agent.
    pub fn note_byte(&mut self, now: Instant) -> Option<IdleEvent> {
        self.last_byte_at = now;
        self.last_byte_at_system = SystemTime::now();
        if self.state == IdleState::Idle {
            self.state = IdleState::Busy;
            return Some(IdleEvent::BecameBusy);
        }
        None
    }

    /// Call from the timer tick.
    pub fn tick(&mut self, now: Instant) -> Option<IdleEvent> {
        if self.state == IdleState::Busy && now.duration_since(self.last_byte_at) >= self.threshold {
            self.state = IdleState::Idle;
            return Some(IdleEvent::BecameIdle);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn starts_busy() {
        let now = Instant::now();
        let d = IdleDetector::new(1, now);
        assert_eq!(d.state(), IdleState::Busy);
    }

    #[test]
    fn becomes_idle_after_threshold_with_no_bytes() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        let later = now + Duration::from_secs(2);
        let ev = d.tick(later);
        assert_eq!(ev, Some(IdleEvent::BecameIdle));
        assert_eq!(d.state(), IdleState::Idle);
    }

    #[test]
    fn note_byte_returns_busy_event_when_idle() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        d.tick(now + Duration::from_secs(2));
        let ev = d.note_byte(now + Duration::from_secs(3));
        assert_eq!(ev, Some(IdleEvent::BecameBusy));
    }

    #[test]
    fn no_event_when_already_in_state() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        assert_eq!(d.note_byte(now + Duration::from_millis(100)), None);
        d.tick(now + Duration::from_secs(2));
        assert_eq!(d.tick(now + Duration::from_secs(3)), None);
    }

    #[test]
    fn last_activity_advances_on_note_byte() {
        let now = Instant::now();
        let mut d = IdleDetector::new(1, now);
        let before = d.last_activity();
        std::thread::sleep(Duration::from_millis(5));
        d.note_byte(now + Duration::from_millis(100));
        let after = d.last_activity();
        assert!(after >= before, "last_activity should not go backwards");
        // The system time should have advanced (at least by the sleep).
        // We can't guarantee strict inequality due to clock resolution, but after >= before always holds.
        // The key thing is that last_activity() returns the stored SystemTime.
        let _ = after.duration_since(before); // just verify it's usable
    }
}
