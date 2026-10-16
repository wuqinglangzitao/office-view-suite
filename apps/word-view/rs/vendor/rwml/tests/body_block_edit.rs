#![cfg(feature = "docx")]

use std::collections::BTreeMap;
use std::io::{Read, Write};

use rwml::{BodyBlockKind, Document};

const WML_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn docx_fixture(document_xml: &str) -> Vec<u8> {
    const CONTENT_TYPES: &str = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="bin" ContentType="application/octet-stream"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    const ROOT_RELS: &str = r#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    const SATELLITE: &[u8] = b"\x00preserve-this-unknown-part\xff";

    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options = zip::write::SimpleFileOptions::default();
        for (name, payload) in [
            ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
            ("_rels/.rels", ROOT_RELS.as_bytes()),
            ("word/document.xml", document_xml.as_bytes()),
            ("custom/preserve.bin", SATELLITE),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(payload).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn docx_fixture_with_bytes(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options = zip::write::SimpleFileOptions::default();
        for (name, payload) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(payload).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn package_parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if entry.is_dir() {
            continue;
        }
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).unwrap();
        parts.insert(entry.name().to_string(), payload);
    }
    parts
}

fn document_xml(parts: &BTreeMap<String, Vec<u8>>) -> String {
    String::from_utf8(parts["word/document.xml"].clone()).unwrap()
}

