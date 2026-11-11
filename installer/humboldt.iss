; HUMBOLDT Windows Inno Setup Script
; Installs HUMBOLDT: Analytics & Real-time Global Information System

#define MyAppName "HUMBOLDT"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "HUMBOLDT Open Source Initiative"
#define MyAppURL "https://github.com/KappinChromie/HUMBOLDT"
#define MyAppExeName "humboldt.exe"
#define MyAUMID "Humboldt.Desktop"

[Setup]
AppId={{E15F54E7-814E-4347-8B2B-17983196E87A}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\Humboldt
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputBaseFilename=humboldt-setup-x64
OutputDir=output
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: isreadme
Source: "..\ARCHITECTURE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\RUNBOOK.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\{#MyAppName}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; AppUserModelID: "{#MyAUMID}"
Name: "{userprograms}\{#MyAppName}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; AppUserModelID: "{#MyAUMID}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
