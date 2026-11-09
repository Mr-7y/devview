//! 极简 Markdown 预览：块级解析 + egui 原生绘制，零依赖。
//!
//! 只覆盖阅读时真正会用到的语法：标题、段落、列表、引用、
//! 围栏代码、分隔线、表格，以及行内的 `code`、**粗体**、*斜体*、
//! ~~删除线~~ 和 [链接](url)。

use crate::theme;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Frame, Margin, RichText, Stroke, Ui, Vec2};

#[derive(Clone)]
pub enum Block {
    Heading(u8, String),
    Para(String),
    Quote(Vec<String>),
    Code(String, String, Vec<crate::syntax::Line>),
    Bullet {
        indent: usize,
        order: Option<usize>,
        text: String,
    },
    Hr,
    Table(Vec<Vec<String>>),
}

/// 把源码切成块。行级语义，不做嵌套解析。
pub fn parse(src: &str) -> Vec<Block> {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let raw = lines[i];
        let t = raw.trim_end();
        let s = t.trim_start();
        if s.is_empty() {
            i += 1;
            continue;
        }

        // 围栏代码
        if s.starts_with("~~~") || s.starts_with("```") {
            let fence: String = s.chars().take(3).collect();
            let lang = s[3..].trim().to_string();
            i += 1;
            let mut buf = String::new();
            while i < lines.len() && !lines[i].trim_start().starts_with(&fence) {
                buf.push_str(lines[i]);
                buf.push('\n');
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            let lines =
                crate::syntax::tokenize(&buf, if lang.is_empty() { "plaintext" } else { &lang });
            out.push(Block::Code(lang, buf, lines));
            continue;
        }

        // 分隔线
        if is_hr(s) {
            out.push(Block::Hr);
            i += 1;
            continue;
        }

        // 标题
        if let Some((lvl, rest)) = heading(s) {
            out.push(Block::Heading(lvl, rest.to_string()));
            i += 1;
            continue;
        }

        // 表格
        if s.starts_with('|') && i + 1 < lines.len() && is_table_sep(lines[i + 1]) {
            let mut rows = Vec::new();
            rows.push(split_row(s));
            i += 2;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(split_row(lines[i].trim()));
                i += 1;
            }
            out.push(Block::Table(rows));
            continue;
        }

        // 引用
        if s.starts_with('>') {
            let mut buf = Vec::new();
            while i < lines.len() {
                let q = lines[i].trim_start();
                if !q.starts_with('>') {
                    break;
                }
                buf.push(q.trim_start_matches('>').trim_start().to_string());
                i += 1;
            }
            out.push(Block::Quote(buf));
            continue;
        }

        // 列表
        if let Some((indent, order, text)) = bullet(s) {
            out.push(Block::Bullet {
                indent,
                order,
                text: text.to_string(),
            });
            i += 1;
            continue;
        }

        // 段落：连续非空行合并
        let mut buf = String::new();
        while i < lines.len() {
            let l = lines[i].trim_end();
            let ls = l.trim_start();
            if ls.is_empty()
                || heading(ls).is_some()
                || is_hr(ls)
                || ls.starts_with('>')
                || ls.starts_with("```")
                || ls.starts_with("~~~")
                || bullet(ls).is_some()
                || ls.starts_with('|')
            {
                break;
            }
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(ls);
            i += 1;
        }
        if buf.is_empty() {
            i += 1;
        } else {
            out.push(Block::Para(buf));
        }
    }
    out
}

fn is_hr(s: &str) -> bool {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3
        && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

fn heading(s: &str) -> Option<(u8, &str)> {
    let n = s.chars().take_while(|c| *c == '#').count();
    if n == 0 || n > 6 {
        return None;
    }
    let rest = &s[n..];
    if !rest.starts_with(' ') {
        return None;
    }
    Some((n as u8, rest.trim_start()))
}

fn bullet(s: &str) -> Option<(usize, Option<usize>, &str)> {
    let indent = s.len() - s.trim_start_matches([' ', '\t']).len();
    let t = s.trim_start();
    for m in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(m) {
            return Some((indent / 2, None, rest.trim()));
        }
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &t[digits.len()..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some((indent / 2, digits.parse().ok(), r.trim()));
        }
    }
    None
}

