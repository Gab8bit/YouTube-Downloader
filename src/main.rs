// In release niente finestra console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod paths;
mod proc;
mod settings;
mod tools;
mod util;
mod ytdlp;

use app::with_app;
use slint::ComponentHandle;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let settings = settings::Settings::load();
    let ui = AppWindow::new()?;
    app::init(&ui, settings);

    let st = ui.global::<AppState>();

    st.on_fetch(|url| {
        with_app(|a| a.fetch(&url));
    });

    let weak = ui.as_weak();
    st.on_paste(move || {
        let Some(ui) = weak.upgrade() else { return };
        let text = arboard::Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        ui.global::<AppState>().set_url(text.as_str().into());
        if text.starts_with("http://") || text.starts_with("https://") {
            with_app(|a| a.fetch(&text));
        }
    });

    st.on_dismiss_info(|| {
        with_app(|a| a.dismiss_info());
    });
    st.on_download(|| {
        with_app(|a| a.download());
    });
    st.on_settings_changed(|| {
        with_app(|a| a.settings_changed());
    });

    let weak = ui.as_weak();
    st.on_choose_folder(move || {
        let Some(ui) = weak.upgrade() else { return };
        // Il dialogo è modale: non tenere lo stato bloccato mentre è aperto.
        let start = with_app(|a| a.current_output_dir()).unwrap_or_else(paths::default_download_dir);
        let picked = rfd::FileDialog::new()
            .set_title("Scegli la cartella di destinazione")
            .set_directory(&start)
            .set_parent(&ui.window().window_handle())
            .pick_folder();
        if let Some(dir) = picked {
            with_app(|a| a.set_output_dir(dir));
        }
    });

    st.on_open_folder(|| {
        if let Some(dir) = with_app(|a| a.current_output_dir()) {
            let _ = std::fs::create_dir_all(&dir);
            util::open_path(&dir);
        }
    });

    st.on_pause(|id| {
        with_app(|a| a.pause(id));
    });
    st.on_resume(|id| {
        with_app(|a| a.resume(id));
    });
    st.on_cancel(|id| {
        with_app(|a| a.cancel(id));
    });
    st.on_retry(|id| {
        with_app(|a| a.retry(id));
    });
    st.on_remove(|id| {
        with_app(|a| a.remove(id));
    });
    st.on_clear_finished(|| {
        with_app(|a| a.clear_finished());
    });
    st.on_open_file(|id| {
        if let Some(Some(path)) = with_app(|a| a.file_of(id)) {
            if path.is_file() {
                util::open_path(&path);
            }
        }
    });
    st.on_show_file(|id| {
        match with_app(|a| (a.file_of(id), a.current_output_dir())) {
            Some((Some(path), _)) if path.exists() => util::reveal_file(&path),
            Some((_, dir)) => util::open_path(&dir),
            None => {}
        }
    });

    st.on_clear_log(|| {
        with_app(|a| a.clear_log());
    });
    st.on_update_ytdlp(|| {
        with_app(|a| a.update_ytdlp());
    });
    st.on_open_url(|url| util::open_url(&url));

    // Chiedi conferma se si chiude con download in corso.
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if with_app(|a| a.has_running_jobs()).unwrap_or(false) {
            let mut dialog = rfd::MessageDialog::new()
                .set_title("YT Downloader")
                .set_description("Ci sono download in corso. Chiudendo verranno interrotti.\n\nChiudere comunque?")
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::YesNo);
            if let Some(ui) = weak.upgrade() {
                dialog = dialog.set_parent(&ui.window().window_handle());
            }
            if dialog.show() != rfd::MessageDialogResult::Yes {
                return slint::CloseRequestResponse::KeepWindowShown;
            }
        }
        slint::CloseRequestResponse::HideWindow
    });

    ui.run()
}
