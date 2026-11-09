use crate::{conf, fsops, gitops, md, syntax, theme, tree};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_VIEW: u64 = 256 * 1024;
const WRAP_LIMIT: usize = 160 * 1024;
const ROW_H: f32 = 24.0;
const TOP_H: f32 = 54.0;
const GIT_H: f32 = 60.0;
const STATUS_H: f32 = 28.0;
const LEFT_W: f32 = 252.0;
const RIGHT_W: f32 = 296.0;

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

pub struct Cur {
    pub path: String,
    pub name: String,
    text: String,
    diff_text: String,
    lines: Vec<syntax::Line>,
    diff_lines: Vec<syntax::Line>,
    gutter: String,
    diff_gutter: String,
    size: u64,
    binary: bool,
    truncated: bool,
    deleted: bool,
    digits: usize,
    gen_lines: usize,
    can_wrap: bool,
    is_md: bool,
    preview: bool,
    dirty: bool,
    blocks: Vec<md::Block>,
    is_img: bool,
}

struct GalleyCache {
    rev: u64,
    w: f32,
    galley: Arc<egui::Galley>,
}

pub struct App {
    filter: String,
    ctx: egui::Context,
    search: bool,
    search_q: String,
    search_hits: Vec<(usize, usize, usize)>,
    search_sel: usize,
    search_cached: (String, u64),
    ctx_menu: Option<(egui::Pos2, String, bool)>,
    img: Option<(String, egui::TextureHandle)>,
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

