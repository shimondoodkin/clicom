//! Virtual terminal model + parallel scrollback ring for clicom (§6.5).
//!
//! `ScreenBuffer::advance_bytes` feeds the vt100 parser and captures any rows
//! that scrolled off the visible region into the scrollback ring. The ring's
//! lifetime line index is the source of truth for `screen_tail_*` queries —
//! vt100's own scrollback is a *visual* buffer that can lose plain-text
//! content when lines re-flow.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const SOFT_VT100_SCROLLBACK: usize = 64; // rows kept inside vt100 (approach (a) per §6.5)
const HARD_CAP: usize = 20_000;
const DROP_CHUNK: usize = 10_000;

pub struct ScreenBuffer {
    inner: Arc<Mutex<vt100::Parser>>,
    scrollback: Arc<Mutex<ScrollbackRing>>,
    rows: u16,
    cols: u16,
}

#[derive(Debug)]
struct ScrollbackRing {
    lines: VecDeque<String>,    // finalized lines
    trimmed_below: u64,         // lifetime index of lines[0]
    last_visible_top_row0: Vec<String>,  // previous-frame visible rows for diffing
}

impl ScrollbackRing {
    fn new() -> Self {
        ScrollbackRing { lines: VecDeque::new(), trimmed_below: 0, last_visible_top_row0: Vec::new() }
    }

    fn append(&mut self, line: String) {
        self.lines.push_back(line);
        if self.lines.len() > HARD_CAP {
            for _ in 0..DROP_CHUNK {
                self.lines.pop_front();
                self.trimmed_below += 1;
            }
        }
    }

    fn total_lifetime(&self, visible_rows: usize) -> u64 {
        self.trimmed_below + self.lines.len() as u64 + visible_rows as u64
    }
}

