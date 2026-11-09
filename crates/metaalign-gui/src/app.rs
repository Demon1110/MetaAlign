//! Application state, shell layout and background runner.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Instant;

use chrono::{DateTime, Local, Utc};
use egui::{
    Align2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect, RichText, Sense, Stroke,
    TextStyle, Ui, Vec2,
};
use metaalign_core::engine::{self, BatchReport, ItemOutcome, ItemStatus};
use metaalign_core::fsmeta::{self, FsSnapshot};
use metaalign_core::model::{BatchPlan, TimePatch};
use metaalign_core::office::{self, OfficeFormat, OfficeProps};
use metaalign_core::scan::{self, ScanFilter};

use crate::icons::{self, Icon};
use crate::pages;
use crate::theme::{self, Palette, ThemeMode};
use crate::ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Files,
    Time,
    Office,
    Run,
    Settings,
}

impl Page {
    pub const ALL: [Page; 5] = [Page::Files, Page::Time, Page::Office, Page::Run, Page::Settings];

    pub fn title(self) -> &'static str {
        match self {
            Self::Files => "文件队列",
            Self::Time => "时间规则",
            Self::Office => "文档属性",
            Self::Run => "执行中心",
            Self::Settings => "设置",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Files => "把需要整理的文件集中到队列里",
            Self::Time => "批量重写创建、修改与访问时间",
            Self::Office => "重写 Office 文档内部的作者与摘要信息",
            Self::Run => "预览、试运行并执行整批变更",
            Self::Settings => "扫描、备份与界面偏好",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Files => Icon::Files,
            Self::Time => Icon::Clock,
            Self::Office => Icon::Document,
            Self::Run => Icon::Play,
            Self::Settings => Icon::Settings,
        }
    }
}

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub path: PathBuf,
    pub format: OfficeFormat,
    pub size: u64,
    pub snapshot: Option<FsSnapshot>,
    pub props: Option<OfficeProps>,
    pub props_loaded: bool,
    pub error: Option<String>,
}

