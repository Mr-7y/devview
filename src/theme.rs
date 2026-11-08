//! 配色与字体：VS Code Dark+ 风格

use crate::syntax;
use egui::{Color32, FontFamily, FontId, TextStyle};

pub const BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
pub const SIDE: Color32 = Color32::from_rgb(0x21, 0x21, 0x22);
pub const TOP: Color32 = Color32::from_rgb(0x2d, 0x2d, 0x2f);
pub const LINE: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2c);
pub const FG: Color32 = Color32::from_rgb(0xd4, 0xd4, 0xd4);
pub const DIM: Color32 = Color32::from_rgb(0x96, 0x96, 0x96);
pub const FAINT: Color32 = Color32::from_rgb(0x6e, 0x6e, 0x6e);
pub const ACCENT: Color32 = Color32::from_rgb(0x0a, 0x84, 0xff);
pub const HOVER: Color32 = Color32::from_rgb(0x2a, 0x2d, 0x2e);
pub const SEL: Color32 = Color32::from_rgb(0x09, 0x47, 0x71);

pub const M: Color32 = Color32::from_rgb(0xe2, 0xc0, 0x8d);
pub const A: Color32 = Color32::from_rgb(0x89, 0xd1, 0x85);
pub const D: Color32 = Color32::from_rgb(0xf1, 0x4c, 0x4c);
pub const U: Color32 = Color32::from_rgb(0x73, 0xc9, 0x91);
pub const R: Color32 = Color32::from_rgb(0x56, 0x9c, 0xd6);

pub fn git_color(s: &str) -> Color32 {
    match s {
        "M" => M,
        "A" => A,
        "D" => D,
        "U" => U,
        "R" => R,
        _ => DIM,
    }
}

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
    if let Some((name, bytes, idx)) = cjk() {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        fonts
            .font_data
            .insert(name.to_owned(), fd);
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

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(0x25, 0x25, 0x26);
    v.extreme_bg_color = Color32::from_rgb(0x18, 0x18, 0x18);
    v.faint_bg_color = HOVER;
    v.override_text_color = Some(FG);
    v.window_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0x3c, 0x3c, 0x3c));
    v.selection.bg_fill = Color32::from_rgb(0x26, 0x4f, 0x78);
    v.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(0x8a, 0xc8, 0xff));
    v.widgets.noninteractive.bg_fill = BG;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x3c, 0x3c, 0x3c);
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x48, 0x48, 0x48);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2f, 0x2f, 0x30);
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.bg_fill = Color32::from_rgb(0x09, 0x47, 0x71);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x37, 0x3b, 0x3d);
    v.widgets.active.bg_stroke = egui::Stroke::NONE;
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);
    style.spacing.scroll.bar_width = 9.0;
    style.spacing.scroll.floating = true;
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(12.5)),
        (TextStyle::Heading, FontId::proportional(15.0)),
        (TextStyle::Monospace, syntax::font()),
    ]
    .into();
    ctx.set_style(style);
}
