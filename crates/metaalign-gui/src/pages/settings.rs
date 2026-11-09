//! Preferences: backup policy, scanning, behaviour, config export and theme.

use egui::{RichText, Ui, Vec2};
use metaalign_core::model::BackupMode;

use crate::app::MetaAlignApp;
use crate::theme::{Palette, ThemeMode};
use crate::ui;

pub fn show(app: &mut MetaAlignApp, ui: &mut Ui, p: &Palette) {
    let mut changed = false;

    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "安全",
            "备份策略",
            "在写入任何属性之前，先把原文件复制一份",
        );
        changed |= ui::segmented(
            ui,
            p,
            "settings.backup",
            &[
                (BackupMode::None, "不备份"),
                (BackupMode::Sidecar, "同名 .bak"),
                (BackupMode::Folder, "backup 目录"),
            ],
            &mut app.plan.options.backup,
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(match app.plan.options.backup {
                BackupMode::None => "最快，但改坏后无法直接还原，建议只在对文件内容有把握时使用。",
                BackupMode::Sidecar => {
                    "在原文件旁边生成「文件名.docx.metaalign.bak」，重名时自动加序号。"
                }
                BackupMode::Folder => {
                    "在原文件所在目录下建立 metaalign_backup/ 子目录，保持工作目录整洁。"
                }
            })
            .size(11.0)
            .color(p.text_dim),
        );
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "扫描",
            "文件夹收集规则",
            "决定「添加文件夹」时把哪些文件放进队列",
        );

        if ui
            .checkbox(&mut app.filter.recursive, "包含子文件夹")
            .changed()
        {
            changed = true;
        }
        if ui
            .checkbox(
                &mut app.filter.skip_lock_files,
                "跳过 Office 临时锁文件（~$ 开头）",
            )
            .changed()
        {
            changed = true;
        }
        ui.add_space(6.0);
        ui.label(RichText::new("扩展名白名单").size(12.0).color(p.text_muted));
        ui.add_space(4.0);
        let response = ui.add(
            egui::TextEdit::singleline(&mut app.extension_draft)
                .desired_width(f32::INFINITY)
                .hint_text("docx, xlsx, pptx, doc, xls, ppt …")
                .margin(Vec2::new(8.0, 6.0)),
        );
        if response.changed() {
            app.sync_extensions();
            changed = true;
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("留空表示收集全部文件；只影响文件夹扫描，手动选择的文件始终会加入。")
                .size(11.0)
                .color(p.text_dim),
        );
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(ui, p, "行为", "运行开关", "");
        if ui
            .checkbox(
                &mut app.plan.options.only_office_files,
                "只处理支持内部属性的 Office 文件",
            )
            .changed()
        {
            changed = true;
        }
        if ui
            .checkbox(
                &mut app.plan.options.skip_unchanged,
                "把「无需修改」的文件记为跳过而不是成功",
            )
            .changed()
        {
            changed = true;
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("正式处理前会自动清除只读以便写入，写完后再按规则恢复。")
                .size(11.0)
                .color(p.text_dim),
        );
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "配置",
            "导入与导出",
            "整份规则可以保存成 JSON，换机器或团队共享都方便",
        );
        ui.horizontal(|ui| {
            if ui::icon_text_button(ui, p, crate::icons::Icon::Export, "导出配置", false).clicked()
            {
                export_plan(app);
            }
            ui.add_space(4.0);
            if ui::icon_text_button(ui, p, crate::icons::Icon::Import, "导入配置", false).clicked()
            {
                import_plan(app);
            }
            ui.add_space(4.0);
            if ui::ghost_button(ui, p, "恢复默认").clicked() {
                app.plan = metaalign_core::model::BatchPlan::default();
                app.filter = metaalign_core::scan::ScanFilter::default();
                app.extension_draft = metaalign_core::office::supported_extensions().join(", ");
                app.refresh_samples();
                changed = true;
            }
        });
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "外观",
            "主题",
            "深色更适合长时间整理，浅色更适合演示",
        );
        let mut mode = app.theme;
        if ui::segmented(
            ui,
            p,
            "settings.theme",
            &[(ThemeMode::Dark, "深色"), (ThemeMode::Light, "浅色")],
            &mut mode,
        ) {
            let ctx = ui.ctx().clone();
            app.set_theme(mode, &ctx);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new("中文字体通过系统字体回退自动注入，无需额外打包字体文件。")
                .size(11.0)
                .color(p.text_dim),
        );
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(ui, p, "关于", "项目信息", "");
        for (label, value) in [
            ("版本", metaalign_core::VERSION.to_string()),
            ("界面", "egui / eframe 0.37".to_string()),
            (
                "核心库",
                "metaalign-core（纯逻辑，wasm32 可用）".to_string(),
            ),
            ("许可证", "MIT".to_string()),
        ] {
            ui::key_value(ui, p, label, &value, false);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new("核心 crate 不依赖任何 GUI 与文件系统 API 的 Windows 细节，浏览器端可复用同一套 OOXML 管线。")
                .size(11.0)
                .color(p.text_dim),
        );
    });

    if changed {
        app.mark_dirty();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn export_plan(app: &mut MetaAlignApp) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("metaalign-plan.json")
        .add_filter("JSON", &["json"])
        .save_file()
    else {
        return;
    };
    match serde_json::to_string_pretty(&app.plan) {
        Ok(text) => match std::fs::write(&path, text) {
            Ok(()) => app.toast(crate::app::ToastKind::Success, "配置已导出"),
            Err(error) => app.toast(crate::app::ToastKind::Error, format!("写入失败：{error}")),
        },
        Err(error) => app.toast(crate::app::ToastKind::Error, format!("序列化失败：{error}")),
    }
}

#[cfg(target_arch = "wasm32")]
fn export_plan(app: &mut MetaAlignApp) {
    app.toast(crate::app::ToastKind::Info, "网页版暂不支持导出配置");
}

#[cfg(not(target_arch = "wasm32"))]
fn import_plan(app: &mut MetaAlignApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
    else {
        return;
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<metaalign_core::model::BatchPlan>(&text) {
            Ok(plan) => {
                app.plan = plan;
                app.refresh_samples();
                app.mark_dirty();
                app.toast(crate::app::ToastKind::Success, "配置已导入");
            }
            Err(error) => app.toast(crate::app::ToastKind::Error, format!("解析失败：{error}")),
        },
        Err(error) => app.toast(crate::app::ToastKind::Error, format!("读取失败：{error}")),
    }
}

#[cfg(target_arch = "wasm32")]
fn import_plan(app: &mut MetaAlignApp) {
    app.toast(crate::app::ToastKind::Info, "网页版暂不支持导入配置");
}
