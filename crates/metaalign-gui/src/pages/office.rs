//! Office document metadata page.

use egui::{Label, RichText, Ui, Vec2};
use metaalign_core::model::{FieldMode, NameRule, NameStyle, TextField, TimeField};
use metaalign_core::names::random_name;

use crate::app::MetaAlignApp;
use crate::icons::Icon;
use crate::theme::Palette;
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextKind {
    Name,
    Revision,
    Plain,
}

pub fn show(app: &mut MetaAlignApp, ui: &mut Ui, p: &Palette) {
    let mut changed = false;

    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "署名",
            "作者与最后修改人",
            "随机模式下每个文件都会得到一个全新的姓名，读起来像不同同事编辑过",
        );
        changed |= text_row(
            ui,
            p,
            "作者 creator",
            TextKind::Name,
            &mut app.plan.office.author,
            &app.plan.names,
            true,
        );
        changed |= text_row(
            ui,
            p,
            "最后修改人 lastModifiedBy",
            TextKind::Name,
            &mut app.plan.office.last_modified_by,
            &app.plan.names,
            true,
        );
        ui.add_space(2.0);
        ui::divider(ui, p);
        ui.horizontal(|ui| {
            ui.label(RichText::new("姓名来源").size(12.0).color(p.text_muted));
            ui.add_space(8.0);
            if ui::segmented(
                ui,
                p,
                "name.style",
                &[
                    (NameStyle::Chinese, "中文"),
                    (NameStyle::English, "英文"),
                    (NameStyle::Mixed, "混合"),
                ],
                &mut app.plan.names.style,
            ) {
                changed = true;
            }
        });
        ui.add_space(6.0);
        if ui
            .checkbox(&mut app.plan.names.use_custom, "使用自定义名单")
            .changed()
        {
            changed = true;
        }
        if app.plan.names.use_custom {
            let response = ui.add(
                egui::TextEdit::multiline(&mut app.name_pool_draft)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .hint_text("每行一个姓名，也可以用逗号分隔"),
            );
            if response.changed() {
                app.sync_name_pool();
                changed = true;
            }
            ui.label(
                RichText::new(format!("已收录 {} 个姓名", app.plan.names.custom.len()))
                    .size(11.0)
                    .color(p.text_dim),
            );
        } else {
            ui.label(
                RichText::new(match app.plan.names.style {
                    NameStyle::Chinese => "内置姓氏 + 名字池，组合数量足够覆盖大批量文件",
                    NameStyle::English => "西式「名 姓」组合，适合英文文档",
                    NameStyle::Mixed => "中英文交替出现，更接近真实机构文档",
                })
                .size(11.0)
                .color(p.text_dim),
            );
        }
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "摘要",
            "文档描述信息",
            "这些字段会写入 docProps/core.xml 或 OLE 属性集",
        );

        changed |= text_row(
            ui,
            p,
            "标题 title",
            TextKind::Plain,
            &mut app.plan.office.title,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "主题 subject",
            TextKind::Plain,
            &mut app.plan.office.subject,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "关键词 keywords",
            TextKind::Plain,
            &mut app.plan.office.keywords,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "分类 category",
            TextKind::Plain,
            &mut app.plan.office.category,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "备注 comments",
            TextKind::Plain,
            &mut app.plan.office.comments,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "公司 company",
            TextKind::Plain,
            &mut app.plan.office.company,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "经理 manager",
            TextKind::Plain,
            &mut app.plan.office.manager,
            &app.plan.names,
            false,
        );
        changed |= text_row(
            ui,
            p,
            "修订号 revision",
            TextKind::Revision,
            &mut app.plan.office.revision,
            &app.plan.names,
            false,
        );
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "文档时间",
            "文档内部的创建 / 修改时间",
            "多数场景下与文件时间保持一致最自然",
        );

        if ui
            .checkbox(
                &mut app.plan.office.follow_file_time,
                "文档时间跟随文件时间（推荐）",
            )
            .changed()
        {
            changed = true;
        }
        if app.plan.office.follow_file_time {
            ui.label(
                RichText::new("文件时间规则生成什么，文档里就写什么；文件时间保留时不同步")
                    .size(11.0)
                    .color(p.text_dim),
            );
        } else {
            ui.add_space(8.0);
            changed |= office_time_row(
                ui,
                p,
                &mut app.buffers,
                "office.created",
                "文档创建时间",
                &mut app.plan.office.created,
                &app.plan.time.range,
            );
            changed |= office_time_row(
                ui,
                p,
                &mut app.buffers,
                "office.modified",
                "文档修改时间",
                &mut app.plan.office.modified,
                &app.plan.time.range,
            );
        }
    });

    ui.add_space(12.0);
    ui::card(ui, p, |ui| {
        ui::section_title(
            ui,
            p,
            "支持范围",
            "哪些文件能写到内部属性",
            "",
        );
        ui.horizontal_wrapped(|ui| {
            for label in [
                "DOCX", "DOCM", "XLSX", "XLSM", "PPTX", "PPTM", "DOC", "XLS", "PPT", "WPS", "ET", "DPS",
            ] {
                ui::chip(ui, p, label, p.accent);
                ui.add_space(2.0);
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new("PDF 暂不支持写入，会在队列里标注为只处理文件系统属性。")
                .size(11.0)
                .color(p.text_muted),
        );
        ui.label(
            RichText::new("OOXML 采用结构保留式补丁，仅替换命中字段的文本，其余命名空间与格式保持原样。")
                .size(11.0)
                .color(p.text_dim),
        );
    });

    if changed {
        app.mark_dirty();
    }
}

fn text_row(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    kind: TextKind,
    field: &mut TextField,
    names: &NameRule,
    dice: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(160.0, 24.0),
            Label::new(RichText::new(label).size(12.0).color(p.text_muted)),
        );
        let mut mode = field.mode;
        if ui::segmented(
            ui,
            p,
            format!("text.{label}"),
            &[
                (FieldMode::Keep, "保留"),
                (FieldMode::Fixed, "指定"),
                (FieldMode::Random, "随机"),
            ],
            &mut mode,
        ) {
            field.mode = mode;
            changed = true;
        }
        ui.add_space(8.0);

        match field.mode {
            FieldMode::Keep => {
                ui.label(RichText::new("不改动").size(11.0).color(p.text_dim));
            }
            FieldMode::Fixed => {
                let width = (ui.available_width() - if dice { 34.0 } else { 6.0 }).max(100.0);
                let response = ui.add(
                    egui::TextEdit::singleline(&mut field.value)
                        .hint_text("输入内容…")
                        .desired_width(width)
                        .margin(Vec2::new(8.0, 5.0)),
                );
                if response.changed() {
                    changed = true;
                }
                if dice && ui::icon_button(ui, p, Icon::Dice, "填入一个随机姓名").clicked() {
                    field.value = random_name(names, &mut rand::rng());
                    changed = true;
                }
            }
            FieldMode::Random => {
                let hint = match kind {
                    TextKind::Name => "每个文件随机生成姓名",
                    TextKind::Revision => "每个文件生成随机修订号",
                    TextKind::Plain => "随机文本",
                };
                ui.label(RichText::new(hint).size(11.0).color(p.text_dim));
            }
        }
    });
    ui.add_space(6.0);
    changed
}


