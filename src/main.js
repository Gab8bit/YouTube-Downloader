const { app, BrowserWindow, ipcMain, dialog, shell } = require('electron')
const path = require('path')
const { spawn } = require('child_process')
const fs = require('fs')

let mainWindow

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 900,
    height: 650,
    minWidth: 700,
    minHeight: 500,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    },
    titleBarStyle: 'hidden', // 'hidden' o 'hiddenInset'
    backgroundColor: '#0f0f11',
    show: false,
    icon: path.join(__dirname, '../assets/icon.png')
  })

  mainWindow.loadFile(path.join(__dirname, 'renderer.html'))

  mainWindow.once('ready-to-show', () => {
    mainWindow.show()
  })

  // apri devtools solo in development
  if (process.env.NODE_ENV === 'development') {
    mainWindow.webContents.openDevTools()
  }
}

app.whenReady().then(() => {
  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

// ─── IPC HANDLERS ────────
ipcMain.on('window-control', (_, action) => {
  if (action === 'minimize') mainWindow.minimize()
  if (action === 'close') mainWindow.close()
})

ipcMain.handle('get-downloads-dir', () => getDefaultDownloadsDir())

ipcMain.handle('choose-folder', async () => {
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory']
  })
  return result.canceled ? null : result.filePaths[0]
})

ipcMain.handle('open-folder', async (_, folderPath) => {
  shell.openPath(folderPath)
})

ipcMain.handle('fetch-info', async (_, url) => {
  return new Promise((resolve, reject) => {
    const ytdlp = getYtdlpPath()
    const proc = spawn(ytdlp, [
      '--dump-json',
      '--flat-playlist',
      url
    ])

    let stdout = ''
    let stderr = ''

    proc.stdout.on('data', d => stdout += d.toString())
    proc.stderr.on('data', d => stderr += d.toString())

    proc.on('close', code => {
    if (code !== 0) return reject(new Error(stderr || 'Errore fetch info'))
    try {
      const lines = stdout.trim().split('\n').filter(Boolean)
      const info = JSON.parse(lines[0])
      const isPlaylist = lines.length > 1
      resolve({
        title: info.title || info.webpage_url_basename,
        thumbnail: info.thumbnail,
        duration: info.duration_string,
        uploader: info.uploader || info.channel,
        isPlaylist,
        playlistCount: isPlaylist ? lines.length : null,
        formats: info.formats?.map(f => ({
          id: f.format_id,
          ext: f.ext,
          resolution: f.resolution || f.audio_ext,
          filesize: f.filesize,
          vcodec: f.vcodec,
          acodec: f.acodec
        })) || []
      })
    } catch {
      reject(new Error('Risposta JSON non valida'))
    }
  })
  })
})

ipcMain.handle('start-download', async (_, { url, outputDir, format, audioOnly, playlist }) => {
  return new Promise((resolve, reject) => {
    const ytdlp = getYtdlpPath()

    const args = [
      '--update',
      playlist ? '--yes-playlist' : '--no-playlist',
      '-P', outputDir,
      '--newline',
      '-o', '%(title)s.%(ext)s',
    ]

    if (audioOnly) {
      args.push('-x', '--audio-format', 'mp3')
    } else {
      if (format) args.push('-f', format)
      args.push('--merge-output-format', 'mp4')
      args.push('--postprocessor-args', 'ffmpeg:-c:a aac')
    }

    args.push(url)

    const proc = spawn(ytdlp, args)
    let lastProgress = null

    proc.stdout.on('data', d => {
      const line = d.toString()
      const itemMatch = line.match(/Downloading item (\d+) of (\d+)/)
      if (itemMatch) {
        mainWindow.webContents.send('download-item', {
          current: parseInt(itemMatch[1]),
          total: parseInt(itemMatch[2])
        })
      }
      if (line.includes('[ExtractAudio]') || line.includes('[Merger]')) {
        mainWindow.webContents.send('download-converting')
      }
      const match = line.match(/\[download\]\s+([\d.]+)%\s+of\s+~?([\d.]+\w+)\s+at\s+([\d.]+\S+)/)
      if (match) {
        const percent = parseFloat(match[1])
        const totalSize = match[2]
        const speed = match[3]
        
        // calcola dimensione scaricata
        const sizeMatch = totalSize.match(/([\d.]+)(\w+)/)
        const downloaded = sizeMatch 
          ? (parseFloat(sizeMatch[1]) * percent / 100).toFixed(1) + sizeMatch[2]
          : ''

        mainWindow.webContents.send('download-progress', {
          percent,
          size: downloaded ? `${downloaded} / ${totalSize}` : totalSize,
          speed
        })
      }

      // File completato
      if (line.includes('[Merger]') || line.includes('has already been downloaded')) {
        mainWindow.webContents.send('download-progress', { percent: 100 })
      }
    })

    proc.stderr.on('data', d => {
      const line = d.toString()
      mainWindow.webContents.send('download-log', line)
    })

    proc.on('close', code => {
      if (code === 0) resolve({ success: true })
      else reject(new Error('Download fallito — controlla i log'))
    })

    proc.on('error', err => {
      reject(new Error(`yt-dlp non trovato: ${err.message}`))
    })
  })
})

ipcMain.handle('get-versions', async () => {
  const ytdlp = getYtdlpPath()
  
  return new Promise((resolve) => {
    const proc = spawn(ytdlp, ['--version'])
    let version = ''
    proc.stdout.on('data', d => version += d.toString())
    proc.on('close', () => resolve({
      app: app.getVersion(),
      ytdlp: version.trim()
    }))
    proc.on('error', () => resolve({ app: app.getVersion(), ytdlp: 'N/A' }))
  })
})

// ─── UTILS ─────────

function getYtdlpPath() {
  const ext = process.platform === 'win32' ? '.exe' : ''
  const binary = `yt-dlp${ext}`

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'bin', binary)
  }
  const localBin = path.join(app.getAppPath(), 'resources', 'bin', binary)
  if (fs.existsSync(localBin)) return localBin
  return binary
}

function getDefaultDownloadsDir() {
  return path.join(require('os').homedir(), 'Downloads')
}