    left_open: bool,
    right_open: bool,
    rtab: usize,
    branches: Vec<String>,
    history: Vec<(String, String, String)>,
    branch_menu: bool,
    show_history: bool,
    commit_msg: String,

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
            filter: String::new(),
            ctx: cc.egui_ctx.clone(),
            search: false,
            search_q: String::new(),
            search_hits: Vec::new(),
            search_sel: 0,
            search_cached: (String::new(), 0),
            ctx_menu: None,
            img: None,
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
            left_open: true,
            right_open: true,
            rtab: 0,
            branches: Vec::new(),
            history: Vec::new(),
            branch_menu: false,
            show_history: false,
            commit_msg: String::new(),
            files: Vec::new(),
            files_loaded: false,
            palette: false,
            pal_q: String::new(),
            pal_sel: 0,
            confirm_del: false,
            toast: String::new(),
            toast_at: Instant::now(),
            dragging: false,
            title: "Ai-View".into(),
            title_dirty: false,
        };
        app.load_project();
        if let Ok(rel) = std::env::var("AI_VIEW_OPEN") {
            if !rel.is_empty() {
                app.open(&rel);
            }
        }
        #[cfg(debug_assertions)]
        if std::env::var("AI_VIEW_SEARCH").as_deref() == Ok("1") {
            app.search = true;
            if let Ok(q) = std::env::var("AI_VIEW_QUERY") {
                app.search_q = q;
            }
        }
        app
    }

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

    fn open(&mut self, rel: &str) {
        let Some(root) = self.root.clone() else { return };
        if self.cur.as_ref().map(|c| c.path == rel && !c.deleted).unwrap_or(false) {
            self.diff_mode = false;
            return;
        }
        let ext = rel.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp") {
            self.open_image(&root, rel);
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
                let lang = f.lang.clone();
                let lines = syntax::tokenize(&text, &lang);
                let n = lines.len().max(1);
                let can_wrap = text.len() < WRAP_LIMIT;
                let gutter = (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
                let is_md = lang == "markdown";
                let blocks = if is_md { md::parse(&text) } else { Vec::new() };
                if !can_wrap {
                    self.wrap = false;
                }
                let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
                self.cur = Some(Cur {
                    path: rel.to_string(),
                    name: name.clone(),
                    text,
                    diff_text: String::new(),
                    lines,
                    diff_lines: Vec::new(),
                    gutter,
                    diff_gutter: String::new(),
                    size: f.size,
                    binary: f.binary,
                    truncated,
                    deleted: false,
                    digits: n.to_string().len(),
                    gen_lines: n,
                    can_wrap,
                    is_md,
                    preview: is_md,
                    dirty: false,
                    blocks,
                    is_img: false,
                });
                self.diff_mode = false;
                self.scroll_y = 0.0;
                self.rev += 1;
                self.title = format!("{} — Ai-View", name);
                self.title_dirty = true;
            }
            Err(e) => self.toast(e),
        }
    }

    fn open_image(&mut self, root: &Path, rel: &str) {
        let full = root.join(rel);
        let bytes = match std::fs::read(&full) {
            Ok(b) if b.len() <= 16 * 1024 * 1024 => b,
            _ => {
                self.toast("图片过大或无法读取");
                return;
            }
        };
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i,
            Err(e) => {
                self.toast(format!("图片解码失败：{}", e));
                return;
            }
        };
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
        let tex = self
            .ctx
            .load_texture(rel, color, egui::TextureOptions::LINEAR);
        self.img = Some((rel.to_string(), tex));
        let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        self.cur = Some(Cur {
            path: rel.to_string(),
            name: name.clone(),
            text: String::new(),
            diff_text: String::new(),
            lines: Vec::new(),
            diff_lines: Vec::new(),
            gutter: String::new(),
            diff_gutter: String::new(),
            size: bytes.len() as u64,
            binary: false,
            truncated: false,
            deleted: false,
            digits: 1,
            gen_lines: 1,
            can_wrap: true,
            is_md: false,
            preview: false,
            dirty: false,
            blocks: Vec::new(),
            is_img: true,
        });
        self.diff_mode = false;
        self.scroll_y = 0.0;
        self.rev += 1;
        self.title = format!("{} — Ai-View", name);
        self.title_dirty = true;
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
        let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        self.cur = Some(Cur {
            path: rel.to_string(),
            name: name.clone(),
            text: String::new(),
            diff_text: d,
            lines: Vec::new(),
            diff_lines: lines,
            gutter: String::new(),
            diff_gutter: gutter,
            size: 0,
            binary: false,
            truncated: false,
            deleted: true,
            digits: n.to_string().len(),
            gen_lines: n,
            can_wrap: true,
            is_md: false,
            preview: false,
            dirty: false,
            blocks: Vec::new(),
            is_img: false,
        });
        self.diff_mode = true;
        self.scroll_y = 0.0;
        self.rev += 1;
        self.title = format!("{} (已删除) — Ai-View", name);
        self.title_dirty = true;
    }

    fn toggle_diff(&mut self) {
        if let Some(cur) = self.cur.as_ref() {
            if cur.deleted || cur.binary {
                return;
            }
        }
        if self.diff_mode {
            self.diff_mode = false;
            self.scroll_y = 0.0;
            self.rev += 1;
            return;
        }
        let Some(root) = self.root.clone() else { return };
        let p = match self.cur.as_ref() {
            Some(c) => c.path.clone(),
            None => return,
        };
        let state = self.git.files.get(&p).cloned();
        let full = root.join(&p);
        let (d, _) = gitops::diff(&root, &p, &full, state.as_deref());
        if d.trim().is_empty() {
            self.toast("该文件没有未提交的改动");
            return;
        }
        let lines = syntax::tokenize(&d, "diff");
        let n = lines.len().max(1);
        let gutter = (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        if let Some(cur) = self.cur.as_mut() {
            cur.diff_text = d;
            cur.diff_lines = lines;
            cur.diff_gutter = gutter;
        }
        self.diff_mode = true;
        self.scroll_y = 0.0;
        self.rev += 1;
    }

    fn rebuild(&mut self) {
        let Some(cur) = self.cur.as_mut() else { return };
        let lang = fsops::detect_lang(&cur.path);
        cur.lines = syntax::tokenize(&cur.text, lang);
        if cur.is_md {
            cur.blocks = md::parse(&cur.text);
        }
        let n = cur.lines.len().max(1);
        cur.gutter = (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        cur.digits = n.to_string().len();
        cur.gen_lines = n;
        self.rev += 1;
    }

    fn save_cur(&mut self) {
        let Some(root) = self.root.clone() else { return };
        let res = {
            let Some(cur) = self.cur.as_ref() else { return };
            if cur.deleted || cur.binary || cur.is_img {
                return;
            }
            fsops::write_file(&root, &cur.path, &cur.text)
        };
        match res {
            Ok(n) => {
                if let Some(cur) = self.cur.as_mut() {
                    cur.dirty = false;
                    cur.size = n;
                }
                self.refresh_git(true);
                self.toast("已保存");
            }
            Err(e) => self.toast(e),
        }
    }

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

    fn topbar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none()
            .fill(theme::panel())
            .inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 0.0, bottom: 0.0 });
        egui::TopBottomPanel::top("top")
            .exact_height(TOP_H)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let drag = ui.interact(ui.max_rect(), egui::Id::new("titlebar"), Sense::drag());
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.spacing_mut().item_spacing.x = 10.0;
                    logo(ui, 24.0);
                    brand(ui);
                    ui.add_space(4.0);
                    self.project_pill(ui);

                    let avail = ui.available_width();
                    let right_w = 336.0;
                    let cmd_w = 470.0_f32.min((avail - right_w - 20.0).max(160.0));
                    ui.add_space(((avail - right_w - cmd_w) / 2.0).max(6.0));

                    let mut open_palette = false;
                    if cmd_bar(ui, cmd_w) {
                        open_palette = true;
                    }
                    if open_palette {
                        self.ensure_files();
                        self.palette = true;
                        self.pal_q.clear();
                        self.pal_sel = 0;
                    }

                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if win_btn(ui, "×").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if win_btn(ui, "口").clicked() {
                                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                            }
                            if win_btn(ui, "—").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                            ui.add_space(10.0);
                            avatar(ui);
                            ui.add_space(4.0);
                            let night = !theme::light();
                            if icon_theme(ui, night).clicked() {
                                theme::toggle();
                                theme::apply(ctx);
                                self.rev += 1;
                            }
                            ui.add_space(8.0);
                            sync_chip(ui, self.git_at.elapsed(), self.git.is_git);
                            ui.add_space(6.0);
                            if icon_panels2(ui, self.left_open && self.right_open)
                                .on_hover_text("两侧栏一起收起 / 展开")
                                .clicked()
                            {
                                let both = self.left_open && self.right_open;
                                self.left_open = !both;
                                self.right_open = !both;
                            }
                            if icon_panel(ui, 1, self.right_open)
                                .on_hover_text("右栏")
                                .clicked()
                            {
                                self.right_open = !self.right_open;
                            }
                            if icon_panel(ui, 0, self.left_open)
                                .on_hover_text("左栏")
                                .clicked()
                            {
                                self.left_open = !self.left_open;
                            }
                        });
                    });
                });
    }
    fn project_pill(&mut self, ui: &mut egui::Ui) {
        let cur_name = self
            .conf
            .projects
            .get(self.conf.active)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "未打开项目".into());
        let branch = if self.git.is_git && !self.git.branch.is_empty() {
            self.git.branch.clone()
        } else {
            String::new()
        };

        let mut pick: Option<usize> = None;
        egui::ComboBox::from_id_salt("proj")
            .width(150.0)
            .selected_text(RichText::new(cur_name).size(11.5))
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
                    ui.label(RichText::new("还没有项目").size(11.5).color(theme::muted()));
                }
                ui.separator();
                if ui.label(RichText::new("＋ 添加目录…").size(11.5)).clicked() {
                    pick = Some(usize::MAX);
                }
            });
        if let Some(i) = pick {
            if i == usize::MAX {
                self.add_project();
            } else if i != self.conf.active {
                self.conf.active = i;
                let _ = conf::save(&self.conf);
                self.load_project();
            }
        }
        if !branch.is_empty() {
            ui.label(RichText::new(branch).size(10.0).color(theme::faint()).monospace());
        }
    }

    fn gitbar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none()
            .fill(theme::bg2())
            .inner_margin(egui::Margin {
                left: 14.0,
                right: 14.0,
                top: 0.0,
                bottom: 0.0,
            });
        egui::TopBottomPanel::top("gitbar")
            .exact_height(GIT_H)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let branch = if self.git.branch.is_empty() {
                        "-".to_string()
                    } else {
                        self.git.branch.clone()
                    };
                    if pill_btn(ui, "⑂", &branch).clicked() {
                        if let Some((pos, path, is_dir)) = self.ctx_menu.clone() {
            let root = self.root.clone();
            egui::Area::new(egui::Id::new("ctx-mask"))
                .order(egui::Order::Middle)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let r = ctx.screen_rect();
                    ui.painter().rect_filled(r, 0.0, Color32::TRANSPARENT);
                    let resp = ui.allocate_rect(r, Sense::click());
                    if resp.clicked() || resp.secondary_clicked() {
                        self.ctx_menu = None;
                    }
                });
            egui::Area::new(egui::Id::new("ctx-menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(theme::raise())
                        .stroke(Stroke::new(1.0, theme::line2()))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::same(6.0))
                        .show(ui, |ui| {
                            ui.set_width(200.0);
                            let is_md_m = !is_dir &&
                                self.git.files.get(&path).map(|v| v == "M").unwrap_or(false);
                            let mut item = |ui: &mut egui::Ui, label: &str, enabled: bool| -> bool {
                                if enabled {
                                    ui.add(
                                        egui::Button::new(
                                            RichText::new(label).size(11.5).color(theme::text()),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(188.0, 26.0)),
                                    )
                                    .clicked()
                                } else {
                                    ui.label(RichText::new(label).size(11.5).color(theme::faint()));
                                    false
                                }
                            };
                            if !is_dir && item(ui, "打开", true) {
                                self.open(&path);
                                self.ctx_menu = None;
                            }
                            if is_dir && item(ui, "展开 / 折叠", true) {
                                if let Some(r) = &root {
                                    self.tree.toggle(r, &path);
                                }
                                self.ctx_menu = None;
                            }
                            if item(ui, "复制路径", true) {
                                ctx.copy_text(path.clone());
                                self.toast("路径已复制");
                                self.ctx_menu = None;
                            }
                            if item(ui, "在资源管理器中显示", true) {
                                if let Some(r) = &root {
                                    reveal_in_explorer(r, &path);
                                }
                                self.ctx_menu = None;
                            }
                            if is_md_m && item(ui, "撤销未提交更改", true) {
                                if let Some(r) = &root {
                                    match gitops::discard(r, &path) {
                                        Ok(s) => {
                                            self.toast(s);
                                            self.refresh_git(true);
                                            if self.cur.as_ref().map(|c| c.path == path).unwrap_or(false) {
                                                self.open(&path);
                                            }
                                        }
                                        Err(e) => self.toast(e),
                                    }
                                }
                                self.ctx_menu = None;
                            }
                        });
                });
        }

        if self.branch_menu { self.branch_menu = false; } else {
                            self.branches = self.root.as_ref().map(|r| gitops::branches(r)).unwrap_or_default();
                            self.branch_menu = true;
                        }
                    }
                    let remote = if self.git.remote.is_empty() {
                        "-".to_string()
                    } else {
                        self.git.remote.clone()
                    };
                    pill_btn(ui, "☁", &remote);
                    ui.label(
                        RichText::new(format!("↑ {}", self.git.ahead))
                            .size(10.5)
                            .color(theme::brass())
                            .monospace(),
                    );
                    ui.label(
                        RichText::new(format!("↓ {}", self.git.behind))
                            .size(10.5)
                            .color(theme::muted())
                            .monospace(),
                    );

                    ui.add_space(4.0);
                    if graph(ui, self.git.ahead, self.git.behind).clicked() {
                        self.history = self.root.as_ref().map(|r| gitops::log(r)).unwrap_or_default();
                        self.show_history = true;
                    }
                    ui.add_space(4.0);

                    let input_w = (ui.available_width() - 520.0).max(120.0);
                    let te = egui::TextEdit::singleline(&mut self.commit_msg)
                        .hint_text("描述这次改动")
                        .desired_width(input_w)
                        .margin(egui::Margin::symmetric(9.0, 6.0));
                    ui.add(te);

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let can = !self.commit_msg.trim().is_empty() && !self.git.files.is_empty();
                        let b =
                            egui::Button::new(RichText::new("提交").size(11.5).color(theme::on_brass()))
                                .fill(theme::brass())
                                .rounding(7.0)
                                .min_size(Vec2::new(52.0, 26.0));
                        if ui.add_enabled(can, b).clicked() {
                            self.do_commit();
                        }
                        ui.label(
                            RichText::new(format!("{} 个文件", self.git.files.len()))
                                .size(9.5)
                                .color(theme::faint()),
                        );
                        if ghost(ui, "推送").clicked() {
                            self.do_push();
                        }
                        if ghost(ui, "拉取").clicked() {
                            self.do_pull();
                        }
                    });
                });
            });
    }

    fn do_commit(&mut self) {
        let Some(root) = self.root.clone() else { return };
        let msg = self.commit_msg.trim().to_string();
        if msg.is_empty() {
            self.toast("先写一句提交说明");
            return;
        }
        gitops::invalidate(&root);
        match gitops::commit(&root, &msg) {
            Ok(s) => {
                self.commit_msg.clear();
                self.refresh_git(true);
                self.toast(s);
            }
            Err(e) => self.toast(e),
        }
    }

    fn do_pull(&mut self) {
        let Some(root) = self.root.clone() else { return };
        gitops::invalidate(&root);
        match gitops::pull(&root) {
            Ok(s) => {
                self.refresh_git(true);
                if let Some(r) = self.root.clone() {
                    self.tree.reload(&r);
                }
                self.toast(s);
            }
            Err(e) => self.toast(e),
        }
    }

    fn do_push(&mut self) {
        let Some(root) = self.root.clone() else { return };
        match gitops::push(&root) {
            Ok(s) => {
                self.refresh_git(true);
                self.toast(s);
            }
            Err(e) => self.toast(e),
        }
    }

    fn side_panels(&mut self, ctx: &egui::Context) {
        if self.left_open {
            egui::SidePanel::left("side")
                .resizable(true)
                .default_width(LEFT_W)
                .width_range(196.0..=430.0)
                .frame(
                    egui::Frame::none()
                        .fill(theme::panel())
                        .inner_margin(egui::Margin::ZERO),
                )
                .show_separator_line(false)
                .show(ctx, |ui| {
                    let r = ui.max_rect();
                    ui.painter()
                        .vline(r.right(), r.y_range(), Stroke::new(1.0, theme::line()));
                    self.sidebar_ui(ui);
                });
        } else {
            egui::SidePanel::left("side-stub")
                .exact_width(24.0)
                .resizable(false)
                .frame(egui::Frame::none().fill(theme::panel2()))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("›").size(15.0).color(theme::muted()),
                                )
                                .frame(false),
                            )
                            .on_hover_text("展开左栏")
                            .clicked()
                        {
                            self.left_open = true;
                        }
                    });
                });
        }

        if self.right_open {
            egui::SidePanel::right("right")
                .resizable(true)
                .default_width(RIGHT_W)
                .width_range(220.0..=420.0)
                .frame(
                    egui::Frame::none()
                        .fill(theme::panel())
                        .inner_margin(egui::Margin::ZERO),
                )
                .show_separator_line(false)
                .show(ctx, |ui| {
                    let r = ui.max_rect();
                    ui.painter()
                        .vline(r.left(), r.y_range(), Stroke::new(1.0, theme::line()));
                    self.right_ui(ui);
                });
        } else {
            egui::SidePanel::right("right-stub")
                .exact_width(24.0)
                .resizable(false)
                .frame(egui::Frame::none().fill(theme::panel2()))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("‹").size(15.0).color(theme::muted()),
                                )
                                .frame(false),
                            )
                            .on_hover_text("展开右栏")
                            .clicked()
                        {
                            self.right_open = true;
                        }
                    });
                });
        }
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(10.0);
            let n = self.git.files.len();
            if pill_tab(ui, "文件", self.tab == 0) {
                self.tab = 0;
            }
            let label = if n > 0 {
                format!("变更 {}", n)
            } else {
                "变更".to_string()
            };
            if pill_tab(ui, &label, self.tab == 1) {
                self.tab = 1;
                self.ensure_files();
            }
            if pill_tab(ui, "远程", self.tab == 2) {
                self.tab = 2;
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if ui
                    .add(egui::Button::new(RichText::new("«").size(13.0).color(theme::faint())).frame(false))
                    .on_hover_text("收起左栏")
                    .clicked()
                {
                    self.left_open = false;
                }
            });
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let w = (ui.available_width() - 10.0).max(60.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::hover());
            ui.painter()
                .rect(rect, 7.0, theme::panel2(), Stroke::new(1.0, theme::line()));
            ui.painter().text(
                egui::pos2(rect.left() + 9.0, rect.center().y),
                Align2::LEFT_CENTER,
                "⌕",
                FontId::proportional(12.0),
                theme::faint(),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(egui::Rect::from_min_max(
                        egui::pos2(rect.left() + 22.0, rect.top() + 3.0),
                        egui::pos2(rect.right() - 8.0, rect.bottom() - 3.0),
                    ))
                    .layout(egui::Layout::left_to_right(Align::Center)),
            );
            child.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("过滤文件")
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(11.0)),
            );
        });
        ui.add_space(6.0);
        let y = ui.cursor().top();
        ui.painter()
            .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::line()));
        ui.add_space(6.0);

        let bottom_h = if self.tab == 0 { 78.0 } else { 0.0 };
        let h = (ui.available_height() - bottom_h).max(40.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), h),
            egui::Layout::top_down(Align::Min),
            |ui| match self.tab {
                0 => self.draw_tree(ui),
                1 => self.draw_changes(ui),
                _ => self.draw_remotes(ui),
            },
        );

        if self.tab == 0 {
            self.remote_card(ui);
        }
    }

    fn remote_card(&mut self, ui: &mut egui::Ui) {
        let r = ui.max_rect();
        let card = egui::Rect::from_min_max(
            egui::pos2(r.left() + 10.0, r.bottom() - 66.0),
            egui::pos2(r.right() - 10.0, r.bottom() - 8.0),
        );
        let remote = self.git.remote.clone();
        let branch = self.git.branch.clone();
        let p = ui.painter();
        p.rect(card, 9.0, theme::panel2(), Stroke::new(1.0, theme::line()));
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 17.0),
            Align2::LEFT_CENTER,
            if remote.is_empty() { "origin" } else { remote.as_str() },
            FontId::proportional(11.5),
            theme::text(),
        );
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 36.0),
            Align2::LEFT_CENTER,
            format!("↑{}   ↓{}", self.git.ahead, self.git.behind),
            FontId::monospace(9.5),
            theme::faint(),
        );
        p.text(
            egui::pos2(card.left() + 12.0, card.top() + 52.0),
            Align2::LEFT_CENTER,
            if branch.is_empty() { "-" } else { branch.as_str() },
            FontId::monospace(9.5),
            theme::brass(),
        );
    }

    fn draw_tree(&mut self, ui: &mut egui::Ui) {
        if self.conf.projects.is_empty() {
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("还没有打开项目").size(12.5).color(theme::muted()));
                ui.add_space(8.0);
                if ui.button("选择目录…").clicked() {
                    self.add_project();
                }
            });
            return;
        }
        if let Some(e) = &self.root_err {
            let e = e.clone();
            ui.add_space(10.0);
            ui.label(RichText::new(e).size(11.5).color(theme::rose()));
            return;
        }

        let q = self.filter.trim().to_lowercase();
        let all = self.tree.flatten();
        let rows: Vec<_> = if q.is_empty() {
            all
        } else {
            all.into_iter()
                .filter(|r| r.name.to_lowercase().contains(&q))
                .collect()
        };
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
                        p.rect_filled(rect.shrink2(Vec2::new(6.0, 0.0)), 6.0, theme::sel());
                        p.rect_filled(
                            egui::Rect::from_min_size(rect.min, Vec2::new(2.0, ROW_H)),
                            1.0,
                            theme::brass(),
                        );
                    } else if resp.hovered() {
                        p.rect_filled(rect.shrink2(Vec2::new(6.0, 0.0)), 6.0, theme::hover());
                    }

                    let x = rect.left() + 14.0 + row.depth as f32 * 13.0;
                    let mid = rect.center().y;

                    if row.dir {
                        let c = egui::pos2(x + 4.0, mid);
                        let pts = if row.open {
                            vec![
                                egui::pos2(c.x - 3.5, c.y - 1.5),
                                egui::pos2(c.x + 3.5, c.y - 1.5),
                                egui::pos2(c.x, c.y + 3.0),
                            ]
                        } else {
                            vec![
                                egui::pos2(c.x - 1.5, c.y - 3.5),
                                egui::pos2(c.x + 3.0, c.y),
                                egui::pos2(c.x - 1.5, c.y + 3.5),
                            ]
                        };
                        p.add(egui::Shape::convex_polygon(pts, theme::faint(), Stroke::NONE));
                    }

                    let nx = x + 12.0;
                    if row.dir {
                        p.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(nx, mid - 4.5), Vec2::new(10.0, 7.0)),
                            1.5,
                            theme::brass().gamma_multiply(0.65),
                        );
                        p.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(nx, mid - 6.0), Vec2::new(4.5, 2.5)),
                            1.0,
                            theme::brass().gamma_multiply(0.65),
                        );
                    } else {
                        p.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(nx + 3.0, mid - 5.0), Vec2::new(2.0, 10.0)),
                            1.0,
                            theme::muted().gamma_multiply(0.8),
                        );
                    }
                    p.text(
                        egui::pos2(nx + 17.0, mid),
                        Align2::LEFT_CENTER,
                        &row.name,
                        FontId::proportional(12.5),
                        theme::text(),
                    );

                    let mark = if row.dir {
                        if self.dirty_dirs.contains(&row.path) {
                            Some(("•".to_string(), theme::brass()))
                        } else {
                            None
                        }
                    } else {
                        self.git
                            .files
                            .get(&row.path)
                            .map(|s| (s.clone(), theme::git_color(s)))
                    };
                    if let Some((m, c)) = mark {
                        p.text(
                            egui::pos2(rect.right() - 14.0, mid),
                            Align2::RIGHT_CENTER,
                            m,
                            FontId::monospace(10.0),
                            c,
                        );
                    }

                    if resp.secondary_clicked() {
                        self.ctx_menu = Some((
                            resp.interact_pointer_pos().unwrap_or(rect.center()),
                            row.path.clone(),
                            row.dir,
                        ));
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
            ui.label(RichText::new("没有可用项目").size(12.0).color(theme::faint()));
            return;
        }
        if !self.git.is_git {
            ui.add_space(14.0);
            ui.label(RichText::new("不是 git 仓库").size(12.0).color(theme::faint()));
            return;
        }
        let files = &self.git.files;
        if files.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("工作区干净，没有变更").size(12.0).color(theme::faint()));
            return;
        }

        let groups: [(&str, &[&str]); 3] = [
            ("已修改", &["M", "A", "R"]),
            ("已删除", &["D"]),
            ("未跟踪", &["U"]),
        ];
        let mut items: Vec<(String, String, usize)> = Vec::new();
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
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(format!("{} · {}", groups[*gi].0, n))
                                    .size(9.5)
                                    .color(theme::faint()),
                            );
                        });
                        ui.add_space(3.0);
                    }
                    let full_w = ui.available_width();
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, ROW_H), Sense::click());
                    let selected = self.cur.as_ref().map(|c| c.path == *path).unwrap_or(false);
                    let p = ui.painter();
                    if selected {
                        p.rect_filled(rect.shrink2(Vec2::new(6.0, 0.0)), 6.0, theme::sel());
                    } else if resp.hovered() {
                        p.rect_filled(rect.shrink2(Vec2::new(6.0, 0.0)), 6.0, theme::hover());
                    }
                    p.text(
                        egui::pos2(rect.left() + 15.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        state,
                        FontId::monospace(10.0),
                        theme::git_color(state),
                    );
                    let i = path.rfind('/');
                    let (dir, name) = match i {
                        Some(k) => (&path[..k + 1], &path[k + 1..]),
                        None => ("", path.as_str()),
                    };
                    p.text(
                        egui::pos2(rect.left() + 30.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        dir,
                        FontId::proportional(10.5),
                        theme::faint(),
                    );
                    let dx = if dir.is_empty() {
                        0.0
                    } else {
                        p.layout_no_wrap(dir.to_string(), FontId::proportional(10.5), theme::faint())
                            .size()
                            .x
                    };
                    p.text(
                        egui::pos2(rect.left() + 30.0 + dx, rect.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(12.0),
                        theme::text(),
                    );
                    if resp.secondary_clicked() {
                        self.ctx_menu = Some((
                            resp.interact_pointer_pos().unwrap_or(rect.center()),
                            path.clone(),
                            false,
                        ));
                    }
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

    fn draw_remotes(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        if !self.git.is_git {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("不是 git 仓库").size(12.0).color(theme::faint()));
            });
            return;
        }
        let remotes = if self.git.remote.is_empty() {
            vec!["origin".to_string()]
        } else {
            vec![self.git.remote.clone()]
        };
        for name in remotes {
            ui.add_space(4.0);
            let branch = if self.git.branch.is_empty() {
                "-".to_string()
            } else {
                self.git.branch.clone()
            };
            let (ahead, behind) = (self.git.ahead, self.git.behind);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&name).size(12.0).color(theme::text()).strong());
                    ui.label(
                        RichText::new(format!("{} · ↑{} ↓{}", branch, ahead, behind))
                            .size(10.0)
                            .color(theme::faint())
                            .monospace(),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ghost(ui, "拉取").clicked() {
                            self.do_pull();
                        }
                        if ghost(ui, "推送").clicked() {
                            self.do_push();
                        }
                    });
                });
            });
            ui.add_space(6.0);
            rule(ui, 12.0);
        }
    }

    fn right_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.add_space(10.0);
            for (i, name) in ["外观", "变更", "信息"].iter().enumerate() {
                if tab_btn(ui, name, self.rtab == i) {
                    self.rtab = i;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if ui
                    .add(egui::Button::new(RichText::new("»").size(13.0).color(theme::faint())).frame(false))
                    .on_hover_text("收起右栏")
                    .clicked()
                {
                    self.right_open = false;
                }
            });
        });
        ui.add_space(8.0);
        rule(ui, 10.0);
        ui.add_space(10.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.rtab {
                0 => self.right_appearance(ui),
                1 => self.right_changes(ui),
                _ => self.right_info(ui),
            });
    }

    fn right_appearance(&mut self, ui: &mut egui::Ui) {
        row_label(ui, "主题", "");
        ui.add_space(6.0);
        let w = (ui.available_width() - 28.0).max(80.0);
        let night = !theme::light();
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let half = (w - 6.0) / 2.0;
            if seg_w(ui, "夜航", night, half).clicked() && !night {
                theme::set_light(false);
                theme::apply(ui.ctx());
                self.rev += 1;
            }
            if seg_w(ui, "日纸", !night, half).clicked() && night {
                theme::set_light(true);
                theme::apply(ui.ctx());
                self.rev += 1;
            }
        });

        ui.add_space(18.0);
        row_label(ui, "代码字体", "");
        ui.add_space(6.0);
        let fonts = theme::alt_fonts().to_vec();
        let cur = theme::code_font();
        let sel = if cur == 0 {
            "默认等宽".to_string()
        } else {
            fonts
                .get((cur as usize - 1).min(fonts.len().saturating_sub(1)))
                .cloned()
                .unwrap_or_else(|| "默认等宽".to_string())
        };
        let mut chosen: Option<u8> = None;
        egui::ComboBox::from_id_salt("code-font")
            .width(w - 14.0)
            .selected_text(RichText::new(sel).size(11.0).color(theme::text()))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(cur == 0, RichText::new("默认等宽").size(11.0))
                    .clicked()
                {
                    chosen = Some(0);
                }
                for (i, f) in fonts.iter().enumerate() {
                    if ui
                        .selectable_label(
                            cur == (i + 1) as u8,
                            RichText::new(f).size(11.0).monospace(),
                        )
                        .clicked()
                    {
                        chosen = Some((i + 1) as u8);
                    }
                }
                if fonts.is_empty() {
                    ui.label(
                        RichText::new("未检测到可选字体")
                            .size(10.0)
                            .color(theme::faint()),
                    );
                }
            });
        if let Some(v) = chosen {
            theme::set_code_font(v);
            self.rev += 1;
        }

        ui.add_space(18.0);
        row_label(ui, "字号", &format!("{:.1} px", theme::code_size()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let mut v = theme::code_size();
            let r = ui.add(egui::Slider::new(&mut v, 12.0..=20.0).show_value(false));
            if r.changed() {
                theme::set_code_size(v);
                self.rev += 1;
            }
        });

        ui.add_space(16.0);
        row_label(ui, "行高", &format!("{:.2}", theme::code_ratio()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let mut v = theme::code_ratio();
            let r = ui.add(egui::Slider::new(&mut v, 1.4..=2.3).show_value(false));
            if r.changed() {
                theme::set_code_ratio(v);
                self.rev += 1;
            }
        });

        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            egui::Frame::none()
                .fill(theme::panel2())
                .stroke(Stroke::new(1.0, theme::line()))
                .rounding(9.0)
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_width((w - 34.0).max(70.0));
                    ui.label(
                        RichText::new("let theme = Night;")
                            .size(theme::code_size())
                            .color(theme::brass())
                            .font(syntax::font()),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("sync(remote);")
                            .size(theme::code_size())
                            .color(theme::text())
                            .font(syntax::font()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("AaBbCc 0123456789 {} => !=")
                            .size(9.5)
                            .color(theme::faint()),
                    );
                });
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            if ghost(ui, "恢复默认").clicked() {
                theme::set_code_size(15.0);
                theme::set_code_ratio(1.9);
                theme::set_code_font(0);
                theme::set_light(false);
                theme::apply(ui.ctx());
                self.rev += 1;
            }
        });
    }
    fn right_changes(&mut self, ui: &mut egui::Ui) {
        let (mut m, mut a, mut d, mut u) = (0, 0, 0, 0);
        for v in self.git.files.values() {
            match v.as_str() {
                "M" => m += 1,
                "A" => a += 1,
                "D" => d += 1,
                "U" => u += 1,
                _ => {}
            }
        }
        row_label(ui, "工作区", &format!("{} 个文件", self.git.files.len()));
        ui.add_space(8.0);
        kv(ui, "修改", &m.to_string());
        kv(ui, "新增", &a.to_string());
        kv(ui, "删除", &d.to_string());
        kv(ui, "未跟踪", &u.to_string());
        ui.add_space(12.0);
        rule(ui, 14.0);
        ui.add_space(8.0);
        let files: Vec<(String, String)> =
            self.git.files.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (path, state) in &files {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(state)
                        .size(10.0)
                        .monospace()
                        .color(theme::git_color(state)),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(path).size(10.0).color(theme::text()));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(14.0);
                    if *state == "M" && ghost(ui, "撤销").clicked() {
                        if let Some(root) = self.root.clone() {
                            match gitops::discard(&root, path) {
                                Ok(s) => {
                                    self.toast(s);
                                    self.refresh_git(true);
                                }
                                Err(e) => self.toast(e),
                            }
                        }
                    }
                });
            });
        }
        ui.add_space(12.0);
        rule(ui, 14.0);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            if ghost(ui, "刷新状态").clicked() {
                self.refresh_git(true);
            }
        });
    }

    fn right_info(&mut self, ui: &mut egui::Ui) {
        let info = self.cur.as_ref().map(|c| {
            let lang = fsops::detect_lang(&c.path).to_string();
            (
                c.name.clone(),
                c.path.clone(),
                c.size,
                c.lines.len(),
                lang,
                c.is_md,
                c.preview,
                c.dirty,
                c.binary,
            )
        });
        if let Some((name, path, size, lines, lang, is_md, preview, dirty, binary)) = info {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&name).size(12.5).color(theme::text()).strong());
                    if dirty {
                        ui.label(RichText::new("有未保存改动").size(10.0).color(theme::brass()));
                    }
                    ui.add_space(2.0);
                    ui.label(RichText::new(&path).size(9.5).color(theme::faint()));
                });
            });
            ui.add_space(10.0);
            kv(ui, "类型", if binary { "二进制" } else { lang.as_str() });
            kv(ui, "体积", &fmt_size(size));
            kv(ui, "行数", &lines.to_string());
            if is_md {
                kv(ui, "视图", if preview { "预览" } else { "源码" });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    if ghost(ui, if preview { "源码" } else { "预览" }).clicked() {
                        if let Some(c) = self.cur.as_mut() {
                            c.preview = !preview;
                        }
                        self.rev += 1;
                    }
                    if ghost(ui, "保存").clicked() {
                        self.save_cur();
                    }
                });
            }
            ui.add_space(12.0);
            rule(ui, 14.0);
        } else {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(RichText::new("没有打开文件").size(11.5).color(theme::faint()));
            });
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("Ai-View {}", env!("CARGO_PKG_VERSION")))
                        .size(10.5)
                        .color(theme::muted()),
                );
                ui.label(
                    RichText::new("只读浏览 · Markdown 可编辑")
                        .size(9.5)
                        .color(theme::faint()),
                );
                ui.label(RichText::new("作者 Mr.7y").size(9.5).color(theme::faint()));
            });
        });
    }

    fn center(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none()
            .fill(theme::code_bg())
            .inner_margin(egui::Margin::ZERO);
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if self.conf.projects.is_empty() {
                self.welcome(
                    ui,
                    "还没有打开项目",
                    "点左上角选择代码目录，或直接把文件夹拖进窗口。",
                );
                return;
            }
            if self.cur.is_none() {
                self.welcome(ui, "Ai-View", "从左侧选择一个文件开始阅读。");
                return;
            }
            self.tabs_bar(ui);
            self.crumb(ui);
            if self.search {
                self.search_bar(ui);
            }
            let y = ui.cursor().top();
            ui.painter()
                .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::line()));
            self.code(ui);
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui, title: &str, sub: &str) {
        let h = ui.available_height();
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), h),
            egui::Layout::top_down(Align::Center),
            |ui| {
                ui.add_space(h * 0.3);
                let (r, _) = ui.allocate_exact_size(Vec2::new(48.0, 48.0), Sense::hover());
                logo_at(ui.painter(), r);
                ui.add_space(14.0);
                ui.label(RichText::new(title).size(17.0).color(theme::text()));
                ui.add_space(6.0);
                ui.label(RichText::new(sub).size(12.0).color(theme::muted()));
            },
        );
    }

    fn tabs_bar(&mut self, ui: &mut egui::Ui) {
        let Some(cur) = self.cur.as_ref() else { return };
        let name = cur.name.clone();
        let dirty = cur.dirty;
        ui.horizontal(|ui| {
            ui.set_height(34.0);
            ui.add_space(10.0);
            let galley = ui.painter().layout_no_wrap(
                name.clone(),
                FontId::proportional(11.5),
                theme::text(),
            );
            let w = galley.size().x + 56.0;
            let (rect, _resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::click());
            ui.painter().rect_filled(rect, 0.0, theme::panel());
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                Stroke::new(1.5, theme::brass()),
            );
            let ix = rect.left() + 10.0;
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(
                    egui::pos2(ix, rect.center().y - 5.0),
                    Vec2::new(9.0, 11.0),
                ),
                1.5,
                Stroke::new(1.0, theme::faint()),
            );
            ui.painter().galley(
                egui::pos2(ix + 15.0, rect.center().y - galley.size().y / 2.0),
                galley,
                theme::text(),
            );
            if dirty {
                ui.painter().circle_filled(
                    egui::pos2(rect.right() - 26.0, rect.center().y),
                    2.5,
                    theme::brass(),
                );
            }
            ui.painter().text(
                egui::pos2(rect.right() - 10.0, rect.center().y),
                Align2::RIGHT_CENTER,
                "×",
                FontId::proportional(10.0),
                theme::faint(),
            );
        });
        let y = ui.cursor().top();
        ui.painter()
            .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::line()));
    }

    fn crumb(&mut self, ui: &mut egui::Ui) {
        let Some(cur) = self.cur.as_ref() else { return };
        let (dir, name) = match cur.path.rfind('/') {
            Some(i) => (cur.path[..i + 1].to_string(), cur.name.clone()),
            None => (String::new(), cur.name.clone()),
        };
        let size = cur.size;
        let truncated = cur.truncated;
        let binary = cur.binary;
        let deleted = cur.deleted;
        let is_md = cur.is_md;
        let preview = cur.preview;
        let dirty = cur.dirty;
        let diff = self.diff_mode;
        let changed = self.git.files.contains_key(&cur.path);

        ui.horizontal(|ui| {
            ui.set_height(38.0);
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            if !dir.is_empty() {
                ui.label(RichText::new(&dir).size(11.0).color(theme::faint()));
            }
            ui.label(RichText::new(&name).size(12.0).color(theme::text()).strong());
            if dirty {
                let (r, _) = ui.allocate_exact_size(Vec2::new(5.0, 5.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 2.5, theme::brass());
            }
            ui.add_space(6.0);

            if binary {
                badge(ui, "二进制", theme::brass());
            } else if deleted {
                badge(ui, "已删除", theme::rose());
            } else if diff {
                badge(ui, "改动视图", theme::brass());
            } else if truncated {
                badge(ui, "已截断", theme::brass());
            } else if is_md {
                badge(ui, "Markdown", theme::jade());
            }

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                if size > 0 {
                    ui.label(
                        RichText::new(fmt_size(size))
                            .size(10.0)
                            .color(theme::faint())
                            .monospace(),
                    );
                }
                if is_md && !deleted && !diff && ghost(ui, if preview { "源码" } else { "预览" }).clicked()
                {
                    if let Some(c) = self.cur.as_mut() {
                        c.preview = !preview;
                    }
                    self.rev += 1;
                }
                if dirty {
                    let b =
                        egui::Button::new(RichText::new("保存").size(11.0).color(theme::on_brass()))
                            .fill(theme::brass())
                            .rounding(6.0)
                            .min_size(Vec2::new(46.0, 24.0));
                    if ui.add(b).clicked() {
                        self.save_cur();
                    }
                }
                if can_wrap_of(&self.cur) {
                    if ghost(ui, if self.wrap { "折行 ✓" } else { "折行" }).clicked() {
                        self.wrap = !self.wrap;
                        self.rev += 1;
                    }
                }
                if changed && !deleted {
                    let b = egui::Button::new(RichText::new("Diff").size(11.0))
                        .rounding(6.0)
                        .min_size(Vec2::new(44.0, 24.0))
                        .selected(diff);
                    if ui.add(b).clicked() {
                        self.toggle_diff();
                    }
                }
            });
        });
    }

    fn md_view(&mut self, ui: &mut egui::Ui) {
        let Some(cur) = self.cur.as_ref() else { return };
        let blocks = &cur.blocks;
        egui::ScrollArea::vertical()
            .id_salt(("md", self.rev))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin {
                        left: 30.0,
                        right: 30.0,
                        top: 16.0,
                        bottom: 48.0,
                    })
                    .show(ui, |ui| {
                        ui.set_width((ui.available_width() - 8.0).max(200.0));
                        md::render(ui, blocks);
                    });
            });
    }

    fn image_view(&mut self, ui: &mut egui::Ui) {
        let Some((_, tex)) = &self.img else {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("图片未加载").size(12.0).color(theme::muted()));
            });
            return;
        };
        let avail = ui.available_size() - Vec2::new(48.0, 48.0);
        let [w, h] = tex.size();
        let s = (avail.x / w as f32).min(avail.y / h as f32).min(1.0);
        let size = Vec2::new(w as f32 * s, h as f32 * s);
        let id = tex.id();
        ui.centered_and_justified(|ui| {
            ui.add(egui::Image::new((id, size)).rounding(8.0));
        });
    }


    fn search_step(&mut self, dir: i32) {
        let n = self.search_hits.len();
        if n == 0 {
            return;
        }
        self.search_sel = ((self.search_sel as i32 + dir).rem_euclid(n as i32)) as usize;
        self.scroll_to_hit();
    }

    fn scroll_to_hit(&mut self) {
        if let Some((li, _, _)) = self.search_hits.get(self.search_sel) {
            self.scroll_y = (*li as f32 * theme::line_height() - 120.0).max(0.0);
        }
    }

    fn search_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.set_height(34.0);
            ui.add_space(10.0);
            let before = self.search_q.clone();
            ui.add(
                egui::TextEdit::singleline(&mut self.search_q)
                    .hint_text("搜索当前文件…")
                    .desired_width(200.0)
                    .font(FontId::proportional(11.5)),
            );
            let changed = self.search_q != before || self.search_cached.1 != self.rev;
            if changed {
                let text = self
                    .cur
                    .as_ref()
                    .map(|c| c.text.clone())
                    .unwrap_or_default();
                self.search_hits = search_hits_of(&text, &self.search_q);
                self.search_cached = (self.search_q.clone(), self.rev);
                self.search_sel = 0;
                self.scroll_to_hit();
            }
            ui.add_space(6.0);
            let n = self.search_hits.len();
            let label = if n == 0 {
                "无结果".to_string()
            } else {
                format!("{}/{} 处", self.search_sel + 1, n)
            };
            ui.label(RichText::new(label).size(11.0).color(theme::muted()));
            if ui
                .add(egui::Button::new(RichText::new("↑").size(12.0)).frame(false).min_size(Vec2::new(22.0, 24.0)))
                .clicked()
            {
                self.search_step(-1);
            }
            if ui
                .add(egui::Button::new(RichText::new("↓").size(12.0)).frame(false).min_size(Vec2::new(22.0, 24.0)))
                .clicked()
            {
                self.search_step(1);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if ui
                    .add(egui::Button::new(RichText::new("×").size(12.0)).frame(false).min_size(Vec2::new(22.0, 24.0)))
                    .clicked()
                {
                    self.search = false;
                }
            });
        });
    }

    fn code(&mut self, ui: &mut egui::Ui) {
        let Some(c) = self.cur.as_ref() else { return };
        let (binary, is_md, preview, deleted, truncated, can_wrap, size, is_img) = (
            c.binary,
            c.is_md,
            c.preview,
            c.deleted,
            c.truncated,
            c.can_wrap,
            c.size,
            c.is_img,
        );
        if is_img {
            self.image_view(ui);
            return;
        }
        let hit_lines: Vec<usize> = if self.search {
            self.search_hits.iter().map(|h| h.0).collect()
        } else {
            Vec::new()
        };
        let hit_cur = if self.search && !self.search_hits.is_empty() {
            self.search_hits[self.search_sel].0
        } else {
            usize::MAX
        };
        if binary {
            let msg = format!("二进制文件 · {}", fmt_size(size));
            let h = ui.available_height();
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), h),
                egui::Layout::top_down(Align::Center),
                |ui| {
                    ui.add_space(h * 0.36);
                    ui.label(RichText::new(msg).size(13.0).color(theme::muted()));
                },
            );
            return;
        }
        if is_md && preview && !self.diff_mode && !deleted {
            self.md_view(ui);
            return;
        }

        let editable = !self.diff_mode && !deleted && !truncated && !binary && !is_img;
        let wrap = self.wrap && can_wrap;
        let rev = self.rev;
        let scroll_y = self.scroll_y;
        let mono = syntax::font();
        let digit_w = ui.fonts(|f| f.glyph_width(&mono, '0'));
        let avail_h = ui.available_height();
        let avail_w = ui.available_width();

        let cache = &self.cache;
        let mut changed = false;
        let mut new_scroll = scroll_y;

        {
            let Some(cur) = self.cur.as_mut() else { return };
            let diff = self.diff_mode;
            let lang = fsops::detect_lang(&cur.path);
            let gen_lines = if diff {
                cur.diff_lines.len()
            } else {
                cur.gen_lines
            };
            let digits = gen_lines.max(1).to_string().len();
            let gutter_w = (digits as f32 * digit_w + 26.0).max(52.0);

            let mut edit_buf = if editable {
                Some(std::mem::take(&mut cur.text))
            } else {
                None
            };

            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                let (grect, _gresp) =
                    ui.allocate_exact_size(Vec2::new(gutter_w, avail_h), Sense::hover());
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(grect)
                        .layout(egui::Layout::top_down(Align::Min)),
                );
                child.set_clip_rect(grect);
                let gutter_text = if diff {
                    cur.diff_gutter.as_str()
                } else {
                    cur.gutter.as_str()
                };
                egui::ScrollArea::vertical()
                    .id_salt(("gutter", rev))
                    .scroll_offset(Vec2::new(0.0, scroll_y))
                    .enable_scrolling(false)
                    .auto_shrink([false, false])
                    .show(&mut child, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut job = LayoutJob::default();
                        job.wrap = egui::text::TextWrapping::no_max_width();
                        job.append(
                            gutter_text,
                            0.0,
                            TextFormat {
                                font_id: mono.clone(),
                                color: theme::faint(),
                                line_height: Some(theme::line_height()),
                                ..Default::default()
                            },
                        );
                        let g = ui.fonts(|f| f.layout_job(job));
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::new(gutter_w, g.size().y + 16.0),
                            Sense::hover(),
                        );
                        ui.painter().galley(
                            egui::pos2(r.right() - 12.0 - g.size().x, r.top() + 10.0),
                            g,
                            theme::faint(),
                        );
                    });

                ui.painter().vline(
                    grect.right(),
                    grect.y_range(),
                    Stroke::new(1.0, theme::line()),
                );

                let desired = if wrap {
                    avail_w - gutter_w - 46.0
                } else {
                    f32::INFINITY
                };

                let area: egui::ScrollArea = egui::ScrollArea::both()
                    .id_salt(("code", rev))
                    .auto_shrink([false, false]);

                if let Some(buf) = edit_buf.as_mut() {
                    // 内容级缓存：内容不变时零 tokenize，输入大文件不卡
                    let edit_cache: RefCell<(String, Option<Arc<egui::Galley>>)> =
                        RefCell::new((String::new(), None));
                    let mut layouter =
                        |ui: &egui::Ui, content: &str, wrap_width: f32| -> Arc<egui::Galley> {
                            let w = if wrap { wrap_width.max(80.0) } else { 0.0 };
                            let mut c = edit_cache.borrow_mut();
                            if c.0 != content {
                                let ls = syntax::tokenize(content, lang);
                                let job = syntax::build_job(&ls, wrap, w);
                                c.1 = Some(ui.fonts(|f| f.layout_job(job)));
                                c.0 = content.to_string();
                            }
                            c.1.clone().unwrap()
                        };
                    let out = area.show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        if !hit_lines.is_empty() {
                            let lh = theme::line_height();
                            for li in &hit_lines {
                                let y = 10.0 + *li as f32 * lh;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(0.0, y),
                                        egui::vec2(4000.0, lh),
                                    ),
                                    0.0,
                                    theme::brass().gamma_multiply(0.13),
                                );
                            }
                            if hit_cur < usize::MAX {
                                let y = 10.0 + hit_cur as f32 * lh;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(0.0, y),
                                        egui::vec2(4000.0, lh),
                                    ),
                                    0.0,
                                    theme::brass().gamma_multiply(0.22),
                                );
                            }
                        }
                        ui.add(
                            egui::TextEdit::multiline(buf)
                                .font(mono.clone())
                                .code_editor()
                                .desired_width(desired)
                                .desired_rows(1)
                                .frame(false)
                                .margin(egui::Margin {
                                    left: 32.0,
                                    right: 22.0,
                                    top: 10.0,
                                    bottom: 30.0,
                                })
                                .layouter(&mut layouter),
                        )
                    });
                    changed = out.inner.changed();
                    new_scroll = out.state.offset.y;
                } else {
                    let mut layouter = |ui: &egui::Ui, _buf: &str, wrap_width: f32| {
                        let w = if wrap { wrap_width } else { 0.0 };
                        if let Some(c) = cache.borrow().as_ref() {
                            if c.rev == rev && (c.w - w).abs() < 0.5 {
                                return c.galley.clone();
                            }
                        }
                        let ls: &[syntax::Line] = if diff {
                            &cur.diff_lines
                        } else {
                            &cur.lines
                        };
                        let job = syntax::build_job(ls, wrap, w);
                        let g = ui.fonts(|f| f.layout_job(job));
                        *cache.borrow_mut() = Some(GalleyCache {
                            rev,
                            w,
                            galley: g.clone(),
                        });
                        g
                    };
                    let mut ro = RoBuf(if diff {
                        cur.diff_text.as_str()
                    } else {
                        cur.text.as_str()
                    });
                    let out = area.show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        if !hit_lines.is_empty() {
                            let lh = theme::line_height();
                            for li in &hit_lines {
                                let y = 10.0 + *li as f32 * lh;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(0.0, y),
                                        egui::vec2(4000.0, lh),
                                    ),
                                    0.0,
                                    theme::brass().gamma_multiply(0.13),
                                );
                            }
                            if hit_cur < usize::MAX {
                                let y = 10.0 + hit_cur as f32 * lh;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(0.0, y),
                                        egui::vec2(4000.0, lh),
                                    ),
                                    0.0,
                                    theme::brass().gamma_multiply(0.22),
                                );
                            }
                        }
                        ui.add(
                            egui::TextEdit::multiline(&mut ro)
                                .font(mono.clone())
                                .code_editor()
                                .desired_width(desired)
                                .desired_rows(1)
                                .frame(false)
                                .margin(egui::Margin {
                                    left: 32.0,
                                    right: 22.0,
                                    top: 10.0,
                                    bottom: 30.0,
                                })
                                .layouter(&mut layouter),
                        )
                    });
                    new_scroll = out.state.offset.y;
                }
            });

            if let Some(buf) = edit_buf {
                cur.text = buf;
            }
        }

        self.scroll_y = new_scroll;
        if changed {
            if let Some(cur) = self.cur.as_mut() {
                cur.dirty = true;
                cur.size = cur.text.len() as u64;
            }
            self.rebuild();
        }
    }

    fn statusbar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none()
            .fill(theme::bg2())
            .inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 0.0, bottom: 0.0 });
        egui::TopBottomPanel::bottom("status")
            .exact_height(STATUS_H)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let drag = ui.interact(ui.max_rect(), egui::Id::new("titlebar"), Sense::drag());
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let branch = if self.git.branch.is_empty() {
                        "-".to_string()
                    } else {
                        self.git.branch.clone()
                    };
                    ui.label(RichText::new("⑂").size(10.0).color(theme::muted()));
                    ui.label(RichText::new(branch).size(10.0).color(theme::text()).monospace());
                    ui.add_space(6.0);
                    let (dot, c) = if self.git.is_git {
                        ("●", theme::jade())
                    } else {
                        ("○", theme::faint())
                    };
                    ui.label(RichText::new(dot).size(8.0).color(c));
                    ui.label(
                        RichText::new(if self.git.is_git { "已同步" } else { "未检测到仓库" })
                            .size(10.0)
                            .color(theme::muted()),
                    );

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("Mr.7y").size(9.5).color(theme::brass()).monospace());
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(9.5)
                                .color(theme::faint())
                                .monospace(),
                        );
                        if let Some(c) = self.cur.as_ref() {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(fsops::detect_lang(&c.path).to_uppercase())
                                    .size(9.5)
                                    .color(theme::faint())
                                    .monospace(),
                            );
                            ui.add_space(8.0);
                            ui.label(RichText::new("UTF-8").size(9.5).color(theme::faint()).monospace());
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("{} 行", c.lines.len().max(1)))
                                    .size(9.5)
                                    .color(theme::faint())
                                    .monospace(),
                            );
                        }
                    });
                });
            });
    }

    fn overlays(&mut self, ctx: &egui::Context) {
        if !self.toast.is_empty() && self.toast_at.elapsed() < Duration::from_secs(3) {
            egui::Area::new("toast".into())
                .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -46.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(theme::raise())
                        .stroke(Stroke::new(1.0, theme::line2()))
                        .rounding(9.0)
                        .inner_margin(egui::Margin {
                            left: 15.0,
                            right: 15.0,
                            top: 8.0,
                            bottom: 8.0,
                        })
                        .show(ui, |ui| {
                            ui.label(RichText::new(&self.toast).size(11.5).color(theme::text()));
                        });
                });
        }

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
                            .color(theme::muted()),
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

        if let Some((pos, path, is_dir)) = self.ctx_menu.clone() {
            let root = self.root.clone();
            egui::Area::new(egui::Id::new("ctx-mask"))
                .order(egui::Order::Middle)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let r = ctx.screen_rect();
                    ui.painter().rect_filled(r, 0.0, Color32::TRANSPARENT);
                    let resp = ui.allocate_rect(r, Sense::click());
                    if resp.clicked() || resp.secondary_clicked() {
                        self.ctx_menu = None;
                    }
                });
            egui::Area::new(egui::Id::new("ctx-menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(theme::raise())
                        .stroke(Stroke::new(1.0, theme::line2()))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::same(6.0))
                        .show(ui, |ui| {
                            ui.set_width(200.0);
                            let is_md_m = !is_dir &&
                                self.git.files.get(&path).map(|v| v == "M").unwrap_or(false);
                            let mut item = |ui: &mut egui::Ui, label: &str, enabled: bool| -> bool {
                                if enabled {
                                    ui.add(
                                        egui::Button::new(
                                            RichText::new(label).size(11.5).color(theme::text()),
                                        )
                                        .frame(false)
                                        .min_size(Vec2::new(188.0, 26.0)),
                                    )
                                    .clicked()
                                } else {
                                    ui.label(RichText::new(label).size(11.5).color(theme::faint()));
                                    false
                                }
                            };
                            if !is_dir && item(ui, "打开", true) {
                                self.open(&path);
                                self.ctx_menu = None;
                            }
                            if is_dir && item(ui, "展开 / 折叠", true) {
                                if let Some(r) = &root {
                                    self.tree.toggle(r, &path);
                                }
                                self.ctx_menu = None;
                            }
                            if item(ui, "复制路径", true) {
                                ctx.copy_text(path.clone());
                                self.toast("路径已复制");
                                self.ctx_menu = None;
                            }
                            if item(ui, "在资源管理器中显示", true) {
                                if let Some(r) = &root {
                                    reveal_in_explorer(r, &path);
                                }
                                self.ctx_menu = None;
                            }
                            if is_md_m && item(ui, "撤销未提交更改", true) {
                                if let Some(r) = &root {
                                    match gitops::discard(r, &path) {
                                        Ok(s) => {
                                            self.toast(s);
                                            self.refresh_git(true);
                                            if self.cur.as_ref().map(|c| c.path == path).unwrap_or(false) {
                                                self.open(&path);
                                            }
                                        }
                                        Err(e) => self.toast(e),
                                    }
                                }
                                self.ctx_menu = None;
                            }
                        });
                });
        }

        if self.branch_menu {
            let branches = self.branches.clone();
            let cur = self.git.branch.clone();
            let resp = egui::Window::new("切换分支")
                .collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 80.0))
                .show(ctx, |ui| {
                    if branches.is_empty() {
                        ui.label(RichText::new("没有本地分支").size(11.5).color(theme::muted()));
                    }
                    for b in &branches {
                        let on = b == &cur;
                        if ui.add(egui::Button::new(RichText::new(b).size(12.0).monospace().color(if on { theme::brass() } else { theme::text() })).frame(false).min_size(Vec2::new(240.0, 26.0))).clicked() {
                            if let Some(root) = self.root.clone() {
                                match gitops::checkout(&root, b) {
                                    Ok(s) => { self.toast(s); self.refresh_git(true); }
                                    Err(e) => self.toast(e),
                                }
                            }
                            self.branch_menu = false;
                        }
                    }
                });
            if resp.is_none() { self.branch_menu = false; }
        }

        if self.show_history {
            let history = self.history.clone();
            let resp = egui::Window::new("提交历史")
                .collapsible(false).resizable(false).default_width(540.0)
                .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 80.0))
                .show(ctx, |ui| {
                    if history.is_empty() {
                        ui.label(RichText::new("没有提交记录").size(11.5).color(theme::muted()));
                        return;
                    }
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        for (h, s, d) in &history {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(h).size(10.5).color(theme::brass()).monospace());
                                ui.label(RichText::new(s).size(11.5).color(theme::text()));
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(d).size(9.5).color(theme::faint()));
                                });
                            });
                        }
                    });
                });
            if resp.is_none() { self.show_history = false; }
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
                    .rect_filled(r, 0.0, Color32::from_black_alpha(140));
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
        let y = screen.top() + 96.0;

        egui::Area::new("palette".into())
            .fixed_pos(egui::pos2(x, y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme::panel())
                    .stroke(Stroke::new(1.0, theme::line2()))
                    .rounding(12.0)
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(w);
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.pal_q)
                                .hint_text("输入文件名，回车打开")
                                .desired_width(f32::INFINITY)
                                .margin(egui::Margin::symmetric(7.0, 6.0)),
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
                                            .size(11.5)
                                            .color(theme::faint()),
                                    );
                                }
                                for (i, (path, _)) in results.iter().enumerate() {
                                    let sel = i == self.pal_sel;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), 26.0),
                                        Sense::click(),
                                    );
                                    let p = ui.painter();
                                    if sel {
                                        p.rect_filled(rect, 6.0, theme::sel());
                                    } else if resp.hovered() {
                                        p.rect_filled(rect, 6.0, theme::hover());
                                    }
                                    let (dir, name) = match path.rfind('/') {
                                        Some(k) => (&path[..k + 1], &path[k + 1..]),
                                        None => ("", path.as_str()),
                                    };
                                    p.text(
                                        egui::pos2(rect.left() + 8.0, rect.center().y),
                                        Align2::LEFT_CENTER,
                                        name,
                                        FontId::proportional(12.0),
                                        theme::text(),
                                    );
                                    let nw = p
                                        .layout_no_wrap(
                                            name.to_string(),
                                            FontId::proportional(12.0),
                                            theme::text(),
                                        )
                                        .size()
                                        .x;
                                    p.text(
                                        egui::pos2(rect.left() + 14.0 + nw, rect.center().y),
                                        Align2::LEFT_CENTER,
                                        dir,
                                        FontId::proportional(10.0),
                                        theme::faint(),
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

    fn input(&mut self, ctx: &egui::Context) {
        let cmd = ctx.input(|i| i.modifiers.command);
        if cmd && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            self.save_cur();
        }
        if cmd && ctx.input(|i| i.key_pressed(egui::Key::F)) && self.cur.is_some() && !self.palette {
            self.search = !self.search;
            self.search_sel = 0;
        }
        if self.search && self.cur.is_some() {
            let down = ctx.input(|i| i.key_pressed(egui::Key::F3))
                || (cmd && ctx.input(|i| i.key_pressed(egui::Key::Enter)));
            let up = ctx.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::F3));
            if down { self.search_step(1); }
            if up { self.search_step(-1); }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.search = false;
            }
        }
        if cmd && ctx.input(|i| i.key_pressed(egui::Key::P)) && !self.palette {
            if !self.conf.projects.is_empty() {
                self.ensure_files();
                self.palette = true;
                self.pal_q.clear();
                self.pal_sel = 0;
            }
            return;
        }
        if cmd && ctx.input(|i| i.key_pressed(egui::Key::B)) {
            self.left_open = !self.left_open;
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
        self.gitbar(ctx);
        self.side_panels(ctx);
        self.center(ctx);
        self.overlays(ctx);

        #[cfg(debug_assertions)]
        {
            crate::shot::want(ctx);
            if let Some((p, size)) = crate::shot::take(ctx) {
                self.toast(format!("截图 {}x{} → {}", size.0, size.1, p.display()));
            }
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for p in dropped {
            if p.is_file() {
                if let Some(root) = &self.root {
                    match p.strip_prefix(root) {
                        Ok(rel) => {
                            let rel = rel.to_string_lossy().replace('\\', "/");
                            self.open(&rel);
                        }
                        Err(_) => self.toast("文件不在当前项目内，请先打开其所在目录"),
                    }
                }
            } else if p.is_dir() {
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
                    p.rect_stroke(r.shrink(9.0), 8.0, Stroke::new(2.0, theme::brass()));
                    p.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        "松手即可添加项目",
                        FontId::proportional(18.0),
                        theme::text(),
                    );
                });
        }
    }
}

