#ifndef RepoRoot
  #define RepoRoot "..\.."
#endif

[Setup]
AppId={{E996DDC0-7724-41A0-A889-FBC9277A7432}
AppName=Ama Bartalap
AppVersion=0.1.0
AppPublisher=Ama Bartalap contributors
DefaultDirName={localappdata}\Programs\AmaBartalap
DefaultGroupName=Ama Bartalap
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#RepoRoot}\release
OutputBaseFilename=AmaBartalap-Setup-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\AmaBartalap.exe
LicenseFile={#RepoRoot}\LICENSE-APACHE

[Files]
Source: "{#RepoRoot}\release\AmaBartalap\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Ama Bartalap"; Filename: "{app}\AmaBartalap.exe"
Name: "{autodesktop}\Ama Bartalap"; Filename: "{app}\AmaBartalap.exe"

[Run]
Filename: "{app}\AmaBartalap.exe"; Description: "Open Ama Bartalap"; Flags: nowait postinstall skipifsilent
