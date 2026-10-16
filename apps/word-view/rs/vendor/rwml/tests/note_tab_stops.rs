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

fn note_tab_docx(footnote_tabs: &str, endnote_tabs: &str, default_tab_stop_twips: u32) -> Vec<u8> {
    let footnotes = format!(
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:tabs><w:tab w:val="left" w:pos="2400"/></w:tabs></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>
            <w:footnote w:id="1">
                <w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>footnote table prefix</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:p><w:pPr>{footnote_tabs}</w:pPr><w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r></w:p>
            </w:footnote>
        </w:footnotes>"#
    );
    let endnotes = format!(
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:type="continuationSeparator" w:id="-1"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:endnote>
            <w:endnote w:id="2"><w:p><w:pPr>{endnote_tabs}</w:pPr><w:r><w:t>C</w:t><w:tab/><w:t>D</w:t></w:r></w:p></w:endnote>
        </w:endnotes>"#
    );
    let settings = format!(
        r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="{default_tab_stop_twips}"/></w:settings>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="8000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/settings.xml", &settings),
        ("word/footnotes.xml", &footnotes),
        ("word/endnotes.xml", &endnotes),
    ])
}

#[test]
fn opened_docx_render_consumes_real_note_paragraph_tab_stops() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_document =
        Document::open(&note_tab_docx("", "", 720)).expect("baseline note-tab fixture opens");
    let baseline_model = baseline_document.model();
    let baseline_pages = baseline_document
        .layout_pages_with_fonts(&fonts)
        .expect("baseline note-tab layout succeeds")
        .pages;
    let baseline = baseline_document.to_pdf_with_fonts(&fonts);
    let render = |footnote_tabs: &str, endnote_tabs: &str, default_stop: u32| {
        let document = Document::open(&note_tab_docx(footnote_tabs, endnote_tabs, default_stop))
            .expect("note-tab fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "real-note tab stops must remain outside the public model"
        );
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("note-tab layout succeeds")
                .pages,
            baseline_pages
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let footnote_explicit = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
        "",
        720,
    );
    let footnote_without_leader = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs>"#,
        "",
        720,
    );
    let endnote_explicit = render(
        "",
        r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
        720,
    );
    let wider_default = render("", "", 1440);
    let malformed = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="invalid" w:leader="dot"/></w:tabs>"#,
        "",
        720,
    );

    assert!(baseline.starts_with(b"%PDF-"));
    for (name, rendered) in [
        ("footnote explicit tab", &footnote_explicit),
        (
            "footnote explicit tab without leader",
            &footnote_without_leader,
        ),
        ("endnote explicit tab", &endnote_explicit),
        ("settings default interval", &wider_default),
    ] {
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, &baseline, "{name} must affect PDF output");
    }
    assert_ne!(footnote_explicit, footnote_without_leader);
    assert_ne!(footnote_explicit, endnote_explicit);
    assert_ne!(footnote_explicit, wider_default);
    assert_ne!(endnote_explicit, wider_default);
    assert_eq!(malformed, baseline);
    assert_eq!(
        footnote_explicit,
        render(
            r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
            "",
            720,
        )
    );
    assert_eq!(
        endnote_explicit,
        render(
            "",
            r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
            720,
        )
    );
    assert_eq!(wider_default, render("", "", 1440));
}

fn note_table_tab_docx(
    footnote_tabs: &str,
    endnote_tabs: &str,
    default_tab_stop_twips: u32,
) -> Vec<u8> {
    let footnotes = format!(
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:type="separator" w:id="-1"><w:tbl><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:p><w:pPr><w:tabs><w:tab w:val="left" w:pos="2400"/></w:tabs></w:pPr><w:r><w:t>X</w:t><w:tab/><w:t>Y</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:footnote>
            <w:footnote w:id="1">
                <w:p><w:r><w:t>footnote block prefix</w:t></w:r></w:p>
                <w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid>
                    <w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:pPr>{footnote_tabs}</w:pPr><w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:r><w:t>covered merge cell</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
            </w:footnote>
        </w:footnotes>"#
    );
    let endnotes = format!(
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:type="continuationSeparator" w:id="-1"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:endnote>
            <w:endnote w:id="2"><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:p><w:pPr>{endnote_tabs}</w:pPr><w:r><w:t>C</w:t><w:tab/><w:t>D</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:endnote>
        </w:endnotes>"#
    );
    let settings = format!(
        r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="{default_tab_stop_twips}"/></w:settings>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="8000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/settings.xml", &settings),
        ("word/footnotes.xml", &footnotes),
        ("word/endnotes.xml", &endnotes),
    ])
}

#[test]
fn opened_docx_render_consumes_real_note_table_cell_tab_stops() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_document = Document::open(&note_table_tab_docx("", "", 720))
        .expect("baseline note-table-tab fixture opens");
    let baseline_model = baseline_document.model();
    let baseline_pages = baseline_document
        .layout_pages_with_fonts(&fonts)
        .expect("baseline note-table-tab layout succeeds")
        .pages;
    let baseline = baseline_document.to_pdf_with_fonts(&fonts);
    let render = |footnote_tabs: &str, endnote_tabs: &str, default_stop: u32| {
        let document = Document::open(&note_table_tab_docx(
            footnote_tabs,
            endnote_tabs,
            default_stop,
        ))
        .expect("note-table-tab fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "real-note table-cell tabs must remain outside the public model"
        );
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("note-table-tab layout succeeds")
                .pages,
            baseline_pages
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let footnote_explicit = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
        "",
        720,
    );
    let footnote_without_leader = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs>"#,
        "",
        720,
    );
    let endnote_explicit = render(
        "",
        r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
        720,
    );
    let wider_default = render("", "", 1440);
    let malformed = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="invalid" w:leader="dot"/></w:tabs>"#,
        "",
        720,
    );

    assert!(baseline.starts_with(b"%PDF-"));
    for (name, rendered) in [
        ("footnote table-cell explicit tab", &footnote_explicit),
        (
            "footnote table-cell explicit tab without leader",
            &footnote_without_leader,
        ),
        ("endnote table-cell explicit tab", &endnote_explicit),
        ("settings default interval", &wider_default),
    ] {
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, &baseline, "{name} must affect PDF output");
    }
    assert_ne!(footnote_explicit, footnote_without_leader);
    assert_ne!(footnote_explicit, endnote_explicit);
    assert_ne!(footnote_explicit, wider_default);
    assert_ne!(endnote_explicit, wider_default);
    assert_eq!(malformed, baseline);
    assert_eq!(
        footnote_explicit,
        render(
            r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
            "",
            720,
        )
    );
    assert_eq!(
        endnote_explicit,
        render(
            "",
            r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
            720,
        )
    );
    assert_eq!(wider_default, render("", "", 1440));
}
