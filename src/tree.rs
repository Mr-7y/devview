//! 懒加载目录树 + 扁平化索引（便于虚拟滚动）

use crate::fsops;
use std::path::Path;

pub struct Node {
    pub name: String,
    pub path: String,
    pub dir: bool,
    pub open: bool,
    pub kids: Option<Vec<Node>>,
}

#[derive(Clone)]
pub struct Row {
    pub name: String,
    pub path: String,
    pub dir: bool,
    pub open: bool,
    pub depth: usize,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    /// 结构变化计数（展开/折叠/重载），树行缓存以此判断失效
    pub rev: u64,
}

fn build(name: String, path: String, dir: bool) -> Node {
    Node {
        name,
        path,
        dir,
        open: false,
        kids: None,
    }
}

impl Tree {
    pub fn new(root: &Path) -> Tree {
        let mut nodes = Vec::new();
        if let Ok(list) = fsops::list_dir(root, "") {
            for e in list {
                nodes.push(build(e.name, e.path, e.dir));
            }
        }
        Tree { nodes, rev: 1 }
    }

    fn find<'a>(nodes: &'a mut Vec<Node>, path: &str) -> Option<&'a mut Node> {
        for n in nodes.iter_mut() {
            if n.path == path {
                return Some(n);
            }
            if n.dir && path.starts_with(n.path.as_str()) && path.as_bytes().get(n.path.len()) == Some(&b'/') {
                if let Some(k) = n.kids.as_mut() {
                    if let Some(x) = Self::find(k, path) {
                        return Some(x);
                    }
                }
            }
        }
        None
    }

    /// 展开 / 折叠；首次展开时读取磁盘
    pub fn toggle(&mut self, root: &Path, path: &str) {
        if let Some(n) = Self::find(&mut self.nodes, path) {
            if !n.dir {
                return;
            }
            if n.open {
                n.open = false;
                self.rev += 1;
                return;
            }
            if n.kids.is_none() {
                if let Ok(list) = fsops::list_dir(root, &n.path) {
                    n.kids = Some(
                        list.into_iter()
                            .map(|e| build(e.name, e.path, e.dir))
                            .collect(),
                    );
                } else {
                    n.kids = Some(Vec::new());
                }
            }
            n.open = true;
            self.rev += 1;
        }
    }

    /// 重新加载根这一层（保留其余展开状态由调用方决定）
    pub fn reload(&mut self, root: &Path) {
        if let Ok(list) = fsops::list_dir(root, "") {
            let fresh: Vec<Node> = list.into_iter().map(|e| build(e.name, e.path, e.dir)).collect();
            self.nodes = fresh;
            self.rev += 1;
        }
    }

    pub fn flatten(&self) -> Vec<Row> {
        let mut out = Vec::new();
        walk(&self.nodes, 0, &mut out);
        out
    }
}

fn walk(nodes: &[Node], depth: usize, out: &mut Vec<Row>) {
    for n in nodes {
        out.push(Row {
            name: n.name.clone(),
            path: n.path.clone(),
            dir: n.dir,
            open: n.open,
            depth,
        });
        if n.dir && n.open {
            if let Some(k) = &n.kids {
                walk(k, depth + 1, out);
            }
        }
    }
}
