//! Host console raw VT mode setup with RAII restore.
//!
//! On Windows, stdin defaults to cooked/line-buffered mode and stdout defaults
//! to NOT interpreting VT escape sequences. A transparent ConPTY wrapper needs
//! the opposite: stdin must deliver every keystroke as raw bytes, and stdout
//! must interpret VT escapes emitted by ConPTY so cursor positioning, clears,
//! and alt-screen toggles land correctly.
//!
//! On Unix, stdin also defaults to canonical (line-buffered) mode with echo
//! on — meaning arrow keys get echoed by the kernel as `^[[C`/`^[[D` and
//! bytes only reach the child after the user hits Enter. The Unix impl puts
//! the tty into per-character no-echo mode with signal generation disabled,
//! so keystrokes flow straight through to the wrapped agent.
//!
//! `enter_raw()` reads and saves the original modes, sets the wrapper modes,
//! and returns a guard. The guard's Drop restores the originals — so panics,
//! early returns, or normal exit all leave the user's terminal as we found it.

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE,
        DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_INSERT_MODE,
        ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_PROCESSED_OUTPUT,
        ENABLE_QUICK_EDIT_MODE, ENABLE_VIRTUAL_TERMINAL_INPUT,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    pub struct ConsoleModeGuard {
        stdin: Option<(HANDLE, CONSOLE_MODE)>,
        stdout: Option<(HANDLE, CONSOLE_MODE)>,
    }

    pub fn enter_raw() -> anyhow::Result<ConsoleModeGuard> {
        let stdin = configure_stdin();
        let stdout = configure_stdout();
        Ok(ConsoleModeGuard { stdin, stdout })
    }

    fn configure_stdin() -> Option<(HANDLE, CONSOLE_MODE)> {
        unsafe {
            let h = GetStdHandle(STD_INPUT_HANDLE).ok()?;
            let mut original = CONSOLE_MODE(0);
            if GetConsoleMode(h, &mut original).is_err() {
                return None;
            }
            let cleared = original.0
                & !ENABLE_LINE_INPUT.0
                & !ENABLE_ECHO_INPUT.0
                & !ENABLE_PROCESSED_INPUT.0
                & !ENABLE_QUICK_EDIT_MODE.0
                & !ENABLE_INSERT_MODE.0;
            let raw = CONSOLE_MODE(cleared | ENABLE_VIRTUAL_TERMINAL_INPUT.0);
            if SetConsoleMode(h, raw).is_err() {
                return None;
            }
            Some((h, original))
        }
    }

    fn configure_stdout() -> Option<(HANDLE, CONSOLE_MODE)> {
        unsafe {
            let h = GetStdHandle(STD_OUTPUT_HANDLE).ok()?;
            let mut original = CONSOLE_MODE(0);
            if GetConsoleMode(h, &mut original).is_err() {
                return None;
            }
            let new_mode = CONSOLE_MODE(
                original.0
                    | ENABLE_PROCESSED_OUTPUT.0
                    | ENABLE_VIRTUAL_TERMINAL_PROCESSING.0
                    | DISABLE_NEWLINE_AUTO_RETURN.0,
            );
            if SetConsoleMode(h, new_mode).is_err() {
                return None;
            }
            Some((h, original))
        }
    }

    impl Drop for ConsoleModeGuard {
        fn drop(&mut self) {
            unsafe {
                if let Some((h, mode)) = self.stdin.take() {
                    let _ = SetConsoleMode(h, mode);
                }
                if let Some((h, mode)) = self.stdout.take() {
                    let _ = SetConsoleMode(h, mode);
                }
            }
        }
    }
}

#[cfg(unix)]
mod imp {
    // Unix mirror of the Windows configure_stdin path: drop the tty out of
    // canonical/echo mode so arrow keys and other escape sequences flow
    // through the input forwarder as raw bytes rather than being line-buffered
    // and echoed locally as `^[[C` / `^[[D`. ISIG/IXON are also cleared so
    // Ctrl-C / Ctrl-S pass through to the wrapped child (matches Windows,
    // which clears ENABLE_PROCESSED_INPUT). c_oflag is intentionally left
    // alone so normal stdout newline handling still works.

    pub struct ConsoleModeGuard {
        original: Option<libc::termios>,
    }

    pub fn enter_raw() -> anyhow::Result<ConsoleModeGuard> {
        unsafe {
            if libc::isatty(libc::STDIN_FILENO) == 0 {
                return Ok(ConsoleModeGuard { original: None });
            }
            let mut original: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut original) != 0 {
                return Ok(ConsoleModeGuard { original: None });
            }
            let mut raw = original;
            raw.c_lflag &= !(libc::ICANON
                | libc::ECHO
                | libc::ECHONL
                | libc::ISIG
                | libc::IEXTEN);
            raw.c_iflag &= !(libc::IGNBRK
                | libc::BRKINT
                | libc::PARMRK
                | libc::ISTRIP
                | libc::INLCR
                | libc::IGNCR
                | libc::ICRNL
                | libc::IXON);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) != 0 {
                return Ok(ConsoleModeGuard { original: None });
            }
            Ok(ConsoleModeGuard { original: Some(original) })
        }
    }

    impl Drop for ConsoleModeGuard {
        fn drop(&mut self) {
            if let Some(orig) = self.original.take() {
                unsafe {
                    let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
                }
            }
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod imp {
    pub struct ConsoleModeGuard;
    pub fn enter_raw() -> anyhow::Result<ConsoleModeGuard> {
        Ok(ConsoleModeGuard)
    }
}

pub use imp::{enter_raw, ConsoleModeGuard};

/// Force-disable every standard VT mouse-tracking mode on the host stdout.
/// Use this when `--mouse` is NOT set: it guarantees mouse capture is off
/// even if a prior program left a mode enabled. The output forwarder then
/// strips any mouse-enable sequences the wrapped TUI emits, so mouse stays
/// off for the whole wrapper lifetime.
///
/// Modes disabled (DEC private mode reset): 9 (X10), 1000-1006 (button/cell/
/// motion/focus/UTF-8/SGR), 1015 (urxvt), 1016 (SGR pixel).
pub fn disable_mouse_modes() {
    use std::io::Write;
    let seq = b"\x1b[?9;1000;1001;1002;1003;1004;1005;1006;1015;1016l";
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(seq);
    let _ = stdout.flush();
}
