# YT Downloader

## Struttura
```
ytdlp-electron/
├── src/
│   ├── main.js        ← processo principale Electron
│   ├── preload.js     ← bridge sicuro IPC
│   └── renderer.html  ← UI
├── bin/               ← metti qui yt-dlp (per la build)
├── assets/            ← icon.png ecc.
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