fn brand(ui: &mut egui::Ui) {
    let mut job = LayoutJob::default();
    let f = FontId::proportional(14.5);
    job.append(
        "Ai",
        0.0,
        TextFormat {
            font_id: f.clone(),
            color: theme::brass(),
            ..Default::default()
        },
    );
    job.append(
        "-View",
        0.0,
        TextFormat {
            font_id: f,
            color: theme::text(),
            ..Default::default()
        },
    );
    ui.label(job);
}

fn logo(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), Sense::hover());
    logo_at(ui.painter(), rect);
}

fn logo_at(p: &egui::Painter, rect: egui::Rect) {
    let size = rect.width();
    p.rect_filled(rect, 7.0, theme::brass());
    let hi = egui::Rect::from_min_size(rect.min, Vec2::new(size, size * 0.46));
    p.rect_filled(hi, 7.0, Color32::from_white_alpha(16));
    let w = size * 0.085;
    let h = size * 0.5;
    let gap = size * 0.208;
    let x0 = rect.center().x - gap - w / 2.0;
    let y0 = rect.center().y - h / 2.0;
    for i in 0..3 {
        let a = [205u8, 150, 100][i];
        let c = Color32::from_rgba_unmultiplied(20, 15, 8, a);
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x0 + i as f32 * gap, y0), Vec2::new(w, h)),
            w / 2.0,
            c,
        );
    }
}

