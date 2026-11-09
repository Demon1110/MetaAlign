//! End-to-end test: build a real DOCX container on disk, run the batch engine
//! against it and inspect both the filesystem timestamps and the document props.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeDelta, Utc};
use metaalign_core::engine::{apply_one, ItemStatus};
use metaalign_core::fsmeta;
use metaalign_core::model::{
    BackupMode, BatchPlan, FieldMode, FlagMode, TextField, TimeField, TimeRange,
};
use metaalign_core::office;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaalign-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_docx(path: &Path) {
    let parts: [(&str, &str); 4] = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>MetaAlign 测试文档</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "docProps/core.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>原始作者</dc:creator><cp:lastModifiedBy>原始修改人</cp:lastModifiedBy><dc:title>原始标题</dc:title><cp:revision>3</cp:revision></cp:coreProperties>"#,
        ),
    ];

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in parts {
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file(name, options).expect("start zip entry");
        writer.write_all(content.as_bytes()).expect("write zip entry");
    }
    let bytes = writer.finish().expect("finish zip").into_inner();
    std::fs::write(path, bytes).expect("write docx");
}

fn plan(start: DateTime<Utc>, end: DateTime<Utc>) -> BatchPlan {
    let mut plan = BatchPlan::default();
    plan.time.range = TimeRange { start, end };
    plan.time.created = TimeField::random();
    plan.time.modified = TimeField::random();
    plan.time.accessed = TimeField::random();
    plan.office.author = TextField::random();
    plan.office.last_modified_by = TextField::random();
    plan.office.company = TextField::fixed("求是研究院");
    plan.office.keywords = TextField::fixed("归档, 整理");
    plan.attrs.archive = FlagMode::On;
    plan.options.backup = BackupMode::None;
    plan.options.dry_run = false;
    plan
}

#[test]
fn rewrites_timestamps_and_document_props() {
    let dir = temp_dir("happy");
    let path = dir.join("报告.docx");
    write_docx(&path);

    let end = Utc::now();
    let start = end - TimeDelta::days(400);
    let plan = plan(start, end);

    let outcome = apply_one(&path, &plan, 0, 1, &mut rand::rng());
    assert!(
        matches!(outcome.status, ItemStatus::Ok),
        "unexpected status: {:?}",
        outcome.status
    );
    assert!(!outcome.changes.is_empty());

    let snapshot = fsmeta::read_snapshot(&path).expect("snapshot after run");
    let created = snapshot.created.expect("created time");
    assert!(created >= start - TimeDelta::seconds(1) && created <= end + TimeDelta::seconds(1));

    let props = office::read_props(&path)
        .expect("read props")
        .expect("docx props");
    let author = props.author.unwrap_or_default();
    assert!(!author.is_empty() && author != "原始作者");
    assert_eq!(props.company.as_deref(), Some("求是研究院"));
    assert_eq!(props.keywords.as_deref(), Some("归档, 整理"));
    assert!(!props.revision.unwrap_or_default().is_empty());

    let bytes = std::fs::read(&path).expect("read zip");
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("open zip");
    let mut file = archive.by_name("word/document.xml").expect("document part");
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).expect("read document part");
    assert!(text.contains("MetaAlign 测试文档"));
}

#[test]
fn sidecar_backup_is_created_once_and_dry_run_writes_nothing() {
    let dir = temp_dir("backup");
    let path = dir.join("预算.docx");
    write_docx(&path);

    let end = Utc::now();
    let start = end - TimeDelta::hours(6);
    let mut plan = plan(start, end);
    plan.options.backup = BackupMode::Sidecar;

    // Dry run must not write the backup.
    plan.options.dry_run = true;
    let preview = apply_one(&path, &plan, 0, 1, &mut rand::rng());
    assert!(matches!(preview.status, ItemStatus::Ok));
    assert!(!dir.join("预算.docx.metaalign.bak").exists());

    plan.options.dry_run = false;
    let outcome = apply_one(&path, &plan, 0, 1, &mut rand::rng());
    assert!(matches!(outcome.status, ItemStatus::Ok));
    assert!(dir.join("预算.docx.metaalign.bak").exists());
}

#[test]
fn keep_mode_plan_is_reported_as_skipped() {
    let dir = temp_dir("noop");
    let path = dir.join("说明.txt");
    std::fs::write(&path, b"hello").expect("seed file");

    let mut plan = BatchPlan::default();
    plan.time.created = TimeField::keep();
    plan.time.modified = TimeField::keep();
    plan.time.accessed = TimeField::keep();
    plan.office.author = TextField::keep();
    plan.office.last_modified_by = TextField::keep();

    let outcome = apply_one(&path, &plan, 0, 1, &mut rand::rng());
    assert!(
        matches!(outcome.status, ItemStatus::Skipped(_)),
        "expected skip, got {:?}",
        outcome.status
    );
}
