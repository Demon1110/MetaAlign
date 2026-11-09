//! Batch execution engine: turns a [`BatchPlan`] into concrete file changes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Utc};
use rand::RngExt;

use crate::error::{MetaError, Result};
use crate::fsmeta::{self, FsPatch, FsSnapshot};
use crate::model::{
    AttributeRule, BackupMode, BatchPlan, FieldMode, NameRule, OfficeRule, TextField, TimePatch,
    TimeRule,
};
use crate::names::{random_name, random_revision};
use crate::office::{self, OfficeFormat, OfficeProps, OfficeUpdate};
use crate::random::generate_times;

/// One field that was changed, rendered for humans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeRecord {
    pub field: String,
    pub from: String,
    pub to: String,
}

impl ChangeRecord {
    pub fn new(field: impl Into<String>, from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            from: from.into(),
            to: to.into(),
        }
    }
}

/// Outcome of processing a single file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemStatus {
    Ok,
    Skipped(String),
    Failed(String),
}

impl ItemStatus {
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Ok => "完成",
            Self::Skipped(reason) => reason,
            Self::Failed(reason) => reason,
        }
    }
}

/// Everything that happened to one file.
#[derive(Clone, Debug)]
pub struct ItemOutcome {
    pub path: PathBuf,
    pub format: OfficeFormat,
    pub status: ItemStatus,
    pub changes: Vec<ChangeRecord>,
    pub duration_ms: u128,
}

/// Aggregate result of a run, also used for dry runs.
#[derive(Clone, Debug, Default)]
pub struct BatchReport {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed_ms: u128,
    pub outcomes: Vec<ItemOutcome>,
}

impl BatchReport {
    pub fn change_count(&self) -> usize {
        self.outcomes.iter().map(|outcome| outcome.changes.len()).sum()
    }
}