fn cmd_bar(ui: &mut egui::Ui, width: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
    let p = ui.painter();
    let border = if resp.hovered() {
        theme::line2()
    } else {
        theme::line()
    };
    p.rect(rect, 9.0, theme::panel2(), Stroke::new(1.0, border));
    p.text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        "搜索文件、打开命令面板",
        FontId::proportional(11.0),
        theme::faint(),
    );
    p.text(
        egui::pos2(rect.right() - 12.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "Ctrl K",
        FontId::monospace(9.5),
        theme::faint(),
    );
    resp.clicked()
}

fn icon_btn(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let color = if on { theme::brass() } else { theme::muted() };
    let b = egui::Button::new(RichText::new(text).size(13.0).color(color))
        .frame(false)
        .min_size(Vec2::new(26.0, 26.0));
    ui.add(b)
}

fn avatar(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 13.0, theme::brass_bg());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "7y",
        FontId::proportional(10.0),
        theme::brass(),
    );
}

fn pill_btn(ui: &mut egui::Ui, icon: &str, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        format!("{}  {}", icon, text),
        FontId::proportional(11.0),
        theme::muted(),
    );
    let pad = Vec2::new(9.0, 5.0);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    let hover = resp.hovered();
    ui.painter().rect(
        rect,
        8.0,
        if hover { theme::panel2() } else { theme::panel() },
        Stroke::new(1.0, if hover { theme::line2() } else { theme::line() }),
    );
    ui.painter().galley(
        egui::pos2(rect.left() + pad.x, rect.center().y - galley.size().y / 2.0),
        galley,
        if hover { theme::text() } else { theme::muted() },
    );
    resp
}

