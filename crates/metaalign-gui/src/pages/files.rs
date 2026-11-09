//! File queue: drop target, filters and the main list.

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, TextEdit, Ui, Vec2,
};
use metaalign_core::office;

use crate::app::{MetaAlignApp, Page, human_size};
use crate::icons::Icon;
use crate::theme::Palette;
use crate::ui;

const ROW_HEIGHT: f32 = 34.0;

struct Columns {
    name: f32,
    format: f32,
    size: f32,
    modified: f32,
    attributes: f32,
    status: f32,
}

fn columns(width: f32) -> Columns {
    let format = 62.0;
    let size = 76.0;
    let modified = 148.0;
    let attributes = 150.0;
    let status = 68.0;
    let name = (width - format - size - modified - attributes - status - 30.0).max(140.0);
    Columns {
        name,
        format,
        size,
        modified,
        attributes,
        status,
    }
}

pub fn show(app: &mut MetaAlignApp, ui: &mut Ui, p: &Palette) {
    let dragging = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
    let drop_response = if app.queue.is_empty() {
        ui::drop_zone(ui, p, dragging)
    } else {
        ui::drop_zone_compact(ui, p, dragging)
    };
    if drop_response.clicked() {
        pick_files(app);
    }

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui::icon_text_button(ui, p, Icon::Plus, "添加文件", true).clicked() {
            pick_files(app);
        }
        ui.add_space(4.0);
        if ui::icon_text_button(ui, p, Icon::Folder, "添加文件夹", false).clicked() {
            pick_folder(app);
        }
        ui.add_space(4.0);
        if ui::icon_text_button(ui, p, Icon::Trash, "清空", false).clicked() {
            app.clear_queue();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !app.queue.is_empty() {
                ui::chip(ui, p, format!("{} 个 Office", app.office_count()), p.accent);
            }
        });
    });

    ui.add_space(14.0);
    let total = app.queue.len();
    let office = app.office_count();
    let plain = total.saturating_sub(office);
    let size_total: u64 = app.queue.iter().map(|item| item.size).sum();
    let size_label = human_size(size_total);
    ui.columns(3, |cols| {
        ui::card(&mut cols[0], p, |ui| {
            ui::stat_block(ui, p, "队列文件", &total.to_string(), &size_label, true);
        });
        ui::card(&mut cols[1], p, |ui| {
            ui::stat_block(
                ui,
                p,
                "Office 文档",
                &office.to_string(),
                "可写入内置属性",
                false,
            );
        });
        ui::card(&mut cols[2], p, |ui| {
            ui::stat_block(
                ui,
                p,
                "普通文件",
                &plain.to_string(),
                "仅文件系统属性",
                false,
            );
        });
    });

    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut app.search)
                .hint_text("按文件名或路径过滤…")
                .desired_width(250.0)
                .margin(Vec2::new(8.0, 6.0)),
        );
        ui.add_space(6.0);
        ui.checkbox(&mut app.office_only_view, "仅显示 Office 文档");
        ui.add_space(6.0);
        if ui::icon_button(ui, p, Icon::Refresh, "重新读取队列属性").clicked() {
            app.refresh_queue();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let visible = app.visible_rows().len();
            ui.label(
                RichText::new(format!("显示 {visible} / {total}"))
                    .size(11.0)
                    .color(p.text_dim),
            );
        });
    });

    ui.add_space(10.0);
    let rows = app.visible_rows();
    let mut clicked: Option<usize> = None;

    ui::card(ui, p, |ui| {
        header_row(ui, p);
        ui.add_space(4.0);
        if rows.is_empty() {
            ui.add_space(28.0);
            if total == 0 {
                ui::empty_state(
                    ui,
                    p,
                    Icon::Files,
                    "队列还是空的",
                    "拖入文件或文件夹，或者用上方按钮添加",
                );
            } else {
                ui::empty_state(
                    ui,
                    p,
                    Icon::Search,
                    "没有匹配的文件",
                    "试着清空搜索或筛选条件",
                );
            }
            ui.add_space(28.0);
        } else {
            let height = (ui.available_height() - 12.0).max(240.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(height)
                .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                    for &index in &rows[range] {
                        let selected = app.selected == Some(index);
                        if draw_row(ui, p, app, index, selected) {
                            clicked = Some(index);
                        }
                    }
                });
        }
    });

    if let Some(index) = clicked {
        app.selected = Some(index);
        app.ensure_props(index);
        if app.inspector_open {
            app.inspector_open = true;
        }
    }

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(if let Some(index) = app.selected {
                app.queue
                    .get(index)
                    .map(|item| format!("已选择：{}", item.file_name()))
                    .unwrap_or_default()
            } else {
                "点击任意一行查看右侧详情".to_string()
            })
            .size(11.0)
            .color(p.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui::secondary_button(ui, p, "下一步：时间规则 →", true, 168.0).clicked() {
                app.page = Page::Time;
            }
        });
    });
}

