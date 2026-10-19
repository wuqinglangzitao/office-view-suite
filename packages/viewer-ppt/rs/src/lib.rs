//! ppt-view-rs:OOXML 视觉模型 → 浏览器布局 DTO
mod ooxml;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = toDocument)]
pub fn to_document(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let doc = ooxml::parse_document(bytes).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&doc).map_err(|e| JsValue::from_str(&format!("serialize: {e}")))
}
