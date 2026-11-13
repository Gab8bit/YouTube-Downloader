//! Stato dell'applicazione e coda dei download.
//!
//! Tutto lo stato vive sul thread della UI (thread-local). I thread di lavoro
//! comunicano solo tramite [`post`], che accoda un [`Event`] nel loop di Slint.

use slint::{ComponentHandle, Image, Model, Rgba8Pixel, SharedPixelBuffer, VecModel, Weak};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::proc::ProcessGroup;
use crate::settings::Settings;
use crate::tools;
use crate::updater::{self, Release};
use crate::util::{human_bytes, human_duration, human_speed};
use crate::ytdlp::{self, DownloadOptions, MediaInfo, OutputLine, StreamLine};
use crate::{AppState, AppWindow, JobState, LogLine, QueueItem};

const MAX_LOG_LINES: usize = 1500;
const UPDATE_INTERVAL_SECS: u64 = 24 * 3600;

pub enum Event {
    Info { seq: u64, result: Result<MediaInfo, String> },
    Thumb { seq: u64, buf: SharedPixelBuffer<Rgba8Pixel> },
    JobThumb { id: i32, buf: SharedPixelBuffer<Rgba8Pixel> },
    Job { id: i32, line: StreamLine },
    JobFinished { id: i32, result: Result<(), String> },
    ToolsReady { result: Result<(), String> },
    Versions { ytdlp: Option<String>, ffmpeg: Option<String> },
    ToolsUpdated { result: Result<Vec<String>, String> },
    UpdateChecked { result: Result<Option<Release>, String>, manual: bool },
    UpdateProgress(f32),
    UpdateDownloaded(Result<PathBuf, String>),
}

/// Valori di `AppState.update-status`.
mod upd {
    pub const IDLE: i32 = 0;
    pub const CHECKING: i32 = 1;
    pub const AVAILABLE: i32 = 2;
    pub const DOWNLOADING: i32 = 3;
    pub const READY: i32 = 4;
    pub const UP_TO_DATE: i32 = 5;
    pub const ERROR: i32 = 6;
}

#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Pause,
    Cancel,
}

struct Job {
    id: i32,
    url: String,
    opts: DownloadOptions,
    state: JobState,
    group: Option<Arc<ProcessGroup>>,
    stop: Option<Stop>,
    files: Vec<PathBuf>,
    temp_paths: Vec<PathBuf>,
    title_is_placeholder: bool,
    cur_index: Option<u32>,
    video_part_seen: bool,
}

impl Job {
    fn is_active(&self) -> bool {
        matches!(self.state, JobState::Starting | JobState::Downloading | JobState::Processing)
    }
}

pub struct App {
    ui: Weak<AppWindow>,
    queue: Rc<VecModel<QueueItem>>,
    log: Rc<VecModel<LogLine>>,
    pub settings: Settings,
    jobs: Vec<Job>,
    next_id: i32,
    fetch_seq: u64,
    info: Option<MediaInfo>,
    info_thumb: Option<Image>,
    tools_ready: bool,
    tools_busy: bool,
    update_status: i32,
    update_release: Option<Release>,
    update_file: Option<PathBuf>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static PENDING: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
}

/// Invia un evento al thread della UI (chiamabile da qualsiasi thread).
pub fn post(ev: Event) {
    let _ = slint::invoke_from_event_loop(move || {
        PENDING.with(|p| p.borrow_mut().push(ev));
        drain();
    });
}

/// Elabora gli eventi in attesa, se lo stato non è già in uso (es. durante un dialogo modale).
fn drain() {
    APP.with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else { return };
        let Some(app) = guard.as_mut() else { return };
        loop {
            let next = PENDING.with(|p| {
                let mut p = p.borrow_mut();
                (!p.is_empty()).then(|| p.remove(0))
            });
            match next {
                Some(ev) => app.handle(ev),
                None => break,
            }
        }
    });
}

/// Esegue `f` sullo stato dell'app (solo dal thread della UI).
pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let r = APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)));
    drain();
    r
}

