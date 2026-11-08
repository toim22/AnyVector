; Vector — Windows installer (Inno Setup 6)
; Build:  "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" installer\vector.iss
; Output: dist\Vector-1.0.0-setup.exe   (requires target\release\vector.exe)

#define AppName "Vector"
#define AppVersion "1.0.0"
#define AppPublisher "Vector Team"
#define AppExe "vector.exe"

[Setup]
AppId={{6B1E3E52-7C4D-4E0B-9A3F-2B8E5D7C1A90}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Vector-{#AppVersion}-setup
SetupIconFile=..\crates\vector-app\assets\vector.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern
WizardImageFile=wizard-large.bmp,wizard-large@2.5x.bmp
WizardSmallImageFile=wizard-small.bmp,wizard-small@2.5x.bmp
WizardImageStretch=yes
Compression=lzma2/ultra64
SolidCompression=yes
ChangesAssociations=yes
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Messages]
english.WelcomeLabel1=Welcome to {#AppName}
english.WelcomeLabel2=This will install {#AppName} {#AppVersion}, the vector graphics editor, on your computer.%n%nDesign illustrations, layouts and logos with precise vector tools.%n%nIt is recommended that you close all other applications before continuing.
english.FinishedHeadingLabel=Ready to take off
english.FinishedLabel={#AppName} has been installed on your computer. Launch it from the Start menu or the desktop icon.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assocvct"; Description: "Open .vct drawings with {#AppName}"; GroupDescription: "File associations:"
Name: "assocsvg"; Description: "Add {#AppName} to ""Open with"" for SVG files"; GroupDescription: "File associations:"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\vector-app\assets\vector.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; .vct native documents
Root: HKA; Subkey: "Software\Classes\.vct"; ValueType: string; ValueName: ""; ValueData: "Vector.Drawing"; Flags: uninsdeletevalue; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\Vector.Drawing"; ValueType: string; ValueName: ""; ValueData: "Vector Drawing"; Flags: uninsdeletekey; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\Vector.Drawing\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\vector.ico"; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\Vector.Drawing\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocvct
; Open with... for SVG
Root: HKA; Subkey: "Software\Classes\.svg\OpenWithProgids"; ValueType: string; ValueName: "Vector.SVG"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocsvg
Root: HKA; Subkey: "Software\Classes\Vector.SVG"; ValueType: string; ValueName: ""; ValueData: "SVG image (Vector)"; Flags: uninsdeletekey; Tasks: assocsvg
Root: HKA; Subkey: "Software\Classes\Vector.SVG\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocsvg

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
