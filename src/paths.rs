//! Percorsi dell'applicazione e ricerca degli eseguibili esterni (yt-dlp, ffmpeg).

use std::path::PathBuf;

pub const APP_DIR_NAME: &str = "YTDownloader";

/// `%LOCALAPPDATA%\YTDownloader` — dati locali (yt-dlp aggiornabile, cache).
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_DIR_NAME)
}

/// `%APPDATA%\YTDownloader` — impostazioni.
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_DIR_NAME)
}

pub fn tools_dir() -> PathBuf {
    data_dir().join("bin")
}

pub fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// Cartelle in cui cercare gli eseguibili inclusi con l'app, in ordine di priorità.
fn bundled_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(exe) = exe_dir() {
        dirs.push(exe.join("bin"));
        dirs.push(exe.clone());
    }
    // In sviluppo (`cargo run`) usa la cartella `bin/` del progetto.
    #[cfg(debug_assertions)]
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin"));
    dirs
}

/// yt-dlp vive in `%LOCALAPPDATA%` così può aggiornarsi da solo senza permessi admin.
pub fn ytdlp_path() -> PathBuf {
    tools_dir().join("yt-dlp.exe")
}

/// Una copia di yt-dlp distribuita con l'app (usata al primo avvio, senza rete).
pub fn bundled_ytdlp() -> Option<PathBuf> {
    bundled_dirs()
        .into_iter()
        .map(|d| d.join("yt-dlp.exe"))
        .find(|p| p.is_file())
}

/// Cartella che contiene `ffmpeg.exe` (e `ffprobe.exe`).
pub fn ffmpeg_dir() -> Option<PathBuf> {
    let mut candidates = vec![tools_dir()];
    candidates.extend(bundled_dirs());
    candidates
        .into_iter()
        .find(|d| d.join("ffmpeg.exe").is_file())
}
