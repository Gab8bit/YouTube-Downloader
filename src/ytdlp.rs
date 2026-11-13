//! Integrazione con yt-dlp: lettura info, costruzione argomenti, parsing dell'output.

use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use crate::paths;
use crate::proc::ProcessGroup;
use crate::util::{human_duration, silent_command};

// ───────────────────────── Info ─────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MediaInfo {
    pub url: String,
    pub title: String,
    pub uploader: String,
    pub duration: String,
    pub thumbnail: Option<String>,
    pub is_playlist: bool,
    /// L'URL punta a un video specifico (eventualmente dentro una playlist).
    pub has_video: bool,
    pub count: i32,
}

fn str_field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str).filter(|s| !s.is_empty()))
        .unwrap_or_default()
        .to_string()
}

fn best_thumbnail(v: &Value) -> Option<String> {
    // Preferisce una miniatura JPG di dimensione media (più leggera della maxres).
    if let Some(id) = v.get("id").and_then(Value::as_str) {
        let extractor = v.get("extractor_key").and_then(Value::as_str).unwrap_or("");
        let ie = v.get("ie_key").and_then(Value::as_str).unwrap_or("");
        if extractor == "Youtube" || ie == "Youtube" {
            return Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));
        }
    }
    if let Some(t) = v.get("thumbnail").and_then(Value::as_str) {
        return Some(t.to_string());
    }
    v.get("thumbnails")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().rev().find_map(|t| t.get("url").and_then(Value::as_str)))
        .map(str::to_string)
}

fn run_json(args: &[&str]) -> Result<Value, String> {
    let output = silent_command(&paths::ytdlp_path())
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("impossibile avviare yt-dlp ({e})"))?;
    if !output.status.success() {
        return Err(last_error(&String::from_utf8_lossy(&output.stderr)));
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "risposta di yt-dlp non valida".to_string())
}

/// Estrae il messaggio d'errore più utile dallo stderr di yt-dlp.
pub fn last_error(stderr: &str) -> String {
    let line = stderr
        .lines()
        .rev()
        .find(|l| l.starts_with("ERROR:"))
        .or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()))
        .unwrap_or("errore sconosciuto");
    clean_error(line)
}

fn clean_error(line: &str) -> String {
    let mut msg = line.trim().trim_start_matches("ERROR:").trim();
    // Rimuove il prefisso "[youtube] <id>: "
    if msg.starts_with('[') {
        if let Some((_, rest)) = msg.split_once("] ") {
            msg = match rest.split_once(": ") {
                Some((id, m)) if !id.contains(' ') => m,
                _ => rest,
            };
        }
    }
    msg.to_string()
}

fn looks_like_video_in_playlist(url: &str) -> bool {
    url.contains("v=") || url.contains("youtu.be/") || url.contains("/shorts/")
}

pub fn fetch_info(url: &str) -> Result<MediaInfo, String> {
    let v = run_json(&["-J", "--flat-playlist", "--no-warnings", "--", url])?;
    let is_playlist = v.get("_type").and_then(Value::as_str) == Some("playlist");

    if !is_playlist {
        return Ok(video_info(url, &v));
    }

    let entries = v.get("entries").and_then(Value::as_array);
    let count = v
        .get("playlist_count")
        .and_then(Value::as_i64)
        .or_else(|| entries.map(|e| e.len() as i64))
        .unwrap_or(0) as i32;

    // Link a un video dentro una playlist: mostra i dati del video.
    if looks_like_video_in_playlist(url) {
        if let Ok(video) = run_json(&["-J", "--no-playlist", "--no-warnings", "--", url]) {
            let mut info = video_info(url, &video);
            info.is_playlist = true;
            info.count = count;
            return Ok(info);
        }
    }

    let thumbnail = best_thumbnail(&v)
        .or_else(|| entries.and_then(|e| e.first()).and_then(best_thumbnail));
    Ok(MediaInfo {
        url: url.to_string(),
        title: str_field(&v, &["title", "id"]),
        uploader: str_field(&v, &["uploader", "channel"]),
        duration: String::new(),
        thumbnail,
        is_playlist: true,
        has_video: false,
        count,
    })
}

fn video_info(url: &str, v: &Value) -> MediaInfo {
    MediaInfo {
        url: url.to_string(),
        title: str_field(v, &["title", "webpage_url_basename", "id"]),
        uploader: str_field(v, &["uploader", "channel", "extractor_key"]),
        duration: v
            .get("duration")
            .and_then(Value::as_f64)
            .map(human_duration)
            .unwrap_or_default(),
        thumbnail: best_thumbnail(v),
        is_playlist: false,
        has_video: true,
        count: 1,
    }
}

// ───────────────────────── Opzioni ─────────────────────────

pub const HEIGHTS: [Option<u32>; 7] = [None, Some(2160), Some(1440), Some(1080), Some(720), Some(480), Some(360)];
pub const AUDIO_FORMATS: [&str; 5] = ["mp3", "m4a", "opus", "flac", "wav"];
pub const AUDIO_QUALITIES: [&str; 5] = ["0", "320K", "256K", "192K", "128K"];

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub audio_only: bool,
    pub quality: usize,
    pub compat: bool,
    pub audio_format: usize,
    pub audio_quality: usize,
    pub playlist: bool,
    pub output_dir: PathBuf,
    pub template: String,
}

