# CODEBUDDY.md

本文件为 CodeBuddy / Claude Code 等 AI 助手在本仓库工作时提供上下文。

## 项目概览

DevView 是一个极简的只读代码浏览器：打开本地目录 → 树形浏览 → 语法高亮 → git 变更标记 → 单文件 diff。

技术形态是**纯 Rust + egui**，没有 WebView、没有 Electron、没有 Python 运行时，release 单文件约 4.6 MB。目标场景是"看代码"而不是编辑代码，因此所有设计都围绕启动快、体积小、内存低展开。

支持 Windows / macOS / Linux，三平台由 GitHub Actions 原生编译。

## 常用命令

```bash
cargo build                 # 调试构建
cargo run                   # 运行（可带目录参数：cargo run -- ~/code/a ~/code/b）
cargo build --release       # 发布构建，产物 target/release/devview(.exe)
cargo fmt --check           # 格式检查
cargo clippy -- -D warnings # 静态检查
```

平台打包脚本：

```bash
./build.sh                  # Linux，等价于 cargo build --release
./pack-macos.sh             # macOS，生成 dist/DevView.app，带 ad-hoc 签名
./build-windows.ps1         # Windows PowerShell 一键打包，打印 exe 体积
```

**本仓库没有任何自动化测试**（无 tests/ 目录、无 #[test]）。改动后的验证方式是 `cargo build --release` 通过 + 手动运行确认对应界面路径，不要臆造测试命令。

CI 只执行构建，不跑 fmt/clippy。代码库没有 rustfmt 配置，且存在超过 100 列的行，全库 `cargo fmt` 会产生大量与本任务无关的 diff；建议只格式化本次改动的文件。

## 模块职责

| 文件 | 职责 |
|---|---|
| `src/main.rs` | 窗口创建、命令行参数（过滤 `-` 开头的参数）、Glow 渲染器、内嵌窗口图标 |
| `src/app.rs` | 全部界面与状态：顶栏 / 侧栏树 / 变更列表 / 代码区 / 状态栏 / 命令面板 / toast / 拖放 |
| `src/theme.rs` | 颜色常量、系统字体探测、egui Visuals 与 Style |
| `src/tree.rs` | 懒加载目录树，`flatten()` 输出带 depth 的行用于渲染 |
| `src/fsops.rs` | 路径安全校验、目录/文件读取、语言识别、自然排序、全量文件名索引 |
| `src/gitops.rs` | 子进程调用 git status/diff，带超时与 3 秒缓存 |
| `src/syntax.rs` | 手写词法高亮，输出 `Vec<Line>`（每行若干 (类型, 文本) span） |
| `src/conf.rs` | `~/.devview/projects.json` 的读写与项目增删 |

数据流是单线程、无消息通道的：`App::update` 依次调用 `statusbar → topbar → sidebar → center → overlays`，各视图直接读写 `App` 字段。没有后台线程、没有 async runtime、没有事件总线，新增功能请沿用这个模型。

## 硬性设计约束

改动前请先确认不违反以下几条，它们决定了这个项目能不能继续"小"：

1. **只读**。`RoBuf` 实现了 `egui::TextBuffer` 且 `is_mutable() == false`，代码区不可编辑。不要为了加编辑功能而放开它——egui 自带的多行编辑撑不住十万行文件。
2. **不引入重量级依赖**。禁止 WebView、HTTP 客户端、异步运行时、图片/字体编解码库。现有依赖只有 eframe/egui、serde、serde_json、dirs、dunce、rfd。
3. **所有 IO 必须有界**。git 是同步子进程 + `try_wait` 轮询超时（status 8s、branch 4s、diff 8s），文件读取有 2 MB 上限，文件名索引遍历有 2 万条上限。新增 IO 必须同样带上限，否则会直接卡住 UI 线程。
4. **路径必须过安全校验**。任何来自 UI 的相对路径都走 `fsops::safe_join`：拒绝 `..` 段，canonicalize 后必须 `starts_with(root)`。
5. **字体不内嵌**。`theme.rs` 在运行时按平台探测系统字体（Windows: msyh/consola；macOS: PingFang/SFNSMono；Linux: NotoSansCJK/DejaVuSansMono）。这是 4.6 MB 体积的前提，不要改成打包字体。
6. **release profile 不要动**。`opt-level = "z"`、`lto`、`codegen-units = 1`、`panic = "abort"`、`strip` 都是为体积服务的。
7. **UI 文案与代码注释统一用简体中文**，与现有风格保持一致。

## 关键阈值与常量

| 位置 | 常量 | 值 | 含义 |
|---|---|---|---|
| `fsops.rs` | `MAX_FILE` | 2 MB | 单文件读入硬上限 |
| `fsops.rs` | `MAX_LIST` | 20000 | 文件名索引上限（变更 tab / Ctrl+P） |
| `app.rs` | `MAX_VIEW` | 256 KB | 实际渲染截断长度（超出提示"已截断"） |
| `app.rs` | `WRAP_LIMIT` | 160 KB | 超过则禁用自动换行，避免排版缓存爆炸 |
| `app.rs` | `ROW_H` | 22.0 | 树 / 变更列表行高 |
| `gitops.rs` | `CACHE_TTL` | 3 s | git status 缓存有效期 |

