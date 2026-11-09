//! Execution center: preview, dry run, progress and results.

use egui::{RichText, ScrollArea, Ui, Vec2};
use metaalign_core::engine::{self, BatchReport, ItemOutcome, ItemStatus};

use crate::app::MetaAlignApp;
use crate::icons::{self, Icon};
use crate::theme::Palette;
use crate::ui;

pub fn show(app: &mut MetaAlignApp, ui: &mut Ui, p: &Palette) {
    let total = app.queue.len();
    let changes = app
        .preview
        .as_ref()
        .map(BatchReport::change_count)
        .unwrap_or(0);
    let running = app.runner.is_some();
    let (done, progress_total) = app.progress;

    ui.columns(4, |cols| {
        ui::card(&mut cols[0], p, |ui| {
            ui::stat_block(ui, p, "待处理", &total.to_string(), "队列中的文件", true);
        });
        ui::card(&mut cols[1], p, |ui| {
            ui::stat_block(
                ui,
                p,
                "预计变更",
                &changes.to_string(),
                if app.preview.is_some() {
                    "来自最近一次预览"
                } else {
                    "先做一次预览"
                },
                false,
            );
        });
        ui::card(&mut cols[2], p, |ui| {
            ui::stat_block(
                ui,
                p,
                "备份",
                app.plan.options.backup.label(),
                "写入前的安全副本",
                false,
            );
        });
        let progress_hint = if running {
            format!("{done} / {progress_total}")
        } else {
            String::new()
        };
        ui::card(&mut cols[3], p, |ui| {
            ui::stat_block(
                ui,
                p,
                "状态",
                if running {
                    "处理中"
                } else if app.report.is_some() {
                    "已完成"
                } else {
                    "待运行"
                },
                &progress_hint,
                running,
            );
        });
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "执行",
            "试运行与正式处理",
            "试运行会完整走一遍流程但不写盘，结果与正式处理一致",
        );

        ui.horizontal(|ui| {
            let enabled = !app.queue.is_empty() && app.runner.is_none();
            if ui::primary_button(ui, p, "开始处理", enabled, 140.0).clicked() {
                app.start_run(false);
            }
            ui.add_space(4.0);
            if ui::secondary_button(ui, p, "试运行（不写盘）", enabled, 150.0).clicked() {
                app.start_run(true);
            }
            ui.add_space(4.0);
            if ui::ghost_button(ui, p, "生成预览").clicked() {
                app.make_preview();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if app.plan.options.dry_run {
                    ui::chip(ui, p, "试运行模式", p.warn);
                }
                if app.plan.options.only_office_files {
                    ui::chip(ui, p, "仅 Office", p.text_muted);
                }
            });
        });

        if running {
            ui.add_space(12.0);
            let fraction = if progress_total > 0 {
                done as f32 / progress_total as f32
            } else {
                0.0
            };
            ui.add(
                egui::ProgressBar::new(fraction)
                    .text(format!("{done} / {progress_total}"))
                    .animate(true)
                    .corner_radius(6)
                    .fill(p.accent),
            );
        } else if let Some(report) = &app.report {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui::chip(ui, p, format!("成功 {}", report.succeeded), p.ok);
                ui::chip(ui, p, format!("跳过 {}", report.skipped), p.text_muted);
                ui::chip(ui, p, format!("失败 {}", report.failed), p.error);
                ui::chip(ui, p, format!("变更 {} 处", report.change_count()), p.accent);
                ui.label(
                    RichText::new(format!("用时 {} ms", report.elapsed_ms))
                        .size(11.0)
                        .color(p.text_dim),
                );
            });
        }

        let warnings = engine::validate(&app.plan);
        if !warnings.is_empty() {
            ui.add_space(10.0);
            for warning in warnings {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("注意").size(10.5).color(p.warn));
                    ui.label(RichText::new(warning).size(11.5).color(p.text_muted));
                });
            }
        }

        if app.queue.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new("队列为空，请先在「文件队列」添加文件。")
                    .size(11.5)
                    .color(p.text_dim),
            );
        }
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("处理结果")
                        .text_style(egui::TextStyle::Name("Eyebrow".into()))
                        .color(p.text_dim),
                );
                ui.label(
                    RichText::new("逐文件的变更明细")
                        .text_style(egui::TextStyle::Name("H2".into()))
                        .color(p.text)
                        .strong(),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let len = app
                    .report
                    .as_ref()
                    .map(|report| report.outcomes.len())
                    .unwrap_or(0);
                if len > 0 {
                    ui::chip(ui, p, format!("{len} 个文件"), p.text_muted);
                }
            });
        });
        ui.add_space(10.0);

        match app.report.as_ref() {
            Some(report) if !report.outcomes.is_empty() => {
                let height = (ui.available_height() - 30.0).max(260.0).min(460.0);
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(height)
                    .show(ui, |ui| {
                        for outcome in &report.outcomes {
                            draw_outcome(ui, p, outcome);
                        }
                    });
            }
            _ => {
                ui.add_space(24.0);
                ui::empty_state(
                    ui,
                    p,
                    Icon::Play,
                    "还没有运行记录",
                    "建议先点「试运行」确认变更，再执行正式处理",
                );
                ui.add_space(24.0);
            }
        }
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        egui::CollapsingHeader::new(
            RichText::new(format!("运行日志 · {} 条", app.logs.len()))
                .size(13.0)
                .color(p.text),
        )
        .default_open(true)
        .show(ui, |ui| {
            if app.logs.is_empty() {
                ui.label(RichText::new("暂无日志").size(11.5).color(p.text_dim));
                return;
            }
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(220.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in &app.logs {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&entry.time)
                                    .size(10.5)
                                    .monospace()
                                    .color(p.text_dim),
                            );
                            ui.label(
                                RichText::new(entry.level.tag())
                                    .size(10.5)
                                    .color(entry.level.color(p)),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&entry.message).size(11.0).color(p.text_muted),
                                )
                                .wrap(),
                            );
                        });
                    }
                });
        });
    });
}

