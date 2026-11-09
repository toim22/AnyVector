; Vector installer (Inno Setup 6.7 or later). Packages target\release\vector.exe.
;
;   cargo build --release
;   "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" installer\vector.iss      →   dist\Vector-<version>-setup.exe
;
; The version comes from the workspace Cargo.toml unless /DAppVersion=x.y.z is passed.
; The wizard pictures are drawn from the brand SVGs by `cargo run -p xtask -- brand`.
;
; Installs for the current user only (no administrator prompt, in %LOCALAPPDATA%\Programs\Vector) or, if the user
; chooses, for all users (in Program Files). Settings, palettes and templates live in the user profile and stay
; there after uninstalling.

#define AppName      "Vector"
#ifndef AppVersion
  ; `version = "x.y.z"` from the [workspace.package] section of Cargo.toml (the INI reader drops the quotes)
  #define AppVersion   ReadIni(SourcePath + "..\Cargo.toml", "workspace.package", "version", "0.0.0")
#endif
#define AppPublisher "Vector Team"
#define AppURL       "https://github.com/toim22/Vector"
#define AppExe       "vector.exe"

[Setup]
; the same AppId since the first version: every earlier version is replaced in place
AppId={{6B1E3E52-7C4D-4E0B-9A3F-2B8E5D7C1A90}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
AppCopyright=Copyright © 2026 {#AppPublisher}
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppName} Setup
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=Vector-{#AppVersion}-setup
SetupIconFile=..\crates\vector-app\assets\vector.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
; the modern wizard in Windows 11's style, light or dark as Windows is; one picture per display scale
WizardStyle=modern dynamic windows11 hidebevels
DisableWelcomePage=no
WizardImageFile=WizardLarge-100.png,WizardLarge-125.png,WizardLarge-150.png,WizardLarge-175.png,WizardLarge-200.png,WizardLarge-250.png
WizardImageFileDynamicDark=WizardLarge-dark-100.png,WizardLarge-dark-125.png,WizardLarge-dark-150.png,WizardLarge-dark-175.png,WizardLarge-dark-200.png,WizardLarge-dark-250.png
WizardSmallImageFile=WizardSmall-100.png,WizardSmall-125.png,WizardSmall-150.png,WizardSmall-175.png,WizardSmall-200.png,WizardSmall-250.png
WizardSmallImageFileDynamicDark=WizardSmall-100.png,WizardSmall-125.png,WizardSmall-150.png,WizardSmall-175.png,WizardSmall-200.png,WizardSmall-250.png
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
; installs for the current user only (no UAC); the dialog lets the user choose "all users". If the user picks a
; protected folder (such as Program Files) in per-user mode, [Code] offers to restart as administrator
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
; an update installs where the previous version is, for all users or only this one as before, without asking again
UsePreviousPrivileges=yes
UsePreviousAppDir=yes
UsePreviousTasks=yes
; the installer/uninstaller closes Vector if it is running (Restart Manager)
CloseApplications=yes
RestartApplications=no
ChangesAssociations=yes
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Messages]
english.WelcomeLabel1=Welcome to {#AppName} {#AppVersion}
english.WelcomeLabel2=This will install {#AppName}, the vector graphics editor, on your computer.%n%nDraw illustrations, layouts and logos with precise vector tools, and save them as .cdr drawings.%n%nIt is recommended that you close all other applications before continuing.
english.FinishedHeadingLabel=Ready to take off
english.FinishedLabel={#AppName} is installed on your computer. Open it from the Start menu.
brazilianportuguese.WelcomeLabel1=Bem-vindo ao {#AppName} {#AppVersion}
brazilianportuguese.WelcomeLabel2=Este assistente instala o {#AppName}, o editor de gráficos vetoriais, no seu computador.%n%nCrie ilustrações, layouts e logotipos com ferramentas vetoriais precisas e salve-os como desenhos .cdr.%n%nRecomenda-se fechar os outros aplicativos antes de continuar.
brazilianportuguese.FinishedHeadingLabel=Pronto para decolar
brazilianportuguese.FinishedLabel=O {#AppName} está instalado no seu computador. Abra-o pelo menu Iniciar.

[CustomMessages]
english.AssocGroup=File types:
english.AssocCdrOpenWith=Add {#AppName} to "Open with" for .cdr drawings
english.AssocCdrDefault=Open .cdr drawings with {#AppName} by default (replaces the current program for .cdr)
english.AssocVct=Open .vct drawings (older {#AppName} format) with {#AppName}
english.AssocSvg=Add {#AppName} to "Open with" for SVG images
english.CdrType=Vector Drawing
english.VctType=Vector Drawing (older format)
english.DirNeedsAdmin=The selected folder requires administrator permission:%n%n%1%n%nTo install there, {#AppName} must be installed for all users. Restart Setup as administrator?
english.DirNotWritable=Setup cannot write to the selected folder:%n%n%1%n%nPlease choose another folder.
brazilianportuguese.AssocGroup=Tipos de arquivo:
brazilianportuguese.AssocCdrOpenWith=Incluir o {#AppName} em "Abrir com" para desenhos .cdr
brazilianportuguese.AssocCdrDefault=Abrir desenhos .cdr com o {#AppName} por padrão (substitui o programa atual do .cdr)
brazilianportuguese.AssocVct=Abrir desenhos .vct (formato antigo do {#AppName}) com o {#AppName}
brazilianportuguese.AssocSvg=Incluir o {#AppName} em "Abrir com" para imagens SVG
brazilianportuguese.CdrType=Desenho do Vector
brazilianportuguese.VctType=Desenho do Vector (formato antigo)
brazilianportuguese.DirNeedsAdmin=A pasta escolhida exige permissão de administrador:%n%n%1%n%nPara instalar nela, o {#AppName} precisa ser instalado para todos os usuários. Deseja reiniciar a instalação como administrador?
brazilianportuguese.DirNotWritable=Não é possível gravar na pasta escolhida:%n%n%1%n%nEscolha outra pasta.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assoccdr"; Description: "{cm:AssocCdrOpenWith}"; GroupDescription: "{cm:AssocGroup}"; Flags: checkablealone
Name: "assoccdr\default"; Description: "{cm:AssocCdrDefault}"; Flags: unchecked
Name: "assocvct"; Description: "{cm:AssocVct}"; GroupDescription: "{cm:AssocGroup}"
Name: "assocsvg"; Description: "{cm:AssocSvg}"; GroupDescription: "{cm:AssocGroup}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\vector-app\assets\vector.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Vector graphics editor"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Vector graphics editor"; Tasks: desktopicon

[Registry]
; the document type every association points to
Root: HKA; Subkey: "Software\Classes\Vector.Drawing"; ValueType: string; ValueName: ""; ValueData: "{cm:CdrType}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Vector.Drawing\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\vector.ico"
Root: HKA; Subkey: "Software\Classes\Vector.Drawing\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
; registered application (lists Vector in Windows' "Open with" and Default apps)
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".cdr"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".vct"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".svg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
; .cdr: always offered in "Open with"; the default program only if the user asks (CorelDRAW may own .cdr)
Root: HKA; Subkey: "Software\Classes\.cdr\OpenWithProgids"; ValueType: string; ValueName: "Vector.Drawing"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoccdr
Root: HKA; Subkey: "Software\Classes\.cdr"; ValueType: string; ValueName: ""; ValueData: "Vector.Drawing"; Flags: uninsdeletevalue; Tasks: assoccdr\default
; .vct: Vector's older drawing format
Root: HKA; Subkey: "Software\Classes\.vct"; ValueType: string; ValueName: ""; ValueData: "Vector.Drawing.Vct"; Flags: uninsdeletevalue; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\Vector.Drawing.Vct"; ValueType: string; ValueName: ""; ValueData: "{cm:VctType}"; Flags: uninsdeletekey; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\Vector.Drawing.Vct\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\vector.ico"; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\Vector.Drawing.Vct\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocvct
; SVG: "Open with" only
Root: HKA; Subkey: "Software\Classes\.svg\OpenWithProgids"; ValueType: string; ValueName: "Vector.SVG"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocsvg
Root: HKA; Subkey: "Software\Classes\Vector.SVG"; ValueType: string; ValueName: ""; ValueData: "SVG image ({#AppName})"; Flags: uninsdeletekey; Tasks: assocsvg
Root: HKA; Subkey: "Software\Classes\Vector.SVG\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocsvg

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
// In "just for me" mode Setup runs without administrator rights; if the user switches to a protected folder (Program
// Files, the root of C:\…), copying would fail with "access denied". When leaving the folder page, Setup checks
// whether it can write there and, if not, offers to restart as administrator (/ALLUSERS) with the chosen folder.
var
  Relaunching: Boolean;

// nearest folder that already exists (the install folder may not exist yet)
function ExistingAncestor(Dir: String): String;
var
  Parent: String;
begin
  Result := RemoveBackslashUnlessRoot(Dir);
  while not DirExists(Result) do
  begin
    Parent := ExtractFileDir(Result);
    if (Parent = '') or (Parent = Result) then
    begin
      Result := '';
      Exit;
    end;
    Result := Parent;
  end;
end;

function CanWriteTo(Dir: String): Boolean;
var
  Probe: String;
begin
  Dir := ExistingAncestor(Dir);
  Result := Dir <> '';
  if not Result then
    Exit;
  Probe := AddBackslash(Dir) + '~vector-setup-' + GetDateTimeString('hhnnsszzz', #0, #0) + '.tmp';
  Result := SaveStringToFile(Probe, '', False);
  if Result then
    DeleteFile(Probe);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  Dir: String;
  ErrorCode: Integer;
begin
  Result := True;
  if (CurPageID <> wpSelectDir) or WizardSilent then
    Exit;
  Dir := WizardDirValue;
  if CanWriteTo(Dir) then
    Exit;
  Result := False;
  if IsAdminInstallMode then
  begin
    MsgBox(FmtMessage(CustomMessage('DirNotWritable'), [Dir]), mbError, MB_OK);
    Exit;
  end;
  if MsgBox(FmtMessage(CustomMessage('DirNeedsAdmin'), [Dir]), mbConfirmation, MB_YESNO) <> IDYES then
    Exit;
  if ShellExec('runas', ExpandConstant('{srcexe}'),
               '/ALLUSERS /LANG=' + ActiveLanguage + ' /DIR="' + Dir + '"',
               '', SW_SHOW, ewNoWait, ErrorCode) then
  begin
    Relaunching := True;
    WizardForm.Close;
  end
  else if ErrorCode <> 1223 then  // 1223: the user declined UAC; stay on this page
    MsgBox(SysErrorMessage(ErrorCode), mbError, MB_OK);
end;

// An earlier version installed in the other mode would stay next to this one. Installing for all users, a copy that
// was installed only for this user is removed first (its uninstaller, silently, before the files are copied).
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{6B1E3E52-7C4D-4E0B-9A3F-2B8E5D7C1A90}_is1';

procedure RemoveOtherInstall(Root: Integer);
var
  Cmd: String;
  Code: Integer;
begin
  if not RegQueryStringValue(Root, UninstallKey, 'UninstallString', Cmd) then
    Exit;
  Cmd := RemoveQuotes(Cmd);
  Log('Removing the per-user install of an earlier version: ' + Cmd);
  if not Exec(Cmd, '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Log('Could not run it: ' + SysErrorMessage(Code));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssInstall) and IsAdminInstallMode then
    RemoveOtherInstall(HKCU);
end;

// closes this instance without asking "Exit Setup?" once the elevated one has opened
procedure CancelButtonClick(CurPageID: Integer; var Cancel, Confirm: Boolean);
begin
  if Relaunching then
    Confirm := False;
end;
