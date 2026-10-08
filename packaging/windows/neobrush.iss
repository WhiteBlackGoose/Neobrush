; Inno Setup script for the Neobrush installer.
; Build: iscc /DAppVersion=1.0.0 /DSourceExe=path\to\neobrush.exe /DOutputDir=dist packaging\windows\neobrush.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef NumVersion
  #define NumVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\target\release\neobrush.exe"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

[Setup]
AppId={{EB72D76E-0C47-4E8A-8BE0-7E9028451CF3}
AppName=Neobrush
AppVersion={#AppVersion}
AppVerName=Neobrush {#AppVersion}
VersionInfoVersion={#NumVersion}
VersionInfoProductName=Neobrush
AppPublisher=WhiteBlackGoose
AppPublisherURL=https://github.com/WhiteBlackGoose/Neobrush
AppSupportURL=https://github.com/WhiteBlackGoose/Neobrush/issues
AppUpdatesURL=https://github.com/WhiteBlackGoose/Neobrush/releases
DefaultDirName={autopf}\Neobrush
DefaultGroupName=Neobrush
DisableProgramGroupPage=yes
DisableWelcomePage=no
; Per-user install by default (no admin prompt); the dialog offers "all users".
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=Neobrush-{#AppVersion}-windows-x86_64-setup
SetupIconFile=..\neobrush.ico
UninstallDisplayIcon={app}\neobrush.exe
UninstallDisplayName=Neobrush
WizardStyle=modern
WizardSizePercent=110
WizardImageFile=wizard-164.bmp,wizard-410.bmp
WizardSmallImageFile=small-55.bmp,small-138.bmp
Compression=lzma2/ultra64
SolidCompression=yes
ChangesAssociations=yes
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "ukrainian"; MessagesFile: "compiler:Languages\Ukrainian.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "hebrew"; MessagesFile: "compiler:Languages\Hebrew.isl"

[CustomMessages]
AssocOra=Open OpenRaster (.ora) files with Neobrush
OpenWith=Add Neobrush to "Open with" for images
Integration=Integration:
german.AssocOra=OpenRaster-Dateien (.ora) mit Neobrush öffnen
german.OpenWith=Neobrush zu „Öffnen mit“ für Bilder hinzufügen
spanish.AssocOra=Abrir archivos OpenRaster (.ora) con Neobrush
spanish.OpenWith=Añadir Neobrush a «Abrir con» para imágenes
spanish.Integration=Integración:
french.AssocOra=Ouvrir les fichiers OpenRaster (.ora) avec Neobrush
french.OpenWith=Ajouter Neobrush à « Ouvrir avec » pour les images
italian.AssocOra=Apri i file OpenRaster (.ora) con Neobrush
italian.OpenWith=Aggiungi Neobrush ad «Apri con» per le immagini
italian.Integration=Integrazione:
portuguese.AssocOra=Abrir ficheiros OpenRaster (.ora) com o Neobrush
portuguese.OpenWith=Adicionar o Neobrush a «Abrir com» para imagens
portuguese.Integration=Integração:
ukrainian.AssocOra=Відкривати файли OpenRaster (.ora) у Neobrush
ukrainian.OpenWith=Додати Neobrush до «Відкрити за допомогою» для зображень
ukrainian.Integration=Інтеграція:
russian.AssocOra=Открывать файлы OpenRaster (.ora) в Neobrush
russian.OpenWith=Добавить Neobrush в «Открыть с помощью» для изображений
russian.Integration=Интеграция:
japanese.AssocOra=OpenRaster (.ora) ファイルを Neobrush で開く
japanese.OpenWith=画像の「プログラムから開く」に Neobrush を追加
japanese.Integration=統合:
hebrew.AssocOra=פתיחת קובצי OpenRaster‏ (.ora) ב־Neobrush
hebrew.OpenWith=הוספת Neobrush ל„פתיחה באמצעות” עבור תמונות
hebrew.Integration=שילוב:

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "assocora"; Description: "{cm:AssocOra}"; GroupDescription: "{cm:Integration}"
Name: "openwith"; Description: "{cm:OpenWith}"; GroupDescription: "{cm:Integration}"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "neobrush.exe"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Neobrush"; Filename: "{app}\neobrush.exe"; Comment: "Raster graphics editor"
Name: "{autodesktop}\Neobrush"; Filename: "{app}\neobrush.exe"; Tasks: desktopicon

[Registry]
; Application registration ("Open with" list, default programs).
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Neobrush"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\DefaultIcon"; ValueType: string; ValueData: "{app}\neobrush.exe,0"
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\shell\open\command"; ValueType: string; ValueData: """{app}\neobrush.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\SupportedTypes"; ValueType: string; ValueName: ".jpg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\SupportedTypes"; ValueType: string; ValueName: ".jpeg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\SupportedTypes"; ValueType: string; ValueName: ".bmp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\SupportedTypes"; ValueType: string; ValueName: ".webp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\neobrush.exe\SupportedTypes"; ValueType: string; ValueName: ".ora"; ValueData: ""
; ProgID for OpenRaster documents.
Root: HKA; Subkey: "Software\Classes\Neobrush.ora"; ValueType: string; ValueData: "OpenRaster image"; Flags: uninsdeletekey; Tasks: assocora
Root: HKA; Subkey: "Software\Classes\Neobrush.ora\DefaultIcon"; ValueType: string; ValueData: "{app}\neobrush.exe,0"; Tasks: assocora
Root: HKA; Subkey: "Software\Classes\Neobrush.ora\shell\open\command"; ValueType: string; ValueData: """{app}\neobrush.exe"" ""%1"""; Tasks: assocora
Root: HKA; Subkey: "Software\Classes\.ora"; ValueType: string; ValueData: "Neobrush.ora"; Flags: uninsdeletevalue; Tasks: assocora
Root: HKA; Subkey: "Software\Classes\.ora\OpenWithProgids"; ValueType: string; ValueName: "Neobrush.ora"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocora
; "Open with" entries for common image types.
Root: HKA; Subkey: "Software\Classes\.png\OpenWithList\neobrush.exe"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.jpg\OpenWithList\neobrush.exe"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.jpeg\OpenWithList\neobrush.exe"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.bmp\OpenWithList\neobrush.exe"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.webp\OpenWithList\neobrush.exe"; Flags: uninsdeletekey; Tasks: openwith

[Run]
Filename: "{app}\neobrush.exe"; Description: "{cm:LaunchProgram,Neobrush}"; Flags: nowait postinstall skipifsilent
