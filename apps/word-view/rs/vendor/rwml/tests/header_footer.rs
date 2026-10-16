#![cfg(feature = "docx")]

use std::io::{Read, Write};

#[cfg(feature = "render")]
use rwml::FieldRole;
use rwml::{Block, Document, HeaderFooterKind};

fn docx_fixture(parts: &[(&str, &str)]) -> Vec<u8> {
    let parts = parts
        .iter()
        .map(|(name, body)| (*name, body.as_bytes()))
        .collect::<Vec<_>>();
    docx_fixture_bytes(&parts)
}

fn docx_fixture_bytes(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let opt = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(*name, opt).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn unzip_parts(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut parts = std::collections::BTreeMap::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).unwrap();
        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        parts.insert(file.name().to_string(), body);
    }
    parts
}

#[cfg(feature = "render")]
fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36,
        0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x60,
        0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

fn header_footer_variants_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDefaultHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFirstHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdEvenHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header3.xml"/><Relationship Id="rIdDefaultFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdFirstFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer3.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdDefaultHeader"/><w:headerReference w:type=" first " r:id="rIdFirstHeader"/><w:headerReference w:type=" even " r:id="rIdEvenHeader"/><w:footerReference w:type="default" r:id="rIdDefaultFooter"/><w:footerReference w:type=" first " r:id="rIdFirstFooter"/><w:footerReference w:type=" even " r:id="rIdEvenFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>DEFAULT HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header3.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>EVEN HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>DEFAULT FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/footer2.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/footer3.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>EVEN FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

fn running_surface_pagination_docx(include_pagination: bool) -> Vec<u8> {
    let paragraph_props = if include_pagination {
        r#"<w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/></w:pPr>"#
    } else {
        ""
    };
    let table_row_props = if include_pagination {
        "<w:trPr><w:cantSplit/></w:trPr>"
    } else {
        ""
    };
    let table_cell_props = paragraph_props;
    let running_part = |root: &str, marker: &str, table: bool| {
        let table = if table {
            format!(
                concat!(
                    "<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w=\"2000\"/></w:tblGrid>",
                    "<w:tr>{table_row_props}<w:tc><w:tcPr/>",
                    "<w:p>{table_cell_props}<w:r><w:t>TABLE CELL</w:t></w:r></w:p>",
                    "</w:tc></w:tr></w:tbl>"
                ),
                table_row_props = table_row_props,
                table_cell_props = table_cell_props,
            )
        } else {
            String::new()
        };
        format!(
            concat!(
                "<w:{root} xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
                "<w:p>{paragraph_props}<w:r><w:t>{marker}</w:t></w:r></w:p>",
                "{table}</w:{root}>"
            ),
            root = root,
            paragraph_props = paragraph_props,
            marker = marker,
            table = table,
        )
    };
    let default_header = running_part("hdr", "DEFAULT HEAD PAGINATION", true);
    let first_header = running_part("hdr", "FIRST HEAD PAGINATION", false);
    let even_header = running_part("hdr", "EVEN HEAD PAGINATION", false);
    let default_footer = running_part("ftr", "DEFAULT FOOT PAGINATION", false);
    let first_footer = running_part("ftr", "FIRST FOOT PAGINATION", false);
    let even_footer = running_part("ftr", "EVEN FOOT PAGINATION", false);

    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDefaultHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFirstHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdEvenHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header3.xml"/><Relationship Id="rIdDefaultFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdFirstFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer3.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdDefaultHeader"/><w:headerReference w:type="first" r:id="rIdFirstHeader"/><w:headerReference w:type="even" r:id="rIdEvenHeader"/><w:footerReference w:type="default" r:id="rIdDefaultFooter"/><w:footerReference w:type="first" r:id="rIdFirstFooter"/><w:footerReference w:type="even" r:id="rIdEvenFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        ("word/header1.xml", &default_header),
        ("word/header2.xml", &first_header),
        ("word/header3.xml", &even_header),
        ("word/footer1.xml", &default_footer),
        ("word/footer2.xml", &first_footer),
        ("word/footer3.xml", &even_footer),
    ])
}

fn paragraph_with_marker<'a>(xml: &'a str, marker: &str) -> &'a str {
    let marker = format!(">{marker}</w:t>");
    let text_start = xml
        .find(&marker)
        .unwrap_or_else(|| panic!("missing paragraph marker {marker:?}: {xml}"));
    let start = xml[..text_start].rfind("<w:p>").expect("paragraph start");
    let end = xml[text_start..]
        .find("</w:p>")
        .map(|offset| text_start + offset + "</w:p>".len())
        .expect("paragraph end");
    &xml[start..end]
}

