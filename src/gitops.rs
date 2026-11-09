use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Default)]
pub struct GitInfo {
    pub is_git: bool,
    pub branch: String,
    pub remote: String,
    pub ahead: usize,
    pub behind: usize,
    pub files: HashMap<String, String>,
}

const CACHE_TTL: Duration = Duration::from_millis(3000);

fn cache() -> &'static Mutex<HashMap<String, (Instant, GitInfo)>> {
    static C: OnceLock<Mutex<HashMap<String, (Instant, GitInfo)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 执行 git 命令，带超时；Windows 下不弹黑框。非零退出码返回 None。
fn run(root: &Path, args: &[&str], timeout_ms: u64) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().ok()?;
    let start = Instant::now();
    let mut ok = false;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                ok = st.success();
                break;
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_millis(timeout_ms) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            Err(_) => return None,
        }
    }
    let mut out = Vec::new();
    child.stdout.take()?.read_to_end(&mut out).ok()?;
    if !ok {
        return None;
    }
    Some(String::from_utf8_lossy(&out).to_string())
}

fn map_state(code: &str) -> Option<&'static str> {
    if code == "??" || code == "!!" {
        return Some("U");
    }
    if code.contains('D') {
        return Some("D");
    }
    if code.contains('R') || code.contains('C') {
        return Some("R");
    }
    if code.contains('A') {
        return Some("A");
    }
    if code.contains('M') || code.contains('T') || code.contains('U') {
        return Some("M");
    }
    None
}

pub fn status(root: &Path, force: bool) -> GitInfo {
    let key = root.to_string_lossy().to_string();
    if !force {
        if let Ok(c) = cache().lock() {
            if let Some((t, info)) = c.get(&key) {
                if t.elapsed() < CACHE_TTL {
                    return info.clone();
                }
            }
        }
    }

    let mut info = GitInfo::default();
    if !root.join(".git").exists() {
    
    if let Ok(mut c) = cache().lock() {
            c.insert(key, (Instant::now(), info.clone()));
        }
        return info;
    }

    if let Some(out) = run(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=dirty",
        ],
        8000,
    ) {
        let parts: Vec<&str> = out.split('\0').collect();
        let mut i = 0;
        while i < parts.len() {
            let item = parts[i];
            i += 1;
            if item.len() < 4 {
                continue;
            }
            let code = &item[..2];
            let path = &item[3..];
            if let Some(st) = map_state(code) {
                info.files.insert(path.to_string(), st.to_string());
            }
            if code.starts_with('R') || code.starts_with('C') {
                i += 1; // 跳过重命名前的原路径
            }
        }
        info.is_git = true;
    }

    if let Some(b) = run(root, &["rev-parse", "--abbrev-ref", "HEAD"], 4000) {
        info.branch = b.trim().to_string();
    }

    if let Some(r) = run(root, &["remote"], 3000) {
        info.remote = r.lines().next().unwrap_or("").trim().to_string();
    }

    // 与上游的差距；没有 upstream 时静默为 0
    if let Some(o) = run(
        root,
        &["rev-list", "--count", "--left-right", "@{upstream}...HEAD"],
        4000,
    ) {
        let parts: Vec<&str> = o.split_whitespace().collect();
        if parts.len() == 2 {
            info.behind = parts[0].parse().unwrap_or(0);
            info.ahead = parts[1].parse().unwrap_or(0);
        }
    }

    if let Some(r) = run(root, &["remote"], 3000) {
        info.remote = r.lines().next().unwrap_or("").trim().to_string();
    }

    // 与上游的差距；没有 upstream 时静默为 0
    if let Some(o) = run(
        root,
        &["rev-list", "--count", "--left-right", "@{upstream}...HEAD"],
        4000,
    ) {
        let parts: Vec<&str> = o.split_whitespace().collect();
        if parts.len() == 2 {
            info.behind = parts[0].parse().unwrap_or(0);
            info.ahead = parts[1].parse().unwrap_or(0);
        }
    }


    if let Ok(mut c) = cache().lock() {
        c.insert(key, (Instant::now(), info.clone()));
    }
    info
}

/// 提交已跟踪文件的改动。未跟踪文件不会被自动加入。
pub fn commit(root: &Path, msg: &str) -> Result<String, String> {
    let out = run(root, &["commit", "-am", msg], 15000)
        .ok_or_else(|| "没有可提交的改动，或提交失败".to_string())?;
    let first = out.lines().next().unwrap_or("已提交").trim().to_string();
    Ok(if first.is_empty() {
        "已提交".to_string()
    } else {
        first
    })
}

/// 仅快进拉取，避免产生意外的合并提交
pub fn pull(root: &Path) -> Result<String, String> {
    run(root, &["pull", "--ff-only"], 30000)
        .map(|o| o.lines().next().unwrap_or("已拉取").trim().to_string())
        .ok_or_else(|| "拉取失败或超时".to_string())
}

pub fn push(root: &Path) -> Result<String, String> {
    run(root, &["push"], 30000)
        .map(|o| {
            let t = o.trim();
            if t.is_empty() {
                "已推送".to_string()
            } else {
                t.lines().last().unwrap_or("已推送").to_string()
            }
        })
        .ok_or_else(|| "推送失败或超时".to_string())
}

/// 让状态缓存立刻失效（提交/拉取/推送之后调用）
pub fn invalidate(root: &Path) {
    let key = root.to_string_lossy().to_string();

    if let Ok(mut c) = cache().lock() {
        c.remove(&key);
    }
}

fn new_file_diff(rel: &str, content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();
    let mut s = String::new();
    s.push_str(&format!("diff --git a/{0} b/{0}\n", rel));
    s.push_str("new file mode 100644\n--- /dev/null\n");
    s.push_str(&format!("+++ b/{}\n", rel));
    s.push_str(&format!("@@ -0,0 +1,{} @@\n", n));
    for l in &lines {
        s.push('+');
        s.push_str(l);
        s.push('\n');
    }
    s
}

pub fn diff(root: &Path, rel: &str, full: &Path, state: Option<&str>) -> (String, String) {
    let st = state.unwrap_or("").to_string();

    if state == Some("U") {
        if let Ok(bytes) = std::fs::read(full) {
            if bytes.iter().take(8192).any(|b| *b == 0) {
                return ("（二进制文件，无 diff）".to_string(), st);
            }
            let text = String::from_utf8_lossy(&bytes).to_string();
            return (new_file_diff(rel, &text), st);
        }
        return (String::new(), st);
    }

    let mut text = run(root, &["diff", "--no-color", "--", rel], 8000).unwrap_or_default();
    if text.trim().is_empty() {
        text = run(
            root,
            &["diff", "--no-color", "--cached", "--", rel],
            8000,
        )
        .unwrap_or_default();
    }
    (text, st)
}