pub fn init(ui: &AppWindow, settings: Settings) {
    let queue = Rc::new(VecModel::<QueueItem>::default());
    let log = Rc::new(VecModel::<LogLine>::default());
    let state = ui.global::<AppState>();
    state.set_queue(queue.clone().into());
    state.set_log(log.clone().into());
    state.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
    state.set_ytdlp_version("…".into());
    state.set_ffmpeg_version("…".into());

    let app = App {
        ui: ui.as_weak(),
        queue,
        log,
        settings,
        jobs: Vec::new(),
        next_id: 1,
        fetch_seq: 0,
        info: None,
        info_thumb: None,
        tools_ready: false,
        tools_busy: true,
        update_status: upd::IDLE,
        update_release: None,
        update_file: None,
    };
    app.apply_settings_to_ui();
    app.ui(|ui| ui.global::<AppState>().set_tools_busy(true));
    APP.with(|cell| *cell.borrow_mut() = Some(app));

    let update_due = now_secs().saturating_sub(with_app(|a| a.settings.ytdlp_last_update).unwrap_or(0)) > UPDATE_INTERVAL_SECS;
    std::thread::spawn(move || {
        let result = tools::ensure_ytdlp().map(|_| ());
        let ok = result.is_ok();
        post(Event::ToolsReady { result });
        post(Event::Versions { ytdlp: tools::ytdlp_version(), ffmpeg: tools::ffmpeg_version() });
        if ok && update_due {
            post(Event::ToolsUpdated { result: tools::update_ytdlp() });
            post(Event::Versions { ytdlp: tools::ytdlp_version(), ffmpeg: tools::ffmpeg_version() });
        } else {
            post(Event::ToolsUpdated { result: Ok(Vec::new()) });
        }
    });

    // Controllo aggiornamenti dell'app qualche secondo dopo l'avvio.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(3));
        post(Event::UpdateChecked { result: updater::check(), manual: false });
    });
    with_app(|a| a.set_update_status(upd::CHECKING));
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl App {
    fn ui<R>(&self, f: impl FnOnce(&AppWindow) -> R) -> Option<R> {
        self.ui.upgrade().map(|ui| f(&ui))
    }

    // ───────────── Log ─────────────

    pub fn log(&self, text: impl Into<String>, kind: i32) {
        let text: String = text.into();
        for line in text.lines() {
            self.log.push(LogLine { text: line.into(), kind });
        }
        let excess = self.log.row_count().saturating_sub(MAX_LOG_LINES);
        for _ in 0..excess {
            self.log.remove(0);
        }
    }

    pub fn clear_log(&self) {
        self.log.set_vec(Vec::new());
    }

    // ───────────── Impostazioni ─────────────

    fn apply_settings_to_ui(&self) {
        let s = &self.settings;
        self.ui(|ui| {
            let st = ui.global::<AppState>();
            st.set_output_dir(s.output_dir.to_string_lossy().as_ref().into());
            st.set_kind(s.kind);
            st.set_quality(s.quality);
            st.set_compat(s.compat);
            st.set_audio_format(s.audio_format);
            st.set_audio_quality(s.audio_quality);
            st.set_filename_template(s.filename_template.as_str().into());
            st.set_max_parallel(s.max_parallel);
            st.set_auto_update(s.auto_update);
        });
    }

    pub fn settings_changed(&mut self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let st = ui.global::<AppState>();
        let s = &mut self.settings;
        s.kind = st.get_kind();
        s.quality = st.get_quality();
        s.compat = st.get_compat();
        s.audio_format = st.get_audio_format();
        s.audio_quality = st.get_audio_quality();
        s.max_parallel = st.get_max_parallel().clamp(1, 6);
        s.auto_update = st.get_auto_update();
        let template = st.get_filename_template().trim().to_string();
        if !template.is_empty() {
            s.filename_template = template;
        }
        s.save();
        self.pump();
    }

    pub fn current_output_dir(&self) -> PathBuf {
        self.settings.output_dir.clone()
    }

    pub fn set_output_dir(&mut self, dir: PathBuf) {
        self.settings.output_dir = dir;
        self.settings.save();
        self.apply_settings_to_ui();
    }

    // ───────────── Ricerca info ─────────────

    pub fn fetch(&mut self, url: &str) {
        let url = url.trim().to_string();
        if url.is_empty() {
            return;
        }
        if !self.tools_ready {
            self.show_error("yt-dlp è in preparazione, riprova tra qualche secondo.");
            return;
        }
        self.fetch_seq += 1;
        let seq = self.fetch_seq;
        self.info = None;
        self.info_thumb = None;
        self.ui(|ui| {
            let st = ui.global::<AppState>();
            st.set_fetching(true);
            st.set_has_info(false);
            st.set_error_message("".into());
        });
        self.log(format!("Recupero informazioni: {url}"), 0);
        std::thread::spawn(move || {
            let result = ytdlp::fetch_info(&url);
            post(Event::Info { seq, result });
        });
    }

    pub fn dismiss_info(&mut self) {
        self.fetch_seq += 1;
        self.info = None;
        self.info_thumb = None;
        self.ui(|ui| {
            let st = ui.global::<AppState>();
            st.set_has_info(false);
            st.set_fetching(false);
            st.set_url("".into());
        });
    }

    fn show_error(&self, msg: &str) {
        self.ui(|ui| ui.global::<AppState>().set_error_message(msg.into()));
    }

    fn on_info(&mut self, info: MediaInfo) {
        self.log(format!("Trovato: {}", info.title), 2);
        self.ui(|ui| {
            let st = ui.global::<AppState>();
            st.set_fetching(false);
            st.set_has_info(true);
            st.set_info_title(info.title.as_str().into());
            st.set_info_uploader(info.uploader.as_str().into());
            st.set_info_duration(info.duration.as_str().into());
            st.set_info_has_thumb(false);
            st.set_info_is_playlist(info.is_playlist);
            st.set_info_has_video(info.has_video);
            st.set_info_count(info.count);
            // Per un link a una playlist "pura" si scarica tutto; per un video in playlist solo il video.
            st.set_playlist_all(info.is_playlist && !info.has_video);
        });
        if let Some(thumb) = info.thumbnail.clone() {
            let seq = self.fetch_seq;
            std::thread::spawn(move || {
                if let Some(buf) = tools::load_thumbnail(&thumb) {
                    post(Event::Thumb { seq, buf });
                }
            });
        }
        self.info = Some(info);
    }

    // ───────────── Coda ─────────────

    fn options_from_ui(&self, playlist: bool) -> DownloadOptions {
        let s = &self.settings;
        DownloadOptions {
            audio_only: s.kind == 1,
            quality: s.quality.max(0) as usize,
            compat: s.compat,
            audio_format: s.audio_format.max(0) as usize,
            audio_quality: s.audio_quality.max(0) as usize,
            playlist,
            output_dir: s.output_dir.clone(),
            template: s.filename_template.clone(),
        }
    }

    pub fn download(&mut self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let st = ui.global::<AppState>();
        let url = st.get_url().trim().to_string();
        if url.is_empty() {
            return;
        }
        self.settings_changed();

        let info = self.info.as_ref().filter(|i| i.url == url).cloned();
        let playlist = info
            .as_ref()
            .map(|i| i.is_playlist && (!i.has_video || st.get_playlist_all()))
            .unwrap_or(false);
        let opts = self.options_from_ui(playlist);
        let (title, placeholder) = match &info {
            Some(i) if !i.title.is_empty() => (i.title.clone(), false),
            _ => (url.clone(), true),
        };
        let thumb = if info.is_some() { self.info_thumb.clone() } else { None };

        let id = self.next_id;
        self.next_id += 1;
        self.queue.push(QueueItem {
            id,
            title: title.as_str().into(),
            detail: opts.describe().into(),
            state: JobState::Queued,
            has_thumb: thumb.is_some(),
            thumb: thumb.unwrap_or_default(),
            is_audio: opts.audio_only,
            ..Default::default()
        });
        self.log(format!("Aggiunto alla coda: {title} ({})", opts.describe()), 0);
        self.jobs.push(Job {
            id,
            url: url.clone(),
            opts,
            state: JobState::Queued,
            group: None,
            stop: None,
            files: Vec::new(),
            temp_paths: Vec::new(),
            title_is_placeholder: placeholder,
            cur_index: None,
            video_part_seen: false,
        });

        // Se non avevamo le info, carica comunque la miniatura in background.
        if info.is_none() {
            std::thread::spawn(move || {
                if let Ok(i) = ytdlp::fetch_info(&url) {
                    if let Some(t) = i.thumbnail {
                        if let Some(buf) = tools::load_thumbnail(&t) {
                            post(Event::JobThumb { id, buf });
                        }
                    }
                }
            });
        }

        self.dismiss_info();
        self.pump();
    }

    fn job_mut(&mut self, id: i32) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    fn update_row(&self, id: i32, f: impl FnOnce(&mut QueueItem)) {
        for i in 0..self.queue.row_count() {
            if let Some(mut row) = self.queue.row_data(i) {
                if row.id == id {
                    f(&mut row);
                    self.queue.set_row_data(i, row);
                    return;
                }
            }
        }
    }

    fn set_state(&mut self, id: i32, state: JobState) {
        if let Some(job) = self.job_mut(id) {
            job.state = state;
        }
        self.update_row(id, |r| r.state = state);
        self.update_counts();
    }

    fn update_counts(&self) {
        let active = self.jobs.iter().filter(|j| j.is_active() || j.state == JobState::Queued).count();
        let finished = self
            .jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::Done | JobState::Failed | JobState::Cancelled))
            .count();
        self.ui(|ui| {
            let st = ui.global::<AppState>();
            st.set_active_count(active as i32);
            st.set_finished_count(finished as i32);
        });
    }

    /// Avvia i download in coda fino al limite di download simultanei.
    fn pump(&mut self) {
        if !self.tools_ready {
            return;
        }
        let max = self.settings.max_parallel.max(1) as usize;
        loop {
            let running = self.jobs.iter().filter(|j| j.is_active()).count();
            if running >= max {
                break;
            }
            let Some(id) = self.jobs.iter().find(|j| j.state == JobState::Queued).map(|j| j.id) else {
                break;
            };
            self.start(id);
        }
        self.update_counts();
    }

    fn start(&mut self, id: i32) {
        let ffmpeg = crate::paths::ffmpeg_dir();
        let Some(job) = self.job_mut(id) else { return };
        let args = job.opts.args(&job.url, ffmpeg.as_deref());
        let group = Arc::new(ProcessGroup::new());
        job.group = Some(group.clone());
        job.stop = None;
        job.cur_index = None;
        job.video_part_seen = false;
        let _ = std::fs::create_dir_all(&job.opts.output_dir);
        self.set_state(id, JobState::Starting);
        self.update_row(id, |r| {
            r.error = "".into();
            r.speed = "".into();
            r.eta = "".into();
        });
        if ffmpeg.is_none() {
            self.log("Attenzione: ffmpeg non trovato, unione e conversione potrebbero non funzionare.", 1);
        }

        std::thread::spawn(move || {
            let cb: Arc<dyn Fn(StreamLine) + Send + Sync> = Arc::new(move |line| post(Event::Job { id, line }));
            let result = ytdlp::run_download(&args, &group, cb);
            post(Event::JobFinished { id, result });
        });
    }

    pub fn pause(&mut self, id: i32) {
        if let Some(job) = self.job_mut(id) {
            if job.is_active() {
                job.stop = Some(Stop::Pause);
                if let Some(g) = &job.group {
                    g.kill();
                }
            }
        }
    }

    pub fn resume(&mut self, id: i32) {
        if let Some(job) = self.job_mut(id) {
            if job.state == JobState::Paused {
                self.set_state(id, JobState::Queued);
                self.pump();
            }
        }
    }

    pub fn cancel(&mut self, id: i32) {
        let Some(job) = self.job_mut(id) else { return };
        match job.state {
            JobState::Queued | JobState::Paused => {
                job.stop = None;
                self.cleanup_temp(id);
                self.set_state(id, JobState::Cancelled);
            }
            _ if job.is_active() => {
                job.stop = Some(Stop::Cancel);
                if let Some(g) = &job.group {
                    g.kill();
                }
            }
            _ => {}
        }
    }

    pub fn retry(&mut self, id: i32) {
        if let Some(job) = self.job_mut(id) {
            if matches!(job.state, JobState::Failed | JobState::Cancelled) {
                job.files.clear();
                self.update_row(id, |r| {
                    r.progress = 0.0;
                    r.has_file = false;
                });
                self.set_state(id, JobState::Queued);
                self.pump();
            }
        }
    }

    pub fn remove(&mut self, id: i32) {
        if self.jobs.iter().any(|j| j.id == id && (j.is_active() || j.state == JobState::Queued)) {
            return;
        }
        self.jobs.retain(|j| j.id != id);
        for i in 0..self.queue.row_count() {
            if self.queue.row_data(i).map(|r| r.id) == Some(id) {
                self.queue.remove(i);
                break;
            }
        }
        self.update_counts();
    }

    pub fn clear_finished(&mut self) {
        let ids: Vec<i32> = self
            .jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::Done | JobState::Failed | JobState::Cancelled))
            .map(|j| j.id)
            .collect();
        for id in ids {
            self.remove(id);
        }
    }

    pub fn file_of(&self, id: i32) -> Option<PathBuf> {
        self.jobs.iter().find(|j| j.id == id).and_then(|j| j.files.last().cloned())
    }

    pub fn has_running_jobs(&self) -> bool {
        self.jobs.iter().any(|j| j.is_active())
    }

    /// Elimina i file temporanei (.part, .ytdl, formati intermedi .fNNN) di un download annullato.
    fn cleanup_temp(&mut self, id: i32) {
        let Some(job) = self.job_mut(id) else { return };
        let paths = std::mem::take(&mut job.temp_paths);
        let finals = job.files.clone();
        for p in paths {
            for ext in ["part", "ytdl"] {
                let mut s = p.clone().into_os_string();
                s.push(format!(".{ext}"));
                let _ = std::fs::remove_file(PathBuf::from(s));
            }
            let intermediate = p
                .file_stem()
                .and_then(|s| PathBuf::from(s).extension().map(|e| e.to_string_lossy().into_owned()))
                .map(|e| e.len() > 1 && e.starts_with('f') && e[1..].chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false);
            if intermediate && !finals.contains(&p) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }

    // ───────────── Eventi ─────────────

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Info { seq, result } => {
                if seq != self.fetch_seq {
                    return;
                }
                match result {
                    Ok(info) => self.on_info(info),
                    Err(e) => {
                        self.ui(|ui| ui.global::<AppState>().set_fetching(false));
                        self.log(format!("Errore: {e}"), 1);
                        self.show_error(&format!("Impossibile leggere il link: {e}"));
                    }
                }
            }
            Event::Thumb { seq, buf } => {
                if seq != self.fetch_seq {
                    return;
                }
                let img = Image::from_rgba8(buf);
                self.info_thumb = Some(img.clone());
                self.ui(|ui| {
                    let st = ui.global::<AppState>();
                    st.set_info_thumb(img);
                    st.set_info_has_thumb(true);
                });
            }
            Event::JobThumb { id, buf } => {
                let img = Image::from_rgba8(buf);
                self.update_row(id, |r| {
                    r.thumb = img;
                    r.has_thumb = true;
                });
            }
            Event::Job { id, line } => self.on_job_line(id, line),
            Event::JobFinished { id, result } => self.on_job_finished(id, result),
            Event::ToolsReady { result } => match result {
                Ok(()) => {
                    self.tools_ready = true;
                    self.ui(|ui| ui.global::<AppState>().set_tools_ready(true));
                    self.pump();
                }
                Err(e) => {
                    self.log(&e, 1);
                    self.show_error(&format!("yt-dlp non disponibile: {e}"));
                }
            },
            Event::Versions { ytdlp, ffmpeg } => {
                self.ui(|ui| {
                    let st = ui.global::<AppState>();
                    st.set_ytdlp_version(ytdlp.unwrap_or_else(|| "non disponibile".into()).into());
                    st.set_ffmpeg_version(
                        ffmpeg.map(|v| format!("Versione {v}")).unwrap_or_else(|| "non trovato".into()).into(),
                    );
                });
            }
            Event::ToolsUpdated { result } => {
                self.tools_busy = false;
                self.ui(|ui| ui.global::<AppState>().set_tools_busy(false));
                match result {
                    Ok(lines) => {
                        if !lines.is_empty() {
                            self.settings.ytdlp_last_update = now_secs();
                            self.settings.save();
                            self.log(lines.join("\n"), 0);
                        }
                    }
                    Err(e) => self.log(format!("Aggiornamento yt-dlp non riuscito: {e}"), 1),
                }
            }
            Event::UpdateChecked { result, manual } => self.on_update_checked(result, manual),
            Event::UpdateProgress(p) => {
                if self.update_status == upd::DOWNLOADING {
                    self.ui(|ui| ui.global::<AppState>().set_update_progress(p));
                }
            }
            Event::UpdateDownloaded(result) => match result {
                Ok(path) => {
                    self.log(format!("Aggiornamento pronto: {}", path.display()), 2);
                    self.update_file = Some(path);
                    self.set_update_status(upd::READY);
                }
                Err(e) => {
                    self.log(format!("Download dell'aggiornamento non riuscito: {e}"), 1);
                    self.ui(|ui| ui.global::<AppState>().set_update_error(e.as_str().into()));
                    self.set_update_status(upd::ERROR);
                }
            },
        }
    }

    // ───────────── Aggiornamenti dell'app ─────────────

    fn set_update_status(&mut self, status: i32) {
        self.update_status = status;
        self.ui(|ui| ui.global::<AppState>().set_update_status(status));
    }

    pub fn check_updates(&mut self) {
        if matches!(self.update_status, upd::CHECKING | upd::DOWNLOADING | upd::READY) {
            return;
        }
        self.set_update_status(upd::CHECKING);
        std::thread::spawn(|| post(Event::UpdateChecked { result: updater::check(), manual: true }));
    }

    fn on_update_checked(&mut self, result: Result<Option<Release>, String>, manual: bool) {
        if matches!(self.update_status, upd::DOWNLOADING | upd::READY) {
            return;
        }
        match result {
            Ok(Some(rel)) => {
                self.log(format!("Disponibile YT Downloader {}", rel.version), 0);
                self.ui(|ui| ui.global::<AppState>().set_update_version(rel.version.as_str().into()));
                self.update_release = Some(rel);
                self.set_update_status(upd::AVAILABLE);
                if self.settings.auto_update {
                    self.start_update();
                }
            }
            Ok(None) => self.set_update_status(if manual { upd::UP_TO_DATE } else { upd::IDLE }),
            Err(e) => {
                self.log(format!("Controllo aggiornamenti non riuscito: {e}"), if manual { 1 } else { 0 });
                if manual {
                    self.ui(|ui| ui.global::<AppState>().set_update_error(e.as_str().into()));
                    self.set_update_status(upd::ERROR);
                } else {
                    self.set_update_status(upd::IDLE);
                }
            }
        }
    }

    pub fn start_update(&mut self) {
        let Some(rel) = self.update_release.clone() else { return };
        if self.update_status == upd::DOWNLOADING {
            return;
        }
        self.set_update_status(upd::DOWNLOADING);
        self.ui(|ui| ui.global::<AppState>().set_update_progress(0.0));
        self.log(format!("Download di YT Downloader {}…", rel.version), 0);
        std::thread::spawn(move || {
            let last = std::sync::atomic::AtomicU32::new(0);
            let result = updater::download(&rel, |p| {
                // Notifica solo a ogni punto percentuale.
                let pct = (p * 100.0) as u32;
                if last.swap(pct, std::sync::atomic::Ordering::Relaxed) != pct {
                    post(Event::UpdateProgress(p));
                }
            });
            post(Event::UpdateDownloaded(result));
        });
    }

    pub fn open_release_page(&self) {
        let url = self
            .update_release
            .as_ref()
            .map(|r| r.page_url.clone())
            .unwrap_or_else(|| format!("https://github.com/{}/releases", updater::REPO));
        crate::util::open_url(&url);
    }

    /// Avvia il setup e riapre l'app al termine. Restituisce `true` se l'app deve chiudersi.
    pub fn install_update_now(&mut self) -> bool {
        let Some(file) = self.update_file.clone().filter(|f| f.is_file()) else { return false };
        match updater::launch_installer(&file, true) {
            Ok(()) => true,
            Err(e) => {
                self.log(format!("Impossibile avviare il setup: {e}"), 1);
                self.show_error(&format!("Impossibile avviare l'aggiornamento: {e}"));
                false
            }
        }
    }

    /// Alla chiusura: se un aggiornamento è pronto lo installa in silenzio.
    pub fn install_update_on_exit(&mut self) {
        if self.update_status == upd::READY {
            if let Some(file) = self.update_file.clone().filter(|f| f.is_file()) {
                let _ = updater::launch_installer(&file, false);
            }
        }
    }

    pub fn update_ytdlp(&mut self) {
        if self.tools_busy {
            return;
        }
        self.tools_busy = true;
        self.ui(|ui| ui.global::<AppState>().set_tools_busy(true));
        self.log("Controllo aggiornamenti di yt-dlp…", 0);
        std::thread::spawn(|| {
            post(Event::ToolsUpdated { result: tools::update_ytdlp() });
            post(Event::Versions { ytdlp: tools::ytdlp_version(), ffmpeg: tools::ffmpeg_version() });
        });
    }

    fn on_job_line(&mut self, id: i32, line: StreamLine) {
        let Some(job) = self.job_mut(id) else { return };
        let stopping = job.stop.is_some();
        match line {
            StreamLine::Stdout(OutputLine::Progress(p)) => {
                if stopping {
                    return;
                }
                let audio_only = job.opts.audio_only;
                if job.cur_index != p.index {
                    job.cur_index = p.index;
                    job.video_part_seen = false;
                }
                let frac = p.fraction().unwrap_or(0.0);
                // Video+audio separati: il video pesa ~85%, l'audio il resto.
                let item_frac = if audio_only {
                    frac
                } else if p.has_video && !p.has_audio {
                    job.video_part_seen = true;
                    frac * 0.85
                } else if !p.has_video && p.has_audio && job.video_part_seen {
                    0.85 + frac * 0.15
                } else {
                    frac
                };
                let count = p.count.unwrap_or(1).max(1);
                let index = p.index.unwrap_or(1).clamp(1, count);
                let overall = if count > 1 { ((index - 1) as f64 + item_frac) / count as f64 } else { item_frac };
                let new_title = (job.title_is_placeholder && !p.title.is_empty() && count <= 1).then(|| {
                    job.title_is_placeholder = false;
                    p.title.clone()
                });
                let state_changed = job.state != JobState::Downloading;
                job.state = JobState::Downloading;
                self.update_row(id, |r| {
                    r.state = JobState::Downloading;
                    r.progress = overall as f32;
                    r.speed = p.speed.map(human_speed).unwrap_or_default().into();
                    r.eta = p.eta.map(human_duration).unwrap_or_default().into();
                    r.size = p.total.map(human_bytes).unwrap_or_default().into();
                    r.item_index = index as i32;
                    r.item_count = count as i32;
                    if let Some(t) = new_title {
                        r.title = t.into();
                    }
                });
                if state_changed {
                    self.update_counts();
                }
            }
            StreamLine::Stdout(OutputLine::Postprocess { status, name }) => {
                if !stopping && status == "started" && !matches!(name.as_str(), "MoveFiles" | "") {
                    self.set_state(id, JobState::Processing);
                }
            }
            StreamLine::Stdout(OutputLine::File(path)) => {
                job.files.push(path.clone());
                self.update_row(id, |r| r.has_file = true);
                self.log(format!("[#{id}] Salvato: {}", path.display()), 2);
            }
            StreamLine::Stdout(OutputLine::Destination(path)) => {
                job.temp_paths.push(path.clone());
                self.log(format!("[#{id}] → {}", path.display()), 0);
            }
            StreamLine::Stdout(OutputLine::Other(text)) => {
                self.log(format!("[#{id}] {text}"), 0);
            }
            StreamLine::Stderr(text) => {
                let kind = if text.starts_with("ERROR") { 1 } else { 0 };
                self.log(format!("[#{id}] {text}"), kind);
            }
        }
    }

    fn on_job_finished(&mut self, id: i32, result: Result<(), String>) {
        let Some(job) = self.job_mut(id) else { return };
        job.group = None;
        let stop = job.stop.take();
        let has_files = !job.files.is_empty();
        let playlist = job.opts.playlist;
        match stop {
            Some(Stop::Pause) => {
                self.set_state(id, JobState::Paused);
                self.log(format!("[#{id}] In pausa"), 0);
            }
            Some(Stop::Cancel) => {
                self.cleanup_temp(id);
                self.set_state(id, JobState::Cancelled);
                self.log(format!("[#{id}] Annullato"), 0);
            }
            None => match result {
                Ok(()) => self.finish_ok(id),
                Err(_) if playlist && has_files => {
                    self.log(format!("[#{id}] Playlist completata con alcuni errori"), 1);
                    self.finish_ok(id);
                }
                Err(e) => {
                    self.update_row(id, |r| r.error = e.as_str().into());
                    self.set_state(id, JobState::Failed);
                    self.log(format!("[#{id}] Download non riuscito: {e}"), 1);
                }
            },
        }
        self.pump();
    }

    fn finish_ok(&mut self, id: i32) {
        self.update_row(id, |r| {
            r.progress = 1.0;
            r.speed = "".into();
            r.eta = "".into();
        });
        self.set_state(id, JobState::Done);
        self.log(format!("[#{id}] ✓ Download completato"), 2);
    }
}