注意 `MAX_FILE`（2 MB）和 `MAX_VIEW`（256 KB）不是同一个东西：前者是磁盘读取上限，后者是 egui 排版上限。

二进制判定规则：读取内容的前 8192 字节里出现 `0x00` 即视为二进制，不渲染内容。`gitops::diff` 里有一份同样的判定。

## 缓存与失效

- **git 状态**：`gitops::status` 按项目根路径缓存 3 秒；`force = true` 跳过缓存。`App` 在窗口有焦点时每 5 秒刷新一次，F5 强制刷新。
- **galley 缓存**：`GalleyCache` 以 `(rev, 宽度 w)` 命中。任何导致文本、diff 模式、文件、换行开关变化的操作都必须 `self.rev += 1`，否则会显示旧内容。
- **目录树**：懒加载，`kids == None` 表示该目录还没读过磁盘；`reload` 只重读根层。
- **文件名索引**：首次打开「变更」tab 或首次 Ctrl+P 时才全量遍历并缓存（`files_loaded`）。

## 目录与文件过滤

`fsops::skip_dir` 是唯一的目录黑名单来源，包含 `.git`、`node_modules`、`vendor`、`target`、`dist`、`build`、`.venv` 等。目录树和 Ctrl+P 索引共用它。符号链接一律跳过，`.DS_Store` 一律忽略。

## git 状态映射

`gitops::map_state` 把 `git status --porcelain=v1` 的两字符状态归并成五种标记：

- `??` / `!!` → `U`（未跟踪）
- 含 `D` → `D`（删除）
- 含 `R` / `C` → `R`（重命名 / 复制）
- 含 `A` → `A`（新增）
- 含 `M` / `T` / `U` → `M`（修改）

`App::refresh_git` 会额外把所有变更文件的父目录写入 `dirty_dirs`，树里对应目录显示 `•`。

已删除文件不能直接读，走 `App::open_deleted` → `gitops::diff(..., Some("D"))` 展示删除前的 diff。未跟踪文件没有 diff，由 `new_file_diff` 手工拼一个全 `+` 的 diff 文本。

## 语法高亮

`syntax.rs` 是零依赖的词法扫描器，不用 tree-sitter 或语法文件。结构是：`Spec`（行注释、块注释、关键字、类型、`$var` / `@var`、大小写敏感）→ `lex` 产出 span 序列 → `split_lines` 按换行切成 `Line`。

特殊语言走独立函数：`lex_markup`（HTML/XML/SVG）、`lex_css`、`lex_markdown`、`lex_diff`。`tokenize` 是唯一入口，遇到未知语言退化为纯文本。

新增语言需要改两处：`fsops::detect_lang` 的扩展名映射，以及 `syntax::spec_of` 里的 `Spec` 分支（词法型语言）或新增 `lex_*` 函数（结构型语言）。

## 平台特有代码

- **Windows**：`windows_subsystem = "windows"` 只在非 debug 生效，避免 release 弹控制台；`gitops::run` 用 `CREATE_NO_WINDOW` 防止 git 子进程闪黑框；rfd 启用 `common-controls-v6` 才有现代文件对话框。
- **macOS**：`pack-macos.sh` 生成 `.app` 并做 ad-hoc 签名；`Info.plist` 有两份（`packaging/Info.plist` 和脚本内 heredoc），改元信息时两处都要动。
- **Linux**：依赖系统 `libxkbcommon`、`libgl`、`libgtk-3`（rfd 需要）；无中文字体时中文显示方块。

## 版本号同步点

发版时以下位置的版本号必须一致：`Cargo.toml`、`Cargo.lock`（构建时自动更新）、`pack-macos.sh` 里的 `CFBundleVersion` / `CFBundleShortVersionString`、`packaging/Info.plist`、`README.md` 的下载表格。

## 发布流程

打 `v*` tag 触发 `.github/workflows/build.yml`：Windows / Linux / macOS 三个 job 各自原生编译并上传 artifact，`release` job 汇总后创建 GitHub Release。所有构建都带 `--locked`，因此 `Cargo.lock` 必须提交且与 `Cargo.toml` 一致。

## 提交规范

Conventional Commits，描述用中文：`feat: 增加变更视图分组`、`fix: 修复删除文件 diff 为空`、`docs: 补充下载入口`。一次提交聚焦一件事，不要顺手改无关的样式或常量。

## 已知限制（不要当 bug 修）

- 单文件渲染上限 256 KB，超出截断并提示，这是排版缓存的成本取舍。
- 大仓库（> 10 万文件）首次 `git status` 会有 1~2 秒延迟，之后走缓存。
- 只读，不支持编辑与保存。
- 三平台字形略有差异，属系统字体方案的固有代价。
