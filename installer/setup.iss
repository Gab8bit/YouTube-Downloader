; Installer di YT Downloader (Inno Setup 6)
; Compilazione:  ISCC /DAppVersion=4.0.0 /DStageDir=..\dist\stage installer\setup.iss
;
; - Installazione per utente in %LOCALAPPDATA%\Programs\YT Downloader (nessun permesso admin)
; - Usato anche dall'aggiornamento automatico dell'app in modalità silenziosa:
;     YTDownloader-Setup-x.y.z.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /relaunch=1

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef StageDir
  #define StageDir "..\dist\stage"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

#define AppName "YT Downloader"
#define AppExe "YTDownloader.exe"
#define AppPublisher "Gab8bit"
#define AppUrl "https://github.com/Gab8bit/YouTube-Downloader"
#define AppUserModelId "Gab8bit.YTDownloader"

[Setup]
AppId={{6B0C2E59-5D3A-4C1E-9E61-7A8F3B2D4C10}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}

; Per utente: niente UAC, aggiornamenti silenziosi possibili.
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
DisableReadyPage=yes
UsePreviousAppDir=yes

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763

OutputDir={#OutputDir}
OutputBaseFilename=YTDownloader-Setup-{#AppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern
; ISCC è a 32 bit: la compressione gira in un processo separato (a 64 bit) per non
; esaurire la memoria con gli eseguibili di ffmpeg (~200 MB l'uno).
Compression=lzma2/max
SolidCompression=yes
LZMAUseSeparateProcess=yes

CloseApplications=yes
RestartApplications=no
ChangesAssociations=no

[Languages]
Name: "it"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
it.RemoveLegacy=Rimuovi la vecchia versione di YT Downloader (Electron)
en.RemoveLegacy=Remove the old YT Downloader version (Electron)
it.LaunchApp=Avvia {#AppName}
en.LaunchApp=Launch {#AppName}

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "removelegacy"; Description: "{cm:RemoveLegacy}"; Check: LegacyInstalled

[Files]
Source: "{#StageDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\bin\ffmpeg.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#StageDir}\bin\ffprobe.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
; Copia iniziale di yt-dlp: l'app la sposta in %LOCALAPPDATA% e la aggiorna da sola.
Source: "{#StageDir}\bin\yt-dlp.exe"; DestDir: "{app}\bin"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelId}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelId}"; Tasks: desktopicon

[Run]
; Installazione interattiva: casella "Avvia" nella pagina finale.
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent
; Aggiornamento dall'app (silenzioso): riavvia l'app se richiesto con /relaunch=1.
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: ShouldRelaunch

[UninstallDelete]
; yt-dlp scaricato/aggiornato dall'app e cache degli aggiornamenti.
Type: filesandordirs; Name: "{localappdata}\YTDownloader"

[Code]
function LegacyUpdateExe(): String;
begin
  Result := ExpandConstant('{localappdata}\ytdlp_electron\Update.exe');
  if not FileExists(Result) then
    Result := ExpandConstant('{localappdata}\ytdlp-electron\Update.exe');
  if not FileExists(Result) then
    Result := '';
end;

function LegacyInstalled(): Boolean;
begin
  Result := LegacyUpdateExe() <> '';
end;

function ShouldRelaunch(): Boolean;
begin
  Result := WizardSilent() and (ExpandConstant('{param:relaunch|0}') = '1');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExitCode: Integer;
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('removelegacy') and LegacyInstalled() then
    Exec(LegacyUpdateExe(), '--uninstall -s', '', SW_HIDE, ewWaitUntilTerminated, ExitCode);
end;