#[test]
fn opened_docx_running_surface_pagination_roundtrips_through_fresh_conversion() {
    let paginated = Document::open(&running_surface_pagination_docx(true)).unwrap();
    let control = Document::open(&running_surface_pagination_docx(false)).unwrap();
    let model = paginated.model();
    assert_eq!(model, control.model());

    let converted = paginated.to_docx();
    let control_converted = control.to_docx();
    assert_eq!(converted, paginated.to_docx());
    assert_ne!(converted, control_converted);

    let parts = unzip_parts(&converted);
    for marker in [
        "DEFAULT HEAD PAGINATION",
        "FIRST HEAD PAGINATION",
        "EVEN HEAD PAGINATION",
        "DEFAULT FOOT PAGINATION",
        "FIRST FOOT PAGINATION",
        "EVEN FOOT PAGINATION",
    ] {
        let xml = parts
            .iter()
            .find_map(|(name, body)| {
                ((name.starts_with("word/header") || name.starts_with("word/footer"))
                    && std::str::from_utf8(body).is_ok_and(|xml| xml.contains(marker)))
                .then(|| std::str::from_utf8(body).unwrap())
            })
            .unwrap_or_else(|| panic!("missing generated running part for {marker}"));
        let paragraph = paragraph_with_marker(xml, marker);
        assert!(paragraph.contains("<w:keepNext/>"), "{marker}: {xml}");
        assert!(paragraph.contains("<w:keepLines/>"), "{marker}: {xml}");
        assert!(
            paragraph.contains(r#"<w:widowControl w:val="0"/>"#),
            "{marker}: {xml}"
        );
    }

    let default_header = parts
        .iter()
        .find_map(|(name, body)| {
            (name.starts_with("word/header")
                && std::str::from_utf8(body)
                    .is_ok_and(|xml| xml.contains("DEFAULT HEAD PAGINATION")))
            .then(|| std::str::from_utf8(body).unwrap())
        })
        .expect("generated default header");
    assert!(
        default_header.contains("<w:cantSplit/>"),
        "{default_header}"
    );
    let cell = paragraph_with_marker(default_header, "TABLE CELL");
    assert!(cell.contains("<w:keepNext/>"), "{cell}");
    assert!(cell.contains("<w:keepLines/>"), "{cell}");
    assert!(cell.contains(r#"<w:widowControl w:val="0"/>"#), "{cell}");

    assert_eq!(
        Document::open(&converted).unwrap().model(),
        Document::open(&control_converted).unwrap().model()
    );
    let standalone = unzip_parts(&rwml::write_docx(&model));
    assert!(standalone.iter().all(|(name, body)| {
        !(name.starts_with("word/header") || name.starts_with("word/footer"))
            || std::str::from_utf8(body).is_ok_and(|xml| {
                !xml.contains("<w:keepNext")
                    && !xml.contains("<w:keepLines")
                    && !xml.contains("<w:widowControl")
                    && !xml.contains("<w:cantSplit")
            })
    }));
}

fn header_footer_alternate_content_refs_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdChoiceHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdChoiceFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdFallbackHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdFallbackFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><mc:AlternateContent><mc:Choice Requires="w14"><w:headerReference w:type="default" r:id="rIdChoiceHeader"/><w:footerReference w:type="first" r:id="rIdChoiceFooter"/></mc:Choice><mc:Fallback><w:headerReference w:type="default" r:id="rIdFallbackHeader"/><w:footerReference w:type="first" r:id="rIdFallbackFooter"/></mc:Fallback></mc:AlternateContent></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>CHOICE HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>CHOICE FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FALLBACK HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer2.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FALLBACK FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

fn multi_section_header_footer_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFirstHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdSecondHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdFirstFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdSecondFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>FIRST BODY</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rIdFirstHeader"/><w:footerReference w:type="default" r:id="rIdFirstFooter"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>SECOND BODY</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdSecondHeader"/><w:footerReference w:type="default" r:id="rIdSecondFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>SECOND HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/footer2.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>SECOND FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

fn multi_section_inherited_header_footer_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFirstHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFirstFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>FIRST BODY</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rIdFirstHeader"/><w:footerReference w:type="default" r:id="rIdFirstFooter"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>SECOND BODY</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/></w:pPr><w:r><w:t>INHERITED HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/></w:pPr><w:r><w:t>INHERITED FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

fn multi_section_variant_header_footer_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/header4.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer4.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFirstDefaultHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFirstFirstHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdFirstEvenHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header3.xml"/><Relationship Id="rIdFirstDefaultFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdFirstFirstFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/><Relationship Id="rIdFirstEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer3.xml"/><Relationship Id="rIdSecondHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header4.xml"/><Relationship Id="rIdSecondFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer4.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>FIRST BODY</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:titlePg/><w:headerReference w:type="default" r:id="rIdFirstDefaultHeader"/><w:headerReference w:type="first" r:id="rIdFirstFirstHeader"/><w:headerReference w:type="even" r:id="rIdFirstEvenHeader"/><w:footerReference w:type="default" r:id="rIdFirstDefaultFooter"/><w:footerReference w:type="first" r:id="rIdFirstFirstFooter"/><w:footerReference w:type="even" r:id="rIdFirstEvenFooter"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>SECOND BODY</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdSecondHeader"/><w:footerReference w:type="default" r:id="rIdSecondFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST DEFAULT HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST FIRST HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header3.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST EVEN HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST DEFAULT FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/footer2.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST FIRST FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/footer3.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FIRST EVEN FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
        (
            "word/header4.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>SECOND HEAD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer4.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>SECOND FOOT</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

#[cfg(feature = "render")]
fn running_surface_image_docx(header_image: bool, footer_image: bool) -> Vec<u8> {
    let empty_header = br#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:hdr>"#;
    let image_header = br#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="Header logo" descr="Header logo"/><a:blip r:embed="rIdImage"/></wp:inline></w:drawing></w:r></w:p></w:hdr>"#;
    let empty_footer = br#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:ftr>"#;
    let image_footer = br#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="Footer mark" descr="Footer mark"/><a:blip r:embed="rIdImage"/></wp:inline></w:drawing></w:r></w:p></w:ftr>"#;
    let header = if header_image {
        image_header.as_slice()
    } else {
        empty_header.as_slice()
    };
    let footer = if footer_image {
        image_footer.as_slice()
    } else {
        empty_footer.as_slice()
    };
    let png = tiny_png();

    docx_fixture_bytes(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1200" w:right="400" w:bottom="1200" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        ("word/header1.xml", header),
        ("word/footer1.xml", footer),
        (
            "word/_rels/header1.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/></Relationships>"#,
        ),
        (
            "word/_rels/footer1.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/></Relationships>"#,
        ),
        ("word/media/logo.png", png.as_slice()),
    ])
}

#[cfg(feature = "render")]
fn running_surface_table_image_docx(header_image: bool, footer_image: bool) -> Vec<u8> {
    let empty_paragraph = "<w:p/>";
    let image_paragraph = r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="Cell image" descr="Cell image"/><a:blip r:embed="rIdImage"/></wp:inline></w:drawing></w:r></w:p>"#;
    let part = |root: &str, with_image: bool| {
        let paragraph = if with_image {
            image_paragraph
        } else {
            empty_paragraph
        };
        format!(
            r#"<w:{root} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc>{paragraph}</w:tc></w:tr></w:tbl></w:{root}>"#
        )
    };
    let header = part("hdr", header_image);
    let footer = part("ftr", footer_image);
    let png = tiny_png();

    docx_fixture_bytes(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1600" w:right="400" w:bottom="1600" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        ("word/header1.xml", header.as_bytes()),
        ("word/footer1.xml", footer.as_bytes()),
        (
            "word/_rels/header1.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/></Relationships>"#,
        ),
        (
            "word/_rels/footer1.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/></Relationships>"#,
        ),
        ("word/media/logo.png", png.as_slice()),
    ])
}

#[cfg(feature = "render")]
fn running_surface_table_docx(header_table: bool, footer_table: bool) -> Vec<u8> {
    let empty_header = r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:hdr>"#;
    let table_header = r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:jc w:val="center"/><w:tblBorders><w:top w:val="single" w:sz="12" w:color="C00000"/><w:left w:val="single" w:sz="12" w:color="C00000"/><w:bottom w:val="single" w:sz="12" w:color="C00000"/><w:right w:val="single" w:sz="12" w:color="C00000"/><w:insideH w:val="single" w:sz="8" w:color="006000"/><w:insideV w:val="single" w:sz="8" w:color="006000"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1800"/><w:gridCol w:w="1800"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:shd w:fill="FFF2CC"/></w:tcPr><w:p><w:r><w:t>HEADER A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>HEADER B</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>HEADER C</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:shd w:fill="DDEBF7"/></w:tcPr><w:p><w:r><w:t>HEADER D</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:hdr>"#;
    let empty_footer = r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:ftr>"#;
    let table_footer = r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:jc w:val="center"/><w:tblBorders><w:top w:val="single" w:sz="12" w:color="0000C0"/><w:left w:val="single" w:sz="12" w:color="0000C0"/><w:bottom w:val="single" w:sz="12" w:color="0000C0"/><w:right w:val="single" w:sz="12" w:color="0000C0"/><w:insideH w:val="single" w:sz="8" w:color="600060"/><w:insideV w:val="single" w:sz="8" w:color="600060"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1800"/><w:gridCol w:w="1800"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:shd w:fill="E2F0D9"/></w:tcPr><w:p><w:r><w:t>FOOTER A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>FOOTER B</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>FOOTER C</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:shd w:fill="FCE4D6"/></w:tcPr><w:p><w:r><w:t>FOOTER D</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:ftr>"#;
    let header = if header_table {
        table_header
    } else {
        empty_header
    };
    let footer = if footer_table {
        table_footer
    } else {
        empty_footer
    };

    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1600" w:right="400" w:bottom="1600" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        ("word/header1.xml", header),
        ("word/footer1.xml", footer),
    ])
}

#[cfg(feature = "render")]
fn running_surface_hyperlink_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1600" w:right="400" w:bottom="1600" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p><w:hyperlink r:id="rIdHeaderLink"><w:r><w:t>HEADER LINK</w:t></w:r></w:hyperlink></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:p><w:hyperlink r:id="rIdTableLink"><w:r><w:t>FOOTER TABLE LINK</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:ftr>"#,
        ),
        (
            "word/_rels/header1.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeaderLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/opened-header" TargetMode="External"/></Relationships>"#,
        ),
        (
            "word/_rels/footer1.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdTableLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/opened-footer-table" TargetMode="External"/></Relationships>"#,
        ),
    ])
}

#[cfg(feature = "render")]
fn running_surface_paragraph_gap_docx(header_before: u32, footer_before: u32) -> Vec<u8> {
    let header = format!(
        r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>HEADER TOP</w:t></w:r></w:p><w:p><w:pPr><w:spacing w:before="{header_before}" w:after="0"/></w:pPr><w:r><w:t>HEADER BOTTOM</w:t></w:r></w:p></w:hdr>"#
    );
    let footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>FOOTER TOP</w:t></w:r></w:p><w:p><w:pPr><w:spacing w:before="{footer_before}" w:after="0"/></w:pPr><w:r><w:t>FOOTER BOTTOM</w:t></w:r></w:p></w:ftr>"#
    );

    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1600" w:right="400" w:bottom="1600" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        ("word/header1.xml", &header),
        ("word/footer1.xml", &footer),
    ])
}

fn single_paragraph_text(blocks: &[Block]) -> String {
    let [Block::Paragraph(paragraph)] = blocks else {
        panic!("expected exactly one paragraph block, got {blocks:?}");
    };
    paragraph.text()
}

