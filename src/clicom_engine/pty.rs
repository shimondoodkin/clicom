//! ConPTY setup via portable-pty.

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};

pub struct PtyHandles {
    pub pair: PtyPair,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub fn spawn(cmd: Vec<String>, size: PtySize) -> anyhow::Result<PtyHandles> {
    if cmd.is_empty() {
        anyhow::bail!("no command to wrap");
    }
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(size)?;

    let mut builder = CommandBuilder::new(&cmd[0]);
    for arg in &cmd[1..] { builder.arg(arg); }
    // Inherit the wrapper's environment so the child can find inboxmcp.exe etc.
    builder.cwd(std::env::current_dir()?);

    let child = pair.slave.spawn_command(builder)?;
    Ok(PtyHandles { pair, child })
}

pub fn current_terminal_size() -> PtySize {
    // Try to read the host terminal's size; fall back to 80x24.
    if let Some((w, h)) = term_size_from_console() {
        return PtySize { rows: h, cols: w, pixel_width: 0, pixel_height: 0 };
    }
    PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }
}

#[cfg(windows)]
fn term_size_from_console() -> Option<(u16, u16)> {
    use windows::Win32::System::Console::{
        GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO, STD_OUTPUT_HANDLE,
    };
    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE).ok()?;
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        if GetConsoleScreenBufferInfo(h, &mut info).is_ok() {
            let cols = (info.srWindow.Right - info.srWindow.Left + 1) as u16;
            let rows = (info.srWindow.Bottom - info.srWindow.Top + 1) as u16;
            return Some((cols.max(1), rows.max(1)));
        }
        None
    }
}

#[cfg(not(windows))]
fn term_size_from_console() -> Option<(u16, u16)> { None }
