#ifndef RepoRoot
  #define RepoRoot "..\.."
#endif

[Setup]
AppId={{E996DDC0-7724-41A0-A889-FBC9277A7432}
AppName=YouCraft
AppVersion=0.1.8
AppPublisher=YouCraft contributors
DefaultDirName={localappdata}\Programs\YouCraft
DefaultGroupName=YouCraft
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#RepoRoot}\release
OutputBaseFilename=YouCraft-Setup-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\YouCraft.exe
LicenseFile={#RepoRoot}\LICENSE-APACHE

[Files]
Source: "{#RepoRoot}\release\YouCraft\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\YouCraft connect AI"; Filename: "{app}\Connect AI.cmd"
Name: "{group}\YouCraft with AI"; Filename: "{app}\YouCraft.exe"; Parameters: "--control 9876"
Name: "{group}\YouCraft model setup"; Filename: "{app}\Setup models.cmd"
Name: "{group}\YouCraft"; Filename: "{app}\YouCraft.exe"
Name: "{autodesktop}\YouCraft"; Filename: "{app}\YouCraft.exe"

[Run]
Filename: "{app}\YouCraft.exe"; Description: "Open YouCraft"; Flags: nowait postinstall skipifsilent

[InstallDelete]
Type: files; Name: "{app}\AmaBartalap.exe"
Type: files; Name: "{app}\Start Ama Bartalap.cmd"
Type: files; Name: "{autodesktop}\Ama Bartalap.lnk"
