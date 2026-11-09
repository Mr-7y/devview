// DevView — 极简代码浏览器（纯原生渲染，无 WebView）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod conf;
mod fsops;
mod gitops;
mod md;
mod syntax;
mod theme;
mod tree;

#[cfg(debug_assertions)]
mod shot;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();

    let mut opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([760.0, 460.0])
            .with_title("DevView")
            .with_icon(egui::IconData {
                rgba: include_bytes!("icon.rgba").to_vec(),
                width: 64,
                height: 64,
            }),
        ..Default::default()
    };
    opts.renderer = eframe::Renderer::Glow;

    eframe::run_native(
        "DevView",
        opts,
        Box::new(move |cc| {
            theme::setup(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc, args)))
        }),
    )
}
