//! Browser bindings for MetaAlign.
//!
//! Only the in-memory Office pipeline is exposed here: all functions take bytes
//! and return bytes, so nothing touches the filesystem and the same core code
//! runs in the desktop app.
//!
//! Build with:
//! `wasm-pack build crates/metaalign-wasm --target web --out-dir ../../dist/wasm`

use chrono::{DateTime, Utc};
use metaalign_core::model::BatchPlan;
use metaalign_core::office::{self, OfficeProps, OfficeUpdate};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn version() -> String {
    metaalign_core::VERSION.to_string()
}

#[wasm_bindgen]
pub fn supported_extensions() -> String {
    serde_json::to_string(metaalign_core::office::supported_extensions())
        .unwrap_or_else(|_| "[]".to_string())
}

/// Read the metadata of an OOXML document from raw bytes and return JSON.
#[wasm_bindgen]
pub fn read_props(bytes: &[u8]) -> Result<String, JsValue> {
    let props = office::ooxml::read_bytes(bytes).map_err(to_js)?;
    serde_json::to_string(&props_to_json(&props)).map_err(to_js)
}

/// Apply an update described by JSON to an OOXML document, returning new bytes.
///
/// Accepted keys: `author`, `lastModifiedBy`, `title`, `subject`, `keywords`,
/// `category`, `comments`, `company`, `manager`, `revision`, `created`,
/// `modified` (RFC 3339 timestamps).
#[wasm_bindgen]
pub fn patch(bytes: &[u8], update_json: &str) -> Result<Vec<u8>, JsValue> {
    let spec: UpdateSpec = serde_json::from_str(update_json).map_err(to_js)?;
    let update = spec.into_update().map_err(to_js)?;
    let (output, _changed) = office::ooxml::write_bytes(bytes, &update).map_err(to_js)?;
    Ok(output)
}

/// Generate sample timestamps for a plan, so a web UI can preview the result.
#[wasm_bindgen]
pub fn sample_times(plan_json: &str, count: usize) -> Result<String, JsValue> {
    let plan: BatchPlan = serde_json::from_str(plan_json).map_err(to_js)?;
    let samples = metaalign_core::random::sample_times(&plan.time, count.min(50));
    let values: Vec<serde_json::Value> = samples
        .into_iter()
        .map(|patch| {
            serde_json::json!({
                "created": patch.created.map(|value| value.to_rfc3339()),
                "modified": patch.modified.map(|value| value.to_rfc3339()),
                "accessed": patch.accessed.map(|value| value.to_rfc3339()),
            })
        })
        .collect();
    serde_json::to_string(&values).map_err(to_js)
}

/// Validate a plan and return a JSON array of user-facing warnings.
#[wasm_bindgen]
pub fn validate(plan_json: &str) -> Result<String, JsValue> {
    let plan: BatchPlan = serde_json::from_str(plan_json).map_err(to_js)?;
    serde_json::to_string(&metaalign_core::engine::validate(&plan)).map_err(to_js)
}

/// A ready-to-edit default plan, so the web UI starts from the same defaults.
#[wasm_bindgen]
pub fn default_plan() -> String {
    serde_json::to_string(&BatchPlan::default()).unwrap_or_else(|_| "{}".to_string())
}

/// Platform guard: filesystem attributes are not available in the browser.
#[wasm_bindgen]
pub fn filesystem_supported() -> bool {
    false
}

fn to_js(message: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&message.to_string())
}

fn props_to_json(props: &OfficeProps) -> serde_json::Value {
    serde_json::json!({
        "title": props.title,
        "subject": props.subject,
        "author": props.author,
        "lastModifiedBy": props.last_modified_by,
        "keywords": props.keywords,
        "category": props.category,
        "comments": props.comments,
        "company": props.company,
        "manager": props.manager,
        "revision": props.revision,
        "application": props.app_name,
        "created": props.created.map(|value| value.to_rfc3339()),
        "modified": props.modified.map(|value| value.to_rfc3339()),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSpec {
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    last_modified_by: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    keywords: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    comments: Option<String>,
    #[serde(default)]
    company: Option<String>,
    #[serde(default)]
    manager: Option<String>,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    modified: Option<String>,
}

impl UpdateSpec {
    fn into_update(self) -> Result<OfficeUpdate, String> {
        Ok(OfficeUpdate {
            author: self.author,
            last_modified_by: self.last_modified_by,
            title: self.title,
            subject: self.subject,
            keywords: self.keywords,
            category: self.category,
            comments: self.comments,
            company: self.company,
            manager: self.manager,
            revision: self.revision,
            created: parse_time(self.created.as_deref())?,
            modified: parse_time(self.modified.as_deref())?,
        })
    }
}

fn parse_time(value: Option<&str>) -> Result<Option<DateTime<Utc>>, String> {
    let Some(text) = value else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(text)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|error| format!("时间格式应为 RFC 3339：{error}"))
}