fn graph(ui: &mut egui::Ui, ahead: usize, behind: usize) -> egui::Response {
    let w = 132.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    let p = ui.painter();
    let y = rect.center().y;
    p.line_segment(
        [
            egui::pos2(rect.left() + 6.0, y),
            egui::pos2(rect.right() - 6.0, y),
        ],
        Stroke::new(1.4, theme::line2()),
    );
    let n = (ahead + behind + 4).min(7);
    for i in 0..n {
        let t = (i as f32 + 1.0) / (n as f32 + 1.0);
        let x = rect.left() + 6.0 + t * (w - 12.0);
        let c = if behind > 0 && i + 1 > n - behind {
            theme::jade()
        } else {
            theme::brass()
        };
        p.circle_filled(egui::pos2(x, y), 3.0, c);
    }
    let c = egui::pos2(rect.right() - 6.0, y);
    p.circle(c, 4.0, theme::bg2(), Stroke::new(1.4, theme::brass()));
    resp
}

fn ghost(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(11.0).color(theme::muted()))
            .frame(true)
            .rounding(6.0)
            .min_size(Vec2::new(0.0, 24.0)),
    )
}

fn seg(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let (fill, color) = if on {
        (theme::brass_bg(), theme::brass())
    } else {
        (Color32::TRANSPARENT, theme::muted())
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.0), color);
    let pad = Vec2::new(11.0, 5.0);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    ui.painter().rect(
        rect,
        7.0,
        fill,
        Stroke::new(1.0, if on { theme::brass() } else { theme::line() }),
    );
    ui.painter().galley(
        egui::pos2(rect.left() + pad.x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    resp
}

fn tab_btn(ui: &mut egui::Ui, text: &str, on: bool) -> bool {
    let color = if on { theme::text() } else { theme::muted() };
    let btn = egui::Button::new(RichText::new(text).size(11.5).color(color))
        .frame(false)
        .min_size(Vec2::new(0.0, 24.0));
    let r = ui.add(btn);
    if on {
        let rect = r.rect;
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + 2.0, rect.bottom() - 1.0),
                egui::pos2(rect.right() - 2.0, rect.bottom() + 1.0),
            ),
            1.0,
            theme::brass(),
        );
    }
    r.clicked()
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(9.5), color);
    let pad = Vec2::new(7.0, 2.0);
    let (rect, _) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::hover());
    ui.painter().rect_filled(rect, 4.0, color.gamma_multiply(0.16));
    ui.painter().galley(
        egui::pos2(rect.left() + pad.x, rect.top() + pad.y),
        galley,
        color,
    );
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(RichText::new(k).size(10.0).color(theme::faint()));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(v)
                    .size(10.5)
                    .color(theme::muted())
                    .monospace(),
            );
        });
    });
}

