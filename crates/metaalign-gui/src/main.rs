//! MetaAlign desktop entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod icons;
mod pages;
mod screenshot;
mod theme;
mod ui;

use app::MetaAlignApp;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("MetaAlign · 元数据批量整理")
        .with_inner_size([1260.0, 820.0])
        .with_min_inner_size([1020.0, 660.0])
        .with_resizable(true);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "MetaAlign",
        options,
        Box::new(|cc| Ok(Box::new(MetaAlignApp::new(cc)) as Box<dyn eframe::App>)),
    )
}

fn window_icon() -> Option<egui::IconData> {
    let decoded = image::load_from_memory(include_bytes!("../assets/icon.png")).ok()?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}
