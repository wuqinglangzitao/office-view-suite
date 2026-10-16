#![cfg(all(feature = "docx", feature = "render"))]

use std::io::Write;

use rwml::Document;

fn nested_table_pagination_docx(keep_lines: bool, cant_split: bool) -> Vec<u8> {
    let pagination = if keep_lines {
        "<w:keepLines/>"
    } else {
        r#"<w:keepLines w:val="off"/>"#
    };
    let row_pagination = if cant_split {
        "<w:trPr><w:cantSplit/></w:trPr>"
    } else {
        r#"<w:trPr><w:cantSplit w:val="off"/></w:trPr>"#
    };
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:pPr><w:spacing w:line="480"/></w:pPr><w:r><w:t>seed</w:t></w:r></w:p>
            <w:tbl><w:tr><w:tc>
                <w:tbl><w:tr>{row_pagination}<w:tc>
                    <w:p><w:pPr>{pagination}<w:widowControl w:val="off"/></w:pPr>
                        <w:r><w:t>one</w:t><w:br/><w:t>two</w:t><w:br/><w:t>three</w:t></w:r>
                    </w:p>
                </w:tc></w:tr></w:tbl>
                <w:p/>
            </w:tc></w:tr></w:tbl>
            <w:p><w:pPr><w:widowControl w:val="off"/></w:pPr><w:r><w:t>after</w:t></w:r></w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
        </w:body></w:document>"#
    );
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
            ),
            ("word/document.xml", document_xml.as_str()),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

#[test]
fn opened_docx_render_honors_nested_table_cell_keep_lines() {
    let splittable =
        Document::open(&nested_table_pagination_docx(false, false)).expect("off fixture opens");
    let kept =
        Document::open(&nested_table_pagination_docx(true, false)).expect("on fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let splittable_pages = splittable
        .layout_pages_with_fonts(&fonts)
        .expect("off fixture lays out")
        .pages;
    let kept_pages = kept
        .layout_pages_with_fonts(&fonts)
        .expect("on fixture lays out")
        .pages;

    assert_eq!(
        (splittable_pages, kept_pages),
        (2, 3),
        "nested keepLines must move the protected paragraph to a fresh page"
    );
}

#[test]
fn opened_docx_render_honors_nested_table_row_cant_split() {
    let splittable =
        Document::open(&nested_table_pagination_docx(false, false)).expect("off fixture opens");
    let kept =
        Document::open(&nested_table_pagination_docx(false, true)).expect("on fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let splittable_pages = splittable
        .layout_pages_with_fonts(&fonts)
        .expect("off fixture lays out")
        .pages;
    let kept_pages = kept
        .layout_pages_with_fonts(&fonts)
        .expect("on fixture lays out")
        .pages;

    assert_eq!(
        (splittable_pages, kept_pages),
        (2, 3),
        "nested cantSplit must move the protected row to a fresh page"
    );
}
