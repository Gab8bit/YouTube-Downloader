# YT Downloader — v3 (Electron)

> [!WARNING]
> **Versione non più manutenuta.** La v3 in Electron funziona ancora ma non riceve aggiornamenti né correzioni.
> La versione attuale è la **v4 nativa in Rust**: codice nel branch [`main`](https://github.com/Gab8bit/YouTube-Downloader/tree/main),
> download dall'[ultima release](https://github.com/Gab8bit/YouTube-Downloader/releases/latest).
>
> | Versione | Tecnologia | Stato | Branch |
> |---|---|---|---|
> | v4 | Rust + Slint | ✅ attuale | [`main`](https://github.com/Gab8bit/YouTube-Downloader/tree/main) |
> | v3 | Electron | ⚠️ non più manutenuta | `legacy/electron` (questo) |
> | v1–v2 | C++ / Java | ❌ deprecata | [`legacy/java`](https://github.com/Gab8bit/YouTube-Downloader/tree/legacy/java) |

## Struttura
```
YTDownloader/
├── src/
│   ├── main.js        ← processo principale Electron
│   ├── preload.js     ← bridge sicuro IPC
│   └── renderer.html  ← UI
├── bin/               ← Eseguibili yt-dlp e ffmpeg
├── assets/            ← Icone e altro
└── package.json
```

## Setup rapido

```bash
npm install
npm start
```

> `yt-dlp` deve essere installato nel PATH per lo sviluppo.
> Per Windows: scarica `yt-dlp.exe` e mettilo in `resources/bin/`

## Build per distribuzione

```bash
npm run build
```

Metti il binario di yt-dlp in `resources/bin/` prima della build:
- Windows → `bin/yt-dlp.exe`
- macOS/Linux → `bin/yt-dlp`

## Note tecniche
- `contextIsolation: true` + `nodeIntegration: false` → sicuro
- IPC gestisce: fetch info, download con progress, scelta cartella
- Progress parsato da stdout di yt-dlp riga per riga
- Il log stderr viene mostrato nella UI in real-time
