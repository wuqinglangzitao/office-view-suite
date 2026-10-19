#![cfg(all(feature = "render", feature = "bundled-fonts"))]

use rwml::DocBuilder;

#[test]
fn bundled_font_rendering_produces_pdf_for_korean_hanja_arabic_hebrew_and_latin_text() {
    let model = DocBuilder::new()
        .paragraph("안녕하세요 rwml 보고서")
        .paragraph("契約書 第一條 (계약서)")
        .paragraph("مرحبا بالعالم ١٢٣")
        .paragraph("שלום עולם 123")
        .paragraph("Latin line")
        .build();

    let pdf = rwml::try_render_pdf_bundled(&model).expect("bundled font render succeeds");

    assert!(!pdf.is_empty());
    assert!(pdf.starts_with(b"%PDF"));
}