fn draw_outcome(ui: &mut Ui, p: &Palette, outcome: &ItemOutcome) {
    let (icon, color, status) = match &outcome.status {
        ItemStatus::Ok => (Icon::Check, p.ok, "完成"),
        ItemStatus::Skipped(reason) => (Icon::Info, p.text_dim, reason.as_str()),
        ItemStatus::Failed(reason) => (Icon::Warning, p.error, reason.as_str()),
    };

    egui::Frame::new()
        .fill(p.sunken)
        .corner_radius(9)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                icons::paint(ui.painter(), rect, icon, color);
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        outcome
                            .path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("（未命名）"),
                    )
                    .size(12.5)
                    .color(p.text),
                );
                ui::chip(ui, p, outcome.format.short_label(), p.text_muted);
                ui.label(RichText::new(status).size(11.0).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} ms", outcome.duration_ms))
                            .size(10.0)
                            .monospace()
                            .color(p.text_dim),
                    );
                    if !outcome.changes.is_empty() {
                        ui.label(
                            RichText::new(format!("{} 处", outcome.changes.len()))
                                .size(10.5)
                                .color(p.accent),
                        );
                    }
                });
            });

            for change in &outcome.changes {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(format!("{}：", change.field))
                            .size(10.5)
                            .color(p.text_dim),
                    );
                    ui.label(
                        RichText::new(&change.from)
                            .size(10.5)
                            .monospace()
                            .color(p.text_dim),
                    );
                    ui.label(RichText::new("→").size(10.5).color(p.text_dim));
                    ui.label(
                        RichText::new(&change.to)
                            .size(10.5)
                            .monospace()
                            .color(p.text_muted),
                    );
                });
            }
        });
    ui.add_space(6.0);
}