impl QueueItem {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("（未命名）")
            .to_string()
    }

    pub fn modified_text(&self) -> String {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.modified)
            .map(format_time)
            .unwrap_or_else(|| "—".to_string())
    }

    pub fn attribute_text(&self) -> String {
        self.snapshot
            .as_ref()
            .map(FsSnapshot::attribute_summary)
            .unwrap_or_else(|| "无法读取".to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl LogLevel {
    pub fn color(self, p: &Palette) -> Color32 {
        match self {
            Self::Info => p.text_muted,
            Self::Success => p.ok,
            Self::Warn => p.warn,
            Self::Error => p.error,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Info => "信息",
            Self::Success => "成功",
            Self::Warn => "注意",
            Self::Error => "错误",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub at: Instant,
}

impl Toast {
    fn color(&self, p: &Palette) -> Color32 {
        match self.kind {
            ToastKind::Info => p.accent,
            ToastKind::Success => p.ok,
            ToastKind::Warn => p.warn,
            ToastKind::Error => p.error,
        }
    }
}

pub enum RunnerEvent {
    Progress {
        done: usize,
        total: usize,
        outcome: ItemOutcome,
    },
    Done(BatchReport),
}

pub struct Runner {
    pub rx: Receiver<RunnerEvent>,
    pub started: Instant,
    pub dry_run: bool,
}

enum InspectorAction {
    Remove,
    Reveal,
    Copy,
}

pub struct MetaAlignApp {
    pub plan: BatchPlan,
    pub queue: Vec<QueueItem>,
    pub selected: Option<usize>,
    pub page: Page,
    pub filter: ScanFilter,
    pub search: String,
    pub office_only_view: bool,
    pub inspector_open: bool,
    pub report: Option<BatchReport>,
    pub preview: Option<BatchReport>,
    pub runner: Option<Runner>,
    pub progress: (usize, usize),
    pub logs: Vec<LogEntry>,
    pub theme: ThemeMode,
    pub toast: Option<Toast>,
    pub show_about: bool,
    pub buffers: BTreeMap<String, String>,
    pub samples: Vec<TimePatch>,
    pub name_pool_draft: String,
    pub extension_draft: String,
    auto_shot: bool,
}

/// Landing page; `METAALIGN_SHOT_PAGE` lets the screenshot script pick another one.
fn initial_page() -> Page {
    match std::env::var("METAALIGN_SHOT_PAGE") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "time" => Page::Time,
            "office" => Page::Office,
            "run" => Page::Run,
            "settings" => Page::Settings,
            _ => Page::Files,
        },
        Err(_) => Page::Files,
    }
}

impl MetaAlignApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::fonts::install(&cc.egui_ctx);

        let theme = ThemeMode::default();
        theme::apply(&cc.egui_ctx, theme);

        let mut app = Self {
            plan: BatchPlan::default(),
            queue: Vec::new(),
            selected: None,
            page: initial_page(),
            filter: ScanFilter::default(),
            search: String::new(),
            office_only_view: false,
            inspector_open: true,
            report: None,
            preview: None,
            runner: None,
            progress: (0, 0),
            logs: Vec::new(),
            theme,
            toast: None,
            show_about: false,
            buffers: BTreeMap::new(),
            samples: Vec::new(),
            name_pool_draft: String::new(),
            extension_draft: app_extension_text(),
            auto_shot: std::env::var_os("METAALIGN_SHOT").is_some(),
        };

        app.log(LogLevel::Info, "MetaAlign 已启动，等待添加文件。");
        app.refresh_samples();

        let args: Vec<PathBuf> = std::env::args_os()
            .skip(1)
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .collect();
        if !args.is_empty() {
            app.add_paths(args);
        }
        if app.auto_shot && !app.queue.is_empty() {
            app.selected = Some(0);
            app.ensure_props(0);
            let landing = initial_page();
            app.make_preview();
            app.report = app.preview.clone();
            // `make_preview` jumps to the run page; screenshots want the requested one.
            app.page = landing;
        }
        app
    }

    // ---------------------------------------------------------------- queue

    pub fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let mut added = 0usize;
        for path in paths {
            if path.is_dir() {
                let result = scan::collect(&[path.clone()], &self.filter);
                for error in &result.errors {
                    self.log(LogLevel::Warn, error);
                }
                for file in result.files {
                    if self.add_path(file) {
                        added += 1;
                    }
                }
            } else if path.is_file() {
                if self.add_path(path) {
                    added += 1;
                }
            } else {
                self.log(
                    LogLevel::Warn,
                    format!("跳过不存在的路径：{}", path.display()),
                );
            }
        }

        self.queue.sort_by(|left, right| {
            left.path
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.path.to_string_lossy().to_lowercase())
        });

        if added > 0 {
            self.mark_dirty();
            self.log(LogLevel::Success, format!("已加入 {added} 个文件"));
            self.toast(ToastKind::Success, format!("已加入 {added} 个文件"));
        } else {
            self.toast(ToastKind::Info, "没有新增文件");
        }
    }

    fn add_path(&mut self, path: PathBuf) -> bool {
        let normalized = path.to_string_lossy().to_lowercase();
        if self
            .queue
            .iter()
            .any(|item| item.path.to_string_lossy().to_lowercase() == normalized)
        {
            return false;
        }
        let format = office::detect_format(&path);
        let snapshot = metaalign_core::fsmeta::read_snapshot(&path).ok();
        let size = snapshot.as_ref().map(|snapshot| snapshot.size).unwrap_or(0);
        let error = if snapshot.is_none() {
            Some("无法读取文件属性（可能被占用或没有权限）".to_string())
        } else {
            None
        };
        self.queue.push(QueueItem {
            path,
            format,
            size,
            snapshot,
            props: None,
            props_loaded: false,
            error,
        });
        true
    }

    pub fn remove_index(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);
            self.selected = if self.queue.is_empty() {
                None
            } else {
                Some(index.min(self.queue.len() - 1))
            };
            self.mark_dirty();
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.selected = None;
        self.report = None;
        self.preview = None;
        self.mark_dirty();
        self.log(LogLevel::Info, "队列已清空");
    }

    pub fn ensure_props(&mut self, index: usize) {
        let Some(item) = self.queue.get_mut(index) else {
            return;
        };
        if item.props_loaded || !item.format.supports_office_meta() {
            return;
        }
        let path = item.path.clone();
        item.props = office::read_props(&path).ok().flatten();
        item.props_loaded = true;
    }

    pub fn mark_dirty(&mut self) {
        self.preview = None;
    }

    /// Re-read timestamps and Office properties for every queued file.
    pub fn refresh_queue(&mut self) {
        for item in &mut self.queue {
            match fsmeta::read_snapshot(&item.path) {
                Ok(snapshot) => {
                    item.size = snapshot.size;
                    item.snapshot = Some(snapshot);
                    item.props = None;
                    item.props_loaded = false;
                    item.error = None;
                }
                Err(error) => {
                    item.snapshot = None;
                    item.props = None;
                    item.props_loaded = false;
                    item.error = Some(error.to_string());
                }
            }
        }
        self.mark_dirty();
        self.toast(ToastKind::Info, "已重新读取队列属性");
        self.log(LogLevel::Info, "重新读取完成");
    }

    pub fn queue_paths(&self) -> Vec<PathBuf> {
        self.queue.iter().map(|item| item.path.clone()).collect()
    }

    pub fn office_count(&self) -> usize {
        self.queue
            .iter()
            .filter(|item| item.format.supports_office_meta())
            .count()
    }

    pub fn visible_rows(&self) -> Vec<usize> {
        let needle = self.search.trim().to_lowercase();
        self.queue
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if self.office_only_view && !item.format.supports_office_meta() {
                    return false;
                }
                if needle.is_empty() {
                    return true;
                }
                item.path.to_string_lossy().to_lowercase().contains(&needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn refresh_samples(&mut self) {
        self.samples = metaalign_core::random::sample_times(&self.plan.time, 5);
    }

    pub fn sync_name_pool(&mut self) {
        self.plan.names.custom = self
            .name_pool_draft
            .lines()
            .flat_map(|line| line.split(['，', ',', ';', '；']))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
    }

    pub fn sync_extensions(&mut self) {
        self.filter.extensions = self
            .extension_draft
            .split(['，', ',', ' ', ';', '；'])
            .map(|item| item.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|item| !item.is_empty())
            .collect();
    }

    // ----------------------------------------------------------------- run

    pub fn make_preview(&mut self) {
        if self.queue.is_empty() {
            self.toast(ToastKind::Warn, "队列为空，先添加文件");
            return;
        }
        let started = Instant::now();
        let mut plan = self.plan.clone();
        plan.options.dry_run = true;
        let paths = self.queue_paths();
        let report = engine::run(&paths, &plan, &mut |_, _, _| {});
        let changes = report.change_count();
        self.log(
            LogLevel::Info,
            format!(
                "预演完成：{} 个文件、{} 处变更，用时 {} ms",
                report.total,
                changes,
                started.elapsed().as_millis()
            ),
        );
        self.preview = Some(report);
        self.page = Page::Run;
        self.toast(ToastKind::Info, format!("预演完成，共 {changes} 处变更"));
    }

    pub fn start_run(&mut self, dry_run: bool) {
        if self.runner.is_some() {
            self.toast(ToastKind::Warn, "任务正在进行中");
            return;
        }
        if self.queue.is_empty() {
            self.toast(ToastKind::Warn, "队列为空，先添加文件");
            return;
        }
        let mut plan = self.plan.clone();
        plan.options.dry_run = dry_run;
        let paths = self.queue_paths();
        let total = paths.len();
        let (tx, rx) = std::sync::mpsc::channel();

        self.progress = (0, total);
        self.runner = Some(Runner {
            rx,
            started: Instant::now(),
            dry_run,
        });
        self.report = None;
        self.logs.clear();
        self.log(
            LogLevel::Info,
            if dry_run {
                format!("开始试运行，共 {total} 个文件（不会写入磁盘）")
            } else {
                format!("开始处理，共 {total} 个文件")
            },
        );
        self.page = Page::Run;

        std::thread::spawn(move || {
            let report = engine::run(&paths, &plan, &mut |done, total, outcome| {
                let _ = tx.send(RunnerEvent::Progress {
                    done,
                    total,
                    outcome: outcome.clone(),
                });
            });
            let _ = tx.send(RunnerEvent::Done(report));
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(runner) = self.runner.take() else {
            return;
        };
        let mut finished: Option<BatchReport> = None;
        let mut received = 0usize;
        while let Ok(event) = runner.rx.try_recv() {
            match event {
                RunnerEvent::Progress {
                    done,
                    total,
                    outcome,
                } => {
                    received += 1;
                    self.progress = (done, total);
                    let level = match outcome.status {
                        ItemStatus::Ok => LogLevel::Success,
                        ItemStatus::Skipped(_) => LogLevel::Info,
                        ItemStatus::Failed(_) => LogLevel::Error,
                    };
                    self.log(
                        level,
                        format!(
                            "{} — {}",
                            describe_outcome(&outcome),
                            outcome.status.message()
                        ),
                    );
                }
                RunnerEvent::Done(report) => finished = Some(report),
            }
        }

        if let Some(report) = finished {
            let elapsed = runner.started.elapsed().as_millis();
            self.log(
                if report.failed > 0 {
                    LogLevel::Warn
                } else {
                    LogLevel::Success
                },
                format!(
                    "完成：成功 {}，跳过 {}，失败 {}，共 {} 处变更，用时 {} ms",
                    report.succeeded,
                    report.skipped,
                    report.failed,
                    report.change_count(),
                    elapsed
                ),
            );
            self.toast(
                if report.failed > 0 {
                    ToastKind::Warn
                } else {
                    ToastKind::Success
                },
                format!(
                    "处理完成：成功 {} / 失败 {}",
                    report.succeeded, report.failed
                ),
            );
            if runner.dry_run {
                self.preview = Some(report.clone());
            }
            self.report = Some(report);
            self.progress = (self.progress.1, self.progress.1);
            self.runner = None;
            ctx.request_repaint();
        } else if received > 0 {
            self.runner = Some(runner);
            ctx.request_repaint();
        } else {
            self.runner = Some(runner);
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    // -------------------------------------------------------------- logging

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        let text = message.into();
        log::log!(
            match level {
                LogLevel::Info => log::Level::Info,
                LogLevel::Success => log::Level::Info,
                LogLevel::Warn => log::Level::Warn,
                LogLevel::Error => log::Level::Error,
            },
            "{text}"
        );
        self.logs.push(LogEntry {
            time: Local::now().format("%H:%M:%S").to_string(),
            level,
            message: text,
        });
        if self.logs.len() > 2000 {
            self.logs.drain(0..self.logs.len() - 2000);
        }
    }

    pub fn toast(&mut self, kind: ToastKind, message: impl Into<String>) {
        // Screenshot runs are for documentation: keep transient toasts out of the frame.
        if self.auto_shot {
            return;
        }
        self.toast = Some(Toast {
            text: message.into(),
            kind,
            at: Instant::now(),
        });
    }

    pub fn set_theme(&mut self, mode: ThemeMode, ctx: &egui::Context) {
        self.theme = mode;
        theme::apply(ctx, mode);
    }

    // ---------------------------------------------------------------- shell

    fn handle_drop(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(dropped);
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, p: &Palette) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(11), p.accent);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "M",
                FontId::proportional(20.0),
                p.on_accent,
            );
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("MetaAlign").size(16.0).strong().color(p.text));
                ui.label(
                    RichText::new("元数据批量整理器")
                        .size(10.5)
                        .color(p.text_dim),
                );
            });
        });

        ui.add_space(22.0);
        ui.label(
            RichText::new("工作区")
                .text_style(TextStyle::Name("Eyebrow".into()))
                .color(p.text_dim),
        );
        ui.add_space(8.0);

        for page in Page::ALL {
            let active = self.page == page;
            let badge = match page {
                Page::Files => Some(self.queue.len()),
                Page::Run => self
                    .report
                    .as_ref()
                    .map(|report| report.outcomes.len())
                    .filter(|count| *count > 0),
                _ => None,
            };
            if nav_item(ui, p, page.icon(), page.title(), active, badge) {
                self.page = page;
            }
        }

        let remaining = ui.available_height() - 108.0;
        if remaining > 0.0 {
            ui.add_space(remaining);
        }

        ui::divider(ui, p);
        ui.horizontal(|ui| {
            let label = self.theme.label();
            let icon = if self.theme.is_dark() {
                Icon::Moon
            } else {
                Icon::Sun
            };
            if nav_item(ui, p, icon, &format!("{label}主题"), false, None) {
                let mode = self.theme.toggled();
                let ctx = ui.ctx().clone();
                self.set_theme(mode, &ctx);
            }
        });
        ui.horizontal(|ui| {
            if nav_item(ui, p, Icon::Info, "关于 MetaAlign", false, None) {
                self.show_about = true;
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("v{} · 构建于 egui", metaalign_core::VERSION))
                .size(10.0)
                .color(p.text_dim),
        );
    }

    fn header(&mut self, ui: &mut Ui, p: &Palette) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.page.title())
                        .text_style(TextStyle::Name("H1".into()))
                        .color(p.text)
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(self.page.subtitle())
                        .size(12.5)
                        .color(p.text_muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::icon_button(ui, p, Icon::Info, "关于").clicked() {
                    self.show_about = true;
                }
                let icon = if self.theme.is_dark() {
                    Icon::Moon
                } else {
                    Icon::Sun
                };
                if ui::icon_button(ui, p, icon, "切换主题").clicked() {
                    let mode = self.theme.toggled();
                    let ctx = ui.ctx().clone();
                    self.set_theme(mode, &ctx);
                }
                let icon = if self.inspector_open {
                    Icon::Document
                } else {
                    Icon::Files
                };
                if ui::icon_button(ui, p, icon, "显示 / 隐藏检查器").clicked() {
                    self.inspector_open = !self.inspector_open;
                }
                if self.runner.is_some() {
                    ui::chip_solid(ui, p, "处理中…", p.accent_soft, p.accent);
                } else if self.queue.is_empty() {
                    ui::chip(ui, p, "队列为空", p.text_dim);
                } else {
                    ui::chip(ui, p, format!("{} 个文件", self.queue.len()), p.text_muted);
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut Ui, p: &Palette) {
        ui.horizontal(|ui| {
            let running = self.runner.is_some();
            let (done, total) = self.progress;
            if running {
                ui::chip_solid(ui, p, "运行中", p.accent_soft, p.accent);
                ui.label(
                    RichText::new(format!("{done} / {total}"))
                        .size(11.5)
                        .monospace()
                        .color(p.text_muted),
                );
            } else {
                ui.label(
                    RichText::new(format!(
                        "队列 {} · Office {} · 其他 {}",
                        self.queue.len(),
                        self.office_count(),
                        self.queue.len().saturating_sub(self.office_count())
                    ))
                    .size(11.5)
                    .color(p.text_dim),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("MetaAlign v{}", metaalign_core::VERSION))
                        .size(10.5)
                        .color(p.text_dim),
                );
                if let Some(report) = &self.report {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(format!(
                            "上次：成功 {} / 跳过 {} / 失败 {} · {} ms",
                            report.succeeded,
                            report.skipped,
                            report.failed,
                            report.elapsed_ms
                        ))
                        .size(10.5)
                        .color(if report.failed > 0 { p.warn } else { p.text_dim }),
                    );
                }
            });
        });
    }

    fn inspector(&mut self, ui: &mut Ui, p: &Palette) {
        let Some(index) = self.selected else {
            ui.add_space(40.0);
            ui::empty_state(
                ui,
                p,
                Icon::Document,
                "未选择文件",
                "在左侧队列里点选一个文件查看详情",
            );
            return;
        };
        if self.queue.get(index).is_none() {
            self.selected = None;
            return;
        }
        self.ensure_props(index);

        let item = self.queue[index].clone();
        let mut action: Option<InspectorAction> = None;

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
            let tint = if item.format.supports_office_meta() {
                p.accent_soft
            } else {
                p.sunken
            };
            ui.painter().rect_filled(rect, CornerRadius::same(9), tint);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                item.format.short_label(),
                FontId::proportional(9.5),
                if item.format.supports_office_meta() {
                    p.accent
                } else {
                    p.text_muted
                },
            );
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(item.file_name()).size(13.5).strong().color(p.text));
                ui.label(
                    RichText::new(item.format.label())
                        .size(10.5)
                        .color(p.text_dim),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::icon_button(ui, p, Icon::Trash, "从队列移除").clicked() {
                    action = Some(InspectorAction::Remove);
                }
            });
        });

        ui.add_space(10.0);
        ui::divider(ui, p);

        ui.label(
            RichText::new("路径")
                .text_style(TextStyle::Name("Eyebrow".into()))
                .color(p.text_dim),
        );
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(item.path.display().to_string())
                        .size(11.0)
                        .monospace()
                        .color(p.text_muted),
                )
                .wrap(),
            );
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui::ghost_button(ui, p, "复制路径").clicked() {
                action = Some(InspectorAction::Copy);
            }
            if ui::ghost_button(ui, p, "在资源管理器中显示").clicked() {
                action = Some(InspectorAction::Reveal);
            }
        });

        ui.add_space(10.0);
        if let Some(error) = &item.error {
            Frame::new()
                .fill(p.error.gamma_multiply(0.12))
                .stroke(Stroke::new(1.0, p.error.gamma_multiply(0.4)))
                .corner_radius(8)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.label(RichText::new(error).size(11.5).color(p.error));
                });
            ui.add_space(8.0);
        }

        ui::key_value(ui, p, "大小", &human_size(item.size), false);
        ui.add_space(2.0);
        if let Some(snapshot) = &item.snapshot {
            ui::key_value(ui, p, "创建时间", &format_time_opt(snapshot.created), true);
            ui::key_value(ui, p, "修改时间", &format_time_opt(snapshot.modified), true);
            ui::key_value(ui, p, "访问时间", &format_time_opt(snapshot.accessed), true);
            ui.add_space(4.0);
            ui::key_value(ui, p, "属性", &snapshot.attribute_summary(), false);
        }

        if item.format.supports_office_meta() {
            ui.add_space(10.0);
            ui::divider(ui, p);
            ui.label(
                RichText::new("文档内置属性")
                    .text_style(TextStyle::Name("Eyebrow".into()))
                    .color(p.text_dim),
            );
            ui.add_space(4.0);
            match &item.props {
                Some(props) if !props.is_empty() => {
                    for (label, value) in [
                        ("作者", props.author.as_deref()),
                        ("最后修改人", props.last_modified_by.as_deref()),
                        ("标题", props.title.as_deref()),
                        ("主题", props.subject.as_deref()),
                        ("关键词", props.keywords.as_deref()),
                        ("分类", props.category.as_deref()),
                        ("备注", props.comments.as_deref()),
                        ("公司", props.company.as_deref()),
                        ("经理", props.manager.as_deref()),
                        ("修订号", props.revision.as_deref()),
                    ] {
                        ui::key_value(ui, p, label, value.unwrap_or("—"), false);
                    }
                    ui::key_value(ui, p, "文档创建", &format_time_opt(props.created), true);
                    ui::key_value(ui, p, "文档修改", &format_time_opt(props.modified), true);
                }
                Some(_) => {
                    ui.label(
                        RichText::new("文档没有任何摘要属性")
                            .size(11.5)
                            .color(p.text_dim),
                    );
                }
                None => {
                    ui.label(
                        RichText::new("无法解析文档属性，写入时会尝试新建属性集")
                            .size(11.5)
                            .color(p.warn),
                    );
                }
            }
        }

        match action {
            Some(InspectorAction::Remove) => {
                self.remove_index(index);
                self.toast(ToastKind::Info, "已从队列移除");
            }
            Some(InspectorAction::Copy) => {
                ui.ctx().copy_text(item.path.display().to_string());
                self.toast(ToastKind::Info, "路径已复制");
            }
            Some(InspectorAction::Reveal) => reveal_in_explorer(&item.path),
            None => {}
        }
    }

    fn toast_overlay(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(toast) = self.toast.clone() else {
            return;
        };
        if toast.at.elapsed() > std::time::Duration::from_secs(4) {
            self.toast = None;
            return;
        }
        let color = toast.color(p);
        egui::Area::new(egui::Id::unique("metaalign.toast"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -58.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0, color))
                    .corner_radius(10)
                    .inner_margin(Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 3.5, color);
                            ui.label(RichText::new(&toast.text).size(12.0).color(p.text));
                        });
                    });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn about_overlay(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.show_about {
            return;
        }
        let screen = ctx.content_rect();
        let mut close = false;

        egui::Area::new(egui::Id::unique("metaalign.about.backdrop"))
            .fixed_pos(screen.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(rect, 0.0, p.overlay);
                if response.clicked() {
                    close = true;
                }
            });

        egui::Area::new(egui::Id::unique("metaalign.about.dialog"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0, p.border))
                    .corner_radius(16)
                    .inner_margin(Margin::same(22))
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
                            ui.painter().rect_filled(icon_rect, CornerRadius::same(11), p.accent);
                            ui.painter().text(
                                icon_rect.center(),
                                Align2::CENTER_CENTER,
                                "M",
                                FontId::proportional(21.0),
                                p.on_accent,
                            );
                            ui.add_space(10.0);
                            ui.vertical(|ui| {
                                ui.label(RichText::new("MetaAlign").size(19.0).strong().color(p.text));
                                ui.label(
                                    RichText::new(format!(
                                        "版本 {} · Windows 元数据批量整理",
                                        metaalign_core::VERSION
                                    ))
                                    .size(11.0)
                                    .color(p.text_dim),
                                );
                            });
                        });
                        ui.add_space(14.0);
                        for line in [
                            "· 批量重写文件创建 / 修改 / 访问时间，支持区间随机填充",
                            "· 批量设置只读、隐藏、系统、存档属性",
                            "· 重写 DOCX / XLSX / PPTX 与旧版 DOC / XLS / PPT 元数据",
                            "· 支持随机中文/英文作者与自定义名单",
                            "· 结构保留式改写，不会破坏文档内容；写入前可自动备份",
                        ] {
                            ui.label(RichText::new(line).size(12.0).color(p.text_muted));
                        }
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if ui::primary_button(ui, p, "知道了", true, 92.0).clicked() {
                                close = true;
                            }
                            ui.label(
                                RichText::new("MIT License").size(10.5).color(p.text_dim),
                            );
                        });
                    });
            });

        if close {
            self.show_about = false;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn capture_screenshot(app: &mut MetaAlignApp, ctx: &egui::Context) {
    if ctx.input(|input| input.key_pressed(egui::Key::F12)) {
        crate::screenshot::save(
            ctx,
            std::path::PathBuf::from("metaalign-screenshot.png"),
            false,
        );
        app.toast(ToastKind::Success, "界面截图已保存为 metaalign-screenshot.png");
    }
    if app.auto_shot && ctx.cumulative_frame_nr() >= 3 {
        app.auto_shot = false;
        let path = std::env::var_os("METAALIGN_SHOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("metaalign-screenshot.png"));
        crate::screenshot::save(ctx, path, true);
    }
}

#[cfg(target_arch = "wasm32")]
fn capture_screenshot(_app: &mut MetaAlignApp, _ctx: &egui::Context) {}

impl eframe::App for MetaAlignApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.handle_drop(&ctx);
        capture_screenshot(self, &ctx);

        let p = theme::palette(self.theme);
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.bg);

        egui::Panel::left("metaalign.sidebar")
            .exact_size(228.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(p.panel)
                    .inner_margin(Margin {
                        left: 14,
                        right: 14,
                        top: 16,
                        bottom: 12,
                    }),
            )
            .show(ui, |ui| self.sidebar(ui, &p));

        egui::Panel::bottom("metaalign.status")
            .exact_size(38.0)
            .resizable(false)
            .show_separator_line(true)
            .frame(
                Frame::new()
                    .fill(p.panel)
                    .inner_margin(Margin::symmetric(16, 7)),
            )
            .show(ui, |ui| self.status_bar(ui, &p));

        if self.inspector_open {
            egui::Panel::right("metaalign.inspector")
                .default_size(330.0)
                .min_size(280.0)
                .max_size(440.0)
                .resizable(true)
                .frame(
                    Frame::new()
                        .fill(p.panel)
                        .inner_margin(Margin::same(16)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.inspector(ui, &p));
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(Margin {
                left: 22,
                right: 22,
                top: 16,
                bottom: 8,
            }))
            .show(ui, |ui| {
                self.header(ui, &p);
                ui.add_space(14.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(2.0);
                        match self.page {
                            Page::Files => pages::files::show(self, ui, &p),
                            Page::Time => pages::time::show(self, ui, &p),
                            Page::Office => pages::office::show(self, ui, &p),
                            Page::Run => pages::run::show(self, ui, &p),
                            Page::Settings => pages::settings::show(self, ui, &p),
                        }
                        ui.add_space(24.0);
                    });
            });

        self.toast_overlay(&ctx, &p);
        self.about_overlay(&ctx, &p);
    }
}