impl DownloadOptions {
    pub fn describe(&self) -> String {
        let mut s = if self.audio_only {
            let fmt = AUDIO_FORMATS.get(self.audio_format).unwrap_or(&"mp3").to_uppercase();
            let lossless = self.audio_format >= 3;
            match AUDIO_QUALITIES.get(self.audio_quality) {
                Some(q) if *q != "0" && !lossless => format!("{fmt} · {} kbps", q.trim_end_matches('K')),
                _ => fmt,
            }
        } else {
            let container = if self.compat { "MP4" } else { "MP4/MKV" };
            match HEIGHTS.get(self.quality).copied().flatten() {
                Some(h) => format!("{container} · {h}p"),
                None => format!("{container} · max"),
            }
        };
        if self.playlist {
            s.push_str(" · playlist");
        }
        s
    }

    pub fn args(&self, url: &str, ffmpeg_dir: Option<&Path>) -> Vec<String> {
        let mut a: Vec<String> = vec![
            "--newline".into(),
            "--no-colors".into(),
            "--no-quiet".into(),
            "--progress".into(),
            "--no-simulate".into(),
            "--no-mtime".into(),
            "--windows-filenames".into(),
            "-P".into(),
            self.output_dir.to_string_lossy().into_owned(),
            "-o".into(),
            self.template.clone(),
            "--progress-template".into(),
            "download:[P]%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|%(info.playlist_index)s|%(info.n_entries)s|%(info.vcodec)s|%(info.acodec)s|%(info.title)s".into(),
            "--progress-template".into(),
            "postprocess:[PP]%(progress.status)s|%(progress.postprocessor)s".into(),
            "--print".into(),
            "after_move:[F]%(filepath)s".into(),
        ];
        if let Some(dir) = ffmpeg_dir {
            a.push("--ffmpeg-location".into());
            a.push(dir.to_string_lossy().into_owned());
        }
        if self.playlist {
            a.extend(["--yes-playlist".into(), "--ignore-errors".into()]);
        } else {
            a.push("--no-playlist".into());
        }

        if self.audio_only {
            let fmt = AUDIO_FORMATS.get(self.audio_format).unwrap_or(&"mp3");
            let q = AUDIO_QUALITIES.get(self.audio_quality).unwrap_or(&"0");
            a.extend([
                "-f".into(),
                "ba/b".into(),
                "-x".into(),
                "--audio-format".into(),
                fmt.to_string(),
                "--audio-quality".into(),
                q.to_string(),
            ]);
        } else {
            let height = HEIGHTS.get(self.quality).copied().flatten();
            let res = height.map(|h| format!("res:{h}"));
            let sort = if self.compat {
                // H.264 + AAC: riproducibile ovunque (TV, telefoni, editor video).
                [Some("vcodec:h264".to_string()), res, Some("acodec:m4a".to_string())]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                res.unwrap_or_else(|| "res".into())
            };
            a.extend(["-f".into(), "bv*+ba/b".into(), "-S".into(), sort]);
            a.push("--merge-output-format".into());
            a.push(if self.compat { "mp4".into() } else { "mp4/mkv".into() });
        }

        // "--" impedisce che un URL che inizia con "-" venga letto come opzione.
        a.push("--".into());
        a.push(url.to_string());
        a
    }
}

// ───────────────────────── Parsing output ─────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub downloaded: Option<f64>,
    pub total: Option<f64>,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub index: Option<u32>,
    pub count: Option<u32>,
    pub has_video: bool,
    pub has_audio: bool,
    pub title: String,
}

impl Progress {
    pub fn fraction(&self) -> Option<f64> {
        match (self.downloaded, self.total) {
            (Some(d), Some(t)) if t > 0.0 => Some((d / t).clamp(0.0, 1.0)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputLine {
    Progress(Progress),
    Postprocess { status: String, name: String },
    File(PathBuf),
    Destination(PathBuf),
    Other(String),
}

fn num(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "NA" || s == "None" {
        None
    } else {
        s.parse::<f64>().ok()
    }
}

fn codec_present(s: &str) -> bool {
    let s = s.trim();
    !(s.is_empty() || s == "none" || s == "NA")
}

pub fn parse_line(line: &str) -> OutputLine {
    if let Some(rest) = line.strip_prefix("[P]") {
        let f: Vec<&str> = rest.splitn(10, '|').collect();
        if f.len() == 10 {
            return OutputLine::Progress(Progress {
                downloaded: num(f[0]),
                total: num(f[1]).or_else(|| num(f[2])),
                speed: num(f[3]),
                eta: num(f[4]),
                index: num(f[5]).map(|v| v as u32),
                count: num(f[6]).map(|v| v as u32),
                has_video: codec_present(f[7]),
                has_audio: codec_present(f[8]),
                title: f[9].trim().to_string(),
            });
        }
    }
    if let Some(rest) = line.strip_prefix("[PP]") {
        let (status, name) = rest.split_once('|').unwrap_or((rest, ""));
        return OutputLine::Postprocess { status: status.trim().into(), name: name.trim().into() };
    }
    if let Some(rest) = line.strip_prefix("[F]") {
        return OutputLine::File(PathBuf::from(rest.trim()));
    }
    for prefix in ["[download] Destination: ", "[ExtractAudio] Destination: "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return OutputLine::Destination(PathBuf::from(rest.trim()));
        }
    }
    if let Some(rest) = line.strip_prefix("[Merger] Merging formats into ") {
        return OutputLine::Destination(PathBuf::from(rest.trim().trim_matches('"')));
    }
    OutputLine::Other(line.to_string())
}

// ───────────────────────── Esecuzione ─────────────────────────

pub enum StreamLine {
    Stdout(OutputLine),
    Stderr(String),
}

fn read_lines(stream: impl Read, mut f: impl FnMut(String)) {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim_end_matches(['\r', '\n']);
                if !line.trim().is_empty() {
                    f(line.to_string());
                }
            }
        }
    }
}