fn rule(ui: &mut egui::Ui, left: f32) {
    ui.horizontal(|ui| {
        ui.add_space(left);
        let (r, _) = ui.allocate_exact_size(
            Vec2::new((ui.available_width() - left).max(40.0), 1.0),
            Sense::hover(),
        );
        ui.painter()
            .hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::line()));
    });
}

fn can_wrap_of(cur: &Option<Cur>) -> bool {
    cur.as_ref().map(|c| c.can_wrap).unwrap_or(true)
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

fn fuzzy_score(needle: &str, hay: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let n: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();
    let h: Vec<char> = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut hi = 0;
    let mut score = 0i32;
    let mut run = 0i32;
    for nc in n.iter() {
        let mut found = false;
        while hi < h.len() {
            if h[hi] == *nc {
                score += 10 + run * 6;
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

fn sync_chip(ui: &mut egui::Ui, ago: std::time::Duration, is_git: bool) {
    let (text, color) = if is_git {
        let secs = ago.as_secs();
        let t = if secs < 60 {
            format!("已同步  {} 秒前", secs)
        } else {
            format!("已同步  {} 分钟前", secs / 60)
        };
        (t, theme::jade())
    } else {
        ("未连接".to_string(), theme::faint())
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text, FontId::proportional(10.0), theme::muted());
    let pad = Vec2::new(10.0, 5.0);
    let (rect, _) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::hover());
    ui.painter().rect(
        rect,
        999.0,
        theme::panel2(),
        Stroke::new(1.0, theme::line()),
    );
    ui.painter()
        .circle_filled(egui::pos2(rect.left() + pad.x + 3.0, rect.center().y), 2.5, color);
    ui.painter().galley(
        egui::pos2(rect.left() + pad.x + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        theme::muted(),
    );
}

fn seg_w(ui: &mut egui::Ui, text: &str, on: bool, width: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
    let (fill, color) = if on {
        (theme::brass_bg(), theme::brass())
    } else {
        (Color32::TRANSPARENT, theme::muted())
    };
    ui.painter().rect(
        rect,
        7.0,
        fill,
        Stroke::new(1.0, if on { theme::brass() } else { theme::line() }),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.0),
        color,
    );
    resp
}

fn row_label(ui: &mut egui::Ui, left: &str, right: &str) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(RichText::new(left).size(10.0).color(theme::faint()));
        if !right.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                ui.label(RichText::new(right).size(9.5).color(theme::faint()));
            });
        }
    });
}

