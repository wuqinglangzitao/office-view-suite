#![cfg(feature = "docx")]

use std::fs;
use std::path::Path;

use rwml::Document;

fn unsupported_objects() -> Document {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus/public/synthetic/unsupported_objects.docx");
    let bytes = fs::read(path).expect("read public report contract fixture");
    Document::open(&bytes).expect("open public report contract fixture")
}

#[test]
fn document_report_json_matches_v1_contract() {
    let actual = unsupported_objects().report().to_json();
    let expected = include_str!("golden/document-report-v1.json").trim_end();

    assert_eq!(actual, expected);
}

#[cfg(feature = "render")]
#[test]
fn render_report_json_matches_v1_contract() {
    let actual = unsupported_objects()
        .try_to_pdf_with_report()
        .expect("render public report contract fixture")
        .report
        .to_json();
    let expected = include_str!("golden/render-report-v1.json").trim_end();

    assert_eq!(actual, expected);
}
