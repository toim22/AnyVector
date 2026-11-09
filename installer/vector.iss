; AnyVector installer (Inno Setup 6.7 or later). Packages target\release\anyvector.exe.
;
;   cargo build --release
;   "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" installer\vector.iss      →   dist\AnyVector-<version>-setup.exe
;
; The version comes from the workspace Cargo.toml unless /DAppVersion=x.y.z is passed.
; The wizard pictures are drawn from the brand SVGs by `cargo run -p xtask -- brand`.
;
; Installs for the current user only (no administrator prompt, in %LOCALAPPDATA%\Programs\AnyVector) or, if the user
; chooses, for all users (in Program Files). Settings, palettes and templates live in the user profile and stay
; there after uninstalling.
;
; Updating an install made while the program was called Vector moves it from its "Vector" folder to an "AnyVector"
; folder and replaces the Vector shortcuts (see FindOldVector in [Code]).
;
; The "Product language" page chooses the language AnyVector starts in (English or Brazilian Portuguese); the choice
; is written to Software\AnyVector (Language, LanguageStamp) and AnyVector applies it once, at its next start. A silent
; setup (the in-app update) leaves the language alone, and /RELAUNCH=1 starts AnyVector again when it is done.

#define AppName      "AnyVector"
#ifndef AppVersion
  ; `version = "x.y.z"` from the [workspace.package] section of Cargo.toml (the INI reader drops the quotes)
  #define AppVersion   ReadIni(SourcePath + "..\Cargo.toml", "workspace.package", "version", "0.0.0")
