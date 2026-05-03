//! Filter VT private-mode mouse-tracking sequences out of an output byte stream.
//!
//! Modern Windows terminals interpret `CSI ? <n> h` mouse-mode enable sequences
//! and switch into mouse-capture mode, which breaks click-drag text selection.
//! Wrapped agents (e.g. Claude Code) emit these unprompted. We strip them from
//! the host-stdout stream while leaving them in the tap (so the snapshot vt100
//! parser sees what the agent intended).
//!
//! Mouse mode params we recognize: 9 (X10), 1000-1006 (button/cell/all motion,
//! focus events, UTF-8 mouse, SGR mouse), 1015 (urxvt mouse), 1016 (SGR pixel).

const MOUSE_MODES: &[u32] = &[9, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1015, 1016];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    EscSeen,        // saw 0x1B
    CsiParams,      // saw 0x1B [ — accumulating params, may or may not become private
    CsiPrivate,     // saw 0x1B [ ? — accumulating private params
}

pub struct MouseFilter {
    state: State,
    buf: Vec<u8>,
}

impl MouseFilter {
    pub fn new() -> Self {
        MouseFilter { state: State::Normal, buf: Vec::with_capacity(32) }
    }

    /// Feed input bytes; returns the filtered bytes that should reach stdout.
    /// Partial escape sequences are buffered across calls.
    pub fn process(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            match self.state {
                State::Normal => {
                    if b == 0x1B {
                        self.state = State::EscSeen;
                        self.buf.clear();
                        self.buf.push(b);
                    } else {
                        out.push(b);
                    }
                }
                State::EscSeen => {
                    self.buf.push(b);
                    if b == b'[' {
                        self.state = State::CsiParams;
                    } else {
                        // ESC followed by something other than `[`. Flush
                        // accumulated bytes and resume normal copying.
                        out.extend_from_slice(&self.buf);
                        self.buf.clear();
                        self.state = State::Normal;
                    }
                }
                State::CsiParams => {
                    self.buf.push(b);
                    // Right after `ESC[`, a `?` marks DEC private mode.
                    if b == b'?' && self.buf.len() == 3 {
                        self.state = State::CsiPrivate;
                    } else if (0x40..=0x7E).contains(&b) {
                        // Final byte of a non-private CSI sequence — pass through.
                        out.extend_from_slice(&self.buf);
                        self.buf.clear();
                        self.state = State::Normal;
                    }
                    // Otherwise: still inside parameter bytes — keep accumulating.
                }
                State::CsiPrivate => {
                    self.buf.push(b);
                    if (0x40..=0x7E).contains(&b) {
                        let drop_it = (b == b'h' || b == b'l') && self.params_contain_mouse_mode();
                        if !drop_it {
                            out.extend_from_slice(&self.buf);
                        }
                        self.buf.clear();
                        self.state = State::Normal;
                    }
                }
            }
        }
        out
    }

    fn params_contain_mouse_mode(&self) -> bool {
        // self.buf is `ESC [ ? <params> <final>`. Params start at index 3, end at len-1.
        if self.buf.len() < 5 { return false; }
        let params = &self.buf[3..self.buf.len() - 1];
        for part in params.split(|&c| c == b';') {
            if let Ok(s) = std::str::from_utf8(part) {
                if let Ok(n) = s.parse::<u32>() {
                    if MOUSE_MODES.contains(&n) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Default for MouseFilter {
    fn default() -> Self { Self::new() }
}
