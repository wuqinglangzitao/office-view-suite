#![cfg(feature = "docx")]

use std::io::Write;

use rwml::{CoreProperties, Document};

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

fn core_properties_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>Quarter &lt;One&gt; &amp; Co</dc:title><dc:subject>Pipeline</dc:subject><dc:creator>Analyst</dc:creator><dc:description><![CDATA[A document summary]]></dc:description><cp:keywords>rwml,metadata</cp:keywords><cp:category>Operations</cp:category><cp:contentStatus>Draft</cp:contentStatus><cp:lastModifiedBy>Reviewer</cp:lastModifiedBy><dcterms:created>2026-06-01T02:03:04Z</dcterms:created><dcterms:modified>2026-06-02T03:04:05Z</dcterms:modified><cp:lastPrinted>2026-06-03T04:05:06Z</cp:lastPrinted><cp:revision>12</cp:revision><cp:version>1.2</cp:version></cp:coreProperties>"#,
        ),
    ])
}

fn expected_core_properties() -> CoreProperties {
    CoreProperties {
        title: Some("Quarter <One> & Co".to_string()),
        subject: Some("Pipeline".to_string()),
        creator: Some("Analyst".to_string()),
        description: Some("A document summary".to_string()),
        keywords: Some("rwml,metadata".to_string()),
        category: Some("Operations".to_string()),
        content_status: Some("Draft".to_string()),
        last_modified_by: Some("Reviewer".to_string()),
        created: Some("2026-06-01T02:03:04Z".to_string()),
        modified: Some("2026-06-02T03:04:05Z".to_string()),
        last_printed: Some("2026-06-03T04:05:06Z".to_string()),
        revision: Some("12".to_string()),
        version: Some("1.2".to_string()),
    }
}

fn model_core_properties(document: &Document) -> CoreProperties {
    let model = document.model();
    let setup = &model.setup;
    CoreProperties {
        title: setup.title.clone(),
        subject: setup.subject.clone(),
        creator: setup.creator.clone(),
        description: setup.description.clone(),
        keywords: setup.keywords.clone(),
        category: setup.category.clone(),
        content_status: setup.content_status.clone(),
        last_modified_by: setup.last_modified_by.clone(),
        created: setup.created.clone(),
        modified: setup.modified.clone(),
        last_printed: setup.last_printed.clone(),
        revision: setup.revision.clone(),
        version: setup.version.clone(),
    }
}

#[test]
fn docx_core_properties_are_extracted() {
    let doc = Document::open(&core_properties_docx()).expect("fixture opens");

    assert_eq!(doc.core_properties(), expected_core_properties());
}

#[test]
fn opened_docx_fresh_conversion_preserves_all_core_properties() {
    let document = Document::open(&core_properties_docx()).expect("fixture opens");
    let expected = expected_core_properties();
    let source_model = document.model();

    assert_eq!(model_core_properties(&document), expected);
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model, "source model was mutated");

    let reopened = Document::open(&converted).expect("converted metadata reopens");
    assert_eq!(reopened.core_properties(), expected);
    assert_eq!(model_core_properties(&reopened), expected);
    assert_eq!(reopened.to_docx(), converted, "reconversion changed bytes");
}

#[test]
fn missing_core_properties_are_empty() {
    assert_eq!(Document::new().core_properties(), CoreProperties::default());
}
