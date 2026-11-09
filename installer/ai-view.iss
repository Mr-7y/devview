; Ai-View Windows 安装向导脚本（Inno Setup 6）
; 编译：ISCC.exe installer\ai-view.iss

[Setup]
AppId={{B3F7A1E2-5C4D-4E9B-8A26-71D3F0C59E44}
AppName=Ai-View
AppVersion=1.3.1
AppPublisher=Mr.7y
DefaultDirName={autopf}\Ai-View
DefaultGroupName=Ai-View
OutputDir=..\dist
OutputBaseFilename=Ai-View-v1.3.1-windows-x64-setup
Compression=lzma2/max
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
UninstallDisplayIcon={app}\ai-view.exe
SetupIconFile=..\icons\icon.ico

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Files]
Source: "..\dist\Ai-View.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Ai-View"; Filename: "{app}\ai-view.exe"
Name: "{group}\卸载 Ai-View"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Ai-View"; Filename: "{app}\ai-view.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式(&D)"; GroupDescription: "附加图标："; Flags: unchecked

[Run]
Filename: "{app}\ai-view.exe"; Description: "立即运行 Ai-View"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{userappdata}\Ai-View"
