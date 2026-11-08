use crate::{conf, fsops, gitops, syntax, theme, tree};
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_VIEW: u64 = 256 * 1024;
const WRAP_LIMIT: usize = 160 * 1024;
const ROW_H: f32 = 22.0;

/* ---------------------------------------------------------------- 只读缓冲 */

struct RoBuf<'a>(&'a str);

impl egui::TextBuffer for RoBuf<'_> {
    fn is_mutable(&self) -> bool {
        false
    }
    fn as_str(&self) -> &str {
        self.0
    }
    fn insert_text(&mut self, _text: &str, _char_index: usize) -> usize {
        0
    }
    fn delete_char_range(&mut self, _char_range: std::ops::Range<usize>) {}
}

/* ---------------------------------------------------------------- 当前文件 */

pub struct Cur {
    pub path: String,
    pub name: String,
    text: String,
    diff_text: String,
    lines: Vec<syntax::Line>,
    diff_lines: Vec<syntax::Line>,
    gutter: String,
    size: u64,
    binary: bool,
    truncated: bool,
    deleted: bool,
    digits: usize,
    can_wrap: bool,
}

impl Cur {
    fn view(&self, diff: bool) -> (&str, &[syntax::Line]) {
        if diff {
            (&self.diff_text, &self.diff_lines)
        } else {
            (&self.text, &self.lines)
        }
    }
}

struct GalleyCache {
    rev: u64,
    w: f32,
    galley: Arc<egui::Galley>,
}

/* ---------------------------------------------------------------- App */

pub struct App {
    conf: conf::Conf,
    root: Option<PathBuf>,
    root_err: Option<String>,
    tree: tree::Tree,
    git: gitops::GitInfo,
    dirty_dirs: HashSet<String>,
    git_at: Instant,

    tab: usize,
    cur: Option<Cur>,
    cache: RefCell<Option<GalleyCache>>,
    rev: u64,
    wrap: bool,
    diff_mode: bool,
    scroll_y: f32,

    files: Vec<String>,
    files_loaded: bool,

    palette: bool,
    pal_q: String,
    pal_sel: usize,

