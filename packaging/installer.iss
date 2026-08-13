; Inno Setup 7 script for Retro Tools 2026.
;
; Builds a Windows installer that bundles the release binaries, the
; third-party CLI tools under resources/ (UnRAR.exe/chdman.exe/7za.exe —
; each under its own license, see docs/COMPILATION.md), and the app icon.
;
; Build from the repo root with:
;   cargo build --workspace --release
;   "C:\Program Files\Inno Setup 7\ISCC.exe" packaging\installer.iss
;
; The compiled installer is written to packaging\output\.

#define AppName "Retro Tools 2026"
#define AppVersion "0.1.0"
#define AppPublisher "Patrick JAILLET"
#define AppExeName "retrotools2026.exe"
#define CliExeName "retrotools-cli.exe"

[Setup]
AppId={{5B7F6F0E-6C0B-4B4A-9E5A-3B1D9C2B7A11}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExeName}
OutputDir=output
OutputBaseFilename=RetroTools2026-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
SetupIconFile=..\crates\ui\assets\icon.ico
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
; No code signing certificate is available in this environment — the
; installer is unsigned. Windows SmartScreen will warn on first run until
; either a certificate is obtained or enough installs build reputation.
PrivilegesRequired=lowest
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Files]
Source: "..\target\release\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#CliExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\resources\*.exe"; DestDir: "{app}\resources"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\CHANGELOG.md"; DestDir: "{app}\docs"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent
