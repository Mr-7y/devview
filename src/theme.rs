//! 配色与字体：Ai-View 视觉体系（夜航 / 日纸双主题）。
//!
//! 颜色一律走函数而不是常量，切换主题只改一个原子标志，
//! 下一次重绘即可整体换肤。

use crate::syntax;
use egui::{Color32, FontFamily, FontId, TextStyle};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::OnceLock;

static LIGHT: AtomicBool = AtomicBool::new(false);

// 代码字号（x10）/ 行高倍率（x100）/ 代码字体选择
static CODE_SIZE: AtomicU32 = AtomicU32::new(150);
static CODE_RATIO: AtomicU32 = AtomicU32::new(190);
static CODE_FONT: AtomicU8 = AtomicU8::new(0);
static ALT_FONTS: OnceLock<Vec<String>> = OnceLock::new();

pub fn code_size() -> f32 { CODE_SIZE.load(Ordering::Relaxed) as f32 / 10.0 }
pub fn set_code_size(v: f32) { CODE_SIZE.store((v * 10.0).round() as u32, Ordering::Relaxed); }
pub fn code_ratio() -> f32 { CODE_RATIO.load(Ordering::Relaxed) as f32 / 100.0 }
pub fn set_code_ratio(v: f32) { CODE_RATIO.store((v * 100.0).round() as u32, Ordering::Relaxed); }
pub fn line_height() -> f32 { code_size() * code_ratio() }
pub fn code_font() -> u8 { CODE_FONT.load(Ordering::Relaxed) }
pub fn set_code_font(v: u8) { CODE_FONT.store(v, Ordering::Relaxed); }
pub fn alt_fonts() -> &'static [String] { ALT_FONTS.get_or_init(Vec::new) }

pub fn light() -> bool {
    LIGHT.load(Ordering::Relaxed)
}

pub fn set_light(v: bool) {
    LIGHT.store(v, Ordering::Relaxed);
}

pub fn toggle() -> bool {
    let v = !light();
    set_light(v);
    v
}

/* ---------------------------------------------------------------- 基础色 */

fn c(dark: u32, bright: u32) -> Color32 {
    let v = if light() { bright } else { dark };
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn bg() -> Color32 {
    c(0x0c0e11, 0xf4f1ea)
}
pub fn bg2() -> Color32 {
    c(0x090a0d, 0xeae6dc)
}
pub fn panel() -> Color32 {
    c(0x111419, 0xfbf9f4)
}
pub fn panel2() -> Color32 {
    c(0x171b21, 0xf2eee5)
}
pub fn raise() -> Color32 {
    c(0x1c2129, 0xfffdf8)
}
pub fn code_bg() -> Color32 {
    c(0x0c0e11, 0xf7f4ed)
}

pub fn text() -> Color32 {
    c(0xe9ebef, 0x1d1f23)
}
pub fn muted() -> Color32 {
    c(0x8d95a2, 0x6c7077)
}
pub fn faint() -> Color32 {
    c(0x5b6370, 0x9a9da3)
}

pub fn line() -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(29, 31, 35, 22)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 17)
    }
}
pub fn line2() -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(29, 31, 35, 43)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 33)
    }
}

pub fn brass() -> Color32 {
    c(0xd3a95f, 0x94662a)
}
pub fn brass2() -> Color32 {
    c(0xefd09a, 0x7a5220)
}
pub fn brass_bg() -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(148, 102, 42, 19)
    } else {
        Color32::from_rgba_unmultiplied(211, 169, 95, 26)
    }
}
pub fn on_brass() -> Color32 {
    c(0x191307, 0xfffaf0)
}

pub fn jade() -> Color32 {
    c(0x63c39a, 0x2f7a58)
}
pub fn rose() -> Color32 {
    c(0xdf746c, 0xb25049)
}

pub fn sel() -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(148, 102, 42, 26)
    } else {
        Color32::from_rgba_unmultiplied(211, 169, 95, 19)
    }
}
pub fn sel2() -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(148, 102, 42, 56)
    } else {
        Color32::from_rgba_unmultiplied(211, 169, 95, 51)
    }
}
pub fn hover() -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(29, 31, 35, 12)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 12)
    }
}

/// 代码 token 颜色
pub fn tok(t: u8) -> Color32 {
    let d = match t {
        syntax::K => 0xddaa6a,
        syntax::S => 0xacc994,
        syntax::N => 0xd0ba7e,
        syntax::C => 0x7d866f,
        syntax::Y => 0x8abfb3,
        syntax::F => 0xe6d6b0,
        syntax::A | syntax::V => 0xb99ac4,
        syntax::G => 0xddaa6a,
        syntax::I => 0x8fd6b0,
        syntax::D => 0xe39b93,
        syntax::H => 0xd3a95f,
        _ => 0xe9ebef,
    };
    let l = match t {
        syntax::K => 0x84501f,
        syntax::S => 0x47643a,
        syntax::N => 0x75551a,
        syntax::C => 0x787565,
        syntax::Y => 0x2f6b62,
        syntax::F => 0x6d4c7c,
        syntax::A | syntax::V => 0x7c5a7f,
        syntax::G => 0x84501f,
        syntax::I => 0x2f7a58,
        syntax::D => 0xb25049,
        syntax::H => 0x94662a,
        _ => 0x1d1f23,
    };
    c(d, l)
}

/// diff 行的底色
pub fn tok_bg(t: u8) -> Color32 {
    match t {
        syntax::I => {
            if light() {
                Color32::from_rgba_unmultiplied(47, 122, 88, 22)
            } else {
                Color32::from_rgba_unmultiplied(99, 195, 154, 20)
            }
        }
        syntax::D => {
            if light() {
                Color32::from_rgba_unmultiplied(178, 80, 73, 22)
            } else {
                Color32::from_rgba_unmultiplied(223, 116, 108, 20)
            }
        }
        _ => Color32::TRANSPARENT,
    }
}

/* ---------------------------------------------------------------- git 标记 */

pub fn git_color(s: &str) -> Color32 {
    match s {
        "M" => brass(),
        "A" => jade(),
        "D" => rose(),
        "U" => jade(),
        _ => muted(),
    }
}

/* ---------------------------------------------------------------- 字体 */

fn cjk() -> Option<(&'static str, Vec<u8>, u32)> {
    #[cfg(target_os = "windows")]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let base = std::path::PathBuf::from(root).join("Fonts");
        for (f, idx) in [
            ("msyh.ttc", 0u32),
            ("msyhl.ttc", 0),
            ("simhei.ttf", 0),
            ("simsun.ttc", 0),
        ] {
            let p = base.join(f);
            if let Ok(b) = std::fs::read(&p) {
                return Some((f, b, idx));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for (p, idx) in [
            ("/System/Library/Fonts/PingFang.ttc", 0u32),
            ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
            ("/Library/Fonts/Arial Unicode.ttf", 0),
            ("/System/Library/Fonts/STHeiti Light.ttc", 0),
        ] {
            if let Ok(b) = std::fs::read(p) {
                return Some(("cjk", b, idx));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for (p, idx) in [
            ("/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf", 0u32),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
            ("/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf", 0),
            ("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 0),
            ("/usr/share/fonts/truetype/arphic/uming.ttc", 0),
        ] {
            if let Ok(b) = std::fs::read(p) {
                return Some(("cjk", b, idx));
            }
        }
    }
    None
}

fn mono_file() -> Option<(&'static str, Vec<u8>)> {
    #[cfg(target_os = "windows")]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let base = std::path::PathBuf::from(root).join("Fonts");
        for f in ["consola.ttf", "CascadiaMono.ttf", "lucon.ttf"] {
            let p = base.join(f);
            if let Ok(b) = std::fs::read(&p) {
                return Some(("mono-ui", b));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for p in [
            "/System/Library/Fonts/SFNSMono.ttf",
            "/System/Library/Fonts/Menlo.ttc",
        ] {
            if let Ok(b) = std::fs::read(p) {
                return Some(("mono-ui", b));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for p in [
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
        ] {
            if let Ok(b) = std::fs::read(p) {
                return Some(("mono-ui", b));
            }
        }
    }
    None
}

pub fn setup(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    if let Some((name, bytes)) = mono_file() {
        fonts
            .font_data
            .insert(name.to_owned(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, name.to_owned());
    }
    // 可选代码字体：存在才注册，供外观面板切换
    let mut loaded: Vec<String> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:".to_string() + &String::from("\\").repeat(1) + "Windows");
        let fdir = std::path::PathBuf::from(windir).join("Fonts");
        for (f, fam) in [("consola.ttf", "Consolas"), ("CascadiaMono.ttf", "Cascadia Mono"), ("CascadiaCode.ttf", "Cascadia Code")] {
            if let Ok(b) = std::fs::read(fdir.join(f)) {
                fonts.font_data.insert(fam.to_string(), egui::FontData::from_owned(b));
                fonts.families.entry(FontFamily::Name(fam.to_string().into())).or_default().push(fam.to_string());
                loaded.push(fam.to_string());
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for (p, fam) in [("/System/Library/Fonts/Menlo.ttc", "Menlo"), ("/System/Library/Fonts/SFNSMono.ttf", "SF Mono")] {
            if let Ok(b) = std::fs::read(p) {
                fonts.font_data.insert(fam.to_string(), egui::FontData::from_owned(b));
                fonts.families.entry(FontFamily::Name(fam.to_string().into())).or_default().push(fam.to_string());
                loaded.push(fam.to_string());
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for (p, fam) in [("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", "DejaVu Sans Mono")] {
            if let Ok(b) = std::fs::read(p) {
                fonts.font_data.insert(fam.to_string(), egui::FontData::from_owned(b));
                fonts.families.entry(FontFamily::Name(fam.to_string().into())).or_default().push(fam.to_string());
                loaded.push(fam.to_string());
            }
        }
    }
    let _ = ALT_FONTS.set(loaded);

    if let Some((name, bytes, idx)) = cjk() {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts.font_data.insert(name.to_owned(), fd);
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push(name.to_owned());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push(name.to_owned());
    }
    ctx.set_fonts(fonts);
    ctx.options_mut(|o| o.warn_on_id_clash = false);
    apply(ctx);
}

/// 把当前主题写进 egui 的 Visuals / Style。切主题后调用一次即可。
pub fn apply(ctx: &egui::Context) {
    let mut v = if light() {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.panel_fill = bg();
    v.window_fill = panel2();
    v.extreme_bg_color = code_bg();
    v.faint_bg_color = hover();
    v.override_text_color = Some(text());
    v.window_stroke = egui::Stroke::new(1.0, line2());
    v.window_rounding = egui::Rounding::same(12.0);
    v.menu_rounding = egui::Rounding::same(10.0);
    v.selection.bg_fill = sel2();
    v.selection.stroke = egui::Stroke::new(1.0, brass());
    v.widgets.noninteractive.bg_fill = panel();
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, line());
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text());
    v.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
    v.widgets.inactive.bg_fill = raise();
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, line2());
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, muted());
    v.widgets.inactive.rounding = egui::Rounding::same(8.0);
    v.widgets.hovered.bg_fill = panel2();
    v.widgets.hovered.weak_bg_fill = hover();
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, line2());
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, text());
    v.widgets.hovered.rounding = egui::Rounding::same(8.0);
    v.widgets.active.bg_fill = sel2();
    v.widgets.active.weak_bg_fill = sel();
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, brass());
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, text());
    v.widgets.active.rounding = egui::Rounding::same(8.0);
    v.widgets.open.bg_fill = panel2();
    v.widgets.open.weak_bg_fill = hover();
    v.widgets.open.rounding = egui::Rounding::same(8.0);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(9.0, 4.0);
    style.spacing.scroll.bar_width = 9.0;
    style.spacing.scroll.floating = true;
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(12.5)),
        (TextStyle::Button, FontId::proportional(12.0)),
        (TextStyle::Heading, FontId::proportional(15.0)),
        (TextStyle::Monospace, syntax::font()),
    ]
    .into();
    ctx.set_style(style);
}
