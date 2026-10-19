#![cfg(feature = "docx")]

use std::collections::BTreeMap;
use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};

use rwml::{CoreProperty, Document};

const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36, 0x88, 0x49,
    0xd6, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x60, 0xc0, 0x02, 0x00,
    0x00, 0x15, 0x00, 0x01, 0x39, 0xc1, 0xe0, 0x23, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

fn package_parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if entry.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        parts.insert(entry.name().to_string(), bytes);
    }
    parts
}

#[test]
fn committed_edit_session_refreshes_cross_part_read_views() {
    let mut document = Document::try_new().unwrap();

    let mut edits = document.edit_session().unwrap();
    edits
        .set_core_property(CoreProperty::Title, "Committed session")
        .unwrap();
    edits.add_image_png(TINY_PNG, "session.png").unwrap();
    edits.commit().unwrap();

    assert_eq!(
        document.core_properties().title.as_deref(),
        Some("Committed session")
    );
    assert!(document
        .images()
        .iter()
        .any(|image| image.bytes.as_deref() == Some(TINY_PNG)));
    let touched = document.edited_parts();
    assert!(touched.iter().any(|part| part == "docProps/core.xml"));
    assert!(touched.iter().any(|part| part == "word/document.xml"));
    assert!(touched.iter().any(|part| part == "word/media/session.png"));

    let reopened = Document::open(&document.save().unwrap()).unwrap();
    assert_eq!(
        reopened.core_properties().title.as_deref(),
        Some("Committed session")
    );
    assert!(reopened
        .images()
        .iter()
        .any(|image| image.bytes.as_deref() == Some(TINY_PNG)));
}

#[test]
fn committed_edit_session_refreshes_body_and_note_read_views() {
    let mut document = Document::open(include_bytes!(
        "../corpus/public/synthetic/kitchen_sink.docx"
    ))
    .unwrap();
    let original_note_count = document.notes().len();

    let mut edits = document.edit_session().unwrap();
    assert_eq!(
        edits.replace_body_text("PLAIN OLD", "PLAIN NEW").unwrap(),
        1
    );
    edits
        .add_footnote_on_text("COMMENTED", "Session footnote")
        .unwrap();
    assert!(edits.main_text().contains("PLAIN OLD"));
    assert!(!edits.main_text().contains("PLAIN NEW"));
    assert_eq!(edits.notes().len(), original_note_count);
    edits.commit().unwrap();

    assert!(!document.main_text().contains("PLAIN OLD"));
    assert!(document.main_text().contains("PLAIN NEW"));
    assert_eq!(document.notes().len(), original_note_count + 1);
    assert!(document
        .notes()
        .iter()
        .any(|note| note.text == "Session footnote"));
}

#[test]
fn body_paragraph_insertion_commits_and_rolls_back_transactionally() {
    let mut document = Document::try_new().unwrap();
    let before_parts = package_parts(&document.save().unwrap());
    let before_text = document.main_text().to_string();
    let before_blocks = document.body_blocks().unwrap().len();

    let mut rolled_back = document.edit_session().unwrap();
    rolled_back
        .insert_body_paragraph(0, "Temporary body paragraph")
        .unwrap();
    assert_eq!(rolled_back.main_text(), before_text);
    assert_eq!(rolled_back.body_blocks().unwrap().len(), before_blocks + 1);
    rolled_back.rollback();

    assert_eq!(package_parts(&document.save().unwrap()), before_parts);
    assert_eq!(document.main_text(), before_text);
    assert_eq!(document.body_blocks().unwrap().len(), before_blocks);
    assert!(document.edited_parts().is_empty());

    let mut committed = document.edit_session().unwrap();
    committed
        .insert_body_paragraph(0, "Committed body paragraph")
        .unwrap();
    assert_eq!(committed.main_text(), before_text);
    committed.commit().unwrap();

    assert!(document.main_text().contains("Committed body paragraph"));
    assert_eq!(document.body_blocks().unwrap().len(), before_blocks + 1);
    assert_eq!(document.edited_parts(), ["word/document.xml"]);
}

#[test]
fn explicit_refresh_updates_body_view_and_preserves_touched_parts() {
    let mut document = Document::open(include_bytes!(
        "../corpus/public/synthetic/kitchen_sink.docx"
    ))
    .unwrap();
    assert_eq!(
        document
            .replace_body_text("PLAIN OLD", "PLAIN NEW")
            .unwrap(),
        1
    );
    let touched = document.edited_parts();

    assert!(document.main_text().contains("PLAIN OLD"));
    assert!(!document.main_text().contains("PLAIN NEW"));
    document.refresh_read_view().unwrap();

    assert!(!document.main_text().contains("PLAIN OLD"));
    assert!(document.main_text().contains("PLAIN NEW"));
    assert_eq!(document.edited_parts(), touched);
}

