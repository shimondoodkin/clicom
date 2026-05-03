//! Renders the nudge text and writes it to the ConPTY's stdin.

use std::io::Write;

pub fn render(template: &str, count: u32) -> String {
    template.replace("{count}", &count.to_string())
}

/// Inject the rendered nudge into the master writer. (Currently unused outside tests
/// because nudges are routed through the input-forwarder channel instead.)
#[allow(dead_code)]
pub fn inject(writer: &mut dyn Write, text: &str) -> anyhow::Result<()> {
    writer.write_all(text.as_bytes())?;
    writer.flush()?;
    Ok(())
}
