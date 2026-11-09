# Ai-View 打包指南

本文档说明如何在三大平台本地打包 Ai-View，以及如何通过 GitHub Actions 自动发布。

## 前置要求（所有平台）

- **Rust**：通过 [rustup](https://rustup.rs) 安装 stable 工具链
- 编译目标：
  - Windows / Linux：`stable-x86_64-pc-windows-msvc`（需 VS Build Tools）或 `stable-x86_64-pc-windows-gnu`
  - macOS：`stable`（Universal 双架构需要额外装 `aarch64-apple-darwin` 与 `x86_64-apple-darwin` 两个 target）

## Windows → 单个 exe

### 前置（一次性）

1. **Rust**：安装 rustup，默认的 `stable-x86_64-pc-windows-msvc` 即可
2. **Visual Studio 2022 Build Tools**：安装时勾选「使用 C++ 的桌面开发」（约 3 GB）

### 打包

在该目录打开 PowerShell：

```powershell
.\build-windows.ps1
```

或直接：

```powershell
cargo build --release
```

产物：`target\release\ai-view.exe`（约 4.7 MB），发给别人只需要这一个 exe。

可选：用 UPX 进一步压缩（会略微提高杀软误报率）：

```powershell
upx --best --lzma target\release\ai-view.exe
```

### 调试环境变量（本地编译）

如果本机用户名含中文，且使用 GNU 工具链，`dlltool` 可能因路径编码失败。MSVC 工具链不受影响。
本地编译时设置以下变量即可绕开对系统 VS 环境的依赖（SDK 从 NuGet 解压）：

```powershell
$env:INCLUDE = "…\MSVC\include;…\SDK\Include\<ver>\ucrt;…\um;…\shared;…\winrt"
$env:LIB     = "…\MSVC\lib\x64;…\SDK\um\x64;…\SDK\ucrt\x64"
```

## macOS → .app

### 前置（一次性）

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

### 打包

```bash
./pack-macos.sh
```

脚本会：

1. 分别编译 ARM64 与 x86_64 两个架构
2. 用 `lipo` 合并为 Universal 二进制
3. 组装 `dist/Ai-View.app`（含图标与 Info.plist）
4. 自动 ad-hoc 签名

产物：`dist/Ai-View.app`，拖进「应用程序」即完成安装。

### 分发签名

要分发给别人且不触发出门拦截，需要 Apple 开发者证书：

```bash
codesign --deep --force --sign "Developer ID Application: 你的名字 (TEAMID)" dist/Ai-View.app
xcrun notarytool submit dist/Ai-View.zip --apple-id ... --team-id ... --password ...
```

## Linux → tar.gz

### 前置（一次性）

```bash
sudo apt install build-essential libgtk-3-dev libxkbcommon-dev libgl1-mesa-dev
```

### 打包

```bash
./build.sh          # 等价于 cargo build --release
```

产物 `target/release/ai-view`，直接运行。依赖系统里的 `libxkbcommon`、`libgl`（桌面发行版一般自带）。

## GitHub Actions 自动发布

打 tag 即触发三平台构建并自动创建 Release：

```bash
git tag v1.x.0
git push origin main --tags
```

CI（`.github/workflows/build.yml`）会：

1. Windows / Linux / macOS 三个 job 各自原生编译（`--locked`，所以请保证 `Cargo.lock` 与 `Cargo.toml` 一致并已提交）
2. 上传 artifact
3. `release` job 汇总资产并创建 GitHub Release

发布完成后，在仓库的 Releases 页即可下载对应平台的安装包。

## 版本号同步清单

发版时以下位置的版本号必须一致：

- `Cargo.toml`
- `Cargo.lock`（构建时自动更新）
- `pack-macos.sh` 里的 `CFBundleVersion` / `CFBundleShortVersionString`
- `packaging/Info.plist`
- `README.md` 的下载表格

## 图标更新

- `src/icon.rgba`：64×64 原始像素，窗口与任务栏图标（`main.rs` 内嵌）
- `icons/`：`icon.ico`（Windows 安装包）、`icon.icns`（macOS）、`icon.png` 与各尺寸 PNG（Linux）

修改 logo 后需同时更新两处，Windows 下改动 `icon.rgba` 后重新编译即可生效。