#[test]
fn active_edit_session_rejects_manual_refresh_and_rolls_back_cleanly() {
    let mut document = Document::open(include_bytes!(
        "../corpus/public/synthetic/kitchen_sink.docx"
    ))
    .unwrap();
    let before = package_parts(&document.save().unwrap());
    let before_text = document.main_text().to_string();

    let mut edits = document.edit_session().unwrap();
    assert_eq!(
        edits.replace_body_text("PLAIN OLD", "PLAIN NEW").unwrap(),
        1
    );
    let error = edits.refresh_read_view().unwrap_err();
    assert!(error.to_string().contains("active edit session"), "{error}");
    edits.rollback();

    assert_eq!(package_parts(&document.save().unwrap()), before);
    assert_eq!(document.main_text(), before_text);
    assert!(document.edited_parts().is_empty());
    document.refresh_read_view().unwrap();
}

#[test]
fn refreshing_untouched_document_preserves_part_payloads() {
    let mut document = Document::open(include_bytes!(
        "../corpus/public/synthetic/kitchen_sink.docx"
    ))
    .unwrap();
    let before = package_parts(&document.save().unwrap());
    let main_text = document.main_text().to_string();

    document.refresh_read_view().unwrap();

    assert_eq!(package_parts(&document.save().unwrap()), before);
    assert_eq!(document.main_text(), main_text);
    assert!(document.edited_parts().is_empty());
}

#[test]
fn explicit_rollback_restores_prior_package_and_touched_parts() {
    let mut document = Document::try_new().unwrap();
    document
        .set_core_property(CoreProperty::Title, "Before session")
        .unwrap();
    let before_parts = package_parts(&document.save().unwrap());
    let before_touched = document.edited_parts();

    let mut edits = document.edit_session().unwrap();
    edits
        .set_core_property(CoreProperty::Subject, "Temporary")
        .unwrap();
    edits.add_image_png(TINY_PNG, "temporary.png").unwrap();
    edits.rollback();

    assert_eq!(package_parts(&document.save().unwrap()), before_parts);
    assert_eq!(document.edited_parts(), before_touched);
}

#[test]
fn dropping_session_after_late_error_rolls_back_earlier_edits() {
    let mut document = Document::try_new().unwrap();
    let before_parts = package_parts(&document.save().unwrap());

    {
        let mut edits = document.edit_session().unwrap();
        edits
            .set_core_property(CoreProperty::Title, "Must roll back")
            .unwrap();
        let error = edits
            .replace_image_png(TINY_PNG, "missing.png")
            .unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    assert_eq!(package_parts(&document.save().unwrap()), before_parts);
    assert!(document.edited_parts().is_empty());
}

#[test]
fn unwinding_uncommitted_session_restores_snapshot() {
    let mut document = Document::try_new().unwrap();
    let before_parts = package_parts(&document.save().unwrap());

    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let mut edits = document.edit_session().unwrap();
        edits
            .set_core_property(CoreProperty::Title, "Must unwind")
            .unwrap();
        panic!("abort edit session");
    }));

    assert!(unwound.is_err());
    assert_eq!(package_parts(&document.save().unwrap()), before_parts);
    assert!(document.edited_parts().is_empty());
}

#[test]
fn handled_operation_error_does_not_poison_session() {
    let mut document = Document::try_new().unwrap();

    let mut edits = document.edit_session().unwrap();
    assert!(edits.replace_image_png(TINY_PNG, "missing.png").is_err());
    edits
        .set_core_property(CoreProperty::Title, "Recovered session")
        .unwrap();
    edits.commit().unwrap();

    let reopened = Document::open(&document.save().unwrap()).unwrap();
    assert_eq!(
        reopened.core_properties().title.as_deref(),
        Some("Recovered session")
    );
    assert!(reopened.images().is_empty());
}

#[test]
fn edit_session_rejects_legacy_doc_before_snapshot() {
    let mut document = Document::open(include_bytes!(
        "../corpus/public/benchmark/sample/nested_tables.doc"
    ))
    .unwrap();

    let error = document.edit_session().unwrap_err();

    assert!(error
        .to_string()
        .contains("editing requires a .docx-backed document"));
}

#[test]
fn refresh_read_view_rejects_legacy_doc() {
    let mut document = Document::open(include_bytes!(
        "../corpus/public/benchmark/sample/nested_tables.doc"
    ))
    .unwrap();

    let error = document.refresh_read_view().unwrap_err();

    assert!(error
        .to_string()
        .contains("editing requires a .docx-backed document"));
}
