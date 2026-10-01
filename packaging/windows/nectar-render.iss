; Nectar Render : installateur Windows (Inno Setup 6).
; Installation par utilisateur, sans droits administrateur :
;   %LOCALAPPDATA%\Programs\Nectar Render
; (le plugin Obsidian cherche l'application à cet endroit par défaut).
;
;   iscc /DAppVersion=0.1.0 packaging\windows\nectar-render.iss

#define AppName "Nectar Render"
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{8C1F3B52-6E0A-4C7D-9B2E-4A1D7F3E9C21}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Yuzuctus
AppPublisherURL=https://github.com/Yuzuctus/Nectar-Render
DefaultDirName={localappdata}\Programs\Nectar Render
DisableDirPage=auto
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=NectarRender-{#AppVersion}-installation
SetupIconFile=..\..\assets\icon\nectar-render.ico
UninstallDisplayIcon={app}\nectar-render.exe
LicenseFile=..\..\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "Créer un raccourci sur le bureau"; Flags: unchecked
Name: "mdcontext"; Description: "Ajouter « Ouvrir avec Nectar Render » au clic droit sur les notes .md"

[Files]
Source: "..\..\target\release\nectar-render.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\target\release\nectar.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\obsidian-plugin\main.js"; DestDir: "{app}\plugin-obsidian\nectar-render"; Flags: ignoreversion
Source: "..\..\obsidian-plugin\manifest.json"; DestDir: "{app}\plugin-obsidian\nectar-render"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\fonts\LICENSE-*"; DestDir: "{app}\licences"; Flags: ignoreversion
Source: "..\..\assets\typst\mitex\LICENSE"; DestDir: "{app}\licences"; DestName: "LICENSE-mitex.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Nectar Render"; Filename: "{app}\nectar-render.exe"
Name: "{autodesktop}\Nectar Render"; Filename: "{app}\nectar-render.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\SystemFileAssociations\.md\shell\NectarRender"; ValueType: string; ValueName: ""; ValueData: "Ouvrir avec Nectar Render"; Flags: uninsdeletekey; Tasks: mdcontext
Root: HKCU; Subkey: "Software\Classes\SystemFileAssociations\.md\shell\NectarRender"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\nectar-render.exe"; Tasks: mdcontext
Root: HKCU; Subkey: "Software\Classes\SystemFileAssociations\.md\shell\NectarRender\command"; ValueType: string; ValueName: ""; ValueData: """{app}\nectar-render.exe"" ""%1"""; Tasks: mdcontext

[Run]
Filename: "{app}\nectar-render.exe"; Description: "Lancer Nectar Render"; Flags: nowait postinstall skipifsilent
