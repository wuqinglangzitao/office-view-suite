#![cfg(feature = "docx")]

use rwml::{DocBuilder, DocumentFormat};

#[test]
fn wasm_read_api_uses_core_document_surface() {
    let model = DocBuilder::new()
        .heading(1, "WASM report")
        .paragraph("Browser diagnostics")
        .build();
    let bytes = rwml::write_docx(&model);

    assert_eq!(
        rwml::wasm::extract_text(&bytes).expect("wasm extract text"),
        "WASM report\nBrowser diagnostics"
    );
    assert_eq!(
        rwml::wasm::markdown(&bytes).expect("wasm markdown"),
        "# **WASM report**\n\nBrowser diagnostics"
    );
    assert!(
        rwml::wasm::html(&bytes)
            .expect("wasm html")
            .contains("<h1><strong>WASM report</strong></h1>"),
        "wasm html should use core exporter"
    );

    let report_json = rwml::wasm::report_json(&bytes).expect("wasm report json");
    assert!(report_json.contains(r#""format":"docx""#), "{report_json}");
    assert!(report_json.contains(r#""paragraphs":2"#), "{report_json}");

    let report = rwml::wasm::report(&bytes).expect("wasm report");
    assert_eq!(report.format, DocumentFormat::Docx);
    assert_eq!(report.stats.paragraphs, 2);
}
