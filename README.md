# Ai-View

极简代码浏览器 —— 纯原生渲染，没有 WebView、没有 Electron、没有 Python 运行时。

打开本地代码目录 → 树形浏览 → 语法高亮 → Markdown 预览 → git 变更标记 → 单文件 diff。

夜航 / 日纸双主题，左右侧栏可独立收合；代码只读，Markdown 可编辑并保存。

[![Build](https://github.com/Mr-7y/ai-view/actions/workflows/build.yml/badge.svg)](https://github.com/Mr-7y/ai-view/actions/workflows/build.yml)

## 下载

不用装 Rust，不用编译，直接下现成的：

| 系统 | 包 | 用法 |
|---|---|---|
| Windows | [Ai-View-v1.3.0-windows-x64.zip](https://github.com/Mr-7y/ai-view/releases/latest) | 解压双击 `Ai-View.exe`，约 4.7 MB，无需运行库 |
| macOS | [Ai-View-v1.3.0-macos-universal.zip](https://github.com/Mr-7y/ai-view/releases/latest) | 解压得到 `Ai-View.app`，Intel / Apple Silicon 通用 |
| Linux | [Ai-View-v1.3.0-linux-x64.tar.gz](https://github.com/Mr-7y/ai-view/releases/latest) | `tar -xzf` 后 `./ai-view` |

三个包都由 GitHub Actions 在各自原生机器上编译（Windows Server、macOS、Ubuntu 22.04），不是交叉编译产物。

## 三个版本，这次是第三种

| | 依赖 | 二进制 | 内存 | 启动 |
|---|---|---|---|---|
| 第一版 Python + 浏览器标签页 | Python + Chrome | 280 KB | 170~270 MB | 0.3 s |
| 第二版 Tauri 2（WebView） | WebView2 / WKWebView | 4.0 MB | ~220 MB | 0.3 s |
| **本版 纯 Rust + egui** | **无** | **4.7 MB** | **见下** | **0.5 s** |

没有 WebView 就没有第二个浏览器内核，这是内存能压下来的唯一原因。

## 实测数据（Linux，无 GPU 的软件渲染环境）

| 指标 | 数值 |
|---|---|
| 二进制 | 4.7 MB（单个可执行文件，字体不内嵌，用系统字体） |
| 启动到窗口出现 | 505 ms（软件渲染；有显卡会更快） |
| 空闲内存（PSS） | 148 MB |
| 空闲 CPU | 6%（软件渲染下每 5 秒重绘一次；有显卡约 1~2%） |

拆开看 148 MB：

- **57 MB 是 `libLLVM.so`** —— Xvfb 没有 GPU，走 llvmpipe 软件光栅化，这块在有显卡的机器上不存在
- **19 MB 是系统中文font**（`NotoSansCJK` / `msyh.ttc`），egui 需要把字体字节常驻在内存里
- 剩下 ~70 MB 是 Rust 堆 + 字形网格 + 窗口缓冲区

**在正常有显卡的 Windows / macOS 上，预期落在 55~75 MB**。同机对比 Tauri 版 221 MB，降低约 65%。

## 运行

```bash
./ai-view                              # 空启动，界面里点 ＋ 选目录
./ai-view ~/code/project-a ~/code/b    # 直接带目录启动（可多个）
```

Windows 双击 `ai-view.exe` 即可，不会弹黑色控制台窗口。

## 功能

- **多项目**：顶栏下拉切换，`＋` 原生目录选择框，`−` 从列表移除（不动磁盘），命令行参数与拖入文件夹也能加
- **目录树**：懒加载，自动跳过 `node_modules` / `vendor` / `target` / `.git` 等；展开状态在刷新后保留
- **git 标记**：`M/A/D/R/U` 着色徽标，有改动的目录打一个 `•`
- **变更视图**：侧栏第二个 tab，按 *已修改 / 已删除 / 未跟踪* 分组，点已删除文件直接看它被删前的 diff
- **语法高亮**：手写词法扫描，覆盖 PHP / JS / TS / Python / Rust / Go / Java / C / C++ / C# / Kotlin / Swift / Ruby / Bash / SQL / HTML / CSS / SCSS / JSON / YAML / TOML / INI / Markdown / Diff / Lua / GraphQL，夜航 / 日纸两套配色
- **diff 视图**：绿色新增、红色删除、青色 hunk 头
- **Markdown 预览**：标题、列表、引用、围栏代码、表格、分隔线，行内 `code` / 粗体 / 斜体 / 删除线 / 链接
- **Markdown 编辑**：`.md` 切到「源码」即可输入，`Ctrl/Cmd + S` 写盘并更新脏标记
- **双主题**：夜航（默认）与日纸，切换后代码 token、选中态、面板底色一起换
- **可折叠双侧栏**：顶栏一键同时收起左右栏，也可单独收起；折叠后留 24px 竖条可点开
- **git 工作流带**：分支、远程、↑↓、提交图、提交框，提交 / 拉取 / 推送直接在顶栏完成
- **Ctrl+P 模糊搜文件**（子序列匹配，词首与连续命中加分）
- **安全**：路径越界拦截、二进制拒渲染、>256 KB 自动截断并提示

## 快捷键

| 键 | 动作 |
|---|---|
| `Ctrl/Cmd + P` | 快速打开文件 |
| `Ctrl/Cmd + S` | 保存当前 Markdown |
| `Ctrl + B` | 收起 / 展开左栏 |
| `F5` | 刷新 git 状态与目录树 |
| `Esc` | 关闭搜索 / 对话框 |
| 鼠标拖选 | 选中代码，`Ctrl/Cmd + C` 复制 |

## 打包

### Windows → 单个 exe

前置（一次性）：

1. 装 **Rust**：<https://rustup.rs> —— 一路回车，默认的 `stable-x86_64-pc-windows-msvc` 就对了
2. 装 **Visual Studio 2022 Build Tools**：<https://visualstudio.microsoft.com/visual-cpp-build-tools/>
   安装时勾选 **「使用 C++ 的桌面开发」**（只勾这个就够，约 3 GB）

然后在该目录打开 PowerShell：

```powershell
.\build-windows.ps1
```

或直接：

```powershell
cargo build --release
```

产物：`target\release\ai-view.exe`（约 4.7 MB）

**发给别人只需要这一个 exe**，不用装 .NET、不用装 WebView2、不用装 Python。首次编译约 2~4 分钟，之后改代码增量 20 秒左右。

想再小一点（可选，会略微提升杀软误报率）：

```powershell
upx --best --lzma target\release\ai-view.exe   # 4.7 MB → 约 1.8 MB
```

### macOS → .app

前置：`xcode-select --install`，再按 rustup.rs 装 Rust。

```bash
./pack-macos.sh
```

产物：`dist/Ai-View.app`，双击即可；拖进「应用程序」就完成安装。脚本会自动做 ad-hoc 签名，第一次打开若提示未验证，右键 →「打开」。

要分发给别人又不触发出门拦截，需要 Apple 开发者证书：

```bash
codesign --deep --force --sign "Developer ID Application: 你的名字 (TEAMID)" dist/Ai-View.app
xcrun notarytool submit ... # 公证
```

### Linux

```bash
./build.sh          # 等价于 cargo build --release
```

产物 `target/release/ai-view`，直接运行。依赖系统里的 `libxkbcommon`、`libgl`（桌面发行版都自带）。

打成 AppImage 需要额外装 `linuxdeploy`，本项目没有内置。

## 项目结构

```
Cargo.toml            # eframe 0.29 + rfd，release 开了 opt-level=z / lto / strip
src/main.rs           # 窗口与启动参数
src/app.rs            # 全部界面：顶栏 / 树 / 变更 / 代码区 / 搜索 / 状态栏
src/syntax.rs         # 手写词法高亮器，约 1000 行，零依赖
src/md.rs             # Markdown 块级解析与预览渲染，零依赖
src/theme.rs          # 配色、字体加载、egui 样式
src/tree.rs           # 懒加载目录树
src/fsops.rs          # 读目录 / 读文件 / 语言识别 / 自然排序
src/gitops.rs         # git status / diff，带超时和缓存
src/conf.rs           # ~/.ai-view/projects.json
```

体积能压到 4.7 MB 的几个关键设置：

```toml
[profile.release]
opt-level = "z"   # 优化体积
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## 已知限制

- **中文走系统字体**，Windows 用微软雅黑、macOS 用苹方、Linux 用 Noto CJK。好处是包小，代价是三平台字形略有差异；Linux 上如果没装中文字体会显示方块，`apt install fonts-noto-cjk` 即可。
- **单文件上限 256 KB**，超出部分截断并显示提示。原因是 egui 要为整段文本生成一份排版缓存，超过这个量级的内存增长不划算。代码文件超过 6000 行的场景很少见。
- **代码只读，Markdown 可编辑**。源码用 `RoBuf` 关闭 `TextBuffer` 的写入；只有 `.md` 文件切到「源码」后可以输入，`Ctrl/Cmd + S` 写盘。egui 自带的多行编辑撑不住十万行代码，所以不放开到所有文件。
- **git 通过子进程调用**，仓库特别大（>10 万文件）时首次 `git status` 会有 1~2 秒延迟，之后走 5 秒缓存。
