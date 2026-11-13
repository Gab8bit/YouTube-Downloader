//! Impostazioni persistenti salvate in `%APPDATA%\YTDownloader\settings.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::paths;

pub const DEFAULT_TEMPLATE: &str = "%(title)s.%(ext)s";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output_dir: PathBuf,
    pub kind: i32,
    pub quality: i32,
    pub compat: bool,
    pub audio_format: i32,
    pub audio_quality: i32,
    pub filename_template: String,
    pub max_parallel: i32,
    /// Ultimo controllo aggiornamenti di yt-dlp (secondi Unix).
    pub ytdlp_last_update: u64,
    /// Scarica in automatico gli aggiornamenti dell'app.
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: paths::default_download_dir(),
            kind: 0,
            quality: 0,
            compat: true,
            audio_format: 0,
            audio_quality: 0,
            filename_template: DEFAULT_TEMPLATE.into(),
            max_parallel: 2,
            ytdlp_last_update: 0,
            auto_update: true,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        paths::config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let mut s: Settings = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        if !s.output_dir.is_dir() {
            s.output_dir = paths::default_download_dir();
        }
        if s.filename_template.trim().is_empty() {
            s.filename_template = DEFAULT_TEMPLATE.into();
        }
        s.max_parallel = s.max_parallel.clamp(1, 6);
        s
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            // Scrittura atomica: file temporaneo + rename.
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}