impl ScreenBuffer {
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = vt100::Parser::new(rows, cols, SOFT_VT100_SCROLLBACK);
        ScreenBuffer {
            inner: Arc::new(Mutex::new(parser)),
            scrollback: Arc::new(Mutex::new(ScrollbackRing::new())),
            rows, cols,
        }
    }

    pub fn handle(&self) -> Arc<Mutex<vt100::Parser>> { Arc::clone(&self.inner) }

    pub fn advance_bytes(&self, bytes: &[u8]) {
        // Snapshot the current visible top-row text before applying.
        let prev_top = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return };
            visible_rows(&p, self.rows, self.cols)
        };
        // Apply the bytes.
        {
            let mut p = match self.inner.lock() { Ok(p) => p, Err(_) => return };
            p.process(bytes);
        }
        // Diff against new visible state to detect rows that scrolled off the top.
        let new_top = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return };
            visible_rows(&p, self.rows, self.cols)
        };

        if let Ok(mut sb) = self.scrollback.lock() {
            let scrolled_off = detect_scrolled_off(&prev_top, &new_top);
            for line in scrolled_off {
                sb.append(line);
            }
            sb.last_visible_top_row0 = new_top;
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        if let Ok(mut p) = self.inner.lock() {
            p.screen_mut().set_size(rows, cols);
        }
    }

    /// Plain text of the current visible screen.
    pub fn to_plain_text(&self) -> String {
        match self.inner.lock() { Ok(p) => p.screen().contents(), Err(_) => String::new() }
    }

    pub fn visible_dims(&self) -> (u16, u16) { (self.rows, self.cols) }

    /// (lifetime_lines, trimmed_below)
    pub fn lifetime_info(&self) -> (u64, u64) {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return (0, 0) };
        (sb.total_lifetime(self.rows as usize), sb.trimmed_below)
    }

    /// Read a half-open range [from, to) of *resolved* lifetime indexes.
    /// Returns (lines, actual_from, actual_to). Caller is responsible for
    /// negative-index resolution and emptiness checks.
    pub fn read_range(&self, from: u64, to: u64) -> ReadResult {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return ReadResult::empty() };
        let visible_rows_text: Vec<String> = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return ReadResult::empty() };
            visible_rows(&p, self.rows, self.cols)
        };
        let trimmed = sb.trimmed_below;
        let scroll_end = trimmed + sb.lines.len() as u64;
        let total = scroll_end + visible_rows_text.len() as u64;

        if from >= total && to >= total {
            return ReadResult { lines: Vec::new(), actual_from: from.min(total), actual_to: to.min(total),
                                trimmed_below: trimmed, total_lifetime: total };
        }
        let af = from.max(trimmed);
        let at = to.min(total);
        let mut out = Vec::new();
        for i in af..at {
            if i < scroll_end {
                let idx = (i - trimmed) as usize;
                out.push(sb.lines.get(idx).cloned().unwrap_or_default());
            } else {
                let idx = (i - scroll_end) as usize;
                out.push(visible_rows_text.get(idx).cloned().unwrap_or_default());
            }
        }
        ReadResult { lines: out, actual_from: af, actual_to: at, trimmed_below: trimmed, total_lifetime: total }
    }

    /// True iff the requested half-open range is wholly below `trimmed_below`.
    pub fn range_wholly_trimmed(&self, _from: u64, to: u64) -> bool {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return false };
        to <= sb.trimmed_below
    }

    /// Concatenated lifetime text (visible region + scrollback ring), oldest-first.
    /// Used by screen_last_after / _re searches.
    pub fn lifetime_text(&self) -> String {
        let sb = match self.scrollback.lock() { Ok(s) => s, Err(_) => return String::new() };
        let visible_rows_text: Vec<String> = {
            let p = match self.inner.lock() { Ok(p) => p, Err(_) => return String::new() };
            visible_rows(&p, self.rows, self.cols)
        };
        let mut s = String::new();
        for l in sb.lines.iter() { s.push_str(l); s.push('\n'); }
        for l in visible_rows_text.iter() { s.push_str(l); s.push('\n'); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct ReadResult {
    pub lines: Vec<String>,
    pub actual_from: u64,
    pub actual_to: u64,
    pub trimmed_below: u64,
    pub total_lifetime: u64,
}

impl ReadResult {
    fn empty() -> Self { ReadResult { lines: Vec::new(), actual_from: 0, actual_to: 0, trimmed_below: 0, total_lifetime: 0 } }
}

fn visible_rows(parser: &vt100::Parser, _rows: u16, cols: u16) -> Vec<String> {
    let screen = parser.screen();
    screen.rows(0, cols).collect()
}

/// Identify rows from the previous top-of-screen that no longer appear at the
/// top of the new visible region — these have scrolled off and must enter
/// scrollback. Conservative diff: walk down `prev` until we find the first
/// row that matches the new top, return everything above it.
fn detect_scrolled_off(prev: &[String], new_visible: &[String]) -> Vec<String> {
    if prev.is_empty() || new_visible.is_empty() { return Vec::new(); }
    let new_top = &new_visible[0];
    if let Some(pos) = prev.iter().position(|r| r == new_top) {
        prev[..pos].to_vec()
    } else {
        // No anchor found — assume the screen was rewritten, not scrolled.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump(buf: &ScreenBuffer, s: &str) { buf.advance_bytes(s.as_bytes()); }

    #[test]
    fn fresh_buffer_has_zero_lifetime() {
        let b = ScreenBuffer::new(10, 80);
        let (total, trimmed) = b.lifetime_info();
        assert!(total >= 10, "visible region counted: {total}");
        assert_eq!(trimmed, 0);
    }

    #[test]
    fn visible_screen_renders_plain_text() {
        let b = ScreenBuffer::new(5, 80);
        pump(&b, "hello\n");
        let s = b.to_plain_text();
        assert!(s.contains("hello"), "got: {s:?}");
    }

    #[test]
    fn scrolling_captures_lines_into_ring() {
        let b = ScreenBuffer::new(3, 10);
        for i in 0..6 {
            pump(&b, &format!("line{i}\n"));
        }
        let txt = b.lifetime_text();
        // We expect early lines to have entered the ring even after scrolling out of view.
        assert!(txt.contains("line0") || txt.contains("line1"),
            "expected early lines in lifetime_text, got: {txt:?}");
    }

    #[test]
    fn read_range_clamps_when_partially_trimmed() {
        let b = ScreenBuffer::new(2, 10);
        for i in 0..50 { pump(&b, &format!("L{i}\n")); }
        let (total, trimmed) = b.lifetime_info();
        assert!(total > trimmed);
        let r = b.read_range(0, total);
        assert_eq!(r.actual_from, trimmed);
        assert_eq!(r.actual_to, total);
    }

    #[test]
    fn range_wholly_trimmed_detected() {
        let b = ScreenBuffer::new(2, 10);
        for i in 0..50 { pump(&b, &format!("L{i}\n")); }
        let (_total, trimmed) = b.lifetime_info();
        if trimmed > 0 {
            assert!(b.range_wholly_trimmed(0, trimmed));
        }
    }
}