/// Avvia yt-dlp e chiama `on_line` per ogni riga di output. Blocca fino alla fine.
pub fn run_download(
    args: &[String],
    group: &ProcessGroup,
    on_line: Arc<dyn Fn(StreamLine) + Send + Sync>,
) -> Result<(), String> {
    let exe = paths::ytdlp_path();
    let mut child = silent_command(&exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("impossibile avviare yt-dlp ({e})"))?;
    group.attach(&child);

    let stderr = child.stderr.take().expect("stderr");
    let err_cb = on_line.clone();
    let err_thread = std::thread::spawn(move || {
        let mut last_error = None;
        read_lines(stderr, |line| {
            if line.starts_with("ERROR:") {
                last_error = Some(clean_error(&line));
            }
            err_cb(StreamLine::Stderr(line));
        });
        last_error
    });

    let stdout = child.stdout.take().expect("stdout");
    read_lines(stdout, |line| on_line(StreamLine::Stdout(parse_line(&line))));

    let status = child.wait().map_err(|e| e.to_string())?;
    let last_error = err_thread.join().ok().flatten();
    if status.success() {
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| format!("yt-dlp è terminato con codice {}", status.code().unwrap_or(-1))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress() {
        let l = parse_line("[P]1048576|NA|4194304|524288.5|6|2|10|avc1.640028|none|Titolo | con barra");
        let OutputLine::Progress(p) = l else { panic!() };
        assert_eq!(p.fraction(), Some(0.25));
        assert_eq!(p.index, Some(2));
        assert_eq!(p.count, Some(10));
        assert!(p.has_video && !p.has_audio);
        assert_eq!(p.title, "Titolo | con barra");
    }

    #[test]
    fn parses_other_lines() {
        assert_eq!(
            parse_line("[F]C:\\Users\\x\\Downloads\\video.mp4"),
            OutputLine::File(PathBuf::from("C:\\Users\\x\\Downloads\\video.mp4"))
        );
        assert_eq!(
            parse_line("[Merger] Merging formats into \"C:\\d\\a b.mp4\""),
            OutputLine::Destination(PathBuf::from("C:\\d\\a b.mp4"))
        );
        assert_eq!(
            parse_line("[PP]started|Merger"),
            OutputLine::Postprocess { status: "started".into(), name: "Merger".into() }
        );
    }

    #[test]
    fn cleans_errors() {
        assert_eq!(
            clean_error("ERROR: [youtube] abc123: Video unavailable"),
            "Video unavailable"
        );
        assert_eq!(clean_error("ERROR: Unsupported URL: x"), "Unsupported URL: x");
    }

    /// Test di rete: `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn fetches_real_info() {
        let v = fetch_info("https://www.youtube.com/watch?v=jNQXAC9IVRw").expect("video");
        assert!(v.has_video && !v.is_playlist);
        assert_eq!(v.duration, "0:19");
        assert!(v.thumbnail.unwrap().contains("ytimg"));
        println!("video: {} · {}", v.title, v.uploader);

        let p = fetch_info("https://www.youtube.com/playlist?list=PLFgquLnL59alCl_2TQvOiD5Vgm1hCaGSI").expect("playlist");
        assert!(p.is_playlist && !p.has_video && p.count > 0);
        println!("playlist: {} · {} video", p.title, p.count);

        assert!(fetch_info("https://www.youtube.com/watch?v=xxxxxxxxxxx").is_err());
    }

    #[test]
    fn builds_compat_args() {
        let o = DownloadOptions {
            audio_only: false,
            quality: 3,
            compat: true,
            audio_format: 0,
            audio_quality: 0,
            playlist: false,
            output_dir: PathBuf::from("C:\\out"),
            template: "%(title)s.%(ext)s".into(),
        };
        let a = o.args("https://youtu.be/x", None);
        let s = a.join(" ");
        assert!(s.contains("-S vcodec:h264,res:1080,acodec:m4a"));
        assert!(s.contains("--merge-output-format mp4"));
        assert!(s.ends_with("-- https://youtu.be/x"));
        assert_eq!(o.describe(), "MP4 · 1080p");
    }
}
