// 跨平台回归：非 Windows 平台无法写创建时间、也没有 DOS 属性位，
// 引擎必须把它们如实标成「已跳过」，而不是假装改成功。
#![cfg(not(windows))]

use chrono::{TimeDelta, Utc};
use metaalign_core::engine::apply_one;
use metaalign_core::fsmeta;
use metaalign_core::model::{BatchPlan, FlagMode, TimeField, TimeRange};

#[test]
fn unsupported_requests_are_reported_as_skipped_on_unix() {
    let dir = std::env::temp_dir().join(format!("metaalign-unix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("样例.txt");
    std::fs::write(&path, b"hi").expect("write");

    let before = fsmeta::read_snapshot(&path).expect("snapshot before");
    let end = Utc::now();
    let start = end - TimeDelta::days(30);
    let mut plan = BatchPlan::default();
    plan.time.range = TimeRange { start, end };
    plan.time.created = TimeField::random();
    plan.attrs.hidden = FlagMode::On;

    let outcome = apply_one(&path, &plan, 0, 1, &mut rand::rng());

    let fields: Vec<&str> = outcome.changes.iter().map(|c| c.field.as_str()).collect();
    assert!(fields.iter().any(|f| f.contains("创建时间")), "{fields:?}");
    assert!(
        outcome
            .changes
            .iter()
            .filter(|c| c.field.contains("创建时间") || c.field.contains("存档位"))
            .all(|c| c.to.contains("已跳过"))
    );

    let after = fsmeta::read_snapshot(&path).expect("snapshot after");
    assert_eq!(after.created, before.created, "创建时间不应被改动");
}