fn header_row(ui: &mut Ui, p: &Palette) {
    let width = ui.available_width();
    let cols = columns(width);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    let font = FontId::proportional(10.5);
    let mut x = rect.left() + 14.0;
    let label_at = |x: f32, text: &str| {
        ui.painter().text(
            Pos2::new(x, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            font.clone(),
            p.text_dim,
        );
    };
    label_at(x, "文件名");
    x += cols.name;
    label_at(x, "格式");
    x += cols.format;
    label_at(x, "大小");
    x += cols.size;
    label_at(x, "修改时间");
    x += cols.modified;
    label_at(x, "属性");
    x += cols.attributes;
    label_at(x, "状态");
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom() - 1.0,
        Stroke::new(1.0, p.border_soft),
    );
}

fn draw_row(ui: &mut Ui, p: &Palette, app: &MetaAlignApp, index: usize, selected: bool) -> bool {
    let Some(item) = app.queue.get(index) else {
        return false;
    };
    let width = ui.available_width();
    let cols = columns(width);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click());

    let fill = if selected {
        p.accent_soft
    } else if response.hovered() || response.highlighted() {
        p.card_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 2.0, rect.top() + 7.0),
                Vec2::new(3.0, rect.height() - 14.0),
            ),
            CornerRadius::same(2),
            p.accent,
        );
    }

    let mut x = rect.left() + 14.0;
    let name_rect = Rect::from_min_size(
        Pos2::new(x, rect.top()),
        Vec2::new(cols.name, rect.height()),
    );
    paint_text(
        ui,
        name_rect,
        &item.file_name(),
        FontId::proportional(12.5),
        if selected { p.text } else { p.text_muted },
    );
    x += cols.name;

    let badge_rect =
        Rect::from_center_size(Pos2::new(x + 26.0, rect.center().y), Vec2::new(44.0, 17.0));
    let accent = item.format.supports_office_meta();
    ui.painter().rect_filled(
        badge_rect,
        CornerRadius::same(4),
        if accent { p.accent_soft } else { p.sunken },
    );
    ui.painter().text(
        badge_rect.center(),
        Align2::CENTER_CENTER,
        item.format.short_label(),
        FontId::monospace(9.5),
        if accent { p.accent } else { p.text_dim },
    );
    x += cols.format;

    paint_text(
        ui,
        Rect::from_min_size(
            Pos2::new(x, rect.top()),
            Vec2::new(cols.size, rect.height()),
        ),
        &human_size(item.size),
        FontId::monospace(11.0),
        p.text_muted,
    );
    x += cols.size;

    paint_text(
        ui,
        Rect::from_min_size(
            Pos2::new(x, rect.top()),
            Vec2::new(cols.modified, rect.height()),
        ),
        &item.modified_text(),
        FontId::monospace(10.5),
        p.text_dim,
    );
    x += cols.modified;

    paint_text(
        ui,
        Rect::from_min_size(
            Pos2::new(x, rect.top()),
            Vec2::new(cols.attributes, rect.height()),
        ),
        &item.attribute_text(),
        FontId::proportional(11.0),
        p.text_muted,
    );
    x += cols.attributes;

    let (status_text, status_color) = if item.error.is_some() {
        ("不可读", p.error)
    } else {
        ("就绪", p.text_dim)
    };
    ui.painter()
        .circle_filled(Pos2::new(x + 5.0, rect.center().y), 3.0, status_color);
    paint_text(
        ui,
        Rect::from_min_size(
            Pos2::new(x + 14.0, rect.top()),
            Vec2::new(cols.status - 14.0, rect.height()),
        ),
        status_text,
        FontId::proportional(10.5),
        status_color,
    );

    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom(),
        Stroke::new(1.0, p.border_soft),
    );

    response.clicked()
}

fn paint_text(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32) {
    let painter = ui
        .painter()
        .with_clip_rect(rect.shrink2(Vec2::new(0.0, 1.0)));
    painter.text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        color,
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_files(app: &mut MetaAlignApp) {
    let dialog = rfd::FileDialog::new()
        .add_filter("Office 文档", office::supported_extensions())
        .add_filter("所有文件", &["*"])
        .set_title("选择要整理的文件");
    if let Some(files) = dialog.pick_files() {
        app.add_paths(files);
    }
}

#[cfg(target_arch = "wasm32")]
fn pick_files(app: &mut MetaAlignApp) {
    app.toast(crate::app::ToastKind::Info, "网页版请使用拖拽添加文件");
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_folder(app: &mut MetaAlignApp) {
    if let Some(folder) = rfd::FileDialog::new().set_title("选择文件夹").pick_folder() {
        app.add_paths(vec![folder]);
    }
}

#[cfg(target_arch = "wasm32")]
fn pick_folder(app: &mut MetaAlignApp) {
    app.toast(crate::app::ToastKind::Info, "网页版请使用拖拽添加文件");
}

impl Columns {
    #[allow(dead_code)]
    fn width_total(&self) -> f32 {
        self.name + self.format + self.size + self.modified + self.attributes + self.status
    }
}
