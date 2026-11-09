# Ai-View 一键安装脚本
# 用法：右键"使用 PowerShell 运行"，或执行 .\install.ps1
$ErrorActionPreference = "Stop"
$src = $PSScriptRoot
if (-not $src) { $src = Split-Path -Parent $MyInvocation.MyCommand.Path }

$name = "Ai-View"
$exe = "ai-view.exe"
$dest = Join-Path $env:LOCALAPPDATA "Programs\$name"

try {
    # 1. 复制程序
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    Copy-Item (Join-Path $src $exe) $dest -Force

    # 2. 开始菜单快捷方式
    $sm = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\$name"
    New-Item -ItemType Directory -Force -Path $sm | Out-Null
    $ws = New-Object -ComObject WScript.Shell
    $lnk = $ws.CreateShortcut((Join-Path $sm "$name.lnk"))
    $lnk.TargetPath = Join-Path $dest $exe
    $lnk.WorkingDirectory = $dest
    $lnk.Save()

    # 3. 桌面快捷方式
    $dt = [Environment]::GetFolderPath("Desktop")
    $lnk2 = $ws.CreateShortcut((Join-Path $dt "$name.lnk"))
    $lnk2.TargetPath = Join-Path $dest $exe
    $lnk2.WorkingDirectory = $dest
    $lnk2.Save()

    # 4. 卸载脚本
    $un = @'
$dest = Join-Path $env:LOCALAPPDATA "Programs\Ai-View"
Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
$sm = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\Ai-View"
Remove-Item $sm -Recurse -Force -ErrorAction SilentlyContinue
$dt = [Environment]::GetFolderPath("Desktop")
Remove-Item (Join-Path $dt "Ai-View.lnk") -Force -ErrorAction SilentlyContinue
Write-Host "Ai-View 已卸载"
'@
    Set-Content -Path (Join-Path $dest "uninstall.ps1") -Value $un -Encoding UTF8

    Write-Host ""
    Write-Host "Ai-View 已安装到 $dest"
    Write-Host "已创建开始菜单与桌面快捷方式"
    Write-Host "卸载：运行 $dest\uninstall.ps1"
} catch {
    Write-Host "安装失败：$($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
