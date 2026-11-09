# Ai-View — Windows 一键打包
# 前置：Rust (rustup, MSVC toolchain) + Visual Studio Build Tools 的「使用 C++ 的桌面开发」
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "未检测到 Rust。请先安装：https://rustup.rs" -ForegroundColor Yellow
    exit 1
}

Write-Host "[1/2] 编译 release ..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = "target\release\ai-view.exe"
if (-not (Test-Path $exe)) {
    Write-Host "编译产物不存在，请检查上方报错。" -ForegroundColor Red
    exit 1
}

$mb = [math]::Round((Get-Item $exe).Length / 1MB, 2)

Write-Host "`n[2/2] 完成" -ForegroundColor Green
Write-Host "  路径: $exe"
Write-Host "  体积: $mb MB"
Write-Host "`n把这个 exe 单独拷走就能用，不需要任何运行时。" -ForegroundColor Gray
Write-Host "想再压小：upx --best --lzma `"$exe`"" -ForegroundColor Gray