#[test]
fn docx_header_footer_side_table_preserves_reference_variants() {
    let doc = Document::open(&header_footer_variants_docx()).expect("fixture opens");

    let records = doc.header_footers();
    let actual = records
        .iter()
        .map(|record| (record.id.as_str(), record.kind, record.text.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            (
                "word/header1.xml#default",
                HeaderFooterKind::Header,
                "DEFAULT HEAD"
            ),
            (
                "word/header2.xml#first",
                HeaderFooterKind::FirstPageHeader,
                "FIRST HEAD"
            ),
            (
                "word/header3.xml#even",
                HeaderFooterKind::EvenPageHeader,
                "EVEN HEAD"
            ),
            (
                "word/footer1.xml#default",
                HeaderFooterKind::Footer,
                "DEFAULT FOOT"
            ),
            (
                "word/footer2.xml#first",
                HeaderFooterKind::FirstPageFooter,
                "FIRST FOOT"
            ),
            (
                "word/footer3.xml#even",
                HeaderFooterKind::EvenPageFooter,
                "EVEN FOOT"
            ),
        ]
    );

    assert_eq!(
        doc.header_text(),
        "DEFAULT HEAD\nFIRST HEAD\nEVEN HEAD\nDEFAULT FOOT\nFIRST FOOT\nEVEN FOOT",
        "header_text should expose all modeled default/first/even running variants"
    );

    let model = doc.model();
    assert_eq!(model.setup.header.len(), 1);
    assert_eq!(model.setup.first_header.len(), 1);
    assert_eq!(model.setup.even_header.len(), 1);
    assert_eq!(model.setup.footer.len(), 1);
    assert_eq!(model.setup.first_footer.len(), 1);
    assert_eq!(model.setup.even_footer.len(), 1);
    let Block::Paragraph(header) = &model.setup.header[0] else {
        panic!("default header block should be a paragraph");
    };
    let Block::Paragraph(first_header) = &model.setup.first_header[0] else {
        panic!("first-page header block should be a paragraph");
    };
    let Block::Paragraph(even_header) = &model.setup.even_header[0] else {
        panic!("even-page header block should be a paragraph");
    };
    let Block::Paragraph(footer) = &model.setup.footer[0] else {
        panic!("default footer block should be a paragraph");
    };
    let Block::Paragraph(first_footer) = &model.setup.first_footer[0] else {
        panic!("first-page footer block should be a paragraph");
    };
    let Block::Paragraph(even_footer) = &model.setup.even_footer[0] else {
        panic!("even-page footer block should be a paragraph");
    };
    assert_eq!(header.text(), "DEFAULT HEAD");
    assert_eq!(first_header.text(), "FIRST HEAD");
    assert_eq!(even_header.text(), "EVEN HEAD");
    assert_eq!(footer.text(), "DEFAULT FOOT");
    assert_eq!(first_footer.text(), "FIRST FOOT");
    assert_eq!(even_footer.text(), "EVEN FOOT");
}

#[test]
fn docx_header_footer_refs_use_single_alternate_content_branch() {
    let doc = Document::open(&header_footer_alternate_content_refs_docx()).expect("fixture opens");

    let records = doc.header_footers();
    let actual = records
        .iter()
        .map(|record| (record.id.as_str(), record.kind, record.text.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            (
                "word/header1.xml#default",
                HeaderFooterKind::Header,
                "CHOICE HEAD"
            ),
            (
                "word/footer1.xml#first",
                HeaderFooterKind::FirstPageFooter,
                "CHOICE FOOT"
            ),
        ]
    );

    assert_eq!(doc.header_text(), "CHOICE HEAD\nCHOICE FOOT");
}

#[test]
fn docx_multi_section_default_headers_attach_to_section_boundaries() {
    let doc = Document::open(&multi_section_header_footer_docx()).expect("fixture opens");
    let model = doc.model();

    let section = model
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::SectionBreak(setup) => Some(setup),
            _ => None,
        })
        .expect("expected a section break for the paragraph sectPr");

    assert_eq!(single_paragraph_text(&section.header), "FIRST HEAD");
    assert_eq!(single_paragraph_text(&section.footer), "FIRST FOOT");
    assert_eq!(single_paragraph_text(&model.setup.header), "SECOND HEAD");
    assert_eq!(single_paragraph_text(&model.setup.footer), "SECOND FOOT");

    assert_eq!(
        doc.header_text(),
        "FIRST HEAD\nFIRST FOOT\nSECOND HEAD\nSECOND FOOT",
        "header_text should expose default running surfaces for each section without folding them all into the final setup"
    );
    let text = doc.text();
    assert!(
        text.contains("FIRST HEAD")
            && text.contains("FIRST FOOT")
            && text.contains("SECOND HEAD")
            && text.contains("SECOND FOOT"),
        "full text should retain all section default header/footer surfaces: {text:?}"
    );
}

#[test]
fn docx_section_defaults_inherit_from_previous_section_when_omitted() {
    let doc = Document::open(&multi_section_inherited_header_footer_docx()).expect("fixture opens");
    let model = doc.model();

    let section = model
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::SectionBreak(setup) => Some(setup),
            _ => None,
        })
        .expect("expected a section break for the paragraph sectPr");

    assert_eq!(single_paragraph_text(&section.header), "INHERITED HEAD");
    assert_eq!(single_paragraph_text(&section.footer), "INHERITED FOOT");
    assert_eq!(single_paragraph_text(&model.setup.header), "INHERITED HEAD");
    assert_eq!(single_paragraph_text(&model.setup.footer), "INHERITED FOOT");
    assert_eq!(
        doc.header_footers().len(),
        2,
        "inherited section surfaces should not duplicate side-table part records"
    );

    let parts = unzip_parts(&doc.to_docx());
    for marker in ["INHERITED HEAD", "INHERITED FOOT"] {
        let selected = parts
            .iter()
            .filter_map(|(name, body)| {
                (name.starts_with("word/header") || name.starts_with("word/footer"))
                    .then(|| std::str::from_utf8(body).unwrap())
                    .filter(|xml| xml.contains(marker))
            })
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 2, "{marker}: {selected:?}");
        for xml in selected {
            let paragraph = paragraph_with_marker(xml, marker);
            assert!(paragraph.contains("<w:keepNext/>"), "{paragraph}");
            assert!(paragraph.contains("<w:keepLines/>"), "{paragraph}");
            assert!(
                paragraph.contains(r#"<w:widowControl w:val="0"/>"#),
                "{paragraph}"
            );
        }
    }
}

