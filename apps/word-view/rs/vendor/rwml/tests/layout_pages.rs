#![cfg(feature = "render")]

use std::panic::{catch_unwind, AssertUnwindSafe};

use rwml::{DocBuilder, Error};

fn fonts() -> Vec<Vec<u8>> {
    vec![rwml_fonts::noto_sans_kr_subset().to_vec()]
}

#[test]
fn layout_pages_rejects_empty_or_unregistered_strict_fonts() {
    let model = DocBuilder::new().paragraph("strict font test").build();

    assert!(matches!(
        rwml::layout_pages_with_fonts(&model, &[]),
        Err(Error::Render(_))
    ));

    let garbage = catch_unwind(AssertUnwindSafe(|| {
        rwml::layout_pages_with_fonts(&model, &[vec![1, 2, 3, 4, 5]])
    }));
    assert!(garbage.is_ok(), "garbage font bytes must not panic");
    assert!(matches!(garbage.unwrap(), Err(Error::Render(_))));
}

#[test]
fn layout_pages_reports_physical_pages_for_body_page_fields() {
    let model = DocBuilder::new()
        .field("PAGE", "stale one")
        .page_break()
        .field("PAGE", "stale two")
        .page_break()
        .paragraph("last page")
        .build();

    let pages = rwml::layout_pages_with_fonts(&model, &fonts()).expect("layout pages");

    assert_eq!(pages.pages, 3);
    assert_eq!(pages.page_fields, vec![Some(1), Some(2)]);
    assert_eq!(
        rwml::layout_pages_with_fonts(&model, &fonts()).expect("layout pages repeat"),
        pages
    );
}

#[test]
fn layout_pages_reports_first_page_each_top_level_block_touches() {
    let model = DocBuilder::new()
        .paragraph("first block")
        .page_break()
        .paragraph("post-break block")
        .build();

    let pages = rwml::layout_pages_with_fonts(&model, &fonts()).expect("layout pages");

    assert_eq!(pages.block_pages[0], Some(1));
    assert_eq!(pages.block_pages[2], Some(2));
}

fn assert_section_target_page(model: rwml::DocModel, expected_pages: usize, target_block: usize) {
    let font_bytes = fonts();
    let pages = rwml::layout_pages_with_fonts(&model, &font_bytes).expect("layout pages");

    assert_eq!(pages.pages, expected_pages);
    assert_eq!(pages.block_pages[target_block], Some(expected_pages));
    assert_eq!(pages.page_fields, vec![Some(expected_pages)]);
    assert_eq!(
        rwml::layout_pages_with_fonts(&model, &font_bytes).expect("layout pages repeat"),
        pages
    );

    let rendered = rwml::render_pdf_with_fonts_and_report(&model, &font_bytes);
    assert_eq!(rendered.report.pages, expected_pages);
    assert!(rendered.pdf.starts_with(b"%PDF-"));
}

#[test]
fn layout_pages_honors_physical_even_and_odd_section_break_parity() {
    let page_one_even = DocBuilder::new()
        .paragraph("page one")
        .section_break_even_page()
        .field("PAGE", "stale even target")
        .build();
    assert_section_target_page(page_one_even, 2, 2);

    let page_one_odd = DocBuilder::new()
        .paragraph("page one")
        .section_break_odd_page()
        .page_number_start(11)
        .field("PAGE", "stale odd target")
        .build();
    assert_section_target_page(page_one_odd, 3, 2);

    let page_two_odd = DocBuilder::new()
        .paragraph("page one")
        .page_break()
        .paragraph("page two")
        .section_break_odd_page()
        .field("PAGE", "stale odd target")
        .build();
    assert_section_target_page(page_two_odd, 3, 4);

    let page_two_even = DocBuilder::new()
        .paragraph("page one")
        .page_break()
        .paragraph("page two")
        .section_break_even_page()
        .field("PAGE", "stale even target")
        .build();
    assert_section_target_page(page_two_even, 4, 4);
}