fn fake_select(ui: &mut egui::Ui, text: &str) {
    let w = (ui.available_width() - 28.0).max(80.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
        ui.painter().rect(rect, 7.0, theme::panel2(), Stroke::new(1.0, theme::line()));
        ui.painter().text(
            egui::pos2(rect.left() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(11.0),
            theme::text(),
        );
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            "v",
            FontId::proportional(9.0),
            theme::faint(),
        );
    });
}

fn slider(ui: &mut egui::Ui, t: f32) {
    let w = (ui.available_width() - 28.0).max(80.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
        let y = rect.center().y;
        let x0 = rect.left() + 2.0;
        let x1 = rect.right() - 2.0;
        ui.painter().line_segment(
            [egui::pos2(x0, y), egui::pos2(x1, y)],
            Stroke::new(2.0, theme::line2()),
        );
        let x = x0 + (x1 - x0) * t.clamp(0.0, 1.0);
        ui.painter().line_segment(
            [egui::pos2(x0, y), egui::pos2(x, y)],
            Stroke::new(2.0, theme::brass()),
        );
        ui.painter().circle_filled(egui::pos2(x, y), 5.0, theme::brass());
    });
}

fn toggle(ui: &mut egui::Ui, on: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(30.0, 16.0), Sense::hover());
    let bg = if on { theme::brass() } else { theme::line2() };
    ui.painter().rect_filled(rect, 8.0, bg);
    let cx = if on { rect.right() - 8.0 } else { rect.left() + 8.0 };
    ui.painter().circle_filled(egui::pos2(cx, rect.center().y), 5.5, theme::panel());
}

