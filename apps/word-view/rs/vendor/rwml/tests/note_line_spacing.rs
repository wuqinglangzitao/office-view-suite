#![cfg(all(feature = "docx", feature = "render"))]

use std::io::Write;

use rwml::Document;

fn docx_fixture(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn note_line_spacing_docx(footnote_properties: &str, endnote_properties: &str) -> Vec<u8> {
    let footnotes = format!(
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>
            <w:footnote w:id="1"><w:p><w:pPr>{footnote_properties}</w:pPr>
                <w:r><w:t>footnote absolute spacing</w:t></w:r></w:p></w:footnote>
        </w:footnotes>"#
    );
    let endnotes = format!(
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:type="continuationSeparator" w:id="-1"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:endnote>
            <w:endnote w:id="2"><w:p><w:pPr>{endnote_properties}</w:pPr>
                <w:r><w:t>endnote absolute spacing</w:t></w:r></w:p></w:endnote>
        </w:endnotes>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/footnotes.xml", &footnotes),
        ("word/endnotes.xml", &endnotes),
    ])
}

#[test]
fn opened_docx_render_consumes_footnote_and_endnote_absolute_line_spacing() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_model = Document::open(&note_line_spacing_docx("", ""))
        .expect("baseline note fixture opens")
        .model();
    let render = |footnote_properties: &str, endnote_properties: &str| {
        let document = Document::open(&note_line_spacing_docx(
            footnote_properties,
            endnote_properties,
        ))
        .expect("note line-spacing fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "absolute note spacing must remain outside the public model"
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let baseline = render("", "");
    let exact = render(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#, "");
    let at_least = render("", r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#);

    assert!(baseline.starts_with(b"%PDF-"));
    assert_ne!(
        exact, baseline,
        "footnote exact spacing must affect PDF output"
    );
    assert_ne!(
        at_least, baseline,
        "endnote minimum spacing must affect PDF output"
    );
    assert_eq!(
        exact,
        render(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#, ""),
        "footnote spacing output must be deterministic"
    );
    assert_eq!(
        at_least,
        render("", r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#),
        "endnote spacing output must be deterministic"
    );
}
