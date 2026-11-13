//! Piccole funzioni di supporto: formattazione e apertura di file/cartelle/URL.

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Flag Win32 per non mostrare la finestra console dei processi figli.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn human_bytes(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if !bytes.is_finite() || bytes <= 0.0 {
        return String::new();
    }
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn human_speed(bytes_per_sec: f64) -> String {
    let s = human_bytes(bytes_per_sec);
    if s.is_empty() {
        s
    } else {
        format!("{s}/s")
    }
}

pub fn human_duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return String::new();
    }
    let total = secs.round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn open_path(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn();
}

/// Apre Esplora file con il file selezionato.
pub fn reveal_file(path: &Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    if let Some(parent) = path.parent() {
        open_path(parent);
    }
}

pub fn open_url(url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") {
        let _ = Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
}

/// Crea un `Command` senza finestra console e con output UTF-8 (yt-dlp è un'app Python).
pub fn silent_command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.env("PYTHONIOENCODING", "utf-8").env("PYTHONUTF8", "1");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(human_bytes(0.0), "");
        assert_eq!(human_bytes(512.0), "512 B");
        assert_eq!(human_bytes(1536.0), "1.5 KB");
        assert_eq!(human_bytes(250.0 * 1024.0 * 1024.0), "250 MB");
    }

    #[test]
    fn formats_duration() {
        assert_eq!(human_duration(65.0), "1:05");
        assert_eq!(human_duration(3725.0), "1:02:05");
    }
}