fn office_time_row(
    ui: &mut Ui,
    p: &Palette,
    buffers: &mut std::collections::BTreeMap<String, String>,
    id: &str,
    label: &str,
    field: &mut TimeField,
    range: &metaalign_core::model::TimeRange,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(120.0, 24.0),
            Label::new(RichText::new(label).size(12.0).color(p.text_muted)),
        );
        changed |= ui::segmented(
            ui,
            p,
            format!("office.time.{id}"),
            &[
                (FieldMode::Keep, "保留"),
                (FieldMode::Fixed, "指定"),
                (FieldMode::Random, "随机"),
            ],
            &mut field.mode,
        );
        ui.add_space(8.0);
        match field.mode {
            FieldMode::Fixed => {
                changed |= ui::time_editor(ui, p, id, &mut field.fixed, buffers);
            }
            FieldMode::Random => {
                ui.label(
                    RichText::new(format!(
                        "在 {} ~ {} 之间取值",
                        crate::app::format_time(range.normalized().0),
                        crate::app::format_time(range.normalized().1)
                    ))
                    .size(11.0)
                    .color(p.text_dim),
                );
            }
            FieldMode::Keep => {
                ui.label(RichText::new("不改动").size(11.0).color(p.text_dim));
            }
        }
    });
    ui.add_space(6.0);
    changed
}