fn is_table_sep(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('|')
        && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
        && t.contains('-')
}

fn split_row(s: &str) -> Vec<String> {
    s.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

/* ---------------------------------------------------------------- 行内 */

/// 行内格式 → LayoutJob：支持 `code`、**粗**、*斜*、~~删~~、[文字](url)
fn inline(text: &str, size: f32, color: Color32, width: f32) -> LayoutJob {
    let mono = FontId::monospace(size - 0.5);
    let body = FontId::proportional(size);
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    job.wrap.break_anywhere = true;

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut buf = String::new();

    let flush = |buf: &mut String, job: &mut LayoutJob| {
        if !buf.is_empty() {
            job.append(
                buf,
                0.0,
                TextFormat {
                    font_id: body.clone(),
                    color,
                    ..Default::default()
                },
            );
            buf.clear();
        }
    };

    while i < chars.len() {
        let ch = chars[i];
        // 行内代码
        if ch == '`' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '`' {
                j += 1;
            }
            if j < chars.len() {
                flush(&mut buf, &mut job);
                let s: String = chars[i + 1..j].iter().collect();
                job.append(
                    &s,
                    0.0,
                    TextFormat {
                        font_id: mono.clone(),
                        color: theme::brass(),
                        background: theme::brass_bg(),
                        ..Default::default()
                    },
                );
                i = j + 1;
                continue;
            }
        }
        // **strong**
        if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(j) = find_pair(&chars, i + 2, '*', '*') {
                flush(&mut buf, &mut job);
                let s: String = chars[i + 2..j].iter().collect();
                job.append(
                    &s,
                    0.0,
                    TextFormat {
                        font_id: body.clone(),
                        color: theme::text(),
                        ..Default::default()
                    },
                );
                i = j + 2;
                continue;
            }
        }
        // ~~del~~
        if ch == '~' && i + 1 < chars.len() && chars[i + 1] == '~' {
            if let Some(j) = find_pair(&chars, i + 2, '~', '~') {
                flush(&mut buf, &mut job);
                let s: String = chars[i + 2..j].iter().collect();
                job.append(
                    &s,
                    0.0,
                    TextFormat {
                        font_id: body.clone(),
                        color: theme::faint(),
                        strikethrough: Stroke::new(1.0, theme::faint()),
                        ..Default::default()
                    },
                );
                i = j + 2;
                continue;
            }
        }
        // *em*
        if ch == '*' {
            if let Some(j) = find_single(&chars, i + 1, '*') {
                if j > i + 1 {
                    flush(&mut buf, &mut job);
                    let s: String = chars[i + 1..j].iter().collect();
                    job.append(
                        &s,
                        0.0,
                        TextFormat {
                            font_id: body.clone(),
                            color,
                            italics: true,
                            ..Default::default()
                        },
                    );
                    i = j + 1;
                    continue;
                }
            }
        }
        // [text](url)
        if ch == '[' {
            if let Some(close) = chars[i..].iter().position(|c| *c == ']').map(|p| i + p) {
                if close + 1 < chars.len() && chars[close + 1] == '(' {
                    if let Some(end) = chars[close..]
                        .iter()
                        .position(|c| *c == ')')
                        .map(|p| close + p)
                    {
                        flush(&mut buf, &mut job);
                        let s: String = chars[i + 1..close].iter().collect();
                        job.append(
                            &s,
                            0.0,
                            TextFormat {
                                font_id: body.clone(),
                                color: theme::brass(),
                                underline: Stroke::new(1.0, theme::brass()),
                                ..Default::default()
                            },
                        );
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        buf.push(ch);
        i += 1;
    }
    flush(&mut buf, &mut job);
    job
}

fn find_pair(c: &[char], from: usize, a: char, b: char) -> Option<usize> {
    let mut i = from;
    while i + 1 < c.len() {
        if c[i] == a && c[i + 1] == b {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_single(c: &[char], from: usize, a: char) -> Option<usize> {
    (from..c.len()).find(|i| c[*i] == a)
}

/* ---------------------------------------------------------------- 渲染 */

pub fn render(ui: &mut Ui, blocks: &[Block]) {
    let w = ui.available_width();
    for b in blocks {
        match b {
            Block::Heading(lvl, text) => {
                let size = match lvl {
                    1 => 25.0,
                    2 => 21.0,
                    3 => 17.5,
                    4 => 15.5,
                    5 => 14.0,
                    _ => 13.0,
                };
                ui.add_space(if *lvl <= 2 { 14.0 } else { 9.0 });
                if *lvl <= 2 {
                    ui.horizontal(|ui| {
                        let (r, _) = ui
                            .allocate_exact_size(Vec2::new(3.0, size * 0.92), egui::Sense::hover());
                        ui.painter().rect_filled(r, 2.0, theme::brass());
                        ui.add_space(8.0);
                        ui.label(RichText::new(text).size(size).color(theme::text()).strong());
                    });
                } else {
                    ui.label(RichText::new(text).size(size).color(theme::text()).strong());
                }
                ui.add_space(4.0);
            }
            Block::Para(text) => {
                let job = inline(text, 14.0, theme::text(), w);
                ui.label(job);
                ui.add_space(6.0);
            }
            Block::Quote(lines) => {
                let text = lines.join("\n");
                ui.horizontal_top(|ui| {
                    let h = (lines.len() as f32 * 21.0).max(18.0);
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::new(3.0, h), egui::Sense::hover());
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(r.min, Vec2::new(3.0, h)),
                        2.0,
                        theme::line2(),
                    );
                    ui.add_space(9.0);
                    ui.label(
                        RichText::new(text)
                            .size(13.5)
                            .color(theme::muted())
                            .italics(),
                    );
                });
                ui.add_space(8.0);
            }
            Block::Code(lang, code, lines) => {
                ui.add_space(4.0);
                Frame::none()
                    .fill(theme::panel2())
                    .stroke(Stroke::new(1.0, theme::line()))
                    .rounding(9.0)
                    .inner_margin(Margin::symmetric(14.0, 11.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if !lang.is_empty() {
                            ui.label(
                                RichText::new(lang.to_uppercase())
                                    .size(9.5)
                                    .color(theme::faint())
                                    .monospace(),
                            );
                            ui.add_space(4.0);
                        }
                        let job = crate::syntax::build_job(lines, false, ui.available_width());
                        egui::ScrollArea::horizontal()
                            .id_salt(("mdcode", code.as_ptr() as usize, code.len()))
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.label(job);
                            });
                    });
                ui.add_space(8.0);
            }
            Block::Bullet {
                indent,
                order,
                text,
            } => {
                ui.horizontal_top(|ui| {
                    ui.add_space(*indent as f32 * 18.0 + 4.0);
                    let marker = match order {
                        Some(n) => format!("{}.", n),
                        None => "•".to_string(),
                    };
                    ui.label(
                        RichText::new(marker)
                            .size(13.0)
                            .color(theme::brass())
                            .monospace(),
                    );
                    ui.add_space(2.0);
                    let ww = ui.available_width().max(80.0);
                    ui.label(inline(text, 14.0, theme::text(), ww));
                });
                ui.add_space(2.0);
            }
            Block::Hr => {
                ui.add_space(9.0);
                let (r, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 1.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::line()));
                ui.add_space(9.0);
            }
            Block::Table(rows) => {
                if rows.is_empty() {
                    continue;
                }
                let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
                let cw = ((w - 12.0) / cols as f32).max(60.0);
                ui.add_space(4.0);
                egui::Grid::new(("mdtable", rows.len(), cols))
                    .striped(true)
                    .min_col_width(cw)
                    .spacing(Vec2::new(10.0, 6.0))
                    .show(ui, |ui| {
                        for (ri, row) in rows.iter().enumerate() {
                            for c in 0..cols {
                                let cell = row.get(c).map(|s| s.as_str()).unwrap_or("");
                                let strong = ri == 0;
                                ui.label(
                                    RichText::new(cell)
                                        .size(12.5)
                                        .color(if strong {
                                            theme::text()
                                        } else {
                                            theme::muted()
                                        })
                                        .strong(),
                                );
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
            }
        }
    }
}