    confirm_del: bool,
    toast: String,
    toast_at: Instant,
    dragging: bool,
    title: String,
    title_dirty: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Vec<String>) -> Self {
        let mut conf = conf::load();
        let mut last: Option<usize> = None;
        for a in &args {
            if let Ok(i) = conf::add(&mut conf, a) {
                last = Some(i);
            }
        }
        if let Some(i) = last {
            conf.active = i;
            let _ = conf::save(&conf);
        }
        let _ = cc;
        let mut app = App {
            conf,
            root: None,
            root_err: None,
            tree: tree::Tree { nodes: Vec::new() },
            git: Default::default(),
            dirty_dirs: HashSet::new(),
            git_at: Instant::now() - Duration::from_secs(60),
            tab: 0,
            cur: None,
            cache: RefCell::new(None),
            rev: 1,
            wrap: false,
            diff_mode: false,
            scroll_y: 0.0,
            files: Vec::new(),
            files_loaded: false,
            palette: false,
            pal_q: String::new(),
            pal_sel: 0,
            confirm_del: false,
            toast: String::new(),
            toast_at: Instant::now(),
            dragging: false,
            title: "DevView".into(),
            title_dirty: false,
        };
        app.load_project();
        app
    }

    /* ------------------------------------------------ 项目 */

    fn load_project(&mut self) {
        self.cur = None;
        self.diff_mode = false;
        self.scroll_y = 0.0;
        self.rev += 1;
        self.files.clear();
        self.files_loaded = false;
        self.root_err = None;

        match conf::resolve(&self.conf, self.conf.active) {
            Ok(p) => {
                self.tree = tree::Tree::new(&p);
                self.root = Some(p);
                self.refresh_git(true);
            }
            Err(e) => {
                self.tree = tree::Tree { nodes: Vec::new() };
                self.root = None;
                self.root_err = Some(e);
                self.git = Default::default();
                self.dirty_dirs.clear();
            }
        }
    }

    fn refresh_git(&mut self, force: bool) {
        let Some(root) = &self.root else { return };
        self.git = gitops::status(root, force);
        self.git_at = Instant::now();
        self.dirty_dirs.clear();
        for f in self.git.files.keys() {
            let mut p = f.as_str();
            while let Some(i) = p.rfind('/') {
                p = &p[..i];
                if !self.dirty_dirs.insert(p.to_string()) {
                    break;
                }
            }
        }
    }

    fn add_project(&mut self) {
        let Some(dir) = rfd::FileDialog::new()
            .set_title("选择代码目录")
            .pick_folder()
        else {
            return;
        };
        let s = dir.to_string_lossy().to_string();
        match conf::add(&mut self.conf, &s) {
            Ok(i) => {
                self.conf.active = i;
                let _ = conf::save(&self.conf);
                self.load_project();
            }
            Err(e) => self.toast(e),
        }
    }

    fn remove_project(&mut self) {
        if self.conf.projects.is_empty() {
            return;
        }
        self.conf.projects.remove(self.conf.active);
        if self.conf.active >= self.conf.projects.len() {
            self.conf.active = self.conf.projects.len().saturating_sub(1);
        }
        let _ = conf::save(&self.conf);
        self.load_project();
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = msg.into();
        self.toast_at = Instant::now();
    }

    /* ------------------------------------------------ 打开文件 */

    fn open(&mut self, rel: &str) {
        let Some(root) = self.root.clone() else { return };
        if self.cur.as_ref().map(|c| c.path == rel && !c.deleted).unwrap_or(false) {
            self.diff_mode = false;
            return;
        }
        let state = self.git.files.get(rel).cloned();
        if state.as_deref() == Some("D") {
            self.open_deleted(&root, rel);
            return;
        }
        match fsops::read_file(&root, rel) {
            Ok(f) => {
                let mut text = f.content;
                let mut truncated = f.truncated;
                if text.len() as u64 > MAX_VIEW {
                    let mut cut = MAX_VIEW as usize;
                    while cut > 0 && !text.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    text.truncate(cut);
                    truncated = true;
                }
                let lines = syntax::tokenize(&text, &f.lang);
                let n = lines.len().max(1);
                let can_wrap = text.len() < WRAP_LIMIT;
                let gutter = (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
                if !can_wrap {
                    self.wrap = false;
                }
                self.cur = Some(Cur {
                    path: rel.to_string(),
                    name: rel.rsplit('/').next().unwrap_or(rel).to_string(),
                    text,
                    diff_text: String::new(),
                    lines,
                    diff_lines: Vec::new(),
                    gutter,
                    size: f.size,
                    binary: f.binary,
                    truncated,
                    deleted: false,
                    digits: n.to_string().len(),
                    can_wrap,
                });
                self.diff_mode = false;
                self.scroll_y = 0.0;
                self.rev += 1;
                self.title = format!("{} — DevView", rel.rsplit('/').next().unwrap_or(rel));
                self.title_dirty = true;
            }
            Err(e) => self.toast(e),
        }
    }

    fn open_deleted(&mut self, root: &Path, rel: &str) {
        let full = root.join(rel);
        let (d, _) = gitops::diff(root, rel, &full, Some("D"));
        let d = if d.trim().is_empty() {
            "（该文件已删除，且没有可显示的 diff）".to_string()
        } else {
            d
        };
        let lines = syntax::tokenize(&d, "diff");
        let n = lines.len().max(1);
        let gutter = (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        self.cur = Some(Cur {
            path: rel.to_string(),
            name: rel.rsplit('/').next().unwrap_or(rel).to_string(),
            text: String::new(),
            diff_text: d,
            lines: Vec::new(),
            diff_lines: lines,
            gutter,
            size: 0,
            binary: false,
            truncated: false,
            deleted: true,
            digits: n.to_string().len(),
            can_wrap: true,
        });
        self.diff_mode = true;
        self.scroll_y = 0.0;
        self.rev += 1;
        self.title = format!("{} (已删除) — DevView", rel.rsplit('/').next().unwrap_or(rel));
        self.title_dirty = true;
    }

    fn toggle_diff(&mut self) {
        let Some(cur) = self.cur.as_mut() else { return };
        if cur.deleted || cur.binary {
            return;
        }
        if self.diff_mode {
            self.diff_mode = false;
        } else {
            let Some(root) = self.root.clone() else { return };
            let state = self.git.files.get(&cur.path).cloned();
            let full = root.join(&cur.path);
            let (d, _) = gitops::diff(&root, &cur.path, &full, state.as_deref());
            if d.trim().is_empty() {
                self.toast("该文件没有未提交的改动");
                return;
            }
            cur.diff_text = d;
            cur.diff_lines = syntax::tokenize(&cur.diff_text, "diff");
            self.diff_mode = true;
        }
        self.scroll_y = 0.0;
        self.rev += 1;
    }

    /* ------------------------------------------------ 文件索引 */

    fn ensure_files(&mut self) {
        if self.files_loaded {
            return;
        }
        self.files_loaded = true;
        if let Some(root) = &self.root {
            let (list, _) = fsops::list_all(root);
            self.files = list;
        }
    }

    /* ------------------------------------------------ 顶部栏 */

    fn topbar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none()
            .fill(theme::TOP)
            .inner_margin(egui::Margin {
                left: 10.0,
                right: 8.0,
                top: 6.0,
                bottom: 6.0,
            });
        egui::TopBottomPanel::top("top")
            .exact_height(36.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    logo(ui);
                    ui.label(RichText::new("DevView").size(12.5).color(theme::FG).strong());

                    ui.add_space(6.0);

                    let cur_name = self
                        .conf
                        .projects
                        .get(self.conf.active)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "未打开项目".into());

                    let mut pick: Option<usize> = None;
                    egui::ComboBox::from_id_salt("proj")
                        .width(150.0)
                        .selected_text(RichText::new(cur_name).size(12.0))
                        .show_ui(ui, |ui| {
                            for (i, p) in self.conf.projects.iter().enumerate() {
                                if ui
                                    .selectable_label(i == self.conf.active, p.name.as_str())
                                    .clicked()
                                {
                                    pick = Some(i);
                                }
                            }
                            if self.conf.projects.is_empty() {
                                ui.label(RichText::new("还没有项目").size(12.0).color(theme::DIM));
                            }
                        });
                    if let Some(i) = pick {
                        if i != self.conf.active {
                            self.conf.active = i;
                            let _ = conf::save(&self.conf);
                            self.load_project();
                        }
                    }

                    if ui.button(RichText::new("＋").size(13.0)).on_hover_text("添加项目目录").clicked() {
                        self.add_project();
                    }
                    if !self.conf.projects.is_empty()
                        && ui
                            .button(RichText::new("−").size(13.0))
                            .on_hover_text("从列表移除（不删磁盘文件）")
                            .clicked()
                    {
                        self.confirm_del = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        let has_file = self
                            .cur
                            .as_ref()
                            .map(|c| !c.binary && !c.deleted)
                            .unwrap_or(false);
                        let changed = self
                            .cur
                            .as_ref()
                            .map(|c| self.git.files.contains_key(&c.path))
                            .unwrap_or(false);

                        if ui
                            .add_enabled(
                                has_file && changed,
                                egui::Button::new(RichText::new("Diff").size(12.0))
                                    .min_size(Vec2::new(46.0, 22.0))
                                    .selected(self.diff_mode),
                            )
                            .on_hover_text("查看该文件的未提交改动")
                            .clicked()
                        {
                            self.toggle_diff();
                        }
                        let can_wrap = self.cur.as_ref().map(|c| c.can_wrap).unwrap_or(true);
                        let w = ui
                            .add_enabled(
                                can_wrap,
                                egui::Button::new(RichText::new("折行").size(11.5))
                                    .min_size(Vec2::new(40.0, 22.0))
                                    .selected(self.wrap),
                            )
                            .on_hover_text(if can_wrap {
                                "长行自动换行"
                            } else {
                                "文件过大，已禁用折行"
                            });
                        if w.clicked() {
                            self.wrap = !self.wrap;
                            self.rev += 1;
                        }
                        if !self.conf.projects.is_empty() {
                            if let Some(p) = self.conf.projects.get(self.conf.active) {
                                ui.label(
                                    RichText::new(short_path(&p.path))
                                        .size(10.5)
                                        .color(theme::FAINT),
                                );
                            }
                        }
                    });
                });
            });
    }

    /* ------------------------------------------------ 侧栏 */

    fn sidebar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none().fill(theme::SIDE);
        egui::SidePanel::left("side")
            .default_width(238.0)
            .width_range(160.0..=520.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    ui.add_space(5.0);
                    let n = self.git.files.len();
                    if self.tab_btn(ui, "文件", self.tab == 0) {
                        self.tab = 0;
                    }
                    let label = if n > 0 {
                        format!("变更  {}", n)
                    } else {
                        "变更".to_string()
                    };
                    if self.tab_btn(ui, &label, self.tab == 1) {
                        self.tab = 1;
                        self.ensure_files();
                    }
                });
                ui.add_space(4.0);
                ui.painter().hline(
                    ui.max_rect().x_range(),
                    ui.cursor().top(),
                    Stroke::new(1.0, theme::LINE),
                );
                ui.add_space(3.0);

                if self.tab == 0 {
                    self.draw_tree(ui);
                } else {
                    self.draw_changes(ui);
                }
            });
    }

    fn tab_btn(&self, ui: &mut egui::Ui, text: &str, on: bool) -> bool {
        let color = if on { theme::FG } else { theme::DIM };
        let btn = egui::Button::new(RichText::new(text).size(11.5).color(color))
            .frame(false)
            .min_size(Vec2::new(0.0, 22.0));
        let r = ui.add(btn);
        if on {
            let rect = r.rect;
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 2.0, rect.bottom() + 1.0),
                    egui::pos2(rect.right() - 2.0, rect.bottom() + 2.0),
                ),
                0.0,
                theme::ACCENT,
            );
        }
        r.clicked()
    }

    fn draw_tree(&mut self, ui: &mut egui::Ui) {
        if self.conf.projects.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("还没有打开项目").size(12.5).color(theme::DIM));
                ui.add_space(6.0);
                if ui.button("选择目录…").clicked() {
                    self.add_project();
                }
            });
            return;
        }
        if let Some(e) = &self.root_err {
            let e = e.clone();
            ui.add_space(10.0);
            ui.label(RichText::new(e).size(11.5).color(theme::D));
            return;
        }

        let rows = self.tree.flatten();
        let h = ui.available_height();
        let mut clicked: Option<String> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(h)
            .show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for i in range {
                    let row = &rows[i];
                    let full_w = ui.available_width();
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, ROW_H), Sense::click());
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let selected = self
                        .cur
                        .as_ref()
                        .map(|c| c.path == row.path)
                        .unwrap_or(false);
                    let p = ui.painter();
                    if selected {
                        p.rect_filled(rect, 0.0, theme::SEL);
                    } else if resp.hovered() {
                        p.rect_filled(rect, 0.0, Color32::from_rgb(0x2f, 0x31, 0x33));
                    }

                    let x = rect.left() + 8.0 + row.depth as f32 * 12.0;
                    let mid = rect.center().y;

                    if row.dir {
                        let c = egui::pos2(x + 5.0, mid);
                        let pts = if row.open {
                            vec![
                                egui::pos2(c.x - 4.0, c.y - 2.0),
                                egui::pos2(c.x + 4.0, c.y - 2.0),
                                egui::pos2(c.x, c.y + 3.5),
                            ]
                        } else {
                            vec![
                                egui::pos2(c.x - 2.0, c.y - 4.0),
                                egui::pos2(c.x + 3.5, c.y),
                                egui::pos2(c.x - 2.0, c.y + 4.0),
                            ]
                        };
                        p.add(egui::Shape::convex_polygon(pts, theme::DIM, Stroke::NONE));
                    }

                    let nx = x + 13.0;
                    let name_color = if row.dir { theme::FG } else { theme::FG };
                    p.text(
                        egui::pos2(nx, mid),
                        Align2::LEFT_CENTER,
                        &row.name,
                        FontId::proportional(12.5),
                        name_color,
                    );

                    // git 标记
                    let mark = if row.dir {
                        if self.dirty_dirs.contains(&row.path) {
                            Some(("•".to_string(), theme::M))
                        } else {
                            None
                        }
                    } else {
                        self.git.files.get(&row.path).map(|s| (s.clone(), theme::git_color(s)))
                    };
                    if let Some((m, c)) = mark {
                        p.text(
                            egui::pos2(rect.right() - 10.0, mid),
                            Align2::RIGHT_CENTER,
                            m,
                            FontId::proportional(10.5),
                            c,
                        );
                    }

                    if resp.clicked() {
                        if row.dir {
                            clicked = Some(row.path.clone());
                        } else if !row.path.is_empty() {
                            self.open(&row.path);
                        }
                    }
                }
            });

        if let Some(path) = clicked {
            if let Some(root) = self.root.clone() {
                self.tree.toggle(&root, &path);
            }
        }
    }

    fn draw_changes(&mut self, ui: &mut egui::Ui) {
        if self.root.is_none() {
            ui.add_space(14.0);
            ui.label(RichText::new("没有可用项目").size(12.0).color(theme::FAINT));
            return;
        }
        if !self.git.is_git {
            ui.add_space(14.0);
            ui.label(RichText::new("不是 git 仓库").size(12.0).color(theme::FAINT));
            return;
        }
        let files = &self.git.files;
        if files.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("工作区干净，没有变更").size(12.0).color(theme::FAINT));
            return;
        }

        let groups: [(&str, &[&str]); 3] = [
            ("已修改", &["M", "A", "R"]),
            ("已删除", &["D"]),
            ("未跟踪", &["U"]),
        ];
        let mut items: Vec<(String, String, usize)> = Vec::new(); // path, state, group
        for (gi, (_, states)) in groups.iter().enumerate() {
            let mut list: Vec<(&String, &String)> = files
                .iter()
                .filter(|(_, v)| states.contains(&v.as_str()))
                .collect();
            list.sort_by(|a, b| fsops::natural_cmp(a.0, b.0));
            for (k, v) in list {
                items.push((k.clone(), v.clone(), gi));
            }
        }

        let mut clicked: Option<String> = None;
        let mut last_group = usize::MAX;
        let h = ui.available_height();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(h)
            .show(ui, |ui| {
                for (path, state, gi) in &items {
                    if *gi != last_group {
                        last_group = *gi;
                        let n = items.iter().filter(|x| x.2 == *gi).count();
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!("{}  {}", groups[*gi].0, n))
                                .size(10.0)
                                .color(Color32::from_rgb(0x84, 0x84, 0x84)),
                        );
                        ui.add_space(2.0);
                    }
                    let full_w = ui.available_width();
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, ROW_H), Sense::click());
                    let selected = self.cur.as_ref().map(|c| c.path == *path).unwrap_or(false);
                    let p = ui.painter();
                    if selected {
                        p.rect_filled(rect, 0.0, theme::SEL);
                    } else if resp.hovered() {
                        p.rect_filled(rect, 0.0, theme::HOVER);
                    }
                    p.text(
                        egui::pos2(rect.left() + 9.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        state,
                        FontId::monospace(10.5),
                        theme::git_color(state),
                    );
                    let i = path.rfind('/');
                    let (dir, name) = match i {
                        Some(k) => (&path[..k + 1], &path[k + 1..]),
                        None => ("", path.as_str()),
                    };
                    let dim = egui::text::LayoutJob {
                        text: format!("{}{}", dir, name),
                        ..Default::default()
                    };
                    let _ = dim;
                    p.text(
                        egui::pos2(rect.left() + 25.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        dir,
                        FontId::proportional(11.0),
                        theme::FAINT,
                    );
                    let dx = if dir.is_empty() {
                        0.0
                    } else {
                        p.layout_no_wrap(
                            dir.to_string(),
                            FontId::proportional(11.0),
                            theme::FAINT,
                        )
                        .size()
                        .x
                    };
                    p.text(
                        egui::pos2(rect.left() + 25.0 + dx, rect.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(12.5),
                        theme::FG,
                    );
                    if resp.clicked() {
                        clicked = Some(path.clone());
                    }
                }
                ui.add_space(30.0);
            });

        if let Some(p) = clicked {
            self.open(&p);
        }
    }

    /* ------------------------------------------------ 中央：标签 + 代码 */

    fn center(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none().fill(theme::BG);
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if self.conf.projects.is_empty() {
                self.welcome(ui, "还没有打开项目", "点左上角的 ＋ 选择代码目录，或直接把文件夹拖进窗口。");
                return;
            }
            if self.cur.is_none() {
                self.welcome(ui, "DevView", "从左侧选择一个文件开始阅读。");
                return;
            }

            self.crumb(ui);
            ui.painter().hline(
                ui.max_rect().x_range(),
                ui.cursor().top(),
                Stroke::new(1.0, theme::LINE),
            );
            ui.add_space(1.0);
            self.code(ui);
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui, title: &str, sub: &str) {
        let h = ui.available_height();
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), h),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.add_space(h * 0.33);
                let (r, _) = ui.allocate_exact_size(Vec2::new(46.0, 46.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(r, 11.0, theme::ACCENT);
                let c = r.center();
                let w = Stroke::new(3.4, Color32::from_rgb(0xf2, 0xf8, 0xff));
                p.line_segment([egui::pos2(c.x - 2.5, c.y - 8.5), egui::pos2(c.x - 9.5, c.y)], w);
                p.line_segment([egui::pos2(c.x - 9.5, c.y), egui::pos2(c.x - 2.5, c.y + 8.5)], w);
                p.line_segment([egui::pos2(c.x + 2.5, c.y - 8.5), egui::pos2(c.x + 9.5, c.y)], w);
                p.line_segment([egui::pos2(c.x + 9.5, c.y), egui::pos2(c.x + 2.5, c.y + 8.5)], w);
                ui.add_space(10.0);
                ui.label(RichText::new(title).size(17.0).color(theme::FG));
                ui.add_space(6.0);
                ui.label(RichText::new(sub).size(12.0).color(theme::DIM));
            },
        );
    }

    fn crumb(&mut self, ui: &mut egui::Ui) {
        let Some(cur) = self.cur.as_ref() else { return };
        let (dir, name) = match cur.path.rfind('/') {
            Some(i) => (&cur.path[..i + 1], cur.name.as_str()),
            None => ("", cur.name.as_str()),
        };
        let size = cur.size;
        let truncated = cur.truncated;
        let binary = cur.binary;
        let deleted = cur.deleted;
        let diff = self.diff_mode;

        ui.horizontal(|ui| {
            ui.set_height(26.0);
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            if !dir.is_empty() {
                ui.label(RichText::new(dir).size(11.0).color(theme::FAINT));
            }
            ui.label(RichText::new(name).size(11.5).color(theme::FG));
            ui.add_space(10.0);

            if binary {
                badge(ui, "二进制", theme::M);
            } else if deleted {
                badge(ui, "已删除", theme::D);
            } else if diff {
                badge(ui, "改动视图", theme::ACCENT);
            } else if truncated {
                badge(ui, "已截断", theme::M);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if size > 0 {
                    ui.label(
                        RichText::new(fmt_size(size))
                            .size(10.5)
                            .color(theme::FAINT),
                    );
                }
            });
        });
    }

    fn code(&mut self, ui: &mut egui::Ui) {
        let Some(cur) = self.cur.as_ref() else { return };
        if cur.binary {
            let msg = format!("二进制文件 · {}", fmt_size(cur.size));
            let h = ui.available_height();
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), h),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_space(h * 0.36);
                    ui.label(RichText::new(msg).size(13.0).color(theme::DIM));
                },
            );
            return;
        }

        let diff = self.diff_mode;
        let (text, lines) = cur.view(diff);
        let wrap = self.wrap;
        let rev = self.rev;
        let scroll_y = self.scroll_y;
        let gutter_text = &cur.gutter;
        let digits = cur.digits;

        let mono = syntax::font();
        let digit_w = ui.fonts(|f| f.glyph_width(&mono, '0'));
        let gutter_w = (digits as f32 * digit_w + 22.0).max(46.0);
        let avail_h = ui.available_height();
        let avail_w = ui.available_width();

        let cache = &self.cache;
        let mut new_scroll = scroll_y;

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // --- 行号槽（垂直跟随，不参与横向滚动）
            let (grect, _gresp) = ui.allocate_exact_size(
                Vec2::new(gutter_w, avail_h),
                Sense::hover(),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(grect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(grect);
            egui::ScrollArea::vertical()
                .id_salt(("gutter", rev))
                .scroll_offset(Vec2::new(0.0, scroll_y))
                .enable_scrolling(false)
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap = egui::text::TextWrapping::no_max_width();
                    job.append(
                        gutter_text,
                        0.0,
                        egui::TextFormat {
                            font_id: mono.clone(),
                            color: Color32::from_rgb(0x85, 0x85, 0x85),
                            ..Default::default()
                        },
                    );
                    let g = ui.fonts(|f| f.layout_job(job));
                    let (r, _) = ui.allocate_exact_size(
                        Vec2::new(gutter_w, g.size().y + 12.0),
                        Sense::hover(),
                    );
                    ui.painter().galley(
                        egui::pos2(r.right() - 10.0 - g.size().x, r.top() + 8.0),
                        g,
                        Color32::from_rgb(0x85, 0x85, 0x85),
                    );
                });

            // 分隔线
            ui.painter().vline(
                grect.right(),
                grect.y_range(),
                Stroke::new(1.0, Color32::from_rgb(0x35, 0x35, 0x37)),
            );

            // --- 代码
            let mut layouter = move |ui: &egui::Ui, _buf: &str, wrap_width: f32| {
                let w = if wrap { wrap_width } else { 0.0 };
                if let Some(c) = cache.borrow().as_ref() {
                    if c.rev == rev && (c.w - w).abs() < 0.5 {
                        return c.galley.clone();
                    }
                }
                let job = syntax::build_job(lines, wrap, w);
                let g = ui.fonts(|f| f.layout_job(job));
                *cache.borrow_mut() = Some(GalleyCache {
                    rev,
                    w,
                    galley: g.clone(),
                });
                g
            };

            let mut ro = RoBuf(text);
            let desired = if wrap { avail_w - gutter_w - 16.0 } else { f32::INFINITY };
            let out = egui::ScrollArea::both()
                .id_salt(("code", rev))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    egui::TextEdit::multiline(&mut ro)
                        .font(mono.clone())
                        .code_editor()
                        .desired_width(desired)
                        .desired_rows(1)
                        .frame(false)
                        .margin(egui::Margin {
                            left: 10.0,
                            right: 10.0,
                            top: 6.0,
                            bottom: 6.0,
                        })
                        .layouter(&mut layouter)
                        .show(ui);
                });
            new_scroll = out.state.offset.y;
        });

        self.scroll_y = new_scroll;
    }

    /* ------------------------------------------------ 状态栏 */

    fn statusbar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none()
            .fill(theme::TOP)
            .inner_margin(egui::Margin {
                left: 10.0,
                right: 10.0,
                top: 3.0,
                bottom: 3.0,
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let p = self.conf.projects.get(self.conf.active);
                    let name = p.map(|x| x.name.clone()).unwrap_or_default();
                    let path = p.map(|x| short_path(&x.path)).unwrap_or_default();

                    let (dot, dot_c) = if self.git.is_git {
                        ("●", Color32::from_rgb(0x4e, 0xc9, 0xb0))
                    } else {
                        ("○", theme::FAINT)
                    };
                    ui.label(RichText::new(dot).size(9.0).color(dot_c));
                    ui.label(RichText::new(name).size(11.0).color(theme::FG));
                    ui.label(RichText::new(path).size(10.5).color(theme::FAINT));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.git.is_git {
                            ui.label(
                                RichText::new(format!("分支 {}", if self.git.branch.is_empty() { "-" } else { &self.git.branch }))
                                    .size(10.5)
                                    .color(theme::DIM),
                            );
                            let mut m = 0;
                            let mut u = 0;
                            for v in self.git.files.values() {
                                if v == "U" {
                                    u += 1
                                } else {
                                    m += 1
                                }
                            }
                            ui.label(RichText::new(format!("U {}", u)).size(10.5).color(theme::U));
                            ui.label(RichText::new(format!("M {}", m)).size(10.5).color(theme::M));
                        }
                    });
                });
            });
    }

    /* ------------------------------------------------ 浮层 */

    fn overlays(&mut self, ctx: &egui::Context) {
        // 提示条
        if !self.toast.is_empty() && self.toast_at.elapsed() < Duration::from_secs(3) {
            egui::Area::new("toast".into())
                .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -46.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(0x3a, 0x2d, 0x1a))
                        .rounding(6.0)
                        .inner_margin(egui::Margin {
                            left: 14.0,
                            right: 14.0,
                            top: 7.0,
                            bottom: 7.0,
                        })
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(&self.toast)
                                    .size(12.0)
                                    .color(Color32::from_rgb(0xe2, 0xc0, 0x8d)),
                            );
                        });
                });
        }

        // 确认移除
        if self.confirm_del {
            let name = self
                .conf
                .projects
                .get(self.conf.active)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let resp = egui::Window::new("移除项目")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.label(format!("从列表移除「{}」？", name));
                    ui.label(
                        RichText::new("只移除记录，不会删除磁盘上的文件。")
                            .size(11.0)
                            .color(theme::DIM),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("移除").clicked() {
                            self.remove_project();
                            self.confirm_del = false;
                        }
                        if ui.button("取消").clicked() {
                            self.confirm_del = false;
                        }
                    });
                });
            if resp.is_none() {
                self.confirm_del = false;
            }
        }

        if self.palette {
            self.palette_ui(ctx);
        }
    }

    fn palette_ui(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("pal-mask"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let r = ctx.screen_rect();
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(118));
                let resp = ui.allocate_rect(r, Sense::click());
                if resp.clicked() {
                    self.palette = false;
                }
            });

        let mut close = false;
        let mut chosen: Option<String> = None;
        let results = fuzzy_files(&self.files, &self.pal_q, 200);

        let screen = ctx.screen_rect();
        let w = 620.0_f32.min(screen.width() - 60.0);
        let x = screen.center().x - w / 2.0;
        let y = screen.top() + 90.0;

        egui::Area::new("palette".into())
            .fixed_pos(egui::pos2(x, y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(Color32::from_rgb(0x25, 0x25, 0x26))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(w);
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.pal_q)
                                .hint_text("输入文件名，回车打开")
                                .desired_width(f32::INFINITY)
                                .margin(egui::Margin::symmetric(6.0, 5.0)),
                        );
                        r.request_focus();

                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            close = true;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                            self.pal_sel = (self.pal_sel + 1).min(results.len().saturating_sub(1));
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                            self.pal_sel = self.pal_sel.saturating_sub(1);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some((p, _)) = results.get(self.pal_sel) {
                                chosen = Some(p.clone());
                            }
                        }

                        ui.add_space(6.0);
                        let max_h = (screen.height() * 0.5).max(200.0);
                        egui::ScrollArea::vertical()
                            .max_height(max_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if results.is_empty() {
                                    ui.label(
                                        RichText::new("没有匹配的文件")
                                            .size(12.0)
                                            .color(theme::FAINT),
                                    );
                                }
                                for (i, (path, _)) in results.iter().enumerate() {
                                    let sel = i == self.pal_sel;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), 22.0),
                                        Sense::click(),
                                    );
                                    let p = ui.painter();
                                    if sel {
                                        p.rect_filled(rect, 3.0, theme::SEL);
                                    } else if resp.hovered() {
                                        p.rect_filled(rect, 3.0, theme::HOVER);
                                    }
                                    let (dir, name) = match path.rfind('/') {
                                        Some(k) => (&path[..k + 1], &path[k + 1..]),
                                        None => ("", path.as_str()),
                                    };
                                    p.text(
                                        egui::pos2(rect.left() + 6.0, rect.center().y),
                                        Align2::LEFT_CENTER,
                                        name,
                                        FontId::proportional(12.5),
                                        if sel { Color32::WHITE } else { theme::FG },
                                    );
                                    let nw = p
                                        .layout_no_wrap(
                                            name.to_string(),
                                            FontId::proportional(12.5),
                                            theme::FG,
                                        )
                                        .size()
                                        .x;
                                    p.text(
                                        egui::pos2(rect.left() + 12.0 + nw, rect.center().y),
                                        Align2::LEFT_CENTER,
                                        dir,
                                        FontId::proportional(10.5),
                                        theme::FAINT,
                                    );
                                    if resp.clicked() {
                                        chosen = Some(path.clone());
                                    }
                                }
                            });
                    });
            });

        if let Some(p) = chosen {
            self.open(&p);
            close = true;
        }
        if close {
            self.palette = false;
            self.pal_q.clear();
            self.pal_sel = 0;
        }
    }

    /* ------------------------------------------------ 输入 */

    fn input(&mut self, ctx: &egui::Context) {
        let cmd = ctx.input(|i| i.modifiers.command);
        if cmd && ctx.input(|i| i.key_pressed(egui::Key::P)) && !self.palette {
            if !self.conf.projects.is_empty() {
                self.ensure_files();
                self.palette = true;
                self.pal_q.clear();
                self.pal_sel = 0;
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.refresh_git(true);
            if let Some(root) = self.root.clone() {
                self.tree.reload(&root);
            }
        }
        if !self.palette && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.confirm_del = false;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.input(ctx);
        if self.title_dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title.clone()));
            self.title_dirty = false;
        }

        let toast_left = Duration::from_secs(3).saturating_sub(self.toast_at.elapsed());
        if !toast_left.is_zero() {
            // 只在提示到期那一刻醒来一次
            ctx.request_repaint_after(toast_left);
        }
        if ctx.input(|i| i.focused) && self.git_at.elapsed() > Duration::from_secs(5) {
            self.refresh_git(false);
        }
        if self.git.is_git {
            let left = Duration::from_secs(5).saturating_sub(self.git_at.elapsed());
            ctx.request_repaint_after(left.max(Duration::from_millis(500)));
        }

        self.statusbar(ctx);
        self.topbar(ctx);
        self.sidebar(ctx);
        self.center(ctx);
        self.overlays(ctx);

        // 拖入文件夹
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for p in dropped {
            if p.is_dir() {
                let s = p.to_string_lossy().to_string();
                if let Ok(i) = conf::add(&mut self.conf, &s) {
                    self.conf.active = i;
                    let _ = conf::save(&self.conf);
                    self.load_project();
                }
            }
        }
        self.dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if self.dragging {
            egui::Area::new(egui::Id::new("drop-mask"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let r = ctx.screen_rect();
                    let p = ui.painter();
                    p.rect_filled(r, 0.0, Color32::from_black_alpha(150));
                    p.rect_stroke(r.shrink(9.0), 8.0, Stroke::new(2.0, theme::ACCENT));
                    p.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        "松手即可添加项目",
                        FontId::proportional(18.0),
                        Color32::WHITE,
                    );
                });
        }
    }
}

