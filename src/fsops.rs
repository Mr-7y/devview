use serde::Serialize;
use std::cmp::Ordering;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const MAX_FILE: u64 = 2 * 1024 * 1024;
pub const MAX_LIST: usize = 20000;

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub path: String,
}

#[derive(Serialize)]
pub struct FilePayload {
    pub path: String,
    pub content: String,
    pub lang: String,
    pub size: u64,
    pub mtime: u64,
    pub truncated: bool,
    pub binary: bool,
}

pub fn skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".svn"
            | ".hg"
            | "node_modules"
            | "bower_components"
            | "vendor"
            | "dist"
            | "build"
            | "out"
            | "target"
            | ".next"
            | ".nuxt"
            | ".turbo"
            | "__pycache__"
            | ".venv"
            | "venv"
            | ".idea"
            | ".vscode"
            | "coverage"
            | ".pytest_cache"
            | ".mypy_cache"
    )
}

/// 拼接并校验路径，禁止越出项目根目录
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.replace('\\', "/");
    let mut full = root.to_path_buf();
    for seg in rel.trim_start_matches('/').split('/') {
        match seg {
            "" | "." => continue,
            ".." => return Err("路径越界".into()),
            s => full.push(s),
        }
    }
    let canon = dunce::canonicalize(&full).map_err(|_| "路径不存在".to_string())?;
    if !canon.starts_with(root) {
        return Err("路径越界".into());
    }
    Ok(canon)
}

fn split_nat(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut last: Option<bool> = None;
    for c in s.chars() {
        let d = c.is_ascii_digit();
        if last.is_some_and(|l| l != d) && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        last = Some(d);
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// 自然排序：file2 排在 file10 前面
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let av = split_nat(&a.to_lowercase());
    let bv = split_nat(&b.to_lowercase());
    for (x, y) in av.iter().zip(bv.iter()) {
        let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(nx), Ok(ny)) => nx.cmp(&ny),
            _ => x.cmp(y),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    av.len().cmp(&bv.len())
}

pub fn list_dir(root: &Path, rel: &str) -> Result<Vec<Entry>, String> {
    let full = safe_join(root, rel)?;
    if !full.is_dir() {
        return Err("目录不存在".into());
    }
    let mut entries = Vec::new();
    let rd = fs::read_dir(&full).map_err(|_| "没有权限读取该目录".to_string())?;
    for item in rd.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        let Ok(ft) = item.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let is_dir = ft.is_dir();
        if is_dir && skip_dir(&name) {
            continue;
        }
        if !is_dir && name == ".DS_Store" {
            continue;
        }
        let path = if rel.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", rel.trim_end_matches('/'), name)
        };
        entries.push(Entry {
            name,
            dir: is_dir,
            path,
        });
    }
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| natural_cmp(&a.name, &b.name)));
    Ok(entries)
}

pub fn detect_lang(path: &str) -> &'static str {
    let base = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    let by_name = match base.as_str() {
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        "rakefile" | "gemfile" => "ruby",
        "nginx.conf" => "nginx",
        ".htaccess" => "apache",
        ".env" | ".npmrc" | ".editorconfig" => "ini",
        _ => "",
    };
    if !by_name.is_empty() {
        return by_name;
    }
    let ext = match base.rfind('.') {
        Some(i) => &base[i..],
        None => "",
    };
    match ext {
        ".php" | ".phtml" => "php",
        ".js" | ".mjs" | ".cjs" | ".jsx" => "javascript",
        ".ts" | ".tsx" => "typescript",
        ".vue" | ".html" | ".htm" | ".xml" | ".svg" => "xml",
        ".py" => "python",
        ".rb" => "ruby",
        ".go" => "go",
        ".rs" => "rust",
        ".java" => "java",
        ".kt" => "kotlin",
        ".c" | ".h" => "c",
        ".cpp" | ".cc" | ".hpp" | ".cxx" => "cpp",
        ".cs" => "csharp",
        ".swift" => "swift",
        ".m" => "objectivec",
        ".css" => "css",
        ".scss" | ".sass" => "scss",
        ".less" => "less",
        ".json" | ".json5" => "json",
        ".yaml" | ".yml" => "yaml",
        ".toml" | ".ini" | ".conf" | ".cfg" => "ini",
        ".sql" => "sql",
        ".sh" | ".bash" | ".zsh" | ".fish" => "bash",
        ".md" | ".markdown" => "markdown",
        ".diff" | ".patch" => "diff",
        ".lua" => "lua",
        ".graphql" | ".gql" => "graphql",
        _ => "plaintext",
    }
}

pub fn read_file(root: &Path, rel: &str) -> Result<FilePayload, String> {
    let full = safe_join(root, rel)?;
    if !full.is_file() {
        return Err("文件不存在".into());
    }
    let meta = fs::metadata(&full).map_err(|e| e.to_string())?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut buf = Vec::with_capacity(size.min(MAX_FILE) as usize + 1);
    let f = fs::File::open(&full).map_err(|_| "无法读取文件".to_string())?;
    f.take(MAX_FILE + 1)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;

    let truncated = buf.len() as u64 > MAX_FILE;
    buf.truncate(MAX_FILE as usize);
    let binary = buf.iter().take(8192).any(|b| *b == 0);

    Ok(FilePayload {
        path: rel.to_string(),
        content: if binary {
            String::new()
        } else {
            String::from_utf8_lossy(&buf).to_string()
        },
        lang: if binary {
            "binary".to_string()
        } else {
            detect_lang(rel).to_string()
        },
        size,
        mtime,
        truncated,
        binary,
    })
}

/// 覆盖写入（仅用于 Markdown 编辑保存）。路径同样过安全校验。
pub fn write_file(root: &Path, rel: &str, content: &str) -> Result<u64, String> {
    let full = safe_join(root, rel)?;
    if !full.is_file() {
        return Err("目标不是文件".into());
    }
    fs::write(&full, content).map_err(|e| format!("保存失败：{}", e))?;
    Ok(content.len() as u64)
}

/// 全量文件名索引（Ctrl+P 用）
pub fn list_all(root: &Path) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for item in rd.flatten() {
            let name = item.file_name().to_string_lossy().to_string();
            let Ok(ft) = item.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let full = item.path();
            if ft.is_dir() {
                if !skip_dir(&name) {
                    stack.push(full);
                }
            } else {
                if name == ".DS_Store" {
                    continue;
                }
                if let Ok(rel) = full.strip_prefix(root) {
                    let p = rel.to_string_lossy().replace('\\', "/");
                    out.push(p);
                    if out.len() >= MAX_LIST {
                        return (out, true);
                    }
                }
            }
        }
    }
    out.sort();
    (out, false)
}
