//! Preparazione e aggiornamento degli strumenti esterni (yt-dlp, ffmpeg) e download HTTP.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::paths;
use crate::util::silent_command;

pub const USER_AGENT: &str = concat!("YTDownloader/", env!("CARGO_PKG_VERSION"));
const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";

pub fn http_get(url: &str) -> Result<ureq::http::Response<ureq::Body>, String> {
    ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())
}

/// Scarica un file in modo atomico (prima in `.part`, poi rename).
pub fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("part");
    let resp = http_get(url)?;
    let mut reader = resp.into_body().into_reader();
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

pub fn http_get_bytes(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let resp = http_get(url)?;
    let mut buf = Vec::new();
    resp.into_body()
        .into_reader()
        .take(limit)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Garantisce che yt-dlp sia presente in `%LOCALAPPDATA%\YTDownloader\bin`.
pub fn ensure_ytdlp() -> Result<PathBuf, String> {
    let target = paths::ytdlp_path();
    if target.is_file() {
        return Ok(target);
    }
    std::fs::create_dir_all(paths::tools_dir()).map_err(|e| e.to_string())?;
    if let Some(bundled) = paths::bundled_ytdlp() {
        if std::fs::copy(&bundled, &target).is_ok() {
            return Ok(target);
        }
    }
    download_file(YTDLP_URL, &target).map_err(|e| format!("download di yt-dlp non riuscito: {e}"))?;
    Ok(target)
}

pub fn ytdlp_version() -> Option<String> {
    let out = silent_command(&paths::ytdlp_path())
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!v.is_empty()).then_some(v)
}

pub fn ffmpeg_version() -> Option<String> {
    let dir = paths::ffmpeg_dir()?;
    let out = silent_command(&dir.join("ffmpeg.exe"))
        .arg("-version")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    // "ffmpeg version 7.1-full_build-www.gyan.dev Copyright ..."
    text.lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(2))
        .map(str::to_string)
}

/// Esegue `yt-dlp -U`. Restituisce le righe di output da mostrare nel log.
pub fn update_ytdlp() -> Result<Vec<String>, String> {
    let out = silent_command(&paths::ytdlp_path())
        .arg("-U")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&out.stderr).lines())
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect();
    if out.status.success() {
        Ok(lines)
    } else {
        lines.push(format!("yt-dlp -U: codice {}", out.status.code().unwrap_or(-1)));
        Err(lines.join("\n"))
    }
}

/// Scarica e ridimensiona una miniatura.
pub fn load_thumbnail(url: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let bytes = http_get_bytes(url, 8 * 1024 * 1024).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = img.thumbnail(320, 320).to_rgba8();
    Some(slint::SharedPixelBuffer::clone_from_slice(img.as_raw(), img.width(), img.height()))
}