/* ---------------------------------------------------------------- 工具 */

fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.5, theme::ACCENT);
    let c = rect.center();
    let w = Stroke::new(1.5, Color32::from_rgb(0xf2, 0xf8, 0xff));
    p.line_segment([egui::pos2(c.x - 1.9, c.y - 3.4), egui::pos2(c.x - 4.2, c.y)], w);
    p.line_segment([egui::pos2(c.x - 4.2, c.y), egui::pos2(c.x - 1.9, c.y + 3.4)], w);
    p.line_segment([egui::pos2(c.x + 1.9, c.y - 3.4), egui::pos2(c.x + 4.2, c.y)], w);
    p.line_segment([egui::pos2(c.x + 4.2, c.y), egui::pos2(c.x + 1.9, c.y + 3.4)], w);
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(9.5),
        color,
    );
    let pad = Vec2::new(7.0, 2.0);
    let (rect, _) = ui.allocate_exact_size(
        galley.size() + pad * 2.0,
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 3.0, color.gamma_multiply(0.18));
    ui.painter().galley(
        egui::pos2(rect.left() + pad.x, rect.top() + pad.y),
        galley,
        color,
    );
}

fn fmt_size(n: u64) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

fn short_path(p: &str) -> String {
    if let Some(h) = dirs::home_dir() {
        let hs = h.to_string_lossy().to_string();
        if let Some(rest) = p.strip_prefix(&hs) {
            return format!("~{}", rest);
        }
    }
    p.to_string()
}