fn pill_tab(ui: &mut egui::Ui, text: &str, on: bool) -> bool {
    let color = if on { theme::text() } else { theme::muted() };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.0), color);
    let pad = Vec2::new(9.0, 5.0);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    if on {
        ui.painter().rect_filled(rect, 6.0, theme::panel2());
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme::hover());
    }
    ui.painter().galley(
        egui::pos2(rect.left() + pad.x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    resp.clicked()
}

fn win_btn(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(46.0, 30.0), Sense::click());
    if resp.hovered() {
        let c = if text == "×" { theme::rose() } else { theme::hover() };
        ui.painter().rect_filled(rect, 6.0, c);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.0),
        if resp.hovered() { theme::text() } else { theme::muted() },
    );
    resp
}

fn icon_panel(ui: &mut egui::Ui, side: u8, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), Sense::click());
    let p = ui.painter();
    let r = rect.shrink(5.0);
    p.rect_stroke(r, 3.0, Stroke::new(1.2, theme::muted()));
    let bw = r.width() * 0.4;
    let bar = if side == 0 {
        egui::Rect::from_min_size(r.min, Vec2::new(bw, r.height()))
    } else {
        egui::Rect::from_min_max(
            egui::pos2(r.right() - bw, r.top()),
            egui::pos2(r.right(), r.bottom()),
        )
    };
    let col = if on { theme::brass() } else { theme::muted().gamma_multiply(0.55) };
    p.rect_filled(bar, 1.5, col);
    resp
}

fn icon_panels2(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), Sense::click());
    let p = ui.painter();
    let r = rect.shrink(5.0);
    p.rect_stroke(r, 3.0, Stroke::new(1.2, theme::muted()));
    let bw = r.width() * 0.3;
    let col = if on { theme::brass() } else { theme::muted().gamma_multiply(0.55) };
    p.rect_filled(egui::Rect::from_min_size(r.min, Vec2::new(bw, r.height())), 1.5, col);
    p.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(r.right() - bw, r.top()),
            egui::pos2(r.right(), r.bottom()),
        ),
        1.5,
        col,
    );
    resp
}

fn icon_theme(ui: &mut egui::Ui, night: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    if night {
        p.circle_filled(c, 7.0, theme::brass());
        p.circle_filled(c + Vec2::new(4.0, -2.5), 6.2, theme::panel());
    } else {
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            let d1 = 6.2;
            let d2 = 9.2;
            p.line_segment(
                [
                    c + Vec2::new(a.cos() * d1, a.sin() * d1),
                    c + Vec2::new(a.cos() * d2, a.sin() * d2),
                ],
                Stroke::new(1.3, theme::brass()),
            );
        }
        p.circle_filled(c, 4.5, theme::brass());
    }
    resp
}

fn search_hits_of(text: &str, q: &str) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    if q.is_empty() {
        return out;
    }
    let ql = q.to_lowercase();
    for (li, line) in text.split('\n').enumerate() {
        let ll = line.to_lowercase();
        let mut from = 0usize;
        while let Some(p) = ll[from..].find(&ql) {
            let s = from + p;
            out.push((li, s, s + ql.len()));
            from = s + ql.len();
        }
    }
    out
}

fn reveal_in_explorer(root: &Path, rel: &str) {
    let full = root.join(rel);
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", full.display()))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(&full).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let dir = full.parent().map(|p| p.to_path_buf()).unwrap_or(full.clone());
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}