#[test]
fn docx_multi_section_first_even_headers_attach_to_section_boundaries() {
    let doc = Document::open(&multi_section_variant_header_footer_docx()).expect("fixture opens");
    let model = doc.model();

    let section = model
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::SectionBreak(setup) => Some(setup),
            _ => None,
        })
        .expect("expected a section break for the paragraph sectPr");

    assert_eq!(single_paragraph_text(&section.header), "FIRST DEFAULT HEAD");
    assert_eq!(
        single_paragraph_text(&section.first_header),
        "FIRST FIRST HEAD"
    );
    assert_eq!(
        single_paragraph_text(&section.even_header),
        "FIRST EVEN HEAD"
    );
    assert_eq!(single_paragraph_text(&section.footer), "FIRST DEFAULT FOOT");
    assert_eq!(
        single_paragraph_text(&section.first_footer),
        "FIRST FIRST FOOT"
    );
    assert_eq!(
        single_paragraph_text(&section.even_footer),
        "FIRST EVEN FOOT"
    );
    assert_eq!(single_paragraph_text(&model.setup.header), "SECOND HEAD");
    assert!(model.setup.first_header.is_empty());
    assert!(model.setup.even_header.is_empty());
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_paints_decoded_running_header_and_footer_images() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let open = |header, footer| {
        Document::open(&running_surface_image_docx(header, footer))
            .expect("running-surface image fixture opens")
    };
    let baseline = open(false, false);
    let header = open(true, false);
    let footer = open(false, true);
    let both = open(true, true);

    let header_model = header.model();
    let [Block::Paragraph(header_paragraph)] = header_model.setup.header.as_slice() else {
        panic!("decoded header image must remain in its source paragraph");
    };
    assert_eq!(
        header_paragraph
            .runs
            .iter()
            .filter(|run| run.image.is_some())
            .count(),
        1
    );
    let footer_model = footer.model();
    let [Block::Paragraph(footer_paragraph)] = footer_model.setup.footer.as_slice() else {
        panic!("decoded footer image must remain in its source paragraph");
    };
    assert_eq!(
        footer_paragraph
            .runs
            .iter()
            .filter(|run| run.image.is_some())
            .count(),
        1
    );
    for document in [&baseline, &header, &footer, &both] {
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running-surface image layout succeeds")
                .pages,
            1
        );
    }

    let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
    let header_pdf = header.to_pdf_with_fonts(&fonts);
    let footer_pdf = footer.to_pdf_with_fonts(&fonts);
    let both_pdf = both.to_pdf_with_fonts(&fonts);
    for rendered in [&header_pdf, &footer_pdf, &both_pdf] {
        assert!(rendered.starts_with(b"%PDF-"));
        assert!(
            rendered != &baseline_pdf,
            "decoded running image was dropped"
        );
    }
    assert_ne!(
        header_pdf, footer_pdf,
        "header and footer positions must differ"
    );
    assert_ne!(both_pdf, header_pdf);
    assert_ne!(both_pdf, footer_pdf);
    assert_eq!(header_pdf, header.to_pdf_with_fonts(&fonts));
    assert_eq!(footer_pdf, footer.to_pdf_with_fonts(&fonts));
    assert_eq!(both_pdf, both.to_pdf_with_fonts(&fonts));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_paints_images_inside_running_table_cells() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let open = |header, footer| {
        Document::open(&running_surface_table_image_docx(header, footer))
            .expect("running table-cell image fixture opens")
    };
    let baseline = open(false, false);
    let header = open(true, false);
    let footer = open(false, true);
    let both = open(true, true);
    let cell_image_count = |blocks: &[Block]| {
        let [Block::Table(table)] = blocks else {
            panic!("expected one running table")
        };
        let [row] = table.rows.as_slice() else {
            panic!("expected one running table row")
        };
        let [cell] = row.cells.as_slice() else {
            panic!("expected one running table cell")
        };
        let [Block::Paragraph(paragraph)] = cell.blocks.as_slice() else {
            panic!("expected one running table-cell paragraph")
        };
        paragraph
            .runs
            .iter()
            .filter(|run| {
                run.image
                    .as_ref()
                    .is_some_and(|image| image.bytes.is_some())
            })
            .count()
    };

    assert_eq!(cell_image_count(&header.model().setup.header), 1);
    assert_eq!(cell_image_count(&footer.model().setup.footer), 1);
    for document in [&baseline, &header, &footer, &both] {
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running table-cell image layout succeeds")
                .pages,
            1
        );
    }

    let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
    let header_pdf = header.to_pdf_with_fonts(&fonts);
    let footer_pdf = footer.to_pdf_with_fonts(&fonts);
    let both_pdf = both.to_pdf_with_fonts(&fonts);
    for rendered in [&header_pdf, &footer_pdf, &both_pdf] {
        assert!(rendered.starts_with(b"%PDF-"));
        assert_ne!(rendered, &baseline_pdf, "running cell image was dropped");
    }
    assert_ne!(header_pdf, footer_pdf);
    assert_ne!(both_pdf, header_pdf);
    assert_ne!(both_pdf, footer_pdf);
    assert_eq!(header_pdf, header.to_pdf_with_fonts(&fonts));
    assert_eq!(footer_pdf, footer.to_pdf_with_fonts(&fonts));
    assert_eq!(both_pdf, both.to_pdf_with_fonts(&fonts));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_paints_running_header_and_footer_tables() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let open = |header, footer| {
        Document::open(&running_surface_table_docx(header, footer))
            .expect("running-surface table fixture opens")
    };
    let baseline = open(false, false);
    let header = open(true, false);
    let footer = open(false, true);
    let both = open(true, true);

    let header_model = header.model();
    let [Block::Table(header_table)] = header_model.setup.header.as_slice() else {
        panic!("decoded header table must remain modeled");
    };
    assert_eq!(header_table.rows.len(), 2);
    assert_eq!(header_table.rows[0].cells.len(), 2);
    assert_eq!(header_table.rows[0].cells[0].text(), "HEADER A");
    assert_eq!(header_table.rows[1].cells[1].text(), "HEADER D");
    let footer_model = footer.model();
    let [Block::Table(footer_table)] = footer_model.setup.footer.as_slice() else {
        panic!("decoded footer table must remain modeled");
    };
    assert_eq!(footer_table.rows.len(), 2);
    assert_eq!(footer_table.rows[0].cells.len(), 2);
    assert_eq!(footer_table.rows[0].cells[0].text(), "FOOTER A");
    assert_eq!(footer_table.rows[1].cells[1].text(), "FOOTER D");
    for document in [&baseline, &header, &footer, &both] {
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running-surface table layout succeeds")
                .pages,
            1
        );
    }

    let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
    let header_pdf = header.to_pdf_with_fonts(&fonts);
    let footer_pdf = footer.to_pdf_with_fonts(&fonts);
    let both_pdf = both.to_pdf_with_fonts(&fonts);
    for rendered in [&header_pdf, &footer_pdf, &both_pdf] {
        assert!(rendered.starts_with(b"%PDF-"));
        assert!(
            rendered != &baseline_pdf,
            "modeled running table was dropped"
        );
    }
    assert_ne!(
        header_pdf, footer_pdf,
        "header and footer positions must differ"
    );
    assert_ne!(both_pdf, header_pdf);
    assert_ne!(both_pdf, footer_pdf);
    assert_eq!(header_pdf, header.to_pdf_with_fonts(&fonts));
    assert_eq!(footer_pdf, footer.to_pdf_with_fonts(&fonts));
    assert_eq!(both_pdf, both.to_pdf_with_fonts(&fonts));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_keeps_running_surface_hyperlink_annotations() {
    const HEADER_URL: &str = "https://example.com/opened-header";
    const TABLE_URL: &str = "https://example.com/opened-footer-table";
    let document =
        Document::open(&running_surface_hyperlink_docx()).expect("running-link fixture opens");
    let model = document.model();
    let [Block::Paragraph(header)] = model.setup.header.as_slice() else {
        panic!("expected one linked header paragraph");
    };
    assert!(matches!(
        &header.runs[0].field,
        FieldRole::Hyperlink { url } if url == HEADER_URL
    ));
    let [Block::Table(footer_table)] = model.setup.footer.as_slice() else {
        panic!("expected one linked footer table");
    };
    let [footer_row] = footer_table.rows.as_slice() else {
        panic!("expected one footer table row");
    };
    let [footer_cell] = footer_row.cells.as_slice() else {
        panic!("expected one footer table cell");
    };
    let [Block::Paragraph(footer_paragraph)] = footer_cell.blocks.as_slice() else {
        panic!("expected one linked footer-cell paragraph");
    };
    assert!(matches!(
        &footer_paragraph.runs[0].field,
        FieldRole::Hyperlink { url } if url == TABLE_URL
    ));

    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    assert_eq!(
        document
            .layout_pages_with_fonts(&fonts)
            .expect("running-link layout succeeds")
            .pages,
        1
    );
    let pdf = document.to_pdf_with_fonts(&fonts);
    for target in [HEADER_URL, TABLE_URL] {
        assert!(
            pdf.windows(target.len())
                .any(|window| window == target.as_bytes()),
            "running target missing from reopened PDF: {target}"
        );
    }
    assert_eq!(pdf, document.to_pdf_with_fonts(&fonts));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_applies_running_header_and_footer_paragraph_gaps() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let open = |header_before, footer_before| {
        Document::open(&running_surface_paragraph_gap_docx(
            header_before,
            footer_before,
        ))
        .expect("running-surface paragraph-gap fixture opens")
    };
    let baseline = open(0, 0);
    let header = open(240, 0);
    let footer = open(0, 240);
    let both = open(240, 240);

    let header_model = header.model();
    let [Block::Paragraph(_), Block::Paragraph(header_second)] =
        header_model.setup.header.as_slice()
    else {
        panic!("decoded header must retain both paragraphs");
    };
    assert_eq!(header_second.props.spacing.before_pt, Some(12.0));
    assert_eq!(header_second.props.spacing.after_pt, Some(0.0));
    let footer_model = footer.model();
    let [Block::Paragraph(_), Block::Paragraph(footer_second)] =
        footer_model.setup.footer.as_slice()
    else {
        panic!("decoded footer must retain both paragraphs");
    };
    assert_eq!(footer_second.props.spacing.before_pt, Some(12.0));
    assert_eq!(footer_second.props.spacing.after_pt, Some(0.0));
    for document in [&baseline, &header, &footer, &both] {
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running-surface paragraph-gap layout succeeds")
                .pages,
            1
        );
    }

    let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
    let header_pdf = header.to_pdf_with_fonts(&fonts);
    let footer_pdf = footer.to_pdf_with_fonts(&fonts);
    let both_pdf = both.to_pdf_with_fonts(&fonts);
    assert_ne!(header_pdf, baseline_pdf, "header gap was dropped");
    assert_ne!(footer_pdf, baseline_pdf, "footer gap was dropped");
    assert_ne!(header_pdf, footer_pdf);
    assert_ne!(both_pdf, header_pdf);
    assert_ne!(both_pdf, footer_pdf);
    assert_eq!(header_pdf, header.to_pdf_with_fonts(&fonts));
    assert_eq!(footer_pdf, footer.to_pdf_with_fonts(&fonts));
    assert_eq!(both_pdf, both.to_pdf_with_fonts(&fonts));
}

