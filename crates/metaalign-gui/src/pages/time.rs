//! Time rules: random window, per-field modes, ordering and attribute flags.

use std::collections::BTreeMap;

use chrono::{Datelike, TimeZone};

use egui::{RichText, Sense, Ui, Vec2};
use metaalign_core::model::{FieldMode, FlagMode, SequenceMode, TimeField, TimeRange};

use crate::app::{MetaAlignApp, format_time_opt};
use crate::theme::Palette;
use crate::ui;

pub fn show(app: &mut MetaAlignApp, ui: &mut Ui, p: &Palette) {
    let mut changed = false;

    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "随机区间",
            "时间填充范围",
            "每个文件都在这个区间内取值，可以单独保留某个时间戳",
        );

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("开始").size(12.0).color(p.text_muted));
                ui.add_space(4.0);
                changed |= ui::time_editor(
                    ui,
                    p,
                    "range.start",
                    &mut app.plan.time.range.start,
                    &mut app.buffers,
                );
            });
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("结束").size(12.0).color(p.text_muted));
                ui.add_space(4.0);
                changed |= ui::time_editor(
                    ui,
                    p,
                    "range.end",
                    &mut app.plan.time.range.end,
                    &mut app.buffers,
                );
            });
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("区间长度").size(12.0).color(p.text_muted));
                ui.add_space(4.0);
                ui::chip(
                    ui,
                    p,
                    span_text(app.plan.time.range.span_millis()),
                    p.accent,
                );
            });
        });

        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("快速设置").size(11.0).color(p.text_dim));
            let now = chrono::Utc::now();
            for (label, days) in [("最近 7 天", 7i64), ("最近 30 天", 30), ("最近一年", 365)]
            {
                if ui::ghost_button(ui, p, label).clicked() {
                    app.plan.time.range = TimeRange {
                        start: now - chrono::TimeDelta::days(days),
                        end: now,
                    };
                    changed = true;
                }
            }
            if ui::ghost_button(ui, p, "今年 1 月至今").clicked() {
                if let Some(date) = chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1) {
                    let naive = date.and_hms_opt(0, 0, 0).unwrap_or_default();
                    let start = match chrono::Local.from_local_datetime(&naive).earliest() {
                        Some(local) => local.with_timezone(&chrono::Utc),
                        None => now,
                    };
                    app.plan.time.range = TimeRange { start, end: now };
                    changed = true;
                }
            }
            if ui::ghost_button(ui, p, "重置").clicked() {
                app.plan.time.range = TimeRange::default();
                changed = true;
            }
        });

        if !app.plan.time.range.is_valid() {
            ui.add_space(8.0);
            warning(ui, p, "开始时间晚于结束时间，运行时将自动按先后交换");
        }
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "时间戳",
            "三个时间字段",
            "保留 = 不改动；指定 = 全部写成同一个值；随机 = 每个文件单独取值",
        );

        changed |= time_field_row(
            ui,
            p,
            &mut app.buffers,
            "time.created",
            "创建时间",
            "资源管理器中的「创建时间」，仅 Windows 可写",
            &mut app.plan.time.created,
        );
        changed |= time_field_row(
            ui,
            p,
            &mut app.buffers,
            "time.modified",
            "修改时间",
            "常规排序与同步工具依赖的时间戳",
            &mut app.plan.time.modified,
        );
        changed |= time_field_row(
            ui,
            p,
            &mut app.buffers,
            "time.accessed",
            "访问时间",
            "打开或读取时由系统更新",
            &mut app.plan.time.accessed,
        );

        ui::divider(ui, p);
        ui.horizontal(|ui| {
            if ui::ghost_button(ui, p, "全部设为随机").clicked() {
                app.plan.time.created = TimeField::random();
                app.plan.time.modified = TimeField::random();
                app.plan.time.accessed = TimeField::random();
                changed = true;
            }
            if ui::ghost_button(ui, p, "全部保留").clicked() {
                app.plan.time.created = TimeField::keep();
                app.plan.time.modified = TimeField::keep();
                app.plan.time.accessed = TimeField::keep();
                changed = true;
            }
            if ui::ghost_button(ui, p, "全部设为现在").clicked() {
                let now = chrono::Utc::now();
                app.plan.time.created = TimeField::fixed(now);
                app.plan.time.modified = TimeField::fixed(now);
                app.plan.time.accessed = TimeField::fixed(now);
                changed = true;
            }
        });
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "生成策略",
            "顺序与一致性",
            "让批量结果看起来像长期自然积累，而不是同一秒生成",
        );

        ui.horizontal(|ui| {
            ui.label(RichText::new("分布方式").size(12.0).color(p.text_muted));
            ui.add_space(8.0);
            egui::ComboBox::from_id_salt("metaalign.sequence")
                .width(150.0)
                .selected_text(app.plan.time.sequence.label())
                .show_ui(ui, |ui| {
                    for mode in [
                        SequenceMode::Independent,
                        SequenceMode::Ascending,
                        SequenceMode::Descending,
                    ] {
                        if ui
                            .selectable_value(&mut app.plan.time.sequence, mode, mode.label())
                            .changed()
                        {
                            changed = true;
                        }
                    }
                });
            ui.add_space(10.0);
            ui.label(
                RichText::new(match app.plan.time.sequence {
                    SequenceMode::Independent => "每个文件完全独立，适合无关文件",
                    SequenceMode::Ascending => "按文件名从早到晚铺开",
                    SequenceMode::Descending => "按文件名从晚到早铺开",
                })
                .size(11.0)
                .color(p.text_dim),
            );
        });

        ui.add_space(6.0);
        if ui
            .checkbox(
                &mut app.plan.time.respect_order,
                "保证 创建 ≤ 修改 ≤ 访问 的先后关系",
            )
            .changed()
        {
            changed = true;
        }
        ui.label(
            RichText::new("关闭后三个时间完全独立，可能出现「修改早于创建」的异常值")
                .size(11.0)
                .color(p.text_dim),
        );

        ui::divider(ui, p);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("抽样预览")
                    .text_style(egui::TextStyle::Name("Eyebrow".into()))
                    .color(p.text_dim),
            );
            if ui::ghost_button(ui, p, "重新抽样").clicked() {
                app.refresh_samples();
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (label, width) in [
                ("文件", 54.0f32),
                ("创建", 152.0),
                ("修改", 152.0),
                ("访问", 152.0),
            ] {
                ui.add_sized(
                    Vec2::new(width, 18.0),
                    egui::Label::new(RichText::new(label).size(10.5).color(p.text_dim)),
                );
            }
        });
        for (index, sample) in app.samples.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_sized(
                    Vec2::new(54.0, 20.0),
                    egui::Label::new(
                        RichText::new(format!("#{}", index + 1))
                            .size(11.0)
                            .monospace()
                            .color(p.text_dim),
                    ),
                );
                for value in [sample.created, sample.modified, sample.accessed] {
                    ui.add_sized(
                        Vec2::new(152.0, 20.0),
                        egui::Label::new(
                            RichText::new(format_time_opt(value))
                                .size(11.0)
                                .monospace()
                                .color(p.text_muted),
                        ),
                    );
                }
            });
        }
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "文件属性",
            "Windows 基础属性位",
            "只影响 Windows 资源管理器显示的属性，不改动文件内容",
        );

        crate::ui::divider(ui, p);
        changed |= flag_row(
            ui,
            p,
            "readonly",
            "只读",
            "防止误编辑；写 Office 属性时会临时解除",
            &mut app.plan.attrs.readonly,
        );
        changed |= flag_row(
            ui,
            p,
            "hidden",
            "隐藏",
            "默认不在资源管理器中显示",
            &mut app.plan.attrs.hidden,
        );
        changed |= flag_row(
            ui,
            p,
            "system",
            "系统",
            "标记为系统文件，通常与隐藏一起使用",
            &mut app.plan.attrs.system,
        );
        changed |= flag_row(
            ui,
            p,
            "archive",
            "存档",
            "备份工具用来判断文件是否已经归档",
            &mut app.plan.attrs.archive,
        );
    });

    if changed {
        app.mark_dirty();
    }
}