/// 子序列模糊匹配，返回得分（越大越好）
fn fuzzy_score(needle: &str, hay: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let n: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();
    let h: Vec<char> = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut hi = 0;
    let mut score = 0i32;
    let mut run = 0i32;
    for (ni, nc) in n.iter().enumerate() {
        let mut found = false;
        while hi < h.len() {
            if h[hi] == *nc {
                score += 10 + run * 6;
                // 词首加分
                if hi == 0 || h[hi - 1] == '/' || h[hi - 1] == '_' || h[hi - 1] == '-' {
                    score += 14;
                }
                run += 1;
                hi += 1;
                found = true;
                break;
            }
            run = 0;
            hi += 1;
            score -= 1;
        }
        if !found {
            let _ = ni;
            return None;
        }
    }
    score += (60 - hay.len().min(60)) as i32;
    Some(score)
}

fn fuzzy_files(files: &[String], q: &str, cap: usize) -> Vec<(String, i32)> {
    let mut out: Vec<(String, i32)> = Vec::new();
    for f in files {
        let base = f.rsplit('/').next().unwrap_or(f);
        if let Some(s) = fuzzy_score(q, base).or_else(|| fuzzy_score(q, f)) {
            out.push((f.clone(), s));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out.truncate(cap);
    out
}
