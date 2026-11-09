//! Shared widget vocabulary: cards, segmented controls, chips, stat blocks.

use std::collections::BTreeMap;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use egui::{
    Align2, Button, Color32, CornerRadius, FontId, Frame, Label, Margin, Response, RichText,
    Sense, Stroke, TextEdit, TextStyle, Ui, Vec2,
};

use crate::icons::{self, Icon};
use crate::theme::Palette;

/// A soft rounded container.
pub fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border_soft))
        .corner_radius(14)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_width((width - 32.0).max(0.0));
            add(ui)
        })
        .inner
}

/// Eyebrow + title + optional subtitle.
pub fn section_title(ui: &mut Ui, p: &Palette, eyebrow: &str, title: &str, subtitle: &str) {
    if !eyebrow.is_empty() {
        ui.label(
            RichText::new(eyebrow.to_uppercase())
                .text_style(TextStyle::Name("Eyebrow".into()))
                .color(p.text_dim),
        );
        ui.add_space(2.0);
    }
    ui.label(
        RichText::new(title)
            .text_style(TextStyle::Name("H2".into()))
            .color(p.text)
            .strong(),
    );
    if !subtitle.is_empty() {
        ui.add_space(1.0);
        ui.label(
            RichText::new(subtitle)
                .size(12.0)
                .color(p.text_muted),
        );
    }
    ui.add_space(12.0);
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top() + 6.0;
    ui.painter().hline(rect.left()..=rect.right(), y, Stroke::new(1.0, p.border_soft));
    ui.add_space(12.0);
}

/// Small rounded label.
pub fn chip(ui: &mut Ui, _p: &Palette, text: impl Into<String>, color: Color32) {
    let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 26);
    let stroke = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70);
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).size(11.0).color(color));
        });
}

pub fn chip_solid(ui: &mut Ui, _p: &Palette, text: impl Into<String>, fill: Color32, fg: Color32) {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, fill))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).size(11.0).color(fg));
        });
}

/// A three way segmented control.
pub fn segmented<T: Copy + PartialEq>(
    ui: &mut Ui,
    p: &Palette,
    id: impl std::hash::Hash,
    options: &[(T, &str)],
    value: &mut T,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let _ = id;
        {
            for (option, label) in options {
                let selected = *value == *option;
                let text = RichText::new(*label)
                    .size(12.0)
                    .color(if selected { p.on_accent } else { p.text_muted });
                let button = Button::new(text)
                    .fill(if selected { p.accent } else { Color32::TRANSPARENT })
                    .stroke(Stroke::new(
                        1.0,
                        if selected { p.accent } else { p.border_soft },
                    ))
                    .corner_radius(7)
                    .min_size(Vec2::new(0.0, 26.0));
                if ui.add(button).clicked() && !selected {
                    *value = *option;
                    changed = true;
                }
            }
        }
    });
    changed
}

/// Small icon button that paints its icon itself.
pub fn icon_button(ui: &mut Ui, p: &Palette, icon: Icon, tooltip: &str) -> Response {
    let size = Vec2::splat(28.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let bg = if hovered { p.card_hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, CornerRadius::same(7), bg);
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(15.0)),
        icon,
        if hovered { p.accent } else { p.text_muted },
    );
    response.on_hover_text(tooltip)
}

use egui::Rect;

pub fn primary_button(ui: &mut Ui, p: &Palette, text: &str, enabled: bool, width: f32) -> Response {
    ui.add_enabled(
        enabled,
        Button::new(
            RichText::new(text)
                .size(14.0)
                .strong()
                .color(if enabled { p.on_accent } else { p.text_dim }),
        )
        .fill(if enabled { p.accent } else { p.card })
        .stroke(Stroke::new(1.0, if enabled { p.accent } else { p.border_soft }))
        .corner_radius(10)
        .min_size(Vec2::new(width, 38.0)),
    )
}

pub fn secondary_button(ui: &mut Ui, p: &Palette, text: &str, enabled: bool, width: f32) -> Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).size(13.0).color(if enabled { p.text } else { p.text_dim }))
            .fill(p.card)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(10)
            .min_size(Vec2::new(width, 38.0)),
    )
}

pub fn ghost_button(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    ui.add(
        Button::new(RichText::new(text).size(12.0).color(p.text_muted))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(7)
            .min_size(Vec2::new(0.0, 26.0)),
    )
}

pub fn stat_block(ui: &mut Ui, p: &Palette, label: &str, value: &str, hint: &str, accent: bool) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(11.0).color(p.text_dim));
        ui.add_space(4.0);
        ui.label(
            RichText::new(value)
                .size(21.0)
                .strong()
                .color(if accent { p.accent } else { p.text }),
        );
        if !hint.is_empty() {
            ui.add_space(2.0);
            ui.label(RichText::new(hint).size(11.0).color(p.text_muted));
        }
    });
}

/// Text edit parsing/formatting helper.
pub fn parse_datetime(text: &str) -> Option<DateTime<Utc>> {
    let trimmed = text.trim();
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
    ];
    for format in formats {
        if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, format) {
            return Some(localize(naive));
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(localize);
    }
    None
}

fn localize(naive: NaiveDateTime) -> DateTime<Utc> {
    match Local.from_local_datetime(&naive).earliest() {
        Some(local) => local.with_timezone(&Utc),
        None => naive.and_utc(),
    }
}

