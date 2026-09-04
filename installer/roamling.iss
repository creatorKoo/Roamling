; SPDX-FileCopyrightText: 2026 GooBeom Jeoung
; SPDX-License-Identifier: GPL-3.0-only
;
; The Windows installer. Build the exe first, then:
;
;   iscc /DAppVersion=0.2.0 installer\roamling.iss
;
; Output lands in build\Roamling-Setup.exe.
;
; Per-user on purpose. PrivilegesRequired=lowest means no UAC prompt at any
; point, {autopf} resolves to %LOCALAPPDATA%\Programs, and the autostart entry
; goes in HKCU where it needs no permission. Asking for administrator approval
; to install a desktop pet is itself a "Never annoying" violation -- and the
; updater depends on it too: replacing the executable in place only works
; where the user can write.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
; Fixed, and never to be changed: it is what tells Windows an install is an
; upgrade of this program rather than a second copy of it.
AppId={{7A5F6C4E-2D91-4B3A-9E58-1C6D0B4F8A27}
AppName=Roamling
AppVersion={#AppVersion}
AppPublisher=GooBeom Jeoung
AppPublisherURL=https://github.com/creatorKoo/Roamling
AppSupportURL=https://github.com/creatorKoo/Roamling/issues
AppUpdatesURL=https://github.com/creatorKoo/Roamling/releases
DefaultDirName={autopf}\Roamling
DefaultGroupName=Roamling
DisableProgramGroupPage=yes
; One file in one folder; there is nothing worth asking about.
DisableDirPage=auto
PrivilegesRequired=lowest
OutputDir=..\build
OutputBaseFilename=Roamling-Setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; curl.exe has been in System32 since Windows 10 1803, and the hook command
; calls it. Below that the integration would install and silently never fire.
MinVersion=10.0.17134
LicenseFile=..\LICENSE
; Only ask which language when the system's own matches neither. Korean Windows
; gets the Korean wizard with no question; everything else gets English with no
; question. Being asked your own language is a question with a known answer.
ShowLanguageDialog=auto
UninstallDisplayName=Roamling
UninstallDisplayIcon={app}\roamling.exe
; The same mark the app carries, so the download in the browser, the wizard and
; the installed program all look like one thing. Generated from the macOS icon
; by scripts/build-ico.py -- see "마크는 하나다" in CLAUDE.md.
SetupIconFile=..\assets\Roamling.ico

[Languages]
; Both ship with Inno Setup 6. The wizard's own text comes from these; the
; three lines that are ours are in [CustomMessages] below. The app's own copy
; is not here -- that lives in the two Localizable.strings the runtime reads.
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[CustomMessages]
english.AutostartTask=Start Roamling when I sign in
english.StartupGroup=Startup
english.LaunchProgram=Start Roamling
korean.AutostartTask=로그인할 때 Roamling 시작
korean.StartupGroup=시작 프로그램
korean.LaunchProgram=Roamling 시작

[Tasks]
Name: "autostart"; Description: "{cm:AutostartTask}"; GroupDescription: "{cm:StartupGroup}"

[Files]
Source: "..\rust\target\release\roamling.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Roamling"; Filename: "{app}\roamling.exe"
Name: "{group}\Uninstall Roamling"; Filename: "{uninstallexe}"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "Roamling"; ValueData: """{app}\roamling.exe"""; \
    Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\roamling.exe"; Description: "{cm:LaunchProgram}"; \
    Flags: nowait postinstall skipifsilent

[InstallDelete]
; A previous version's updater may have left these beside the executable. The
; installer did not create them, so it has to be told about them.
Type: files; Name: "{app}\roamling.exe.old"
Type: files; Name: "{app}\roamling.exe.new"

[UninstallDelete]
Type: files; Name: "{app}\roamling.exe.old"
Type: files; Name: "{app}\roamling.exe.new"

[Code]
// The running pet holds its own executable open, so an install over the top of
// it fails the way `cargo build` does. Inno will ask the user to close it, but
// a tray application with no window is not something anyone can obviously
// "close" -- so it is closed here instead, and started again at the end.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM roamling.exe /F',
       '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := '';
end;

function InitializeUninstall(): Boolean;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM roamling.exe /F',
       '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := True;
end;
