# YT Downloader

GUI nativa per Windows 11 di [yt-dlp](https://github.com/yt-dlp/yt-dlp), scritta in Rust + [Slint](https://slint.dev) con stile Fluent.

> La v4 sostituisce la vecchia versione Electron (v3.x): un unico `.exe` leggero, avvio istantaneo, tema chiaro/scuro e colore d'accento di sistema.

## Funzionalità

- Info del link (titolo, canale, durata, miniatura) e riconoscimento playlist
- Video fino a 4K, con modalità **compatibile** (MP4 H.264 + AAC) o **massima qualità** (VP9/AV1)
- Solo audio: MP3, M4A, Opus, FLAC, WAV con scelta del bitrate
- **Coda di download** in parallelo con pausa, ripresa, annulla e riprova
- yt-dlp sempre aggiornato automaticamente (in `%LOCALAPPDATA%\YTDownloader\bin`)
- Impostazioni salvate (`%APPDATA%\YTDownloader\settings.json`), modello nome file personalizzabile
- Log in tempo reale

## Sviluppo

Requisiti: [Rust](https://rustup.rs) (toolchain MSVC) e Visual Studio Build Tools con il carico "Sviluppo di applicazioni desktop C++".

Per lo sviluppo metti `yt-dlp.exe`, `ffmpeg.exe` e `ffprobe.exe` nella cartella `bin/` del progetto (ignorata da git):
al primo avvio yt-dlp viene copiato in `%LOCALAPPDATA%` e da lì si aggiorna da solo.

```bash
cargo run            # avvia in modalità debug
cargo test           # test unitari
cargo test -- --ignored   # test che usano la rete (lettura info da YouTube)
cargo build --release     # target/release/YTDownloader.exe
```

## Installer e release

`scripts/build-release.ps1` compila l'app e crea in `dist/`:

- `YTDownloader-Setup-x.y.z.exe` — installer per utente (Inno Setup, nessun permesso admin)
- `YTDownloader-x.y.z-win64.zip` — versione portabile
- `SHA256SUMS.txt` — checksum usati dall'aggiornamento automatico

Per pubblicare una nuova versione:

1. aggiorna `version` in `Cargo.toml` e fai il commit
2. crea e pubblica il tag: `git tag v4.0.1` → `git push origin v4.0.1`
3. GitHub Actions (`.github/workflows/release.yml`) compila tutto e crea la release

L'app controlla le GitHub Releases all'avvio, scarica il nuovo setup, ne verifica lo SHA-256
e lo installa in silenzio alla chiusura (o subito con «Riavvia e aggiorna»).

## Struttura

```
ui/            interfaccia Slint (pagine, componenti, tema Fluent)
src/app.rs     stato dell'app e coda dei download
src/ytdlp.rs   argomenti, lettura info e parsing del progresso di yt-dlp
src/tools.rs   preparazione/aggiornamento di yt-dlp e ffmpeg, HTTP
src/proc.rs    Job Object di Windows (annulla = termina yt-dlp + ffmpeg)
src/updater.rs aggiornamento automatico dalle GitHub Releases
installer/     script Inno Setup
scripts/       build locale di setup e zip
```
