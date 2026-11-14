#define AppName "Super Download Manager"
#define AppVersion "0.1.0"
#define AppExe "download-manager-ui.exe"

[Setup]
AppId={{5522D0CF-FB82-46BE-8C02-5E288746C68E}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Super Download Manager
DefaultDirName={localappdata}\Programs\Super Download Manager
DefaultGroupName={#AppName}
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=output
OutputBaseFilename=Super-Download-Manager-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\..\LICENSE
UninstallDisplayIcon={app}\{#AppExe}

[Files]
Source: "..\..\frontend\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent
