//! Screenshot capture.
//!
//! Press <kbd>F12</kbd> to store a PNG of the current window, which is handy when
//! reporting a visual problem. Set `METAALIGN_SHOT=<path>` before launching and the
//! app grabs one frame automatically and exits — used by the release checklist.

use std::path::{Path, PathBuf};

pub fn save(ctx: &egui::Context, path: PathBuf, quit_after: bool) {
    let ctx = ctx.clone();
    let painter_ctx = ctx.clone();
    ctx.request_screenshot(move |image| {
        match encode(&image, &path) {
            Ok(()) => log::info!("界面截图已保存：{}", path.display()),
            Err(error) => log::warn!("截图保存失败：{error}"),
        }
        if quit_after {
            painter_ctx.request_repaint();
            std::process::exit(0);
        }
    });
}

fn encode(image: &egui::ColorImage, path: &Path) -> Result<(), String> {
    let [width, height] = image.size;
    let mut bytes = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&pixel.to_array());
    }
    image::save_buffer(
        path,
        &bytes,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|error| error.to_string())
}
