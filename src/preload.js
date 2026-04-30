const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('ytdlp', {
  getDownloadsDir: () => ipcRenderer.invoke('get-downloads-dir'),
  chooseFolder: () => ipcRenderer.invoke('choose-folder'),
  openFolder: (path) => ipcRenderer.invoke('open-folder', path),
  fetchInfo: (url) => ipcRenderer.invoke('fetch-info', url),
  startDownload: (opts) => ipcRenderer.invoke('start-download', opts),
  onProgress: (callback) => { ipcRenderer.on('download-progress', (_, data) => callback(data)) },
  onItem: (callback) => ipcRenderer.on('download-item', (_, data) => callback(data)),
  onConverting: (callback) => ipcRenderer.on('download-converting', () => callback()),
  onLog: (callback) => { ipcRenderer.on('download-log', (_, line) => callback(line)) },
  windowControl: (action) => ipcRenderer.send('window-control', action),
  getVersions: () => ipcRenderer.invoke('get-versions'),
  removeAllListeners: () => {
    ipcRenderer.removeAllListeners('download-progress')
    ipcRenderer.removeAllListeners('download-log')
    ipcRenderer.removeAllListeners('download-converting')
    ipcRenderer.removeAllListeners('download-item')
  }
})
