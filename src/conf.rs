use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    #[serde(default)]
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Conf {
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub active: usize,
}

pub fn conf_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".devview")
        .join("projects.json")
}

pub fn load() -> Conf {
    fs::read_to_string(conf_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(conf: &Conf) -> Result<(), String> {
    let p = conf_path();
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(conf).map_err(|e| e.to_string())?;
    fs::write(&p, s).map_err(|e| e.to_string())
}

pub fn expand(s: &str) -> PathBuf {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(h) = dirs::home_dir() {
            return h.join(rest);
        }
    }
    PathBuf::from(s)
}

/// 把配置里的第 index 个项目解析成规范化绝对路径
pub fn resolve(conf: &Conf, index: usize) -> Result<PathBuf, String> {
    let p = conf.projects.get(index).ok_or_else(|| "无效的项目编号".to_string())?;
    let raw = expand(&p.path);
    let path = dunce::canonicalize(&raw).unwrap_or(raw);
    if !path.is_dir() {
        return Err(format!("目录不存在: {}", p.path));
    }
    Ok(path)
}

pub fn add(conf: &mut Conf, raw: &str) -> Result<usize, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("路径为空".into());
    }
    let path = expand(raw);
    let path = dunce::canonicalize(&path).unwrap_or(path);
    if !path.is_dir() {
        return Err(format!("目录不存在: {}", raw));
    }
    let key = path.to_string_lossy().to_string();
    if let Some(i) = conf.projects.iter().position(|p| p.path == key) {
        return Ok(i);
    }
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| key.clone());
    conf.projects.push(Project { name, path: key });
    Ok(conf.projects.len() - 1)
}
