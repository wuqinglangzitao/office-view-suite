//! Browser/WASM adapter functions over the same core [`crate::Document`] API.
//!
//! This module is intentionally thin: it does not parse Word files a second way.
//! The exported functions accept raw Word bytes, call the normal Rust core, and
//! return strings that are convenient for JavaScript demos and browser tests.

use crate::Document;
#[cfg(not(target_arch = "wasm32"))]
use crate::{DocumentReport, Result};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Extract normalized plain text from `.doc` or `.docx` bytes.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = extractText)]
pub fn extract_text(bytes: &[u8]) -> std::result::Result<String, JsValue> {
    crate::extract_text(bytes).map_err(js_error)
}

/// Extract normalized plain text from `.doc` or `.docx` bytes.
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_text(bytes: &[u8]) -> Result<String> {
    crate::extract_text(bytes)
}

/// Convert Word bytes to Markdown through the core document model.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = markdown)]
pub fn markdown(bytes: &[u8]) -> std::result::Result<String, JsValue> {
    Document::open(bytes)
        .map(|document| document.to_markdown())
        .map_err(js_error)
}

/// Convert Word bytes to Markdown through the core document model.
#[cfg(not(target_arch = "wasm32"))]
pub fn markdown(bytes: &[u8]) -> Result<String> {
    Document::open(bytes).map(|document| document.to_markdown())
}

/// Convert Word bytes to HTML through the core document model.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = html)]
pub fn html(bytes: &[u8]) -> std::result::Result<String, JsValue> {
    Document::open(bytes)
        .map(|document| document.to_html())
        .map_err(js_error)
}

/// Convert Word bytes to HTML through the core document model.
#[cfg(not(target_arch = "wasm32"))]
pub fn html(bytes: &[u8]) -> Result<String> {
    Document::open(bytes).map(|document| document.to_html())
}

/// Return compact diagnostics JSON for Word bytes.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = reportJson)]
pub fn report_json(bytes: &[u8]) -> std::result::Result<String, JsValue> {
    Document::open(bytes)
        .map(|document| document.report().to_json())
        .map_err(js_error)
}

/// Return compact diagnostics JSON for Word bytes.
#[cfg(not(target_arch = "wasm32"))]
pub fn report_json(bytes: &[u8]) -> Result<String> {
    Document::open(bytes).map(|document| document.report().to_json())
}

/// Return the typed report for native Rust callers and host-side tests.
#[cfg(not(target_arch = "wasm32"))]
pub fn report(bytes: &[u8]) -> Result<DocumentReport> {
    Document::open(bytes).map(|document| document.report())
}

#[cfg(target_arch = "wasm32")]
fn js_error(error: crate::Error) -> JsValue {
    JsValue::from_str(&error.to_string())
}
