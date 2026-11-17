; Copyright (c) 2026 Open Download Manager contributors
; SPDX-License-Identifier: MIT
; Reason: feat(packaging): Inno Setup per-user Windows installer with silent upgrade and protocol registration

#define MyAppName "Open Download Manager"
#define MyAppVersion "0.3.0"
#define MyAppPublisher "Open Download Manager contributors"
#define MyAppURL "https://github.com/devansh-pc/open-download-manager"
#define MyAppExeName "open-download-manager.exe"
#define MyNativeHostExeName "open-download-manager-native-host.exe"
#define MyAppId "{{C24A4B69-7F3A-4F8A-9A83-E8D5F6A529B1}}"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\Open Download Manager
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=OpenDownloadManager-Setup-x64
SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=yes
ShowUndisplayableLanguages=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "assocmagnet"; Description: "Associate with magnet: links"; GroupDescription: "File Associations:"; Flags: checkedonce
Name: "assoctorrent"; Description: "Associate with .torrent files"; GroupDescription: "File Associations:"; Flags: checkedonce

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#MyNativeHostExeName}"; DestDir: "{app}"; Flags: ignoreversion optional
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Magnet URI Protocol Handler
Root: HKCU; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: ""; ValueData: "URL:BitTorrent Magnet Link"; Flags: uninsdeletekey; Tasks: assocmagnet
Root: HKCU; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; Flags: uninsdeletekey; Tasks: assocmagnet
Root: HKCU; Subkey: "Software\Classes\magnet\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Flags: uninsdeletevalue; Tasks: assocmagnet
Root: HKCU; Subkey: "Software\Classes\magnet\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: assocmagnet

; .torrent File Association
Root: HKCU; Subkey: "Software\Classes\.torrent"; ValueType: string; ValueName: ""; ValueData: "OpenDownloadManager.Torrent"; Flags: uninsdeletevalue; Tasks: assoctorrent
Root: HKCU; Subkey: "Software\Classes\OpenDownloadManager.Torrent"; ValueType: string; ValueName: ""; ValueData: "BitTorrent File"; Flags: uninsdeletekey; Tasks: assoctorrent
Root: HKCU; Subkey: "Software\Classes\OpenDownloadManager.Torrent\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Flags: uninsdeletevalue; Tasks: assoctorrent
Root: HKCU; Subkey: "Software\Classes\OpenDownloadManager.Torrent\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: assoctorrent

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
// Helper function to check if WebView2 Evergreen runtime is installed
function IsWebView2Installed(): Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(HKCU, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) or
            RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) or
            RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version);
  if Result then
    Result := (Length(Version) > 0) and (Version <> '0.0.0.0');
end;

// Detect and silently uninstall legacy NSIS installation if present
procedure RemoveLegacyNsisInstallation();
var
  UninstallExe: String;
  ResultCode: Integer;
begin
  if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\open-download-manager', 'UninstallString', UninstallExe) then
  begin
    if (Length(UninstallExe) > 0) and FileExists(RemoveQuotes(UninstallExe)) then
    begin
      // Execute legacy NSIS uninstaller silently
      Exec(RemoveQuotes(UninstallExe), '/S _?=' + ExtractFilePath(RemoveQuotes(UninstallExe)), '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    end;
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  // Cleanup legacy NSIS if detected
  RemoveLegacyNsisInstallation();

  // Advise user if WebView2 runtime is missing
  if not IsWebView2Installed() then
  begin
    if MsgBox('Open Download Manager requires the Microsoft Edge WebView2 Runtime to run.' + #13#10 +
              'Would you like to continue the installation anyway?', mbConfirmation, MB_YESNO) = IDNO then
    begin
      Result := False;
    end;
  end;
end;