fn time_field_row(
    ui: &mut Ui,
    p: &Palette,
    buffers: &mut BTreeMap<String, String>,
    id: &str,
    label: &str,
    hint: &str,
    field: &mut TimeField,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(64.0, 24.0),
            egui::Label::new(RichText::new(label).size(12.5).color(p.text_muted)),
        );
        changed |= ui::segmented(
            ui,
            p,
            format!("mode.{id}"),
            &[
                (FieldMode::Keep, "保留"),
                (FieldMode::Fixed, "指定"),
                (FieldMode::Random, "随机"),
            ],
            &mut field.mode,
        );
        ui.add_space(8.0);
        if field.mode == FieldMode::Fixed {
            changed |= ui::time_editor(ui, p, id, &mut field.fixed, buffers);
        } else {
            ui.label(
                RichText::new(match field.mode {
                    FieldMode::Keep => hint,
                    FieldMode::Random => "每个文件在区间内单独生成",
                    FieldMode::Fixed => "",
                })
                .size(11.0)
                .color(p.text_dim),
            );
        }
    });
    ui.add_space(8.0);
    changed
}

fn flag_row(
    ui: &mut Ui,
    p: &Palette,
    id: &str,
    label: &str,
    hint: &str,
    flag: &mut FlagMode,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(64.0, 24.0),
            egui::Label::new(RichText::new(label).size(12.5).color(p.text_muted)),
        );
        changed |= ui::segmented(
            ui,
            p,
            format!("flag.{id}"),
            &[
                (FlagMode::Keep, "保留"),
                (FlagMode::On, "开启"),
                (FlagMode::Off, "关闭"),
            ],
            flag,
        );
        ui.add_space(8.0);
        ui.label(RichText::new(hint).size(11.0).color(p.text_dim));
    });
    ui.add_space(8.0);
    changed
}

fn warning(ui: &mut Ui, p: &Palette, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(7),
        egui::Color32::from_rgba_unmultiplied(p.warn.r(), p.warn.g(), p.warn.b(), 30),
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(11.5),
        p.warn,
    );
}

fn span_text(millis: i64) -> String {
    let hours = millis / 3_600_000;
    if hours < 1 {
        return format!("{} 分钟", (millis / 60_000).max(1));
    }
    let days = hours / 24;
    if days < 1 {
        format!("{hours} 小时")
    } else {
        format!("{days} 天")
    }
}
