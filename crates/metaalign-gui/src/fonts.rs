//! CJK font support.
//!
//! egui ships Latin-only fonts, so Chinese text would render as tofu boxes.
//! Instead of embedding a multi-megabyte TTF in the binary, MetaAlign loads a
//! Simplified-Chinese font straight from the Windows font directory and registers
//! it as a fallback family. A system font provider is installed as well, so
//! emoji and other scripts keep working.

use std::path::PathBuf;
use std::sync::Arc;

use egui::FontData;
use egui_system_font_provider::SystemFontProvider;

const FAMILY: &str = "metaalign-cjk";

#[cfg(not(target_arch = "wasm32"))]
const CANDIDATES: [&str; 6] = [
    "msyh.ttc",   // 微软雅黑
    "msyh.ttf",   // 旧版微软雅黑
    "Deng.ttf",   // 等线
    "simhei.ttf", // 黑体
    "simsun.ttc", // 宋体
    "msyhbd.ttc",
];

pub fn install(ctx: &egui::Context) {
    ctx.add_font_provider(Arc::new(
        SystemFontProvider::new().with_locale(Some("zh-CN")),
    ));

    if let Some(data) = load_system_cjk() {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert(FAMILY.to_owned(), Arc::new(data));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(FAMILY.to_owned());
        }
        ctx.set_fonts(fonts);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_system_cjk() -> Option<FontData> {
    let dir = font_dir()?;
    for name in CANDIDATES {
        let path = dir.join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            // The first face of a `.ttc` collection is the regular weight.
            let mut data = FontData::from_owned(bytes);
            data.index = 0;
            return Some(data);
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk() -> Option<FontData> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn font_dir() -> Option<PathBuf> {
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let dir = windir.join("Fonts");
    dir.is_dir().then_some(dir)
}
