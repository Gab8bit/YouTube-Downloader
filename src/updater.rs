//! Aggiornamento automatico dell'app dalle GitHub Releases.
//!
//! 1. `check()` legge l'ultima release pubblicata e la confronta con la versione corrente.
//! 2. `download()` scarica il setup in `%LOCALAPPDATA%\YTDownloader\updates` e ne verifica
//!    lo SHA-256 con il file `SHA256SUMS.txt` della release.
//! 3. `launch_installer()` avvia il setup in modalità silenziosa; l'app poi si chiude.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths;
use crate::tools::http_get;

pub const REPO: &str = "Gab8bit/YouTube-Downloader";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub page_url: String,
    pub setup_name: String,
    pub setup_url: String,
    pub sums_url: Option<String>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

/// "v4.1.0" / "4.1.0-beta" → (4, 1, 0)
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().unwrap_or(Ok(0)).ok()?;
    let patch = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn is_trusted_url(url: &str) -> bool {
    url.starts_with("https://github.com/") || url.starts_with("https://objects.githubusercontent.com/")
}

/// Restituisce la release più recente se è più nuova della versione in uso.
pub fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url)
        .header("User-Agent", crate::tools::USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call();
    let mut resp = match resp {
        Ok(r) => r,
        // Nessuna release pubblicata.
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let rel: GhRelease = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    if rel.draft || rel.prerelease || !is_newer(&rel.tag_name, CURRENT_VERSION) {
        return Ok(None);
    }
    let setup = rel
        .assets
        .iter()
        .find(|a| a.name.starts_with("YTDownloader-Setup") && a.name.ends_with(".exe"))
        .ok_or("la release non contiene il setup")?;
    if !is_trusted_url(&setup.browser_download_url) {
        return Err("URL del setup non attendibile".into());
    }
    let sums_url = rel
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        .map(|a| a.browser_download_url.clone());
    Ok(Some(Release {
        version: rel.tag_name.trim_start_matches(['v', 'V']).to_string(),
        page_url: rel.html_url,
        setup_name: setup.name.clone(),
        setup_url: setup.browser_download_url.clone(),
        sums_url,
    }))
}

fn expected_hash(rel: &Release) -> Result<Option<String>, String> {
    let Some(url) = &rel.sums_url else { return Ok(None) };
    let bytes = crate::tools::http_get_bytes(url, 64 * 1024)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        let hash = it.next()?;
        let name = it.next()?.trim_start_matches('*');
        (name == rel.setup_name && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    }))
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn updates_dir() -> PathBuf {
    paths::data_dir().join("updates")
}

/// Scarica e verifica il setup. `progress` riceve valori 0..1.
pub fn download(rel: &Release, progress: impl Fn(f32)) -> Result<PathBuf, String> {
    let dir = updates_dir();
    // Rimuove setup vecchi.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&rel.setup_name);
    let tmp = dir.join(format!("{}.part", rel.setup_name));

    let expected = expected_hash(rel)?;

    let resp = http_get(&rel.setup_url)?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let mut reader = resp.into_body().into_reader();
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 16];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if let Some(t) = total.filter(|t| *t > 0) {
            progress((done as f64 / t as f64).min(1.0) as f32);
        }
    }
    drop(file);

    if let Some(expected) = expected {
        let actual = file_sha256(&tmp)?;
        if actual != expected {
            let _ = std::fs::remove_file(&tmp);
            return Err("verifica SHA-256 del setup non riuscita".into());
        }
    }
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Avvia il setup in modalità silenziosa. Con `relaunch` l'app viene riaperta a fine installazione.
pub fn launch_installer(setup: &Path, relaunch: bool) -> Result<(), String> {
    let mut args = vec!["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"];
    if relaunch {
        args.push("/relaunch=1");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Il setup deve sopravvivere alla chiusura dell'app anche se questa è in un Job Object.
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        if Command::new(setup).args(&args).creation_flags(CREATE_BREAKAWAY_FROM_JOB).spawn().is_ok() {
            return Ok(());
        }
    }
    Command::new(setup).args(&args).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v4.0.1", "4.0.0"));
        assert!(is_newer("4.1", "4.0.9"));
        assert!(is_newer("v5.0.0-beta", "4.9.9"));
        assert!(!is_newer("v4.0.0", "4.0.0"));
        assert!(!is_newer("v3.0.1", "4.0.0"));
        assert!(!is_newer("latest", "4.0.0"));
    }
}
