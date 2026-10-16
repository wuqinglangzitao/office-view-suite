#![cfg(feature = "docx")]

//! REF numeric-picture (`\#`), lock-result (`\!`), and non-numeric-`\*` pin
//! coverage, driven end-to-end through the public `Document::open(..)` API.

use std::io::Write;

use rwml::{Document, FieldEvaluationReason, FieldEvaluationReasonCount};

fn docx_fixture(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let opt = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(*name, opt).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn ref_docx(body_inner: &str) -> Vec<u8> {
    let document = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body_inner}</w:body></w:document>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
    ])
}

/// Return the computed result for the single REF field whose instruction
/// matches `instruction`.
fn ref_computed(doc: &Document, instruction: &str) -> Option<String> {
    doc.fields()
        .into_iter()
        .find(|field| field.instruction == instruction)
        .unwrap_or_else(|| panic!("REF field `{instruction}` not extracted"))
        .computed_result
}

/// Return the cached result text for the single REF field matching `instruction`.
fn ref_cached(doc: &Document, instruction: &str) -> String {
    doc.fields()
        .into_iter()
        .find(|field| field.instruction == instruction)
        .unwrap_or_else(|| panic!("REF field `{instruction}` not extracted"))
        .result
}

/// Aggregate unsupported field-evaluation reasons for the whole document.
/// Each fixture below carries exactly one REF field, so this is a per-field
/// assertion in practice.
fn reasons(doc: &Document) -> Vec<FieldEvaluationReasonCount> {
    doc.report().features.unsupported_field_reasons
}

/// Slice 1 — `REF <bmk> \# "<picture>"` formats the numeric bookmark text.
#[test]
fn ref_numeric_picture_formats_bookmark_number() {
    let doc = Document::open(&ref_docx(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Total"/><w:r><w:t>1234.5</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF Total \# &quot;$#,##0.00&quot; "><w:r><w:t>stale</w:t></w:r></w:fldSimple></w:p>"#,
    ))
    .expect("fixture opens");

    let instruction = r#"REF Total \# "$#,##0.00""#;
    assert_eq!(
        ref_computed(&doc, instruction).as_deref(),
        Some("$1,234.50")
    );
    // Cached text stays available; the field computes, so nothing unsupported.
    assert_eq!(ref_cached(&doc, instruction), "stale");
    assert_eq!(reasons(&doc), vec![]);
}

/// Slice 1 guard — non-numeric bookmark text under `\#` keeps cached text and
/// reports `NoComputedResult` (parses fine, but nothing to compute).
#[test]
fn ref_numeric_picture_non_numeric_keeps_cached() {
    let doc = Document::open(&ref_docx(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Name"/><w:r><w:t>Alice</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF Name \# &quot;$#,##0.00&quot; "><w:r><w:t>stale</w:t></w:r></w:fldSimple></w:p>"#,
    ))
    .expect("fixture opens");

    let instruction = r#"REF Name \# "$#,##0.00""#;
    assert_eq!(ref_computed(&doc, instruction), None);
    assert_eq!(ref_cached(&doc, instruction), "stale");
    assert_eq!(
        reasons(&doc),
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 1,
        }]
    );
}

/// Slice 1 ordering — `\#` numeric picture applies first, then a trailing
/// `\* Upper` text transform runs on the formatted result.
#[test]
fn ref_numeric_picture_then_case_transform() {
    let doc = Document::open(&ref_docx(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Qty"/><w:r><w:t>7</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF Qty \# &quot;0&quot; \* Upper "><w:r><w:t>stale</w:t></w:r></w:fldSimple></w:p>"#,
    ))
    .expect("fixture opens");

    let instruction = r#"REF Qty \# "0" \* Upper"#;
    assert_eq!(ref_computed(&doc, instruction).as_deref(), Some("7"));
    assert_eq!(reasons(&doc), vec![]);
}

/// Slice 3 — `REF <bmk> \!` (lock result) is a neutral switch: it parses and
/// computes the normal bookmark text.
#[test]
fn ref_lock_result_switch_is_neutral() {
    let doc = Document::open(&ref_docx(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \! "><w:r><w:t>stale</w:t></w:r></w:fldSimple></w:p>"#,
    ))
    .expect("fixture opens");

    let instruction = r#"REF Figure1 \!"#;
    assert_eq!(ref_computed(&doc, instruction).as_deref(), Some("Figure 1"));
    assert_eq!(reasons(&doc), vec![]);
}

/// Slice 2 (pin) — `REF <name> \* Arabic` with non-numeric bookmark text keeps
/// cached text and reports `NoComputedResult`. Pins existing behavior.
#[test]
fn ref_arabic_non_numeric_keeps_cached_pin() {
    let doc = Document::open(&ref_docx(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Name"/><w:r><w:t>Alice</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF Name \* Arabic "><w:r><w:t>stale</w:t></w:r></w:fldSimple></w:p>"#,
    ))
    .expect("fixture opens");

    let instruction = r#"REF Name \* Arabic"#;
    assert_eq!(ref_computed(&doc, instruction), None);
    assert_eq!(ref_cached(&doc, instruction), "stale");
    assert_eq!(
        reasons(&doc),
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 1,
        }]
    );
}
