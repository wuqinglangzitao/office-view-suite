//! pdfium-view-rs:pdfium-render 封装,线程本地缓存已打开文档,
//! 避免逐页渲染时重复解析同一 PDF。
use pdfium_render::prelude::*;
use serde::Serialize;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    /// Box::leak 得到 'static 生命周期,使 PdfDocument 可跨调用缓存
    static PDFIUM: &'static Pdfium = Box::leak(Box::new(Pdfium::default()));
    static DOC: RefCell<Option<PdfDocument<'static>>> = const { RefCell::new(None) };
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PageSize {
    w_pt: f32,
    h_pt: f32,
}

/// 打开并缓存文档,返回每页尺寸(点)
#[wasm_bindgen(js_name = "pageInfo")]
pub fn page_info(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let doc = PDFIUM
        .with(|pdfium| pdfium.load_pdf_from_byte_vec(bytes.to_vec(), None))
        .map_err(|e| JsValue::from_str(&format!("load: {e}")))?;
    let pages: Vec<PageSize> = doc
        .pages()
        .iter()
        .map(|p| PageSize {
            w_pt: p.width().value,
            h_pt: p.height().value,
        })
        .collect();
    DOC.with(|slot| *slot.borrow_mut() = Some(doc));
    serde_wasm_bindgen::to_value(&pages).map_err(|e| JsValue::from_str(&format!("serialize: {e}")))
}

/// 渲染缓存文档的第 index 页为 ImageData(按指定宽度等比)
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "renderPage")]
pub fn render_page(index: u32, width: u32) -> Result<web_sys::ImageData, JsValue> {
    DOC.with(|slot| {
        let mut guard = slot.borrow_mut();
        let doc = guard
            .as_mut()
            .ok_or_else(|| JsValue::from_str("no document opened"))?;
        let page = doc
            .pages()
            .get(index as i32)
            .map_err(|e| JsValue::from_str(&format!("page {index}: {e}")))?;
        let height = ((page.height().value / page.width().value) * width as f32).round() as i32;
        let bitmap = page
            .render_with_config(&PdfRenderConfig::new().set_target_size(width as i32, height))
            .map_err(|e| JsValue::from_str(&format!("render: {e}")))?;
        bitmap
            .as_image_data()
            .map_err(|_| JsValue::from_str("image conversion failed"))
    })
}