fn running_surface_line_spacing_docx(
    ending_header_properties: &str,
    final_header_properties: &str,
    even_footer_properties: &str,
) -> Vec<u8> {
    let ending_header = format!(
        r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr>{ending_header_properties}</w:pPr><w:r><w:t>ENDING RUNNING HEADER</w:t></w:r></w:p></w:hdr>"#
    );
    let final_header = format!(
        r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr>{final_header_properties}</w:pPr><w:r><w:t>FINAL RUNNING HEADER</w:t></w:r></w:p></w:hdr>"#
    );
    let even_footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr>{even_footer_properties}</w:pPr><w:r><w:t>EVEN RUNNING FOOTER</w:t></w:r></w:p></w:ftr>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdEndingHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFinalHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>
                <w:p><w:r><w:t>ending section body</w:t></w:r></w:p>
                <w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1200" w:right="400" w:bottom="1200" w:left="400"/><w:headerReference w:type="default" r:id="rIdEndingHeader"/></w:sectPr></w:pPr></w:p>
                <w:p><w:r><w:t>final section body</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1200" w:right="400" w:bottom="1200" w:left="400"/><w:headerReference w:type="default" r:id="rIdFinalHeader"/><w:footerReference w:type="even" r:id="rIdEvenFooter"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/header1.xml", &ending_header),
        ("word/header2.xml", &final_header),
        ("word/footer1.xml", &even_footer),
    ])
}