/// Format an instant for display (local time on desktop, UTC in the browser).
pub fn format_time(value: Option<DateTime<Utc>>) -> String {
    let Some(value) = value else {
        return "—".to_string();
    };
    #[cfg(not(target_arch = "wasm32"))]
    let value = value.with_timezone(&chrono::Local);
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn resolve_text(
    field: &TextField,
    names: &NameRule,
    revision: bool,
    rng: &mut impl RngExt,
) -> Option<String> {
    match field.mode {
        FieldMode::Keep => None,
        FieldMode::Fixed => Some(field.value.clone()),
        FieldMode::Random => Some(if revision {
            random_revision(rng)
        } else {
            random_name(names, rng)
        }),
    }
}

fn office_update(
    office: &OfficeRule,
    time: &TimeRule,
    times: &TimePatch,
    names: &NameRule,
    rng: &mut impl RngExt,
) -> OfficeUpdate {
    let created = if office.follow_file_time {
        if time.created.mode == FieldMode::Keep {
            None
        } else {
            times.created
        }
    } else {
        match office.created.mode {
            FieldMode::Keep => None,
            FieldMode::Fixed => Some(office.created.fixed),
            FieldMode::Random => times.created.or(times.modified),
        }
    };
    let modified = if office.follow_file_time {
        if time.modified.mode == FieldMode::Keep {
            None
        } else {
            times.modified
        }
    } else {
        match office.modified.mode {
            FieldMode::Keep => None,
            FieldMode::Fixed => Some(office.modified.fixed),
            FieldMode::Random => times.modified,
        }
    };

    OfficeUpdate {
        author: resolve_text(&office.author, names, false, rng),
        last_modified_by: resolve_text(&office.last_modified_by, names, false, rng),
        title: resolve_text(&office.title, names, false, rng),
        subject: resolve_text(&office.subject, names, false, rng),
        keywords: resolve_text(&office.keywords, names, false, rng),
        category: resolve_text(&office.category, names, false, rng),
        comments: resolve_text(&office.comments, names, false, rng),
        company: resolve_text(&office.company, names, false, rng),
        manager: resolve_text(&office.manager, names, false, rng),
        revision: resolve_text(&office.revision, names, true, rng),
        created,
        modified,
    }
}

fn attribute_patch(rule: &AttributeRule) -> FsPatch {
    FsPatch {
        readonly: rule.readonly.as_patch(),
        hidden: rule.hidden.as_patch(),
        system: rule.system.as_patch(),
        archive: rule.archive.as_patch(),
        ..Default::default()
    }
}

fn diff_attributes(changes: &mut Vec<ChangeRecord>, snapshot: &FsSnapshot, patch: &FsPatch) {
    for (label, before, after) in [
        ("只读", snapshot.readonly, patch.readonly),
        ("隐藏", snapshot.hidden, patch.hidden),
        ("系统", snapshot.system, patch.system),
        ("存档", snapshot.archive, patch.archive),
    ] {
        if let Some(after) = after
            && after != before
        {
            changes.push(ChangeRecord::new(
                label,
                if before { "开启" } else { "关闭" },
                if after { "开启" } else { "关闭" },
            ));
        }
    }
}

fn diff_times(changes: &mut Vec<ChangeRecord>, snapshot: &FsSnapshot, patch: &FsPatch) {
    for (label, before, after) in [
        ("文件创建时间", snapshot.created, patch.created),
        ("文件修改时间", snapshot.modified, patch.modified),
        ("文件访问时间", snapshot.accessed, patch.accessed),
    ] {
        if let Some(after) = after
            && Some(after) != before
        {
            changes.push(ChangeRecord::new(
                label,
                format_time(before),
                format_time(Some(after)),
            ));
        }
    }
}

fn text_or_dash(value: Option<&String>) -> String {
    match value {
        Some(text) if !text.is_empty() => text.clone(),
        _ => "—".to_string(),
    }
}

fn diff_office(
    changes: &mut Vec<ChangeRecord>,
    props: Option<&OfficeProps>,
    update: &OfficeUpdate,
) {
    let empty = OfficeProps::default();
    let props = props.unwrap_or(&empty);

    let mut text_change = |label: &str, before: Option<&String>, after: Option<&String>| {
        if let Some(after) = after
            && text_or_dash(before) != text_or_dash(Some(after))
        {
            changes.push(ChangeRecord::new(
                label,
                text_or_dash(before),
                text_or_dash(Some(after)),
            ));
        }
    };

    text_change("文档作者", props.author.as_ref(), update.author.as_ref());
    text_change(
        "最后修改人",
        props.last_modified_by.as_ref(),
        update.last_modified_by.as_ref(),
    );
    text_change("文档标题", props.title.as_ref(), update.title.as_ref());
    text_change("文档主题", props.subject.as_ref(), update.subject.as_ref());
    text_change("关键词", props.keywords.as_ref(), update.keywords.as_ref());
    text_change("分类", props.category.as_ref(), update.category.as_ref());
    text_change("备注", props.comments.as_ref(), update.comments.as_ref());
    text_change("公司", props.company.as_ref(), update.company.as_ref());
    text_change("经理", props.manager.as_ref(), update.manager.as_ref());
    text_change("修订号", props.revision.as_ref(), update.revision.as_ref());

    for (label, before, after) in [
        ("文档创建时间", props.created, update.created),
        ("文档修改时间", props.modified, update.modified),
    ] {
        if let Some(after) = after
            && Some(after) != before
        {
            changes.push(ChangeRecord::new(
                label,
                format_time(before),
                format_time(Some(after)),
            ));
        }
    }
}

fn make_backup(path: &Path, mode: BackupMode) -> Result<Option<PathBuf>> {
    if mode == BackupMode::None {
        return Ok(None);
    }
    let Some(parent) = path.parent() else {
        return Ok(None);
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("backup");

    let target = match mode {
        BackupMode::None => return Ok(None),
        BackupMode::Sidecar => parent.join(format!("{name}.metaalign.bak")),
        BackupMode::Folder => parent.join("metaalign_backup").join(name),
    };

    if let Some(folder) = target.parent() {
        std::fs::create_dir_all(folder)?;
    }

    let mut candidate = target.clone();
    let mut counter = 1;
    while candidate.exists() {
        let stem = target
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("backup");
        let extension = target
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bak");
        let folder = target.parent().map(Path::to_path_buf).unwrap_or_default();
        candidate = folder.join(format!("{stem}.{counter}.{extension}"));
        counter += 1;
        if counter > 999 {
            break;
        }
    }

    std::fs::copy(path, &candidate)?;
    Ok(Some(candidate))
}

/// Process one file according to the plan. Never panics; failures are reported.
pub fn apply_one(
    path: &Path,
    plan: &BatchPlan,
    index: usize,
    total: usize,
    rng: &mut impl RngExt,
) -> ItemOutcome {
    let started = Instant::now();
    let format = office::detect_format(path);

    let finish = |status: ItemStatus, changes: Vec<ChangeRecord>| ItemOutcome {
        path: path.to_path_buf(),
        format,
        status,
        changes,
        duration_ms: started.elapsed().as_millis(),
    };

    if plan.is_noop() {
        return finish(ItemStatus::Skipped("没有勾选任何要修改的项".to_string()), Vec::new());
    }
    if plan.options.only_office_files && !format.supports_office_meta() {
        return finish(ItemStatus::Skipped("不是可写元数据的 Office 文件".to_string()), Vec::new());
    }

    let snapshot = match fsmeta::read_snapshot(path) {
        Ok(snapshot) => snapshot,
        Err(error) => return finish(ItemStatus::Failed(error.to_string()), Vec::new()),
    };

    let times = generate_times(&plan.time, index, total, rng);
    let mut fs_patch = attribute_patch(&plan.attrs);
    fs_patch.created = times.created;
    fs_patch.modified = times.modified;
    fs_patch.accessed = times.accessed;

    // Platforms without the Win32 attribute APIs cannot honour these requests.
    // Drop them up front, so the reported change list matches what really lands
    // on disk instead of promising a creation time that will never be written.
    let mut limitations: Vec<&'static str> = Vec::new();
    if !fsmeta::supports_created_time() && fs_patch.created.is_some() {
        fs_patch.created = None;
        limitations.push("文件创建时间");
    }
    if !fsmeta::supports_attribute_bits()
        && (fs_patch.hidden.is_some() || fs_patch.system.is_some() || fs_patch.archive.is_some())
    {
        fs_patch.hidden = None;
        fs_patch.system = None;
        fs_patch.archive = None;
        limitations.push("隐藏 / 系统 / 存档位");
    }

    let office_supported = format.supports_office_meta();
    let update = if office_supported && plan.office.any_change() {
        office_update(&plan.office, &plan.time, &times, &plan.names, rng)
    } else {
        OfficeUpdate::default()
    };

    if fs_patch.is_empty() && update.is_empty() {
        if !limitations.is_empty() {
            let changes: Vec<_> = limitations
                .iter()
                .map(|field| ChangeRecord::new(*field, "需要写入", "已跳过（当前平台不支持）"))
                .collect();
            return finish(
                ItemStatus::Skipped("当前平台不支持勾选的项".to_string()),
                changes,
            );
        }
        return finish(ItemStatus::Skipped("当前设置不会改变这个文件".to_string()), Vec::new());
    }

    let mut changes = Vec::new();
    diff_times(&mut changes, &snapshot, &fs_patch);
    diff_attributes(&mut changes, &snapshot, &fs_patch);
    let props = if update.is_empty() {
        None
    } else {
        office::read_props(path).unwrap_or_default()
    };
    if !update.is_empty() {
        diff_office(&mut changes, props.as_ref(), &update);
    }
    for field in &limitations {
        changes.push(ChangeRecord::new(
            *field,
            "需要写入",
            "已跳过（当前平台不支持）",
        ));
    }

    if plan.options.dry_run {
        return finish(ItemStatus::Ok, changes);
    }

    if let Err(error) = make_backup(path, plan.options.backup) {
        return finish(
            ItemStatus::Failed(format!("备份失败：{error}")),
            changes,
        );
    }

    if !update.is_empty() {
        let was_readonly = match fsmeta::clear_readonly_for_write(path) {
            Ok(flag) => flag,
            Err(error) => return finish(ItemStatus::Failed(error.to_string()), changes),
        };
        let result = office::update_props(path, &update);
        if was_readonly && let Err(error) = fsmeta::restore_readonly(path, true) {
            log::warn!("恢复只读属性失败：{error}");
        }
        if let Err(error) = result {
            return finish(ItemStatus::Failed(error.to_string()), changes);
        }
    }

    if !fs_patch.is_empty()
        && let Err(error) = fsmeta::apply_patch(path, &fs_patch)
    {
        return finish(ItemStatus::Failed(error.to_string()), changes);
    }

    finish(ItemStatus::Ok, changes)
}

/// Run the plan over every path, reporting progress after each file.
pub fn run(
    paths: &[PathBuf],
    plan: &BatchPlan,
    on_progress: &mut dyn FnMut(usize, usize, &ItemOutcome),
) -> BatchReport {
    let started = Instant::now();
    let total = paths.len();
    let mut report = BatchReport {
        total,
        ..Default::default()
    };
    let mut rng = rand::rng();

    for (index, path) in paths.iter().enumerate() {
        let outcome = apply_one(path, plan, index, total, &mut rng);
        match outcome.status {
            ItemStatus::Ok => report.succeeded += 1,
            ItemStatus::Failed(_) => report.failed += 1,
            ItemStatus::Skipped(_) => report.skipped += 1,
        }
        report.outcomes.push(outcome.clone());
        on_progress(index + 1, total, &outcome);
    }

    report.elapsed_ms = started.elapsed().as_millis();
    report
}

/// Validate a plan before running it, returning user-facing warnings.
pub fn validate(plan: &BatchPlan) -> Vec<String> {
    let mut warnings = Vec::new();
    if !plan.time.range.is_valid() {
        warnings.push("随机时间区间的开始晚于结束，运行时将自动交换".to_string());
    }
    if plan.time.any_time_needed() && plan.time.range.span_millis() < 60_000 {
        warnings.push("随机时间区间不足 1 分钟，生成的时间会非常接近".to_string());
    }
    if plan.office.any_change() && !plan.office.follow_file_time && plan.office.created.is_keep()
        && plan.office.modified.is_keep()
    {
        // nothing to warn about, kept for symmetry with future checks
    }
    if plan.options.backup == BackupMode::None {
        warnings.push("未开启备份，修改后无法直接恢复原文件".to_string());
    }
    if plan.time.sequence != crate::model::SequenceMode::Independent && plan.time.range.span_millis() < 60_000 {
        warnings.push("顺序模式下建议把区间放大到至少几小时".to_string());
    }
    warnings
}

/// Read a snapshot without failing the caller on permission errors.
pub fn try_snapshot(path: &Path) -> Option<FsSnapshot> {
    fsmeta::read_snapshot(path).ok()
}

/// Read Office properties, ignoring parse failures.
pub fn try_props(path: &Path) -> Option<OfficeProps> {
    office::read_props(path).ok().flatten()
}

/// Convert an engine error into a message that can be logged.
pub fn describe_error(error: &MetaError) -> String {
    format!("[{}] {error}", error.category())
}