// ------------------------------------------------------------------ helpers

fn nav_item(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    label: &str,
    active: bool,
    badge: Option<usize>,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let fill = if active {
        p.accent_soft
    } else if response.hovered() {
        p.card_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    if active {
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 1.0, rect.top() + 9.0),
                Vec2::new(3.0, rect.height() - 18.0),
            ),
            CornerRadius::same(2),
            p.accent,
        );
    }
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 19.0, rect.center().y),
        Vec2::splat(15.0),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        icon,
        if active { p.accent } else { p.text_muted },
    );
    ui.painter().text(
        Pos2::new(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        if active { p.text } else { p.text_muted },
    );
    if let Some(count) = badge {
        if count > 0 {
            ui.painter().text(
                Pos2::new(rect.right() - 8.0, rect.center().y),
                Align2::RIGHT_CENTER,
                count.to_string(),
                FontId::proportional(11.0),
                if active { p.accent } else { p.text_dim },
            );
        }
    }
    response.clicked()
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_time(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub fn format_time_opt(value: Option<DateTime<Utc>>) -> String {
    value.map(format_time).unwrap_or_else(|| "—".to_string())
}

fn describe_outcome(outcome: &ItemOutcome) -> String {    let name = outcome
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("（未命名）");
    format!("{name} — {}", outcome.status.message())
}

fn app_extension_text() -> String {
    metaalign_core::office::supported_extensions().join(", ")
}

#[cfg(windows)]
fn reveal_in_explorer(path: &Path) {
    let _ = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(path)
        .spawn();
}

#[cfg(not(windows))]
fn reveal_in_explorer(path: &Path) {
    let _ = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
}