pub fn format_local(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Date + time text field bound to a UTC instant.
pub fn time_editor(
    ui: &mut Ui,
    p: &Palette,
    id: &str,
    value: &mut DateTime<Utc>,
    buffers: &mut BTreeMap<String, String>,
) -> bool {
    let formatted = format_local(*value);
    let buffer = buffers.entry(id.to_string()).or_insert(formatted).clone();
    let mut edited = buffer;
    let response = ui.add(
        TextEdit::singleline(&mut edited)
            .desired_width(172.0)
            .font(TextStyle::Monospace)
            .text_color(p.text)
            .margin(Vec2::new(8.0, 6.0)),
    );

    let mut changed = false;
    if response.changed() {
        if let Some(parsed) = parse_datetime(&edited) {
            *value = parsed;
            changed = true;
            response.clone().on_hover_text("已更新");
        } else {
            response.clone().on_hover_text("格式：2024-05-01 09:30:00");
        }
    }
    if response.has_focus() {
        buffers.insert(id.to_string(), edited);
    } else {
        buffers.insert(id.to_string(), format_local(*value));
    }
    changed
}

/// Render a row of text with muted left label and right aligned value.
pub fn key_value(ui: &mut Ui, p: &Palette, label: &str, value: &str, mono: bool) {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(96.0, 18.0),
            Label::new(RichText::new(label).size(12.0).color(p.text_dim)),
        );
        let mut text = RichText::new(value).size(12.0);
        text = if mono {
            text.monospace().color(p.text_muted)
        } else {
            text.color(p.text)
        };
        ui.add(Label::new(text).wrap());
    });
}

/// Centered empty state.
pub fn empty_state(ui: &mut Ui, p: &Palette, icon: Icon, title: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
        icons::paint(ui.painter(), rect, icon, p.text_dim);
        ui.add_space(6.0);
        ui.label(RichText::new(title).size(14.0).color(p.text_muted));
        ui.add_space(2.0);
        ui.label(RichText::new(hint).size(11.5).color(p.text_dim));
    });
}

/// Large drop target with dashed border.
pub fn drop_zone(ui: &mut Ui, p: &Palette, active: bool) -> Response {
    let desired = Vec2::new(ui.available_width(), 116.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    let fill = if active { p.accent_soft } else { p.sunken };
    ui.painter().rect_filled(rect, CornerRadius::same(14), fill);

    let stroke = Stroke::new(1.4, if active { p.accent } else { p.border });
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for index in 0..4 {
        let shapes = egui::Shape::dashed_line(
            &[corners[index], corners[(index + 1) % 4]],
            stroke,
            9.0,
            7.0,
        );
        ui.painter().extend(shapes);
    }

    let icon_rect = Rect::from_center_size(
        rect.center() + Vec2::new(0.0, -14.0),
        Vec2::splat(30.0),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        Icon::Folder,
        if active { p.accent } else { p.text_muted },
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 20.0),
        Align2::CENTER_CENTER,
        if active { "松开即可加入队列" } else { "拖拽文件或文件夹到这里" },
        FontId::proportional(14.0),
        p.text,
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 40.0),
        Align2::CENTER_CENTER,
        "或使用下方按钮选择 · 支持 Office、WPS 与任意普通文件",
        FontId::proportional(11.5),
        p.text_dim,
    );

    response
}

/// Slim drop strip used once the queue already holds files.
pub fn drop_zone_compact(ui: &mut Ui, p: &Palette, active: bool) -> Response {
    let desired = Vec2::new(ui.available_width(), 46.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    let fill = if active { p.accent_soft } else { p.sunken };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);

    let stroke = Stroke::new(
        1.2,
        if active { p.accent } else { p.border_soft },
    );
    let corners = [
        rect.left_top() + Vec2::new(1.0, 1.0),
        rect.right_top() + Vec2::new(-1.0, 1.0),
        rect.right_bottom() + Vec2::new(-1.0, -1.0),
        rect.left_bottom() + Vec2::new(1.0, -1.0),
    ];
    for index in 0..4 {
        let shapes = egui::Shape::dashed_line(
            &[corners[index], corners[(index + 1) % 4]],
            stroke,
            7.0,
            5.0,
        );
        ui.painter().extend(shapes);
    }

    let icon_rect = Rect::from_center_size(
        egui::Pos2::new(rect.left() + 26.0, rect.center().y),
        Vec2::splat(20.0),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        Icon::Folder,
        if active { p.accent } else { p.text_muted },
    );
    ui.painter().text(
        egui::Pos2::new(rect.left() + 46.0, rect.center().y),
        Align2::LEFT_CENTER,
        if active {
            "松开即可继续加入队列"
        } else {
            "继续拖入文件或文件夹，或使用下方按钮"
        },
        FontId::proportional(12.5),
        if active { p.accent } else { p.text_muted },
    );

    response
}

/// Toolbar style pill with icon + text.
pub fn icon_text_button(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    text: &str,
    primary: bool,
) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(12.5),
        if primary { p.on_accent } else { p.text },
    );
    let size = Vec2::new(galley.size().x + 42.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if primary {
        if response.hovered() { p.accent.gamma_multiply(0.9) } else { p.accent }
    } else if response.hovered() {
        p.card_hover
    } else {
        p.card
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    if !primary {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(9),
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
    }
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 18.0, rect.center().y),
        Vec2::splat(14.0),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        icon,
        if primary { p.on_accent } else { p.text_muted },
    );
    ui.painter().galley(
        Pos2::new(rect.left() + 30.0, rect.center().y - galley.size().y * 0.5),
        galley,
        if primary { p.on_accent } else { p.text },
    );
    response
}

use egui::Pos2;
