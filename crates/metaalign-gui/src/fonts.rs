//! CJK font support.
//!
//! egui ships Latin-only fonts, so Chinese text would render as tofu boxes.
//! Instead of embedding a multi-megabyte TTF in the binary, MetaAlign loads a
//! Simplified-Chinese font from the host's own font directories and registers it
//! as a fallback family. A system font provider is installed as well, so emoji
//! and other scripts keep working.

use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::path::Path;

use egui::FontData;
use egui_system_font_provider::SystemFontProvider;

const FAMILY: &str = "metaalign-cjk";

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
    for path in candidate_paths() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        // The first face of a `.ttc` collection is the regular weight.
        let mut data = FontData::from_owned(bytes);
        data.index = 0;
        log::info!("中文字体：{}", path.display());
        return Some(data);
    }
    log::warn!("没有找到可用的中文字体，界面可能显示缺字");
    None
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk() -> Option<FontData> {
    None
}

/// Font files worth trying, most specific first. Missing paths are skipped.
#[cfg(not(target_arch = "wasm32"))]
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        let windir = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let dir = windir.join("Fonts");
        for name in [
            "msyh.ttc",   // 微软雅黑
            "msyh.ttf",   // 旧版微软雅黑
            "Deng.ttf",   // 等线
            "simhei.ttf", // 黑体
            "simsun.ttc", // 宋体
            "msyhbd.ttc",
        ] {
            paths.push(dir.join(name));
        }
    }

    #[cfg(target_os = "linux")]
    {
        for path in [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
        ] {
            paths.push(PathBuf::from(path));
        }
        // 兜底：发行版的字体布局各不相同，扫一遍常见目录找像中文的字体。
        for root in ["/usr/share/fonts", "/usr/local/share/fonts"] {
            collect_cjk_fonts(Path::new(root), 3, &mut paths);
        }
        if let Some(home) = std::env::var_os("HOME") {
            collect_cjk_fonts(&Path::new(&home).join(".local/share/fonts"), 3, &mut paths);
        }
    }

    #[cfg(target_os = "macos")]
    {
        for path in [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ] {
            paths.push(PathBuf::from(path));
        }
    }

    paths
}

#[cfg(target_os = "linux")]
fn collect_cjk_fonts(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_cjk_fonts(&path, depth - 1, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        let is_font = lower.ends_with(".ttc") || lower.ends_with(".ttf") || lower.ends_with(".otf");
        let looks_cjk = lower.contains("cjk")
            || lower.contains("wqy")
            || lower.contains("hansans")
            || lower.contains("sourcehansans")
            || lower.contains("sarasa");
        if is_font && looks_cjk {
            out.push(path);
        }
    }
}