#endif
#define AppPublisher "AnyVector Team"
#define AppURL       "https://github.com/toim22/Vector"
#define AppExe       "anyvector.exe"

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
OutputBaseFilename=AnyVector-{#AppVersion}-setup
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
; the installer/uninstaller closes AnyVector if it is running (Restart Manager)
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
english.CdrType=AnyVector Drawing
english.VctType=AnyVector Drawing (older format)
english.DirNeedsAdmin=The selected folder requires administrator permission:%n%n%1%n%nTo install there, {#AppName} must be installed for all users. Restart Setup as administrator?
english.DirNotWritable=Setup cannot write to the selected folder:%n%n%1%n%nPlease choose another folder.
english.LangCaption=Product language
english.LangDescription=Choose the language AnyVector starts in.
english.LangSub=AnyVector starts in this language after setup. You can change it later in Tools ▸ Options ▸ Global ▸ General ▸ User interface language.
english.LangEnglish=English
english.LangPortuguese=Portuguese (Brazilian)
brazilianportuguese.AssocGroup=Tipos de arquivo:
brazilianportuguese.AssocCdrOpenWith=Incluir o {#AppName} em "Abrir com" para desenhos .cdr
brazilianportuguese.AssocCdrDefault=Abrir desenhos .cdr com o {#AppName} por padrão (substitui o programa atual do .cdr)
brazilianportuguese.AssocVct=Abrir desenhos .vct (formato antigo do {#AppName}) com o {#AppName}
brazilianportuguese.AssocSvg=Incluir o {#AppName} em "Abrir com" para imagens SVG
brazilianportuguese.CdrType=Desenho do AnyVector
brazilianportuguese.VctType=Desenho do AnyVector (formato antigo)
brazilianportuguese.DirNeedsAdmin=A pasta escolhida exige permissão de administrador:%n%n%1%n%nPara instalar nela, o {#AppName} precisa ser instalado para todos os usuários. Deseja reiniciar a instalação como administrador?
brazilianportuguese.DirNotWritable=Não é possível gravar na pasta escolhida:%n%n%1%n%nEscolha outra pasta.
brazilianportuguese.LangCaption=Idioma do produto
brazilianportuguese.LangDescription=Escolha o idioma em que o AnyVector será iniciado.
brazilianportuguese.LangSub=O AnyVector será iniciado neste idioma após a instalação. Você pode alterá-lo depois em Ferramentas ▸ Opções ▸ Global ▸ Geral ▸ Idioma da interface do usuário.
brazilianportuguese.LangEnglish=Inglês
brazilianportuguese.LangPortuguese=Português Brasileiro

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assoccdr"; Description: "{cm:AssocCdrOpenWith}"; GroupDescription: "{cm:AssocGroup}"; Flags: checkablealone
Name: "assoccdr\default"; Description: "{cm:AssocCdrDefault}"; Flags: unchecked
Name: "assocvct"; Description: "{cm:AssocVct}"; GroupDescription: "{cm:AssocGroup}"
Name: "assocsvg"; Description: "{cm:AssocSvg}"; GroupDescription: "{cm:AssocGroup}"; Flags: unchecked

[InstallDelete]
; the program was called Vector (vector.exe) before 0.1.0
Type: files; Name: "{app}\vector.exe"
; and had these shortcuts (the oldest versions also had a Start menu folder with an uninstall shortcut); they are
; removed only when updating an install of Vector, so another program's "Vector" shortcuts are never touched
Type: files; Name: "{autoprograms}\Vector.lnk"; Check: UpdatingVector
Type: files; Name: "{autodesktop}\Vector.lnk"; Check: UpdatingVector
Type: files; Name: "{autoprograms}\Vector\Vector.lnk"; Check: UpdatingVector
Type: files; Name: "{autoprograms}\Vector\Uninstall Vector.lnk"; Check: UpdatingVector
Type: dirifempty; Name: "{autoprograms}\Vector"; Check: UpdatingVector

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\vector-app\assets\vector.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Vector graphics editor"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Vector graphics editor"; Tasks: desktopicon

[Registry]
; the start language chosen on the Product language page (not by a silent setup: an update keeps the user's language)
Root: HKA; Subkey: "Software\AnyVector"; Flags: uninsdeletekeyifempty
Root: HKA; Subkey: "Software\AnyVector"; ValueType: string; ValueName: "Language"; ValueData: "{code:AppLanguage}"; Flags: uninsdeletevalue; Check: not WizardSilent
Root: HKA; Subkey: "Software\AnyVector"; ValueType: string; ValueName: "LanguageStamp"; ValueData: "{code:LanguageStamp}"; Flags: uninsdeletevalue; Check: not WizardSilent
; the names used while the program was called Vector
Root: HKA; Subkey: "Software\Classes\Vector.Drawing"; Flags: deletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\Vector.Drawing.Vct"; Flags: deletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\Vector.SVG"; Flags: deletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\Applications\vector.exe"; Flags: deletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\.cdr\OpenWithProgids"; ValueName: "Vector.Drawing"; Flags: deletevalue dontcreatekey
Root: HKA; Subkey: "Software\Classes\.svg\OpenWithProgids"; ValueName: "Vector.SVG"; Flags: deletevalue dontcreatekey
; the document type every association points to
Root: HKA; Subkey: "Software\Classes\AnyVector.Drawing"; ValueType: string; ValueName: ""; ValueData: "{cm:CdrType}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\AnyVector.Drawing\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\vector.ico"
Root: HKA; Subkey: "Software\Classes\AnyVector.Drawing\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
; registered application (lists AnyVector in Windows' "Open with" and Default apps)
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".cdr"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".vct"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".svg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
; .cdr: always offered in "Open with"; the default program only if the user asks (CorelDRAW may own .cdr)
Root: HKA; Subkey: "Software\Classes\.cdr\OpenWithProgids"; ValueType: string; ValueName: "AnyVector.Drawing"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoccdr
Root: HKA; Subkey: "Software\Classes\.cdr"; ValueType: string; ValueName: ""; ValueData: "AnyVector.Drawing"; Flags: uninsdeletevalue; Tasks: assoccdr\default
; .vct: AnyVector's older drawing format
Root: HKA; Subkey: "Software\Classes\.vct"; ValueType: string; ValueName: ""; ValueData: "AnyVector.Drawing.Vct"; Flags: uninsdeletevalue; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\AnyVector.Drawing.Vct"; ValueType: string; ValueName: ""; ValueData: "{cm:VctType}"; Flags: uninsdeletekey; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\AnyVector.Drawing.Vct\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\vector.ico"; Tasks: assocvct
Root: HKA; Subkey: "Software\Classes\AnyVector.Drawing.Vct\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocvct
; SVG: "Open with" only
Root: HKA; Subkey: "Software\Classes\.svg\OpenWithProgids"; ValueType: string; ValueName: "AnyVector.SVG"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocsvg
Root: HKA; Subkey: "Software\Classes\AnyVector.SVG"; ValueType: string; ValueName: ""; ValueData: "SVG image ({#AppName})"; Flags: uninsdeletekey; Tasks: assocsvg
Root: HKA; Subkey: "Software\Classes\AnyVector.SVG\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocsvg

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
; the in-app update runs Setup with /SILENT /RELAUNCH=1: AnyVector starts again when it is done
Filename: "{app}\{#AppExe}"; Flags: nowait runasoriginaluser; Check: ShouldRelaunch

[Code]
// In "just for me" mode Setup runs without administrator rights; if the user switches to a protected folder (Program
// Files, the root of C:\…), copying would fail with "access denied". When leaving the folder page, Setup checks
// whether it can write there and, if not, offers to restart as administrator (/ALLUSERS) with the chosen folder.
var
  Relaunching: Boolean;
  LangPage: TInputOptionWizardPage;
  OldVectorDir: String;

// Before 0.1.0 the program was called Vector and was installed in a "Vector" folder. Updating such an install puts
// AnyVector in an "AnyVector" folder next to it; once installed, the old folder's files are removed (RemoveOldVector)
// and so are the "Vector" shortcuts ([InstallDelete]). A folder the user named differently is updated in place.
procedure FindOldVector;
var
  Prev: String;
begin
  Prev := RemoveBackslashUnlessRoot(WizardForm.PrevAppDir);
  if Prev = '' then
    Exit;
  if FileExists(AddBackslash(Prev) + 'vector.exe') or SameText(ExtractFileName(Prev), 'Vector') then
    OldVectorDir := Prev;
  // only the folder of the previous version, and not when Setup was given a folder (/DIR=)
  if SameText(ExtractFileName(Prev), 'Vector') and (ExpandConstant('{param:DIR|}') = '')
     and SameText(RemoveBackslashUnlessRoot(WizardForm.DirEdit.Text), Prev) then
  begin
    WizardForm.DirEdit.Text := AddBackslash(ExtractFileDir(Prev)) + '{#AppName}';
    Log('Updating Vector in ' + Prev + ': installing in ' + WizardForm.DirEdit.Text);
  end;
end;

function UpdatingVector: Boolean;
begin
  Result := OldVectorDir <> '';
end;

procedure RemoveOldVector;
var
  NewDir: String;
begin
  NewDir := RemoveBackslashUnlessRoot(ExpandConstant('{app}'));
  if (OldVectorDir = '') or SameText(NewDir, OldVectorDir) then
    Exit;
  // the uninstall entry now points to the new folder's uninstaller, so the old one goes too
  DelayDeleteFile(OldVectorDir + '\vector.exe', 8);
  DelayDeleteFile(OldVectorDir + '\{#AppExe}', 8);
  DeleteFile(OldVectorDir + '\vector.ico');
  DeleteFile(OldVectorDir + '\unins000.exe');
  DeleteFile(OldVectorDir + '\unins000.dat');
  if RemoveDir(OldVectorDir) then
    Log('Removed the old folder ' + OldVectorDir)
  else
    Log('Kept the old folder ' + OldVectorDir + ' (it still has other files, or a file is in use)');
end;

// Product language: the language AnyVector starts in, the setup's own language unless the user picks the other one
procedure InitializeWizard;
begin
  FindOldVector;
  LangPage := CreateInputOptionPage(wpSelectTasks, CustomMessage('LangCaption'), CustomMessage('LangDescription'),
    CustomMessage('LangSub'), True, False);
  LangPage.Add(CustomMessage('LangEnglish'));
  LangPage.Add(CustomMessage('LangPortuguese'));
  if ActiveLanguage = 'brazilianportuguese' then
    LangPage.SelectedValueIndex := 1
  else
    LangPage.SelectedValueIndex := 0;
end;

function AppLanguage(Param: String): String;
begin
  if LangPage.SelectedValueIndex = 1 then
    Result := 'pt-BR'
  else
    Result := 'en-US';
end;

// a new stamp on every interactive setup, so AnyVector applies the choice once
function LanguageStamp(Param: String): String;
begin
  Result := GetDateTimeString('yyyymmddhhnnss', #0, #0);
end;

function ShouldRelaunch: Boolean;
begin
  Result := WizardSilent and (ExpandConstant('{param:RELAUNCH|0}') = '1');
end;

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
  Probe := AddBackslash(Dir) + '~anyvector-setup-' + GetDateTimeString('hhnnsszzz', #0, #0) + '.tmp';
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
  if CurStep = ssPostInstall then
    RemoveOldVector;
end;

// closes this instance without asking "Exit Setup?" once the elevated one has opened
procedure CancelButtonClick(CurPageID: Integer; var Cancel, Confirm: Boolean);
begin
  if Relaunching then
    Confirm := False;
end;