#[test]
fn insert_body_paragraph_preserves_parts_and_reopens_in_atomic_order() {
    let bytes = docx_fixture(&format!(
        r#"<w:document xmlns:w="{WML_NS}"><w:body>
            <w:p data-id="A"><w:r><w:t>A</w:t></w:r><w:unknown keep="1"/></w:p>
            <w:tbl data-id="B"><w:tr><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
            <w:sdt data-id="C"><w:sdtPr/><w:sdtContent><w:p><w:r><w:t>C</w:t></w:r></w:p></w:sdtContent></w:sdt>
            <w:sectPr/>
        </w:body></w:document>"#
    ));
    let before = package_parts(&bytes);
    let mut document = Document::open(&bytes).unwrap();

    document
        .insert_body_paragraph(1, "Inserted middle")
        .unwrap();
    let append_position = document.body_blocks().unwrap().len();
    document
        .insert_body_paragraph(append_position, "Inserted tail")
        .unwrap();

    assert_eq!(document.main_text(), "A\nB\nC", "read view must stay stale");
    assert_eq!(document.edited_parts(), ["word/document.xml"]);

    let saved = document.save().unwrap();
    let after = package_parts(&saved);
    for (name, payload) in &before {
        if name != "word/document.xml" {
            assert_eq!(after.get(name), Some(payload), "changed part {name}");
        }
    }
    assert_eq!(
        package_parts(&document.save().unwrap()),
        after,
        "repeated saves must preserve the same part payloads"
    );

    let xml = document_xml(&after);
    let a = xml.find(r#"data-id="A""#).unwrap();
    let middle = xml.find("<w:t>Inserted middle</w:t>").unwrap();
    let b = xml.find(r#"data-id="B""#).unwrap();
    let c = xml.find(r#"data-id="C""#).unwrap();
    let tail = xml.find("<w:t>Inserted tail</w:t>").unwrap();
    let section = xml.find("<w:sectPr").unwrap();
    assert!(
        a < middle && middle < b && b < c && c < tail && tail < section,
        "unexpected retained body order: {xml}"
    );
    assert!(xml.contains("<w:unknown keep=\"1\"/>"), "{xml}");

    let reopened = Document::open(&saved).unwrap();
    assert_eq!(
        reopened.main_text(),
        "A\nInserted middle\nB\nC\nInserted tail"
    );
    assert_eq!(
        reopened
            .body_blocks()
            .unwrap()
            .into_iter()
            .map(|block| block.kind)
            .collect::<Vec<_>>(),
        vec![
            BodyBlockKind::Paragraph,
            BodyBlockKind::Paragraph,
            BodyBlockKind::Table,
            BodyBlockKind::ContentControl,
            BodyBlockKind::Paragraph,
        ]
    );
}

#[test]
fn insert_body_paragraph_encodes_plain_text_and_supports_default_namespace() {
    let bytes = docx_fixture(&format!(
        r#"<document xmlns="{WML_NS}"><body><p><r><t>Existing</t></r></p><sectPr/></body></document>"#
    ));
    let mut document = Document::open(&bytes).unwrap();
    let text = " <&> 한글\tline\nnext\u{1}\u{b}\u{c}\u{ffff} tail ";

    document.insert_body_paragraph(0, text).unwrap();
    document
        .insert_body_paragraph(document.body_blocks().unwrap().len(), "")
        .unwrap();

    let saved = document.save().unwrap();
    let parts = package_parts(&saved);
    let xml = document_xml(&parts);
    assert!(
        xml.contains(
            r#"<w:t xml:space="preserve"> &lt;&amp;&gt; 한글</w:t><w:tab/><w:t>line</w:t><w:br/><w:t xml:space="preserve">next tail </w:t>"#
        ),
        "inserted text was not encoded through the WML text encoder: {xml}"
    );
    assert!(!xml.contains('\u{1}'));
    assert!(!xml.contains('\u{b}'));
    assert!(!xml.contains('\u{c}'));
    assert!(!xml.contains('\u{ffff}'));
    assert!(
        xml.find("<w:p xmlns:w=").unwrap() < xml.find("<p>").unwrap(),
        "prefixed inserted paragraph must precede the default-namespace host paragraph: {xml}"
    );
    assert!(
        xml.rfind("<w:p xmlns:w=").unwrap() < xml.find("<sectPr").unwrap(),
        "blank appended paragraph must remain before final section properties: {xml}"
    );

    let reopened = Document::open(&saved).unwrap();
    assert_eq!(reopened.body_blocks().unwrap().len(), 3);
    assert!(reopened.main_text().contains("한글"));
    assert!(reopened.main_text().contains("next tail"));
    assert!(reopened.main_text().contains("Existing"));
}

#[test]
fn insert_body_paragraph_preserves_internal_section_boundary_adjacency() {
    let bytes = docx_fixture(&format!(
        r#"<w:document xmlns:w="{WML_NS}"><w:body>
            <w:p data-id="A"><w:r><w:t>A</w:t></w:r></w:p>
            <w:p data-id="S"><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr><w:r><w:t>Section end</w:t></w:r></w:p>
            <w:p data-id="B"><w:r><w:t>B</w:t></w:r></w:p>
            <w:sectPr/>
        </w:body></w:document>"#
    ));
    let mut document = Document::open(&bytes).unwrap();

    document
        .insert_body_paragraph(1, "Before section boundary")
        .unwrap();
    document
        .insert_body_paragraph(3, "After section boundary")
        .unwrap();

    let saved = document.save().unwrap();
    let xml = document_xml(&package_parts(&saved));
    let a = xml.find(r#"data-id="A""#).unwrap();
    let before = xml.find("<w:t>Before section boundary</w:t>").unwrap();
    let section_block = xml.find(r#"data-id="S""#).unwrap();
    let section_props = xml.find(r#"<w:type w:val="nextPage"/>"#).unwrap();
    let after = xml.find("<w:t>After section boundary</w:t>").unwrap();
    let b = xml.find(r#"data-id="B""#).unwrap();
    assert!(
        a < before
            && before < section_block
            && section_block < section_props
            && section_props < after
            && after < b,
        "internal section boundary moved or insertion order changed: {xml}"
    );

    let reopened = Document::open(&saved).unwrap();
    assert_eq!(
        reopened.main_text(),
        "A\nBefore section boundary\nSection end\nAfter section boundary\nB"
    );
    assert_eq!(reopened.body_blocks().unwrap().len(), 5);
}

#[test]
fn insert_body_paragraph_rejects_invalid_positions_and_structural_hazards_atomically() {
    let safe = docx_fixture(&format!(
        r#"<w:document xmlns:w="{WML_NS}"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#
    ));
    let safe_before = package_parts(&safe);
    let mut out_of_range = Document::open(&safe).unwrap();
    let error = out_of_range.insert_body_paragraph(2, "X").unwrap_err();
    assert!(error.to_string().contains("out of range"), "{error}");
    assert!(out_of_range.edited_parts().is_empty());
    assert_eq!(
        package_parts(&out_of_range.save().unwrap()),
        safe_before,
        "out-of-range insertion mutated the retained package"
    );

    for body in [
        r#"<w:p><w:r><w:t>A</w:t></w:r></w:p><w:altChunk/>"#,
        concat!(
            r#"<w:p><w:bookmarkStart w:id="7"/><w:r><w:t>A</w:t></w:r></w:p>"#,
            r#"<w:p><w:r><w:t>B</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p>"#
        ),
        concat!(
            r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r></w:p>"#,
            r#"<w:p><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#
        ),
    ] {
        let bytes = docx_fixture(&format!(
            r#"<w:document xmlns:w="{WML_NS}"><w:body>{body}<w:sectPr/></w:body></w:document>"#
        ));
        let before = package_parts(&bytes);
        let mut document = Document::open(&bytes).unwrap();

        assert!(document.insert_body_paragraph(0, "X").is_err(), "{body}");
        assert!(document.edited_parts().is_empty(), "{body}");
        assert_eq!(
            package_parts(&document.save().unwrap()),
            before,
            "failed structural preflight mutated the package: {body}"
        );
    }
}

#[test]
fn remove_body_block_prunes_only_unreferenced_image_media() {
    let bytes = docx_fixture_with_bytes(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/media/removed.png" ContentType="image/png"/></Types>"#,
        ),
        (
            "_rels/.rels",
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDocument" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdRemoved" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/removed.png"/><Relationship Id="rIdSharedRemoved" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/shared.png"/><Relationship Id="rIdKept" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/kept.png"/><Relationship Id="rIdSharedKept" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/shared.png"/><Relationship Id="rIdVendor" Type="https://vendor.example/image" Target="media/vendor.bin"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p data-id="removed"><w:r><w:drawing><w:opaque r:embed="rIdRemoved"/><w:opaque r:embed="rIdSharedRemoved"/><w:opaque r:embed="rIdVendor"/></w:drawing></w:r></w:p><w:p data-id="kept" data-mention="rIdRemoved"><w:r><w:drawing><w:opaque r:embed="rIdKept"/><w:opaque r:embed="rIdSharedKept"/></w:drawing></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        ("word/media/removed.png", b"removed-media"),
        ("word/media/kept.png", b"kept-media"),
        ("word/media/shared.png", b"shared-media"),
        ("word/media/vendor.bin", b"vendor-media"),
    ]);
    let mut document = Document::open(&bytes).unwrap();

    document.remove_body_block(0).unwrap();
    let saved = document.save().unwrap();
    let parts = package_parts(&saved);
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    assert!(!content_types.contains("/word/media/removed.png"));
    assert!(!parts.contains_key("word/media/removed.png"));
    assert!(parts.contains_key("word/media/kept.png"));
    assert!(parts.contains_key("word/media/shared.png"));
    assert!(parts.contains_key("word/media/vendor.bin"));
    assert!(!rels.contains("rIdRemoved"), "{rels}");
    assert!(!rels.contains("rIdSharedRemoved"), "{rels}");
    assert!(rels.contains("rIdKept"), "{rels}");
    assert!(rels.contains("rIdSharedKept"), "{rels}");
    assert!(rels.contains("rIdVendor"), "{rels}");
    assert!(document
        .edited_parts()
        .iter()
        .any(|part| part == "word/media/removed.png"));
    assert!(document
        .edited_parts()
        .iter()
        .any(|part| part == "[Content_Types].xml"));
    assert_eq!(Document::open(&saved).unwrap().main_text(), "");
}