#[test]
fn opened_docx_running_surface_absolute_spacing_roundtrips_through_fresh_conversion() {
    let document = Document::open(&running_surface_line_spacing_docx(
        r#"<w:spacing w:line="100" w:lineRule="exact"/>"#,
        r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#,
        r#"<w:spacing w:line="120" w:lineRule="exact"/>"#,
    ))
    .expect("running-surface line-spacing fixture opens");
    let model = document.model();
    let converted = document.to_docx();

    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(
        Document::open(&converted)
            .expect("fresh conversion reopens")
            .model(),
        model
    );

    let parts = unzip_parts(&converted);
    let running_part = |needle: &str| {
        parts
            .iter()
            .find_map(|(name, body)| {
                ((name.starts_with("word/header") || name.starts_with("word/footer"))
                    && std::str::from_utf8(body).is_ok_and(|xml| xml.contains(needle)))
                .then(|| std::str::from_utf8(body).unwrap())
            })
            .unwrap_or_else(|| panic!("missing generated running part containing {needle:?}"))
    };
    assert!(running_part("ENDING RUNNING HEADER")
        .contains(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#));
    assert!(running_part("FINAL RUNNING HEADER")
        .contains(r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#));
    assert!(running_part("EVEN RUNNING FOOTER")
        .contains(r#"<w:spacing w:line="120" w:lineRule="exact"/>"#));

    let standalone = unzip_parts(&rwml::write_docx(&model));
    assert!(standalone.iter().all(|(name, body)| {
        !(name.starts_with("word/header") || name.starts_with("word/footer"))
            || !std::str::from_utf8(body).unwrap().contains("w:lineRule=")
    }));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_section_and_variant_running_surface_absolute_spacing() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_model = Document::open(&running_surface_line_spacing_docx("", "", ""))
        .expect("baseline running-surface fixture opens")
        .model();
    let render = |ending_header_properties: &str,
                  final_header_properties: &str,
                  even_footer_properties: &str| {
        let document = Document::open(&running_surface_line_spacing_docx(
            ending_header_properties,
            final_header_properties,
            even_footer_properties,
        ))
        .expect("running-surface fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "absolute running-surface spacing must remain outside the public model"
        );
        let layout = document
            .layout_pages_with_fonts(&fonts)
            .expect("running-surface layout succeeds");
        assert_eq!(layout.pages, 2, "fixture must keep one page per section");
        document.to_pdf_with_fonts(&fonts)
    };

    let baseline = render("", "", "");
    let ending_exact = render(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#, "", "");
    let final_minimum = render("", r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#, "");
    let even_footer_exact = render("", "", r#"<w:spacing w:line="100" w:lineRule="exact"/>"#);

    for (name, rendered) in [
        ("ending-section exact header", &ending_exact),
        ("final-section minimum header", &final_minimum),
        ("even-page exact footer", &even_footer_exact),
    ] {
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, &baseline, "{name} must affect PDF output");
    }
    assert_ne!(ending_exact, final_minimum);
    assert_ne!(ending_exact, even_footer_exact);
    assert_ne!(final_minimum, even_footer_exact);
    assert_eq!(
        ending_exact,
        render(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#, "", "")
    );
    assert_eq!(
        final_minimum,
        render("", r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#, "")
    );
    assert_eq!(
        even_footer_exact,
        render("", "", r#"<w:spacing w:line="100" w:lineRule="exact"/>"#)
    );
}

fn running_surface_table_line_spacing_docx(
    header_cell_properties: &str,
    footer_cell_properties: &str,
    even_footer_cell_properties: &str,
) -> Vec<u8> {
    let table = |label: &str, properties: &str| {
        format!(
            r#"<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:p><w:pPr>{properties}</w:pPr><w:r><w:t>{label}</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#
        )
    };
    let header = format!(
        r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>HEADER PREFIX</w:t></w:r></w:p>{}</w:hdr>"#,
        table("HEADER TABLE", header_cell_properties)
    );
    let footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>FOOTER PREFIX</w:t></w:r></w:p>{}</w:ftr>"#,
        table("DEFAULT FOOTER TABLE", footer_cell_properties)
    );
    let even_footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>EVEN FOOTER PREFIX</w:t></w:r></w:p>{}</w:ftr>"#,
        table("EVEN FOOTER TABLE", even_footer_cell_properties)
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>
                <w:p><w:r><w:t>PAGE ONE</w:t></w:r></w:p>
                <w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="4400" w:h="7000"/><w:pgMar w:top="1800" w:right="400" w:bottom="1800" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:pPr></w:p>
                <w:p><w:r><w:t>PAGE TWO</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="7000"/><w:pgMar w:top="1800" w:right="400" w:bottom="1800" w:left="400"/><w:footerReference w:type="even" r:id="rIdEvenFooter"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/header1.xml", &header),
        ("word/footer1.xml", &footer),
        ("word/footer2.xml", &even_footer),
    ])
}

#[test]
fn opened_docx_running_table_cell_absolute_spacing_roundtrips_through_fresh_conversion() {
    let document = Document::open(&running_surface_table_line_spacing_docx(
        r#"<w:spacing w:line="100" w:lineRule="exact"/>"#,
        r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#,
        r#"<w:spacing w:line="120" w:lineRule="exact"/>"#,
    ))
    .expect("running-table line-spacing fixture opens");
    let model = document.model();
    let converted = document.to_docx();

    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(
        Document::open(&converted)
            .expect("fresh conversion reopens")
            .model(),
        model
    );

    let parts = unzip_parts(&converted);
    let running_parts = |needle: &str| {
        parts
            .iter()
            .filter(|(name, body)| {
                (name.starts_with("word/header") || name.starts_with("word/footer"))
                    && std::str::from_utf8(body).is_ok_and(|xml| xml.contains(needle))
            })
            .map(|(_, body)| std::str::from_utf8(body).unwrap())
            .collect::<Vec<_>>()
    };
    let headers = running_parts("HEADER TABLE");
    assert_eq!(
        headers.len(),
        2,
        "default header is effective in both sections"
    );
    assert!(headers
        .iter()
        .all(|xml| { xml.contains(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#) }));

    let default_footers = running_parts("DEFAULT FOOTER TABLE");
    assert_eq!(
        default_footers.len(),
        2,
        "default footer is effective in both sections"
    );
    assert!(default_footers
        .iter()
        .all(|xml| { xml.contains(r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#) }));

    let even_footers = running_parts("EVEN FOOTER TABLE");
    assert_eq!(even_footers.len(), 1);
    assert!(even_footers[0].contains(r#"<w:spacing w:line="120" w:lineRule="exact"/>"#));

    let standalone = unzip_parts(&rwml::write_docx(&model));
    assert!(standalone.iter().all(|(name, body)| {
        !(name.starts_with("word/header") || name.starts_with("word/footer"))
            || !std::str::from_utf8(body).unwrap().contains("w:lineRule=")
    }));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_running_table_cell_absolute_spacing() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_model = Document::open(&running_surface_table_line_spacing_docx("", "", ""))
        .expect("baseline running-table fixture opens")
        .model();
    let render = |header: &str, footer: &str, even_footer: &str| {
        let document = Document::open(&running_surface_table_line_spacing_docx(
            header,
            footer,
            even_footer,
        ))
        .expect("running-table fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "running-table absolute spacing must remain outside the public model"
        );
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running-table layout succeeds")
                .pages,
            2
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let baseline = render("", "", "");
    let header_exact = render(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#, "", "");
    let footer_minimum = render("", r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#, "");
    let even_footer_exact = render("", "", r#"<w:spacing w:line="100" w:lineRule="exact"/>"#);

    for (name, rendered) in [
        ("default-header exact cell", &header_exact),
        ("default-footer minimum cell", &footer_minimum),
        ("even-footer exact cell", &even_footer_exact),
    ] {
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, &baseline, "{name} must affect PDF output");
    }
    assert_ne!(header_exact, footer_minimum);
    assert_ne!(header_exact, even_footer_exact);
    assert_ne!(footer_minimum, even_footer_exact);
    assert_eq!(
        header_exact,
        render(r#"<w:spacing w:line="100" w:lineRule="exact"/>"#, "", "")
    );
    assert_eq!(
        footer_minimum,
        render("", r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#, "")
    );
    assert_eq!(
        even_footer_exact,
        render("", "", r#"<w:spacing w:line="100" w:lineRule="exact"/>"#)
    );
}

fn running_surface_paragraph_tab_docx(
    header_tabs: &str,
    even_footer_tabs: &str,
    default_tab_stop_twips: u32,
) -> Vec<u8> {
    let paragraph = |label: &str, tabs: &str| {
        format!(
            r#"<w:p><w:pPr>{tabs}</w:pPr><w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r></w:p><w:p><w:r><w:t>{label}</w:t></w:r></w:p>"#
        )
    };
    let header = format!(
        r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{}</w:hdr>"#,
        paragraph("DEFAULT HEADER MARKER", header_tabs)
    );
    let footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{}</w:ftr>"#,
        paragraph("DEFAULT FOOTER MARKER", "")
    );
    let even_footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{}</w:ftr>"#,
        paragraph("EVEN FOOTER MARKER", even_footer_tabs)
    );
    let settings = format!(
        r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="{default_tab_stop_twips}"/></w:settings>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>
                <w:p><w:r><w:t>PAGE ONE</w:t></w:r></w:p>
                <w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="4400" w:h="7000"/><w:pgMar w:top="1800" w:right="400" w:bottom="1800" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:pPr></w:p>
                <w:p><w:r><w:t>PAGE TWO</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="7000"/><w:pgMar w:top="1800" w:right="400" w:bottom="1800" w:left="400"/><w:footerReference w:type="default" r:id="rIdFooter"/><w:footerReference w:type="even" r:id="rIdEvenFooter"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/settings.xml", &settings),
        ("word/header1.xml", &header),
        ("word/footer1.xml", &footer),
        ("word/footer2.xml", &even_footer),
    ])
}

#[test]
fn opened_docx_running_surface_paragraph_tabs_roundtrip_through_fresh_conversion() {
    let document = Document::open(&running_surface_paragraph_tab_docx(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
        r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
        720,
    ))
    .expect("running-paragraph tab fixture opens");
    let model = document.model();
    let converted = document.to_docx();

    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(
        Document::open(&converted)
            .expect("fresh conversion reopens")
            .model(),
        model
    );

    let parts = unzip_parts(&converted);
    let running_parts = |needle: &str| {
        parts
            .iter()
            .filter(|(name, body)| {
                (name.starts_with("word/header") || name.starts_with("word/footer"))
                    && std::str::from_utf8(body).is_ok_and(|xml| xml.contains(needle))
            })
            .map(|(_, body)| std::str::from_utf8(body).unwrap())
            .collect::<Vec<_>>()
    };
    let headers = running_parts("DEFAULT HEADER MARKER");
    assert_eq!(
        headers.len(),
        2,
        "default header is effective in both sections"
    );
    assert!(headers
        .iter()
        .all(|xml| xml
            .contains(r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#)));

    let default_footers = running_parts("DEFAULT FOOTER MARKER");
    assert_eq!(default_footers.len(), 1);
    assert!(default_footers.iter().all(|xml| !xml.contains("<w:tabs>")));

    let even_footers = running_parts("EVEN FOOTER MARKER");
    assert_eq!(even_footers.len(), 1);
    assert!(even_footers[0]
        .contains(r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#));

    let standalone = unzip_parts(&rwml::write_docx(&model));
    assert!(standalone.iter().all(|(name, body)| {
        !(name.starts_with("word/header") || name.starts_with("word/footer"))
            || !std::str::from_utf8(body).unwrap().contains("<w:tabs>")
    }));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_running_surface_paragraph_tab_stops() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_model = Document::open(&running_surface_paragraph_tab_docx("", "", 720))
        .expect("baseline running-paragraph tab fixture opens")
        .model();
    let render = |header_tabs: &str, even_footer_tabs: &str, default_stop: u32| {
        let document = Document::open(&running_surface_paragraph_tab_docx(
            header_tabs,
            even_footer_tabs,
            default_stop,
        ))
        .expect("running-paragraph tab fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "running-paragraph tab stops must remain outside the public model"
        );
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running-paragraph tab layout succeeds")
                .pages,
            2
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let baseline = render("", "", 720);
    let header_explicit = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
        "",
        720,
    );
    let header_without_leader = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs>"#,
        "",
        720,
    );
    let even_footer_explicit = render(
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

    for (name, rendered) in [
        ("default-header explicit tab", &header_explicit),
        (
            "default-header explicit tab without leader",
            &header_without_leader,
        ),
        ("even-footer explicit tab", &even_footer_explicit),
        ("settings default interval", &wider_default),
    ] {
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, &baseline, "{name} must affect PDF output");
    }
    assert_ne!(header_explicit, header_without_leader);
    assert_ne!(header_explicit, even_footer_explicit);
    assert_ne!(header_explicit, wider_default);
    assert_ne!(even_footer_explicit, wider_default);
    assert_eq!(malformed, baseline);
    assert_eq!(
        header_explicit,
        render(
            r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
            "",
            720,
        )
    );
    assert_eq!(
        even_footer_explicit,
        render(
            "",
            r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
            720,
        )
    );
    assert_eq!(wider_default, render("", "", 1440));
}

fn running_surface_table_tab_docx(
    header_tabs: &str,
    even_footer_tabs: &str,
    default_tab_stop_twips: u32,
) -> Vec<u8> {
    let table = |tabs: &str| {
        format!(
            r#"<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:p><w:pPr>{tabs}</w:pPr><w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#
        )
    };
    let header = format!(
        r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>HEADER PREFIX</w:t></w:r></w:p>{}</w:hdr>"#,
        table(header_tabs)
    );
    let footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>DEFAULT FOOTER PREFIX</w:t></w:r></w:p>{}</w:ftr>"#,
        table("")
    );
    let even_footer = format!(
        r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>EVEN FOOTER PREFIX</w:t></w:r></w:p>{}</w:ftr>"#,
        table(even_footer_tabs)
    );
    let settings = format!(
        r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="{default_tab_stop_twips}"/></w:settings>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>
                <w:p><w:r><w:t>PAGE ONE</w:t></w:r></w:p>
                <w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="4400" w:h="7000"/><w:pgMar w:top="1800" w:right="400" w:bottom="1800" w:left="400"/><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:pPr></w:p>
                <w:p><w:r><w:t>PAGE TWO</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="7000"/><w:pgMar w:top="1800" w:right="400" w:bottom="1800" w:left="400"/><w:footerReference w:type="default" r:id="rIdFooter"/><w:footerReference w:type="even" r:id="rIdEvenFooter"/></w:sectPr>
            </w:body></w:document>"#,
        ),
        ("word/settings.xml", &settings),
        ("word/header1.xml", &header),
        ("word/footer1.xml", &footer),
        ("word/footer2.xml", &even_footer),
    ])
}

#[test]
fn opened_docx_running_table_cell_tabs_roundtrip_through_fresh_conversion() {
    let document = Document::open(&running_surface_table_tab_docx(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
        r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
        720,
    ))
    .expect("running-table tab fixture opens");
    let model = document.model();
    let converted = document.to_docx();

    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(
        Document::open(&converted)
            .expect("fresh conversion reopens")
            .model(),
        model
    );

    let parts = unzip_parts(&converted);
    let running_parts = |needle: &str| {
        parts
            .iter()
            .filter(|(name, body)| {
                (name.starts_with("word/header") || name.starts_with("word/footer"))
                    && std::str::from_utf8(body).is_ok_and(|xml| xml.contains(needle))
            })
            .map(|(_, body)| std::str::from_utf8(body).unwrap())
            .collect::<Vec<_>>()
    };
    let headers = running_parts("HEADER PREFIX");
    assert_eq!(
        headers.len(),
        2,
        "default header is effective in both sections"
    );
    assert!(headers
        .iter()
        .all(|xml| xml
            .contains(r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#)));

    let default_footers = running_parts("DEFAULT FOOTER PREFIX");
    assert_eq!(default_footers.len(), 1);
    assert!(default_footers.iter().all(|xml| !xml.contains("<w:tabs>")));

    let even_footers = running_parts("EVEN FOOTER PREFIX");
    assert_eq!(even_footers.len(), 1);
    assert!(even_footers[0]
        .contains(r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="hyphen"/></w:tabs>"#));

    let standalone = unzip_parts(&rwml::write_docx(&model));
    assert!(standalone.iter().all(|(name, body)| {
        !(name.starts_with("word/header") || name.starts_with("word/footer"))
            || !std::str::from_utf8(body).unwrap().contains("<w:tabs>")
    }));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_running_table_cell_tab_stops() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_model = Document::open(&running_surface_table_tab_docx("", "", 720))
        .expect("baseline running-table tab fixture opens")
        .model();
    let render = |header_tabs: &str, even_footer_tabs: &str, default_stop: u32| {
        let document = Document::open(&running_surface_table_tab_docx(
            header_tabs,
            even_footer_tabs,
            default_stop,
        ))
        .expect("running-table tab fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "running-table tab stops must remain outside the public model"
        );
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("running-table tab layout succeeds")
                .pages,
            2
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let baseline = render("", "", 720);
    let header_explicit = render(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
        "",
        720,
    );
    let even_footer_explicit = render(
        "",
        r#"<w:tabs><w:tab w:val="left" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
        720,
    );
    let wider_default = render("", "", 1440);

    for (name, rendered) in [
        ("default-header explicit tab", &header_explicit),
        ("even-footer explicit tab", &even_footer_explicit),
        ("settings default interval", &wider_default),
    ] {
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, &baseline, "{name} must affect PDF output");
    }
    assert_ne!(header_explicit, even_footer_explicit);
    assert_ne!(header_explicit, wider_default);
    assert_ne!(even_footer_explicit, wider_default);
    assert_eq!(
        header_explicit,
        render(
            r#"<w:tabs><w:tab w:val="left" w:pos="1440" w:leader="dot"/></w:tabs>"#,
            "",
            720,
        )
    );
    assert_eq!(
        even_footer_explicit,
        render(
            "",
            r#"<w:tabs><w:tab w:val="left" w:pos="1200" w:leader="hyphen"/></w:tabs>"#,
            720,
        )
    );
    assert_eq!(wider_default, render("", "", 1440));
}

#[cfg(feature = "render")]
fn running_surface_distance_docx(page_margin_attributes: &str) -> Vec<u8> {
    let document = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="4400" w:h="6000"/><w:pgMar w:top="1600" w:right="400" w:bottom="1600" w:left="400" {page_margin_attributes}/><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#
    );
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>HEADER DISTANCE</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>FOOTER DISTANCE</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_running_surface_distances() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let baseline_document =
        Document::open(&running_surface_distance_docx("")).expect("baseline fixture opens");
    let baseline_model = baseline_document.model();
    let baseline = baseline_document.to_pdf_with_fonts(&fonts);

    let render = |attributes: &str| {
        let document = Document::open(&running_surface_distance_docx(attributes))
            .expect("distance fixture opens");
        assert_eq!(
            document.model(),
            baseline_model,
            "running-surface distances must remain outside the public model"
        );
        assert_eq!(
            document
                .layout_pages_with_fonts(&fonts)
                .expect("distance layout succeeds")
                .pages,
            1
        );
        document.to_pdf_with_fonts(&fonts)
    };

    let header = render(r#"w:header="1000""#);
    let footer = render(r#"w:footer="800""#);
    let both = render(r#"w:header="1000" w:footer="800""#);

    assert_ne!(header, baseline, "header distance must affect PDF output");
    assert_ne!(footer, baseline, "footer distance must affect PDF output");
    assert_ne!(both, header);
    assert_ne!(both, footer);
    assert_eq!(baseline, render(r#"w:header="-1" w:footer="invalid""#));
    assert_eq!(header, render(r#"w:header="1000""#));
    assert_eq!(footer, render(r#"w:footer="800""#));
    assert_eq!(both, render(r#"w:header="1000" w:footer="800""#));
}

fn running_surface_nested_table_layout_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>PARENT A</w:t><w:br w:type="column"/><w:t>PARENT B</w:t></w:r></w:p><w:sdt><w:sdtContent><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr/><w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/><w:spacing w:line="240" w:lineRule="exact"/><w:tabs><w:tab w:val="center" w:pos="720" w:leader="hyphen"/></w:tabs></w:pPr><w:r><w:t>NESTED A</w:t><w:tab/><w:t>NESTED B</w:t><w:br w:type="column"/><w:t>NESTED C</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:sdtContent></w:sdt></w:tc></w:tr></w:tbl></w:hdr>"#,
        ),
    ])
}

#[test]
fn opened_docx_running_table_nested_layout_roundtrips_through_fresh_conversion() {
    let document = Document::open(&running_surface_nested_table_layout_docx())
        .expect("running nested-table fixture opens");
    let model = document.model();
    let standalone = rwml::write_docx(&model);
    let normalized_model = Document::open(&standalone)
        .expect("standalone normalization reopens")
        .model();
    let converted = document.to_docx();

    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(
        Document::open(&converted)
            .expect("fresh conversion reopens")
            .model(),
        normalized_model
    );

    let parts = unzip_parts(&converted);
    let header = parts
        .iter()
        .find_map(|(name, body)| {
            let xml = std::str::from_utf8(body).ok()?;
            (name.starts_with("word/header") && xml.contains("NESTED A")).then_some(xml)
        })
        .expect("converted nested running header exists");
    assert_eq!(
        header.matches(r#"<w:br w:type="column"/>"#).count(),
        2,
        "direct and nested running-table column breaks survive"
    );
    assert!(header.contains("<w:cantSplit/>"));
    assert!(header.contains("<w:keepNext/>"));
    assert!(header.contains("<w:keepLines/>"));
    assert!(header.contains(r#"<w:widowControl w:val="0"/>"#));
    assert!(header.contains(r#"<w:spacing w:line="240" w:lineRule="exact"/>"#));
    assert!(header
        .contains(r#"<w:tabs><w:tab w:val="center" w:pos="720" w:leader="hyphen"/></w:tabs>"#));

    let standalone = unzip_parts(&standalone);
    assert!(standalone.iter().all(|(name, body)| {
        !(name.starts_with("word/header") || name.starts_with("word/footer"))
            || std::str::from_utf8(body).is_ok_and(|xml| {
                !xml.contains(r#"<w:br w:type="column"/>"#)
                    && !xml.contains("<w:cantSplit/>")
                    && !xml.contains("w:lineRule=")
                    && !xml.contains("<w:tabs>")
            })
    }));
}

fn running_surface_six_variant_nested_table_docx() -> Vec<u8> {
    let running_part = |root: &str,
                        label: &str,
                        line_twips: u32,
                        tab_twips: u32,
                        hyperlink: bool| {
        let top_tail = if hyperlink {
            format!(
                r#"<w:hyperlink r:id="rIdLink"><w:r><w:br w:type="column"/><w:t>{label} TOP B</w:t></w:r></w:hyperlink>"#
            )
        } else {
            format!(r#"<w:r><w:br w:type="column"/><w:t>{label} TOP B</w:t></w:r>"#)
        };
        let nested_hyperlink = if hyperlink {
            r#"<w:hyperlink r:id="rIdLink"><w:r><w:t>LOCAL LINK</w:t></w:r></w:hyperlink>"#
        } else {
            ""
        };
        format!(
            r#"<w:{root} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:sdt><w:sdtContent><w:p><w:r><w:t>{label} TOP A</w:t></w:r>{top_tail}</w:p></w:sdtContent></w:sdt><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>{label} PARENT A</w:t><w:br w:type="column"/><w:t>{label} PARENT B</w:t><w:br w:type="page"/><w:t>{label} PARENT C</w:t></w:r></w:p><w:sdt><w:sdtContent><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/><w:spacing w:line="{line_twips}" w:lineRule="exact"/><w:tabs><w:tab w:val="center" w:pos="{tab_twips}" w:leader="hyphen"/></w:tabs></w:pPr><w:r><w:t>{label} NESTED A</w:t><w:tab/><w:t>{label} NESTED B</w:t><w:br w:type="column"/><w:t>{label} NESTED C</w:t><w:br w:type="page"/><w:t>{label} NESTED D</w:t></w:r>{nested_hyperlink}</w:p><w:customXml><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3200"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr/><w:p><w:pPr><w:keepLines/><w:spacing w:line="{deep_line}" w:lineRule="atLeast"/><w:tabs><w:tab w:val="right" w:pos="{deep_tab}" w:leader="dot"/></w:tabs></w:pPr><w:r><w:t>{label} DEEP A</w:t><w:br w:type="column"/><w:t>{label} DEEP B</w:t><w:br w:type="page"/><w:t>{label} DEEP C</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:customXml></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:pPr><w:spacing w:line="999" w:lineRule="exact"/><w:tabs><w:tab w:val="left" w:pos="999"/></w:tabs></w:pPr><w:r><w:t>{label} LEAK</w:t><w:br w:type="column"/></w:r></w:p></w:tc></w:tr></w:tbl></w:sdtContent></w:sdt></w:tc></w:tr></w:tbl></w:{root}>"#,
            deep_line = line_twips + 100,
            deep_tab = tab_twips + 100,
        )
    };
    let default_header = running_part("hdr", "DEFAULT HEADER", 201, 701, true);
    let first_header = running_part("hdr", "FIRST HEADER", 202, 702, false);
    let even_header = running_part("hdr", "EVEN HEADER", 203, 703, false);
    let default_footer = running_part("ftr", "DEFAULT FOOTER", 204, 704, false);
    let first_footer = running_part("ftr", "FIRST FOOTER", 205, 705, false);
    let even_footer = running_part("ftr", "EVEN FOOTER", 206, 706, false);

    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/footer3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDefaultHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFirstHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdEvenHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header3.xml"/><Relationship Id="rIdDefaultFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rIdFirstFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer2.xml"/><Relationship Id="rIdEvenFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer3.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>SECTION ONE</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:headerReference w:type="default" r:id="rIdDefaultHeader"/><w:headerReference w:type="first" r:id="rIdFirstHeader"/><w:headerReference w:type="even" r:id="rIdEvenHeader"/><w:footerReference w:type="default" r:id="rIdDefaultFooter"/><w:footerReference w:type="first" r:id="rIdFirstFooter"/><w:footerReference w:type="even" r:id="rIdEvenFooter"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>SECTION TWO</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        ("word/header1.xml", &default_header),
        ("word/header2.xml", &first_header),
        ("word/header3.xml", &even_header),
        ("word/footer1.xml", &default_footer),
        ("word/footer2.xml", &first_footer),
        ("word/footer3.xml", &even_footer),
        (
            "word/_rels/header1.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/running-nested" TargetMode="External"/></Relationships>"#,
        ),
    ])
}

#[test]
fn opened_docx_running_layout_isolates_six_variants_and_inheritance() {
    let document = Document::open(&running_surface_six_variant_nested_table_docx())
        .expect("six-variant running nested-table fixture opens");
    let model = document.model();
    assert!(!format!("{model:?}").contains("LEAK"));
    let standalone = rwml::write_docx(&model);
    let normalized_model = Document::open(&standalone)
        .expect("standalone normalization reopens")
        .model();
    let converted = document.to_docx();

    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(
        Document::open(&converted)
            .expect("fresh conversion reopens")
            .model(),
        normalized_model
    );

    let parts = unzip_parts(&converted);
    for (label, line_twips, tab_twips, expected_parts) in [
        ("DEFAULT HEADER", 201, 701, 2),
        ("FIRST HEADER", 202, 702, 1),
        ("EVEN HEADER", 203, 703, 1),
        ("DEFAULT FOOTER", 204, 704, 2),
        ("FIRST FOOTER", 205, 705, 1),
        ("EVEN FOOTER", 206, 706, 1),
    ] {
        let running_parts = parts
            .iter()
            .filter_map(|(name, body)| {
                let xml = std::str::from_utf8(body).ok()?;
                ((name.starts_with("word/header") || name.starts_with("word/footer"))
                    && xml.contains(&format!("{label} NESTED A")))
                .then_some(xml)
            })
            .collect::<Vec<_>>();
        assert_eq!(running_parts.len(), expected_parts, "{label}");
        for xml in running_parts {
            let top_marker = format!("{label} TOP A");
            let top_paragraph = paragraph_with_marker(xml, &top_marker);
            assert_eq!(
                xml.matches(r#"<w:br w:type="column"/>"#).count(),
                4,
                "{label}: {xml}"
            );
            assert_eq!(
                xml.matches(r#"<w:br w:type="page"/>"#).count(),
                3,
                "{label}: {xml}"
            );
            assert_eq!(
                top_paragraph.matches(r#"<w:br w:type="column"/>"#).count(),
                1,
                "{label}: {top_paragraph}"
            );
            assert!(!top_paragraph.contains("<w:br/>"), "{label}");
            assert_eq!(xml.matches("<w:cantSplit/>").count(), 2, "{label}");
            assert!(xml.contains("<w:keepNext/>"), "{label}");
            assert_eq!(xml.matches("<w:keepLines/>").count(), 4, "{label}");
            assert!(xml.contains(r#"<w:widowControl w:val="0"/>"#), "{label}");
            assert!(
                xml.contains(&format!(
                    r#"<w:spacing w:line="{line_twips}" w:lineRule="exact"/>"#
                )),
                "{label}"
            );
            assert!(
                xml.contains(&format!(
                    r#"<w:spacing w:line="{}" w:lineRule="atLeast"/>"#,
                    line_twips + 100
                )),
                "{label}"
            );
            assert!(
                xml.contains(&format!(
                    r#"<w:tab w:val="center" w:pos="{tab_twips}" w:leader="hyphen"/>"#
                )),
                "{label}"
            );
            assert!(
                xml.contains(&format!(
                    r#"<w:tab w:val="right" w:pos="{}" w:leader="dot"/>"#,
                    tab_twips + 100
                )),
                "{label}"
            );
            assert!(!xml.contains("LEAK"), "{label}");
            assert!(!xml.contains(r#"w:line="999""#), "{label}");
        }
    }

    assert_eq!(
        parts
            .iter()
            .filter(|(name, body)| {
                name.starts_with("word/_rels/header")
                    && name.ends_with(".xml.rels")
                    && std::str::from_utf8(body)
                        .is_ok_and(|xml| xml.contains("https://example.com/running-nested"))
            })
            .count(),
        2,
        "the inherited default header keeps a local hyperlink relationship in both sections"
    );

    let standalone = unzip_parts(&standalone);
    for (name, body) in &standalone {
        if !(name.starts_with("word/header") || name.starts_with("word/footer")) {
            continue;
        }
        let xml = std::str::from_utf8(body).unwrap();
        assert!(!xml.contains(r#"<w:br w:type="column"/>"#), "{name}");
        assert_eq!(
            xml.matches(r#"<w:br w:type="page"/>"#).count(),
            3,
            "{name}: {xml}"
        );
        assert!(!xml.contains("<w:cantSplit/>"), "{name}");
        assert!(!xml.contains("<w:keepNext/>"), "{name}");
        assert!(!xml.contains("<w:keepLines/>"), "{name}");
        assert!(!xml.contains("w:lineRule="), "{name}");
        assert!(!xml.contains("<w:tabs>"), "{name}");
    }
}
