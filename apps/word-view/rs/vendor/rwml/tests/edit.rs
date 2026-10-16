#![cfg(feature = "docx")]

use std::io::{Read, Write};

use rwml::{Block, CoreProperty, Document, NoteKind, RevisionKind, RevisionView};

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

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

fn append_png_chunk(out: &mut Vec<u8>, typ: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(typ);
    out.extend_from_slice(data);
    let mut crc_input = Vec::new();
    crc_input.extend_from_slice(typ);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36,
        0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x60,
        0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

fn tiny_png_with_marker(marker: &[u8]) -> Vec<u8> {
    let png = tiny_png();
    let iend_start = png.len() - 12;
    let mut out = png[..iend_start].to_vec();
    append_png_chunk(&mut out, b"tEXt", marker);
    out.extend_from_slice(&png[iend_start..]);
    out
}

fn tiny_jpeg() -> Vec<u8> {
    vec![
        0xFF, 0xD8, // SOI
        0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00,
        0x01, 0x00, 0x00, // APP0/JFIF
        0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x03, 0x00, 0x02, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11,
        0x00, 0x03, 0x11, 0x00, // SOF0, 2x3 RGB
        0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x3F, 0x00,
        0x00, // SOS + minimal entropy payload
        0xFF, 0xD9, // EOI
    ]
}

fn tiny_jpeg_with_comment(marker: &[u8]) -> Vec<u8> {
    let jpeg = tiny_jpeg();
    let app0_end = 2 + 2 + u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
    let mut out = jpeg[..app0_end].to_vec();
    out.extend_from_slice(&[0xFF, 0xFE]);
    out.extend_from_slice(&((marker.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(marker);
    out.extend_from_slice(&jpeg[app0_end..]);
    out
}

fn tiny_gif() -> Vec<u8> {
    vec![
        b'G', b'I', b'F', b'8', b'9', b'a', 0x02, 0x00, 0x03, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xff, 0xff, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x02,
        0x02, 0x44, 0x01, 0x00, 0x3b,
    ]
}

fn tiny_gif_with_comment(marker: &[u8]) -> Vec<u8> {
    let gif = tiny_gif();
    let mut out = gif[..gif.len() - 1].to_vec();
    out.extend_from_slice(&[0x21, 0xfe, marker.len() as u8]);
    out.extend_from_slice(marker);
    out.extend_from_slice(&[0x00, 0x3b]);
    out
}

fn tiny_bmp() -> Vec<u8> {
    let pixel_bytes = vec![0u8; 24];
    let file_size = 54 + pixel_bytes.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&2i32.to_le_bytes());
    out.extend_from_slice(&3i32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixel_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&pixel_bytes);
    out
}

fn tiny_bmp_with_marker(marker: &[u8]) -> Vec<u8> {
    let mut bmp = tiny_bmp();
    bmp.extend_from_slice(marker);
    bmp
}

fn tiny_webp() -> Vec<u8> {
    vec![
        b'R', b'I', b'F', b'F', 22, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b'X', 10, 0,
        0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0,
    ]
}

fn tiny_webp_with_marker(marker: &[u8]) -> Vec<u8> {
    let mut webp = tiny_webp();
    webp.extend_from_slice(marker);
    webp
}

fn tiny_tiff() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&256u16.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&257u16.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

fn tiny_tiff_with_marker(marker: &[u8]) -> Vec<u8> {
    let mut tiff = tiny_tiff();
    tiff.extend_from_slice(marker);
    tiff
}

fn unzip_parts(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut parts = std::collections::BTreeMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        parts.insert(file.name().to_string(), bytes);
    }
    parts
}

fn header_footer_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p><w:sectPr><w:headerReference w:type=" default " r:id=" rIdHeader "/><w:footerReference w:type=" default " r:id=" rIdFooter "/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:p><w:r><w:t>OLD</w:t></w:r></w:p><w:p><w:r><w:drawing><a:t>OLD</a:t></w:drawing></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:ftr>"#,
        ),
    ])
}

fn symbol_body_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Alpha </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> Beta</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn symbol_alternate_content_body_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Alpha </w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:sym w:font="Symbol" w:char="F0B7"/></mc:Choice><mc:Fallback><w:t>fallback</w:t></mc:Fallback></mc:AlternateContent><w:t> Beta</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn table_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r><w:r><w:t>1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>AFTER</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn grid_span_table_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val=" 2 "/></w:tcPr><w:p><w:r><w:t>Merged AB</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>C1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>C2</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

fn vmerge_table_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:vMerge w:val=" restart "/></w:tcPr><w:p><w:r><w:t>A merged</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

fn nested_table_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Outer</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

fn notes_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY</w:t></w:r><w:r><w:footnoteReference w:id=" 1 "/></w:r><w:r><w:endnoteReference w:id=" 2 "/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type=" separator " w:id="-1"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:footnote><w:footnote w:id=" 1 "><w:p><w:r><w:t>OLD</w:t></w:r><w:r><w:t> foot</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id=" 2 "><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn alternate_content_note_entries_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="w14"><w:footnote w:id="1"><w:p><w:r><w:t>Choice footnote</w:t></w:r></w:p></w:footnote></mc:Choice><mc:Fallback><w:footnote w:id="9"><w:p><w:r><w:t>Fallback footnote</w:t></w:r></w:p></w:footnote></mc:Fallback></mc:AlternateContent></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="w14"><w:endnote w:id="2"><w:p><w:r><w:t>Choice endnote</w:t></w:r></w:p></w:endnote></mc:Choice><mc:Fallback><w:endnote w:id="8"><w:p><w:r><w:t>Fallback endnote</w:t></w:r></w:p></w:endnote></mc:Fallback></mc:AlternateContent></w:endnotes>"#,
        ),
    ])
}

fn notes_with_blank_ids_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Valid</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p><w:p><w:r><w:t>Blank</w:t></w:r><w:r><w:footnoteReference w:id=" "/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:p><w:r><w:t>Valid note</w:t></w:r></w:p></w:footnote><w:footnote w:id=" "><w:p><w:r><w:t>Blank note</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Foot before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:t>End before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_simple_field_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" QUOTE &quot;Fresh foot&quot; "><w:r><w:t>stale foot</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" QUOTE &quot;Fresh end&quot; "><w:r><w:t>stale end</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_complex_field_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh foot&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale foot</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh end&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale end</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_style_ref_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#,
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Intro Heading</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale earlier style ref</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Anchor Heading</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale simple style ref</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> STYLEREF &quot;heading 1&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex style ref</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_deleted_style_ref_before_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Live Heading</w:t></w:r></w:p><w:p><w:del w:id="64" w:author="Bob"><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:delText>DELETED CACHE</w:delText></w:r></w:fldSimple></w:del><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>BAD CURRENT CACHE</w:t></w:r></w:fldSimple><w:r><w:footnoteReference w:id="7"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_dynamic_field_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" IF 1 = 1 &quot;Fresh foot&quot; &quot;stale branch&quot; "><w:r><w:t>stale foot</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> IF 2 &gt; 1 &quot;Fresh end&quot; &quot;stale branch&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale end</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_local_field_bookmark_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SET Client &quot;Acme&quot; "/><w:fldSimple w:instr=" IF Client = &quot;Acme&quot; &quot;Fresh foot&quot; &quot;stale branch&quot; "><w:r><w:t>stale foot</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_display_action_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SYMBOL 183 \f Symbol "><w:r><w:t>stale symbol</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MACROBUTTON RunReport &quot;Fresh foot&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale action</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_marker_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TC &quot;Foot entry&quot; "><w:r><w:t>stale tc marker</w:t></w:r></w:fldSimple><w:r><w:t>Foot before </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> XE &quot;Foot index&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale xe marker</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_numbering_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SEQ FootItem "><w:r><w:t>stale foot sequence one</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SEQ FootItem </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale foot sequence two</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_document_info_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Side Table Plan</dc:title></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TITLE "><w:r><w:t>stale foot title</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_revision_number_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"><cp:revision>17</cp:revision></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" REVNUM "><w:r><w:t>stale foot revision</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_section_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale prior section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>Second section </w:t></w:r><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale anchor section</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_legacy_form_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Direct A"/><w:listEntry w:val="Direct B"/></w:ddList></w:ffData><w:r><w:t>stale direct option</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Anchor A"/><w:listEntry w:val="Anchor B"/></w:ddList></w:ffData><w:r><w:t>stale anchor option</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_text_form_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Anchor default"/></w:textInput></w:ffData><w:r><w:t>Anchor typed</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_complex_text_form_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:textInput><w:default w:val="Anchor default"/></w:textInput></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMTEXT \*Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Anchor typed</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_alternate_content_text_form_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><mc:AlternateContent><mc:Choice Requires="w14"><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Anchor default"/></w:textInput></w:ffData><w:r><w:t>Anchor typed</w:t></w:r></w:fldSimple></mc:Choice><mc:Fallback><w:r><w:t>fallback anchor</w:t></w:r></mc:Fallback></mc:AlternateContent><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_document_bookmark_formula_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceSubtotal"/><w:r><w:t>42</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" = InvoiceSubtotal + 8 "><w:r><w:t>stale foot formula</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_document_bookmark_merge_control_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="Gate"/><w:r><w:t>Ready</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:r><w:t>before </w:t></w:r><w:fldSimple w:instr=" NEXTIF Gate = &quot;Ready&quot; "><w:r><w:t>stale foot nextif </w:t></w:r></w:fldSimple><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_document_bookmark_ref_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="ClauseText"/><w:r><w:t>clause one</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF ClauseText \* Upper "><w:r><w:t>stale foot ref</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_note_ref_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>stale anchor note</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:p><w:r><w:t>Reference note</w:t></w:r></w:p></w:footnote><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_ref_note_mark_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" REF FootOne \f "><w:r><w:t>stale anchor ref note mark</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:p><w:r><w:t>Reference note</w:t></w:r></w:p></w:footnote><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_toc_field_anchor_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; "><w:r><w:t>stale anchor toc</w:t></w:r></w:fldSimple><w:r><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn notes_with_symbol_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Foot </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:t>End </w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:sym w:font="Symbol" w:char="F0B7"/></mc:Choice><mc:Fallback><w:t>fallback</w:t></mc:Fallback></mc:AlternateContent><w:t> before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_expanded_symbol_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Foot </w:t><w:sym w:font="Symbol" w:char="F0B7"></w:sym><w:t> before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:t>End </w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:sym w:font="Symbol" w:char="F0B7"></w:sym></mc:Choice><mc:Fallback><w:t>fallback</w:t></mc:Fallback></mc:AlternateContent><w:t> before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_expanded_marker_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Foot</w:t><w:tab></w:tab><w:t>Tab</w:t><w:br></w:br><w:t>Line</w:t><w:noBreakHyphen></w:noBreakHyphen><w:t>Hard</w:t><w:softHyphen></w:softHyphen><w:t>Soft </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>foot after</w:t></w:r></w:p><w:p><w:r><w:t>End</w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:tab></w:tab><w:t>Tab</w:t><w:br w:type="page"></w:br><w:t>Page</w:t><w:noBreakHyphen></w:noBreakHyphen><w:t>Hard</w:t><w:softHyphen></w:softHyphen><w:t>Soft </w:t></mc:Choice><mc:Fallback><w:t>fallback </w:t></mc:Fallback></mc:AlternateContent></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>end after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn notes_with_revision_wrapped_anchor_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:ins w:id="20" w:author="Editor"><w:p><w:r><w:t>Inserted foot before </w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t>inserted foot after</w:t></w:r></w:p></w:ins><w:moveTo w:id="21" w:author="Editor"><w:p><w:r><w:t>Moved end before </w:t></w:r><w:r><w:endnoteReference w:id="8"/></w:r><w:r><w:t>moved end after</w:t></w:r></w:p></w:moveTo></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="8"><w:p><w:r><w:t>End body</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn text_box_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:t>BODY </w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn header_footer_text_box_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
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
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:p><w:r><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>HEADER BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></w:r></w:p></w:hdr>"#,
        ),
    ])
}

fn note_text_box_docx() -> Vec<u8> {
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
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEndnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:footnote w:id="1"><w:p><w:r><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Foot box</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:endnote w:id="2"><w:p><w:r><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>End box</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn anchored_text_box_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="1"><wp:docPr id="7" name="Anchored box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_simple_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" QUOTE &quot;Fresh anchor&quot; "><w:r><w:t>stale anchor</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="3"><wp:docPr id="9" name="Anchored quote box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_complex_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh anchor&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale anchor</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="4"><wp:docPr id="10" name="Anchored complex quote box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_dynamic_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" IF 1 = 1 &quot;Fresh simple&quot; &quot;stale branch&quot; "><w:r><w:t>stale simple</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="5"><wp:docPr id="11" name="Anchored simple dynamic box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>SIMPLE BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> IF 2 &gt; 1 &quot;Fresh complex&quot; &quot;stale branch&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="6"><wp:docPr id="12" name="Anchored complex dynamic box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>COMPLEX BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_local_field_bookmark_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" SET Client &quot;Acme&quot; "/><w:fldSimple w:instr=" IF Client = &quot;Acme&quot; &quot;Fresh anchor&quot; &quot;stale branch&quot; "><w:r><w:t>stale anchor</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="7"><wp:docPr id="13" name="Anchored local state box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>LOCAL BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_display_action_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" SYMBOL 183 \f Symbol "><w:r><w:t>stale symbol</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MACROBUTTON RunReport &quot;Fresh anchor&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale action</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="8"><wp:docPr id="14" name="Anchored action display box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>ACTION DISPLAY BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_marker_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" TC &quot;Anchor entry&quot; "><w:r><w:t>stale tc marker</w:t></w:r></w:fldSimple><w:r><w:t>Before </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> XE &quot;Anchor index&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale xe marker</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:drawing><wp:anchor relativeHeight="9"><wp:docPr id="15" name="Anchored marker box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>MARKER BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_numbering_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" LISTNUM NumberDefault "><w:r><w:t>stale list one</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> LISTNUM NumberDefault </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale list two</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="10"><wp:docPr id="16" name="Anchored numbering box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>NUMBERING BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_document_info_field_anchor_docx() -> Vec<u8> {
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
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Side Table Plan</dc:title></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" TITLE "><w:r><w:t>stale anchor title</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="11"><wp:docPr id="17" name="Anchored document info box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>DOCUMENT INFO BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_revision_number_field_anchor_docx() -> Vec<u8> {
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
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"><cp:revision>17</cp:revision></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" REVNUM "><w:r><w:t>stale anchor revision</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="18"><wp:docPr id="18" name="Anchored revision box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>REVISION BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_document_bookmark_formula_field_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceSubtotal"/><w:r><w:t>42</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" = InvoiceSubtotal + 8 "><w:r><w:t>stale anchor formula</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="19"><wp:docPr id="19" name="Anchored formula box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>FORMULA BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_document_bookmark_merge_control_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="1" w:name="Gate"/><w:r><w:t>Ready</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:r><w:t>Before</w:t></w:r><w:fldSimple w:instr=" SKIPIF Gate &lt;&gt; &quot;Ready&quot; "><w:r><w:t>stale anchor skipif</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="20"><wp:docPr id="20" name="Anchored merge control box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>MERGE CONTROL BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_document_bookmark_ref_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceTotal"/><w:r><w:t>21</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" InvoiceTotal \* Ordinal "><w:r><w:t>stale anchor direct ref</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="21"><wp:docPr id="21" name="Anchored ref box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>REF BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_note_ref_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="1" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>stale anchor note</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="22"><wp:docPr id="22" name="Anchored note box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>NOTE BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_toc_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; "><w:r><w:t>stale anchor toc</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="23"><wp:docPr id="23" name="Anchored TOC box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>TOC BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_legacy_form_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Direct A"/><w:listEntry w:val="Direct B"/></w:ddList></w:ffData><w:r><w:t>stale direct option</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Anchor A"/><w:listEntry w:val="Anchor B"/></w:ddList></w:ffData><w:r><w:t>stale anchor option</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="24"><wp:docPr id="24" name="Anchored legacy form box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>LEGACY FORM BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_text_form_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Anchor default"/></w:textInput></w:ffData><w:r><w:t>Anchor typed</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="25"><wp:docPr id="25" name="Anchored text form box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>TEXT FORM BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn anchored_text_box_complex_text_form_anchor_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:textInput><w:default w:val="Anchor default"/></w:textInput></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMTEXT \*Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Anchor typed</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="26"><wp:docPr id="26" name="Anchored complex text form box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>COMPLEX TEXT FORM BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn duplicate_anchored_text_box_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>First </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="1"><wp:docPr id="7" name="First anchored box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>done</w:t></w:r></w:p><w:p><w:r><w:t>Second </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="2"><wp:docPr id="8" name="Second anchored box"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX TEXT</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>done</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn revision_wrapped_text_box_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:drawing><w:txbxContent><w:p><w:r><w:t>Direct box</w:t></w:r></w:p></w:txbxContent></w:drawing></w:r></w:p><w:ins w:id="20" w:author="Editor"><w:p><w:r><w:drawing><w:txbxContent><w:p><w:r><w:t>Inserted box</w:t></w:r></w:p></w:txbxContent></w:drawing></w:r></w:p></w:ins><w:moveTo w:id="21" w:author="Editor"><w:p><w:r><w:drawing><w:txbxContent><w:p><w:r><w:t>Moved-to box</w:t></w:r></w:p></w:txbxContent></w:drawing></w:r></w:p></w:moveTo><w:del w:id="22" w:author="Editor"><w:p><w:r><w:drawing><w:txbxContent><w:p><w:r><w:delText>Deleted box</w:delText></w:r></w:p></w:txbxContent></w:drawing></w:r></w:p></w:del><w:moveFrom w:id="23" w:author="Editor"><w:p><w:r><w:drawing><w:txbxContent><w:p><w:r><w:delText>Moved-from box</w:delText></w:r></w:p></w:txbxContent></w:drawing></w:r></w:p></w:moveFrom></w:body></w:document>"#,
        ),
    ])
}

fn no_notes_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn split_run_no_notes_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BO</w:t></w:r><w:r><w:t>DY</w:t></w:r><w:r><w:t> tail</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn image_docx(png: &[u8]) -> Vec<u8> {
    let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/></Relationships>"#;
    let body = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="image1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed=" rIdImg "/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    docx_fixture_bytes(&[
        ("[Content_Types].xml", ct),
        ("_rels/.rels", root_rels),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/document.xml", body),
        ("word/media/image1.png", png),
    ])
}

fn jpeg_image_docx(jpeg: &[u8]) -> Vec<u8> {
    let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="jpeg" ContentType="image/jpeg"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/photo.jpeg"/></Relationships>"#;
    let body = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="image1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    docx_fixture_bytes(&[
        ("[Content_Types].xml", ct),
        ("_rels/.rels", root_rels),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/document.xml", body),
        ("word/media/photo.jpeg", jpeg),
    ])
}

fn gif_image_docx(gif: &[u8]) -> Vec<u8> {
    let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="gif" ContentType="image/gif"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/anim.gif"/></Relationships>"#;
    let body = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="image1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    docx_fixture_bytes(&[
        ("[Content_Types].xml", ct),
        ("_rels/.rels", root_rels),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/document.xml", body),
        ("word/media/anim.gif", gif),
    ])
}

fn bmp_image_docx(bmp: &[u8]) -> Vec<u8> {
    let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="bmp" ContentType="image/bmp"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/bitmap.bmp"/></Relationships>"#;
    let body = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="image1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    docx_fixture_bytes(&[
        ("[Content_Types].xml", ct),
        ("_rels/.rels", root_rels),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/document.xml", body),
        ("word/media/bitmap.bmp", bmp),
    ])
}

fn webp_image_docx(webp: &[u8]) -> Vec<u8> {
    let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="webp" ContentType="image/webp"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/picture.webp"/></Relationships>"#;
    let body = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="image1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    docx_fixture_bytes(&[
        ("[Content_Types].xml", ct),
        ("_rels/.rels", root_rels),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/document.xml", body),
        ("word/media/picture.webp", webp),
    ])
}

fn tiff_image_docx(tiff: &[u8]) -> Vec<u8> {
    let ct = br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="tiff" ContentType="image/tiff"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/scan.tiff"/></Relationships>"#;
    let body = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:r><w:t>BEFORE</w:t></w:r></w:p><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="image1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    docx_fixture_bytes(&[
        ("[Content_Types].xml", ct),
        ("_rels/.rels", root_rels),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/document.xml", body),
        ("word/media/scan.tiff", tiff),
    ])
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>Old title</dc:title><dc:creator>Old Author</dc:creator><dcterms:created>old created</dcterms:created><dcterms:modified>old modified</dcterms:modified></cp:coreProperties>"#,
        ),
    ])
}

fn hyperlink_docx() -> Vec<u8> {
    docx_fixture(&[
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
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFirst" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/first" TargetMode="External"/><Relationship Id="rIdOrphan" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/orphan" TargetMode="External"/><Relationship Id="rIdSecond" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/second" TargetMode="External"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:hyperlink r:id="rIdFirst"><w:r><w:t>First</w:t></w:r></w:hyperlink><w:r><w:t> and </w:t></w:r><w:hyperlink r:id=" rIdSecond "><w:r><w:t>Second</w:t></w:r></w:hyperlink></w:p></w:body></w:document>"#,
        ),
    ])
}

fn content_control_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:sdt><w:sdtPr><w:alias w:val="Client name"/><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old</w:t></w:r><w:r><w:t> Client</w:t></w:r></w:p></w:sdtContent></w:sdt><w:sdt><w:sdtPr><w:alias w:val="Project"/><w:tag w:val="project-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Keep Project</w:t></w:r></w:p></w:sdtContent></w:sdt><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old Again</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[test]
fn body_level_content_controls_keep_metadata_in_model() {
    let doc = Document::open(&content_control_docx()).expect("fixture opens");
    let model = doc.model();
    let Block::Paragraph(client) = &model.blocks[1] else {
        panic!("expected client content-control paragraph");
    };
    for run in &client.runs {
        let control = run
            .content_control
            .as_ref()
            .expect("client run carries content-control metadata");
        assert_eq!(control.alias.as_deref(), Some("Client name"));
        assert_eq!(control.tag.as_deref(), Some("client-name"));
    }
    let Block::Paragraph(tag_only) = &model.blocks[3] else {
        panic!("expected tag-only content-control paragraph");
    };
    assert_eq!(
        tag_only.runs[0]
            .content_control
            .as_ref()
            .and_then(|control| control.tag.as_deref()),
        Some("client-name")
    );
}

fn merge_template_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:sdt><w:sdtPr><w:alias w:val="Client name"/><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old Client Control</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:fldSimple w:instr=" MERGEFIELD  client-name  \* MERGEFORMAT "><w:r><w:t>Old Client Field</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MERGEFIELD &quot;project-name&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Old Project Field</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:p><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn unquoted_multi_token_merge_template_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" MERGEFIELD Client Name \* MERGEFORMAT "><w:r><w:t>Old Client Field</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

fn header_footer_merge_template_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \* MERGEFORMAT "><w:r><w:t>Old Header Client</w:t></w:r></w:fldSimple></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \* MERGEFORMAT "><w:r><w:t>Orphan Header Client</w:t></w:r></w:fldSimple></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MERGEFIELD &quot;project-name&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Old Footer Project</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:ftr>"#,
        ),
    ])
}

fn header_footer_content_control_template_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:alias w:val="Client"/><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old Header Client</w:t></w:r></w:p></w:sdtContent></w:sdt></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Orphan Header Client</w:t></w:r></w:p></w:sdtContent></w:sdt></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:tag w:val="project-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old</w:t></w:r><w:r><w:t> Footer Project</w:t></w:r></w:p></w:sdtContent></w:sdt></w:ftr>"#,
        ),
    ])
}

fn header_footer_merge_template_missing_result_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \* MERGEFORMAT "><w:r><w:t>Old Body Client</w:t></w:r></w:fldSimple></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \* MERGEFORMAT "/></w:p></w:hdr>"#,
        ),
    ])
}

fn note_template_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:t>Separator</w:t></w:r></w:p></w:footnote><w:footnote w:id="1"><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old Foot Client</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:fldSimple w:instr=" MERGEFIELD project-name \* MERGEFORMAT "><w:r><w:t>Old Foot Project</w:t></w:r></w:fldSimple></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="2"><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MERGEFIELD &quot;client-name&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Old End Client</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:sdt><w:sdtPr><w:tag w:val="project-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old End</w:t></w:r><w:r><w:t> Project</w:t></w:r></w:p></w:sdtContent></w:sdt></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn alternate_content_template_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:compat="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><compat:AlternateContent><compat:Choice Requires="w14"><alt:AlternateContent xmlns:alt="http://schemas.openxmlformats.org/markup-compatibility/2006"><alt:Fallback><w:sdt><w:sdtPr><w:tag w:val="body-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Body selected</w:t></w:r></w:p></w:sdtContent></w:sdt></alt:Fallback></alt:AlternateContent></compat:Choice><compat:Fallback><w:sdt><w:sdtPr><w:tag w:val="body-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Body fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></compat:Fallback></compat:AlternateContent><w:p><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mcx="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:footnote w:type="separator" w:id="-1"><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot separator</w:t></w:r></w:p></w:sdtContent></w:sdt></w:footnote><mcx:AlternateContent><mcx:Choice Requires="w14"><w:footnote w:id="1"><mcx:AlternateContent><mcx:Choice Requires="w15"><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot selected</w:t></w:r></w:p></w:sdtContent></w:sdt></mcx:Choice><mcx:Fallback><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></mcx:Fallback></mcx:AlternateContent></w:footnote></mcx:Choice><mcx:Fallback><w:footnote w:id="9"><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot entry fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></w:footnote></mcx:Fallback></mcx:AlternateContent></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:alt="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:endnote w:type="continuationSeparator" w:id="0"><w:p><w:fldSimple w:instr=" MERGEFIELD end-name "><w:r><w:t>End separator</w:t></w:r></w:fldSimple></w:p></w:endnote><w:endnote w:id="2"><alt:AlternateContent><alt:Choice Requires="w14"><w:p><w:fldSimple w:instr=" MERGEFIELD keep-name "><w:r><w:t>End selected keep</w:t></w:r></w:fldSimple></w:p></alt:Choice><alt:Fallback><w:p><w:fldSimple w:instr=" MERGEFIELD end-name "><w:r><w:t>End fallback hidden</w:t></w:r></w:fldSimple></w:p></alt:Fallback></alt:AlternateContent><w:p><w:fldSimple w:instr=" MERGEFIELD end-name "><w:r><w:t>End visible</w:t></w:r></w:fldSimple></w:p></w:endnote></w:endnotes>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/markup-compatibility/2006"><m:AlternateContent><m:Choice Requires="w14"><w:sdt><w:sdtPr><w:tag w:val="header-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Header selected</w:t></w:r></w:p></w:sdtContent></w:sdt></m:Choice><m:Fallback><w:sdt><w:sdtPr><w:tag w:val="header-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Header fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></m:Fallback></m:AlternateContent></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:tag w:val="header-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Orphan header</w:t></w:r></w:p></w:sdtContent></w:sdt></w:hdr>"#,
        ),
    ])
}

fn tracked_revisions_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Before</w:t></w:r><w:ins w:id="1" w:author="Alice"><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="2" w:author="Bob"><w:r><w:delText> removed</w:delText></w:r></w:del><w:moveFrom w:id="3"><w:r><w:delText> from</w:delText></w:r></w:moveFrom><w:moveTo w:id="4"><w:r><w:t> to</w:t></w:r></w:moveTo><w:r><w:t> After</w:t></w:r></w:p><w:p><w:pPr><w:jc w:val="left"/><w:pPrChange w:id="5" w:author="Editor"><w:pPr><w:jc w:val="right"/></w:pPr></w:pPrChange></w:pPr><w:r><w:rPr><w:b/><w:rPrChange w:id="6"><w:rPr><w:i/></w:rPr></w:rPrChange></w:rPr><w:t>Props</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn symbol_tracked_revisions_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Before</w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t>base</w:t></w:r><w:ins w:id="1" w:author="Alice"><w:r><w:t>Insert </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> keep</w:t></w:r></w:ins><w:del w:id="2" w:author="Bob"><w:r><w:delText>Delete </w:delText><w:sym w:font="Symbol" w:char="F0B7"/><w:delText> drop</w:delText></w:r></w:del><w:moveFrom w:id="3"><w:r><w:delText>From </w:delText><w:sym w:font="Symbol" w:char="F0B7"/><w:delText> old</w:delText></w:r></w:moveFrom><w:moveTo w:id="4"><w:r><w:t>To </w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:sym w:font="Symbol" w:char="F0B7"/></mc:Choice><mc:Fallback><w:t>fallback</w:t></mc:Fallback></mc:AlternateContent><w:t> new</w:t></w:r></w:moveTo><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn expanded_symbol_tracked_revisions_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="1" w:author="Alice"><w:r><w:t>Insert </w:t><w:sym w:font="Symbol" w:char="F0B7"></w:sym><w:t> keep</w:t></w:r></w:ins><w:del w:id="2" w:author="Bob"><w:r><w:delText>Delete </w:delText><w:sym w:font="Symbol" w:char="F0B7"></w:sym><w:delText> drop</w:delText></w:r></w:del><w:moveFrom w:id="3"><w:r><w:delText>From </w:delText><w:sym w:font="Symbol" w:char="F0B7"></w:sym><w:delText> old</w:delText></w:r></w:moveFrom><w:moveTo w:id="4"><w:r><w:t>To </w:t><w:sym w:font="Symbol" w:char="F0B7"></w:sym><w:t> new</w:t></w:r></w:moveTo></w:p></w:body></w:document>"#,
        ),
    ])
}

fn tracked_note_revisions_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:p><w:ins w:id="7"><w:r><w:t>Foot added</w:t></w:r></w:ins><w:del w:id="8"><w:r><w:delText>Foot removed</w:delText></w:r></w:del></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="2"><w:p><w:moveFrom w:id="9"><w:r><w:delText>End from</w:delText></w:r></w:moveFrom><w:moveTo w:id="10"><w:r><w:t>End to</w:t></w:r></w:moveTo></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn tracked_header_footer_revisions_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/><w:footerReference w:type="default" r:id="rIdFooter"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:ins w:id="7"><w:r><w:t>Header added</w:t></w:r></w:ins><w:del w:id="8"><w:r><w:delText>Header removed</w:delText></w:r></w:del></w:p></w:hdr>"#,
        ),
        (
            "word/footer1.xml",
            r#"<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:moveFrom w:id="9"><w:r><w:delText>Footer from</w:delText></w:r></w:moveFrom><w:moveTo w:id="10"><w:r><w:t>Footer to</w:t></w:r></w:moveTo></w:p></w:ftr>"#,
        ),
    ])
}

fn revision_switched_header_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdCurrent" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdOld" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:ins w:id="1"><w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rIdCurrent"/></w:sectPr></w:pPr></w:p></w:ins><w:del w:id="2"><w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rIdOld"/></w:sectPr></w:pPr></w:p></w:del></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:ins w:id="3"><w:r><w:t>Current added</w:t></w:r></w:ins><w:del w:id="4"><w:r><w:delText>Current removed</w:delText></w:r></w:del></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:ins w:id="5"><w:r><w:t>Old added</w:t></w:r></w:ins><w:del w:id="6"><w:r><w:delText>Old removed</w:delText></w:r></w:del></w:p></w:hdr>"#,
        ),
    ])
}

#[test]
fn fill_content_control_by_tag_updates_matching_controls() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");

    let changed = doc
        .fill_content_control_by_tag("client-name", "Acme & Co")
        .expect("tagged content controls filled");

    assert_eq!(changed, 2);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains(r#"<w:alias w:val="Client name"/>"#)
            && body.contains(r#"<w:tag w:val="client-name"/>"#),
        "content-control metadata not preserved: {body}"
    );
    assert!(
        body.contains("<w:t>Acme &amp; Co</w:t>"),
        "filled text missing or unescaped: {body}"
    );
    assert!(
        body.contains("<w:t></w:t>"),
        "stale second run in first content control should be cleared: {body}"
    );
    assert!(
        body.contains("<w:t>Keep Project</w:t>"),
        "non-matching content control changed: {body}"
    );
    assert!(
        body.contains("<w:t>Before</w:t>") && body.contains("<w:t>After</w:t>"),
        "ordinary body text changed: {body}"
    );
    assert!(
        !body.contains(">Old<") && !body.contains(">Old Again<"),
        "old tagged values leaked after fill: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert!(reopened.text().contains("Acme & Co"));
    assert!(reopened.text().contains("Keep Project"));
}

#[test]
fn fill_content_control_by_tag_writes_markers_and_clears_them_on_refill() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");
    let marked_value = "Client\tName\nSecond line";

    assert_eq!(
        doc.fill_content_control_by_tag("client-name", marked_value)
            .expect("tagged content controls filled with markers"),
        2
    );

    let marked = doc.save().expect("save marker-filled docx");
    let marked_body = String::from_utf8(unzip_parts(&marked)["word/document.xml"].clone()).unwrap();
    let marked_xml = "<w:t>Client</w:t><w:tab/><w:t>Name</w:t><w:br/><w:t>Second line</w:t>";
    assert_eq!(
        marked_body.matches(marked_xml).count(),
        2,
        "each matching content control should use WML run-content markers: {marked_body}"
    );
    assert!(
        marked_body.contains("<w:t>Keep Project</w:t>"),
        "non-matching content control changed: {marked_body}"
    );
    let reopened_marked = Document::open(&marked).expect("reopen marker-filled docx");
    assert_eq!(
        reopened_marked.text().matches(marked_value).count(),
        2,
        "reopened content-control values lost markers"
    );

    assert_eq!(
        doc.fill_content_control_by_tag("client-name", marked_value)
            .expect("repeat marker fill"),
        2
    );
    let repeated = doc.save().expect("save repeated marker fill");
    assert_eq!(
        unzip_parts(&repeated)["word/document.xml"],
        unzip_parts(&marked)["word/document.xml"],
        "repeating the same marker fill should not accumulate empty text fragments"
    );

    assert_eq!(
        doc.fill_content_control_by_tag("client-name", "Plain client")
            .expect("refill tagged content controls"),
        2
    );
    let plain = doc.save().expect("save refilled docx");
    let plain_body = String::from_utf8(unzip_parts(&plain)["word/document.xml"].clone()).unwrap();
    assert_eq!(
        plain_body.matches("<w:t>Plain client</w:t>").count(),
        2,
        "plain refill missing: {plain_body}"
    );
    assert!(
        !plain_body.contains("<w:tab") && !plain_body.contains("<w:br"),
        "plain refill retained stale run-content markers: {plain_body}"
    );
    let reopened_plain = Document::open(&plain).expect("reopen plain-refilled docx");
    assert_eq!(reopened_plain.text().matches("Plain client").count(), 2);
    assert!(!reopened_plain.text().contains(marked_value));
}

#[test]
fn fill_content_control_marker_only_value_remains_refillable_after_reopen() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");

    assert_eq!(
        doc.fill_content_control_by_tag("client-name", "\t\n")
            .expect("tagged controls filled with marker-only value"),
        2
    );
    let marked = doc.save().expect("save marker-only controls");
    let marked_body = String::from_utf8(unzip_parts(&marked)["word/document.xml"].clone()).unwrap();
    assert_eq!(
        marked_body.matches("<w:tab/><w:br/><w:t></w:t>").count(),
        2,
        "marker-only fills should retain an empty refill anchor: {marked_body}"
    );

    let mut reopened = Document::open(&marked).expect("reopen marker-only controls");
    assert_eq!(
        reopened
            .fill_content_control_by_tag("client-name", "Refilled")
            .expect("refill marker-only controls"),
        2
    );
    let refilled = reopened.save().expect("save refilled controls");
    let refilled_body =
        String::from_utf8(unzip_parts(&refilled)["word/document.xml"].clone()).unwrap();
    assert_eq!(refilled_body.matches("<w:t>Refilled</w:t>").count(), 2);
    assert!(
        !refilled_body.contains("<w:tab") && !refilled_body.contains("<w:br"),
        "refill retained marker-only siblings: {refilled_body}"
    );
}

#[test]
fn fill_content_control_markers_follow_wml_prefix_and_preserve_other_breaks() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<x:document xmlns:x="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:f="urn:foreign"><x:body><x:sdt><x:sdtPr><x:tag x:val="client-name"/></x:sdtPr><x:sdtContent><x:p><x:pPr><x:tabs><x:tab x:val="left" x:pos="720"/></x:tabs></x:pPr><x:r><x:t>Old</x:t><x:br x:type="page"/><x:br x:type="column"/><x:br type="page"/><x:br f:type="page"/><x:br/><x:tab/><f:tab/><x:t>Tail</x:t></x:r></x:p></x:sdtContent></x:sdt><x:p><x:r><x:tab/><x:br/><x:t>Sentinel</x:t></x:r></x:p></x:body></x:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("alternate-prefix fixture opens");

    assert_eq!(
        doc.fill_content_control_by_tag("client-name", "New\tLine\nNext")
            .expect("alternate-prefix control filled"),
        1
    );
    let saved = doc.save().expect("save alternate-prefix control");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<x:t>New</x:t><x:tab/><x:t>Line</x:t><x:br/><x:t>Next</x:t>"),
        "generated markers did not use the bound WML prefix: {body}"
    );
    assert!(
        !body.contains("<w:"),
        "generated an unbound canonical WML prefix: {body}"
    );
    assert!(
        body.contains(r#"<x:br x:type="page"/>"#)
            && body.contains(r#"<x:br x:type="column"/>"#)
            && body.contains(r#"<x:br type="page"/>"#),
        "page or column break was consumed by a text fill: {body}"
    );
    assert!(
        body.contains(r#"<x:pPr><x:tabs><x:tab x:val="left" x:pos="720"/></x:tabs></x:pPr>"#),
        "paragraph tab-stop definition was consumed by a text fill: {body}"
    );
    assert!(
        !body.contains(r#"<x:br f:type="page"/>"#),
        "foreign break type prevented cleanup of a default text-wrapping break: {body}"
    );
    assert!(
        body.contains("<f:tab/>"),
        "foreign same-local-name marker was consumed: {body}"
    );
    assert_eq!(
        body.matches("<x:tab/>").count(),
        2,
        "stale WML tab survived the replacement: {body}"
    );
    assert_eq!(
        body.matches("<x:br/>").count(),
        2,
        "stale text-wrapping break survived the replacement: {body}"
    );
    assert!(
        body.contains("<x:tab/><x:br/><x:t>Sentinel</x:t>"),
        "unmatched same-part markers were consumed: {body}"
    );
    Document::open(&saved).expect("alternate-prefix result reopens");
}

#[test]
fn fill_content_control_by_tag_skips_deleted_controls() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:del w:id="1"><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Deleted client</w:t></w:r></w:p></w:sdtContent></w:sdt></w:del><w:moveFrom w:id="2"><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Moved client</w:t></w:r></w:p></w:sdtContent></w:sdt></w:moveFrom><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Current client</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(
        doc.fill_content_control_by_tag("client-name", "Acme")
            .unwrap(),
        1
    );

    let body = String::from_utf8(
        unzip_parts(&doc.save().expect("save edited docx"))["word/document.xml"].clone(),
    )
    .unwrap();
    assert!(body.contains("<w:t>Deleted client</w:t>"), "{body}");
    assert!(body.contains("<w:t>Moved client</w:t>"), "{body}");
    assert!(body.contains("<w:t>Acme</w:t>"), "{body}");
}

#[test]
fn fill_content_control_by_tag_trims_ooxml_tag_value() {
    let mut doc = Document::open(&docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val=" client-name "/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old Client</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");

    let changed = doc
        .fill_content_control_by_tag("client-name", "Acme")
        .expect("tagged content control filled");

    assert_eq!(changed, 1);
    let saved = doc.save().expect("save edited docx");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:t>Acme</w:t>"),
        "filled text missing: {body}"
    );
    assert!(!body.contains("Old Client"), "old text leaked: {body}");
}

#[test]
fn fill_content_control_by_tag_missing_tag_is_noop() {
    let fixture = content_control_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(
        doc.fill_content_control_by_tag("missing", "Value").unwrap(),
        0
    );
    assert!(doc.edited_parts().is_empty());

    let after = unzip_parts(&doc.save().expect("save noop"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "missing tag fill should not canonicalize document.xml"
    );
}

#[test]
fn fill_content_control_by_tag_rejects_empty_tag() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");

    let err = doc
        .fill_content_control_by_tag("", "Value")
        .expect_err("empty tag rejected");

    assert!(err.to_string().contains("tag must not be empty"), "{err}");
    assert!(doc.edited_parts().is_empty());
}

#[test]
fn fill_content_controls_by_tag_updates_multiple_tags_atomically() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");

    let changed = doc
        .fill_content_controls_by_tag([("client-name", "Acme & Co"), ("project-name", "Roadmap")])
        .expect("tagged content controls filled");

    assert_eq!(changed, 3);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert_eq!(
        body.matches("<w:t>Acme &amp; Co</w:t>").count(),
        2,
        "both client-name controls should be filled: {body}"
    );
    assert!(
        body.contains("<w:t>Roadmap</w:t>"),
        "project-name control should be filled: {body}"
    );
    assert!(
        !body.contains(">Old<")
            && !body.contains(">Old Again<")
            && !body.contains(">Keep Project<"),
        "old template values leaked after multi-fill: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert!(reopened.text().contains("Acme & Co"));
    assert!(reopened.text().contains("Roadmap"));
}

#[test]
fn fill_content_controls_by_tag_empty_input_is_noop() {
    let fixture = content_control_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(
        doc.fill_content_controls_by_tag(std::iter::empty::<(&str, &str)>())
            .unwrap(),
        0
    );
    assert!(doc.edited_parts().is_empty());

    let after = unzip_parts(&doc.save().expect("save noop"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "empty multi-fill should not canonicalize document.xml"
    );
}

#[test]
fn fill_content_controls_by_tag_rejects_empty_tag() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");

    let err = doc
        .fill_content_controls_by_tag([("", "Value"), ("project-name", "Roadmap")])
        .expect_err("empty tag rejected");

    assert!(err.to_string().contains("tag must not be empty"), "{err}");
    assert!(doc.edited_parts().is_empty());
}

#[test]
fn fill_content_controls_by_tag_rejects_duplicate_tag() {
    let mut doc = Document::open(&content_control_docx()).expect("fixture opens");

    let err = doc
        .fill_content_controls_by_tag([("client-name", "First"), ("client-name", "Second")])
        .expect_err("duplicate tag rejected");

    assert!(err.to_string().contains("duplicate tag"), "{err}");
    assert!(doc.edited_parts().is_empty());
}

#[test]
fn fill_template_fields_updates_content_controls_and_merge_fields() {
    let mut doc = Document::open(&merge_template_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].instruction,
        "MERGEFIELD client-name \\* MERGEFORMAT"
    );
    assert_eq!(
        fields[1].instruction,
        "MERGEFIELD \"project-name\" \\* MERGEFORMAT"
    );

    let changed = doc
        .fill_template_fields([("client-name", "Acme & Co"), ("project-name", "Roadmap")])
        .expect("template fields filled");

    assert_eq!(changed, 3);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        body.contains(r#"<w:alias w:val="Client name"/>"#)
            && body.contains(r#"<w:tag w:val="client-name"/>"#),
        "content-control metadata not preserved: {body}"
    );
    assert!(
        body.contains("<w:t>Acme &amp; Co</w:t>") && body.contains("<w:t>Roadmap</w:t>"),
        "filled template text missing or unescaped: {body}"
    );
    assert!(
        body.contains(r#"MERGEFIELD  client-name  \* MERGEFORMAT"#)
            && body.contains(r#"MERGEFIELD "project-name" \* MERGEFORMAT"#),
        "merge field instructions should be preserved: {body}"
    );
    assert!(
        !body.contains("Old Client Control")
            && !body.contains("Old Client Field")
            && !body.contains("Old Project Field"),
        "old template values leaked after fill: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let reopened_fields = reopened.fields();
    assert_eq!(reopened_fields[0].result, "Acme & Co");
    assert_eq!(reopened_fields[1].result, "Roadmap");
    assert!(reopened.text().contains("Acme & Co"));
    assert!(reopened.text().contains("Roadmap"));
}

#[test]
fn fill_template_fields_writes_body_markers_and_clears_them_on_refill() {
    let mut doc = Document::open(&merge_template_docx()).expect("fixture opens");
    let client = "Client\tValue\nNext";
    let project = "Project\tValue\nNext";

    assert_eq!(
        doc.fill_template_fields([("client-name", client), ("project-name", project)])
            .expect("body template fields filled with markers"),
        3
    );

    let marked = doc.save().expect("save marker-filled template");
    let marked_body = String::from_utf8(unzip_parts(&marked)["word/document.xml"].clone()).unwrap();
    let client_xml = "<w:t>Client</w:t><w:tab/><w:t>Value</w:t><w:br/><w:t>Next</w:t>";
    let project_xml = "<w:t>Project</w:t><w:tab/><w:t>Value</w:t><w:br/><w:t>Next</w:t>";
    assert_eq!(
        marked_body.matches(client_xml).count(),
        2,
        "content control and simple MERGEFIELD should both use markers: {marked_body}"
    );
    assert_eq!(
        marked_body.matches(project_xml).count(),
        1,
        "complex MERGEFIELD should use markers: {marked_body}"
    );
    let mut reopened_marked = Document::open(&marked).expect("reopen marker-filled template");
    let marked_fields = reopened_marked.fields();
    assert_eq!(marked_fields[0].result, client);
    assert_eq!(marked_fields[1].result, project);
    assert_eq!(reopened_marked.text().matches(client).count(), 2);
    assert!(reopened_marked.text().contains(project));
    assert_eq!(
        reopened_marked
            .fill_template_fields([
                ("client-name", "Reopened client"),
                ("project-name", "Reopened project"),
            ])
            .expect("refill reopened body template fields"),
        3
    );
    let reopened_refill = reopened_marked
        .save()
        .expect("save reopened template refill");
    let reopened_body =
        String::from_utf8(unzip_parts(&reopened_refill)["word/document.xml"].clone()).unwrap();
    assert!(
        !reopened_body.contains("<w:tab") && !reopened_body.contains("<w:br"),
        "post-reopen refill retained stale template markers: {reopened_body}"
    );
    let reopened_refill_doc =
        Document::open(&reopened_refill).expect("reopen post-reopen template refill");
    let reopened_refill_fields = reopened_refill_doc.fields();
    assert_eq!(reopened_refill_fields[0].result, "Reopened client");
    assert_eq!(reopened_refill_fields[1].result, "Reopened project");

    assert_eq!(
        doc.fill_template_fields([
            ("client-name", "Plain client"),
            ("project-name", "Plain project"),
        ])
        .expect("refill body template fields"),
        3
    );
    let plain = doc.save().expect("save refilled template");
    let plain_body = String::from_utf8(unzip_parts(&plain)["word/document.xml"].clone()).unwrap();
    assert!(
        !plain_body.contains("<w:tab") && !plain_body.contains("<w:br"),
        "plain refill retained stale template markers: {plain_body}"
    );
    let reopened_plain = Document::open(&plain).expect("reopen refilled template");
    let plain_fields = reopened_plain.fields();
    assert_eq!(plain_fields[0].result, "Plain client");
    assert_eq!(plain_fields[1].result, "Plain project");
}

#[test]
fn fill_template_fields_marker_only_results_remain_refillable_after_reopen() {
    let mut doc = Document::open(&merge_template_docx()).expect("fixture opens");

    assert_eq!(
        doc.fill_template_fields([("client-name", "\t\n"), ("project-name", "\t\n")])
            .expect("body template fields filled with marker-only values"),
        3
    );
    let marked = doc.save().expect("save marker-only template fields");
    let marked_body = String::from_utf8(unzip_parts(&marked)["word/document.xml"].clone()).unwrap();
    assert_eq!(
        marked_body.matches("<w:tab/><w:br/><w:t></w:t>").count(),
        3,
        "marker-only template fills should retain empty refill anchors: {marked_body}"
    );

    let mut reopened = Document::open(&marked).expect("reopen marker-only template fields");
    assert_eq!(
        reopened
            .fill_template_fields([
                ("client-name", "Refilled client"),
                ("project-name", "Refilled project"),
            ])
            .expect("refill marker-only template fields"),
        3
    );
    let refilled = reopened.save().expect("save refilled template fields");
    let refilled_body =
        String::from_utf8(unzip_parts(&refilled)["word/document.xml"].clone()).unwrap();
    assert!(
        !refilled_body.contains("<w:tab") && !refilled_body.contains("<w:br"),
        "refill retained marker-only template siblings: {refilled_body}"
    );
    let reopened_refill = Document::open(&refilled).expect("reopen refilled template fields");
    let fields = reopened_refill.fields();
    assert_eq!(fields[0].result, "Refilled client");
    assert_eq!(fields[1].result, "Refilled project");
}

#[test]
fn fill_template_fields_markers_follow_alternate_wml_prefix_for_merge_fields() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<x:document xmlns:x="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><x:body><x:p><x:fldSimple x:instr=" MERGEFIELD client-name "><x:r><x:t>Old client</x:t></x:r></x:fldSimple></x:p><x:p><x:r><x:fldChar x:fldCharType="begin"/></x:r><x:r><x:instrText> MERGEFIELD project-name </x:instrText></x:r><x:r><x:fldChar x:fldCharType="separate"/></x:r><x:r><x:t>Old project</x:t></x:r><x:r><x:fldChar x:fldCharType="end"/></x:r></x:p></x:body></x:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("alternate-prefix merge fixture opens");

    assert_eq!(
        doc.fill_template_fields([
            ("client-name", "Client\tValue\nNext"),
            ("project-name", "Project\tValue\nNext"),
        ])
        .expect("alternate-prefix merge fields filled"),
        2
    );
    let saved = doc.save().expect("save alternate-prefix merge fields");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<x:t>Client</x:t><x:tab/><x:t>Value</x:t><x:br/><x:t>Next</x:t>")
            && body.contains("<x:t>Project</x:t><x:tab/><x:t>Value</x:t><x:br/><x:t>Next</x:t>"),
        "simple or complex MERGEFIELD did not use the bound WML prefix: {body}"
    );
    assert!(
        !body.contains("<w:"),
        "alternate-prefix MERGEFIELD generated an unbound canonical prefix: {body}"
    );
    let reopened = Document::open(&saved).expect("alternate-prefix merge result reopens");
    let fields = reopened.fields();
    assert_eq!(fields[0].result, "Client\tValue\nNext");
    assert_eq!(fields[1].result, "Project\tValue\nNext");
}

#[test]
fn fill_template_fields_preserves_paragraph_tab_stops_in_complex_field_results() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MERGEFIELD project-name </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r></w:p><w:p><w:pPr><w:tabs><w:tab w:val="right" w:pos="1440"/></w:tabs></w:pPr><w:r><w:t>Old</w:t><w:tab/><w:t>result</w:t><w:br/></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("paragraph-spanning merge fixture opens");

    assert_eq!(
        doc.fill_template_fields([("project-name", "Project\tValue\nNext")])
            .expect("paragraph-spanning merge field filled"),
        1
    );
    let saved = doc.save().expect("save paragraph-spanning merge field");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains(r#"<w:pPr><w:tabs><w:tab w:val="right" w:pos="1440"/></w:tabs></w:pPr>"#),
        "complex field fill consumed a paragraph tab-stop definition: {body}"
    );
    assert!(
        body.contains("<w:t>Project</w:t><w:tab/><w:t>Value</w:t><w:br/><w:t>Next</w:t>"),
        "complex field fill did not replace actual run content: {body}"
    );

    Document::open(&saved).expect("paragraph-spanning merge result reopens");
}

#[test]
fn fill_template_fields_rejects_overlapping_body_targets_without_mutation() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val="outer-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Outer</w:t></w:r><w:sdt><w:sdtPr><w:tag w:val="inner-name"/></w:sdtPr><w:sdtContent><w:r><w:t>Inner</w:t></w:r></w:sdtContent></w:sdt></w:p></w:sdtContent></w:sdt></w:body></w:document>"#,
        ),
    ]);
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("nested body template opens");

    let err = doc
        .fill_template_fields([
            ("outer-name", "Outer\tvalue"),
            ("inner-name", "Inner\nvalue"),
        ])
        .expect_err("overlapping body template targets rejected");

    assert!(err.to_string().contains("overlap"), "{err}");
    assert!(doc.edited_parts().is_empty());
    assert_eq!(
        unzip_parts(&doc.save().expect("save rejected body overlap")),
        before,
        "body overlap rejection changed the package"
    );
}

#[test]
fn fill_template_fields_rejects_overlapping_story_targets_without_mutation() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:tag w:val="outer-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Outer</w:t></w:r><w:sdt><w:sdtPr><w:tag w:val="inner-name"/></w:sdtPr><w:sdtContent><w:r><w:t>Inner</w:t></w:r></w:sdtContent></w:sdt></w:p></w:sdtContent></w:sdt></w:hdr>"#,
        ),
    ]);
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("nested story template opens");

    let err = doc
        .fill_template_fields([
            ("outer-name", "Outer\tvalue"),
            ("inner-name", "Inner\nvalue"),
        ])
        .expect_err("overlapping story template targets rejected");

    assert!(err.to_string().contains("overlap"), "{err}");
    assert!(doc.edited_parts().is_empty());
    assert_eq!(
        unzip_parts(&doc.save().expect("save rejected story overlap")),
        before,
        "story overlap rejection changed the package"
    );
}

#[test]
fn fill_template_fields_updates_unquoted_multi_token_merge_field_names() {
    let mut doc =
        Document::open(&unquoted_multi_token_merge_template_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].instruction,
        "MERGEFIELD Client Name \\* MERGEFORMAT"
    );
    assert_eq!(fields[0].result, "Old Client Field");

    let changed = doc
        .fill_template_fields([("Client Name", "Acme Corp")])
        .expect("template fields filled");

    assert_eq!(changed, 1);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        body.contains(r#"MERGEFIELD Client Name \* MERGEFORMAT"#),
        "merge field instruction should be preserved: {body}"
    );
    assert!(
        body.contains("<w:t>Acme Corp</w:t>"),
        "filled value missing: {body}"
    );
    assert!(
        !body.contains("Old Client Field"),
        "old merge field value leaked after fill: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let reopened_fields = reopened.fields();
    assert_eq!(reopened_fields[0].result, "Acme Corp");
    assert!(reopened.text().contains("Acme Corp"));
}

#[test]
fn fill_template_fields_missing_names_are_noop() {
    let fixture = merge_template_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(doc.fill_template_fields([("missing", "Value")]).unwrap(), 0);
    assert!(doc.edited_parts().is_empty());

    let after = unzip_parts(&doc.save().expect("save noop"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "missing template field should not canonicalize document.xml"
    );
}

#[test]
fn fill_template_fields_updates_referenced_header_footer_merge_fields() {
    let mut doc = Document::open(&header_footer_merge_template_docx()).expect("fixture opens");

    let changed = doc
        .fill_template_fields([("client-name", "Acme & Co"), ("project-name", "Roadmap")])
        .expect("header/footer merge fields filled");

    assert_eq!(changed, 2);
    assert_eq!(doc.edited_parts(), ["word/footer1.xml", "word/header1.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let orphan_header = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        body.contains("<w:t>Body</w:t>"),
        "body text should stay unchanged: {body}"
    );
    assert!(
        header.contains("<w:t>Acme &amp; Co</w:t>")
            && header.contains(r#"MERGEFIELD client-name \* MERGEFORMAT"#),
        "referenced header field should be filled while preserving instruction: {header}"
    );
    assert!(
        footer.contains("<w:t>Roadmap</w:t>")
            && footer.contains(r#"MERGEFIELD "project-name" \* MERGEFORMAT"#),
        "referenced footer field should be filled while preserving instruction: {footer}"
    );
    assert!(
        orphan_header.contains("Orphan Header Client") && !orphan_header.contains("Acme"),
        "unreferenced header part should not be edited: {orphan_header}"
    );
}

#[test]
fn fill_template_fields_updates_referenced_header_footer_content_controls() {
    let mut doc =
        Document::open(&header_footer_content_control_template_docx()).expect("fixture opens");

    let changed = doc
        .fill_template_fields([("client-name", "Acme & Co"), ("project-name", "Roadmap")])
        .expect("header/footer content controls filled");

    assert_eq!(changed, 2);
    assert_eq!(doc.edited_parts(), ["word/footer1.xml", "word/header1.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let orphan_header = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        body.contains("<w:t>Body</w:t>"),
        "body text should stay unchanged: {body}"
    );
    assert!(
        header.contains(r#"<w:tag w:val="client-name"/>"#)
            && header.contains("<w:t>Acme &amp; Co</w:t>"),
        "referenced header content control should be filled and keep metadata: {header}"
    );
    assert!(
        footer.contains(r#"<w:tag w:val="project-name"/>"#)
            && footer.contains("<w:t>Roadmap</w:t>")
            && footer.contains("<w:t></w:t>"),
        "referenced footer content control should fill first run and clear stale runs: {footer}"
    );
    assert!(
        orphan_header.contains("Orphan Header Client") && !orphan_header.contains("Acme"),
        "unreferenced header part should not be edited: {orphan_header}"
    );
}

#[test]
fn fill_template_fields_updates_note_content_controls_and_merge_fields() {
    let mut doc = Document::open(&note_template_docx()).expect("fixture opens");

    let changed = doc
        .fill_template_fields([("client-name", "Acme & Co"), ("project-name", "Roadmap")])
        .expect("note template fields filled");

    assert_eq!(changed, 4);
    assert_eq!(
        doc.edited_parts(),
        ["word/endnotes.xml", "word/footnotes.xml"]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();

    assert!(
        body.contains("<w:t>Body</w:t>") && !body.contains("Acme"),
        "body text should stay unchanged: {body}"
    );
    assert!(
        footnotes.contains(r#"<w:tag w:val="client-name"/>"#)
            && footnotes.contains("<w:t>Acme &amp; Co</w:t>")
            && footnotes.contains(r#"MERGEFIELD project-name \* MERGEFORMAT"#)
            && footnotes.contains("<w:t>Roadmap</w:t>"),
        "footnote template fields should be filled while preserving metadata/instructions: {footnotes}"
    );
    assert!(
        endnotes.contains(r#"MERGEFIELD "client-name" \* MERGEFORMAT"#)
            && endnotes.contains("<w:t>Acme &amp; Co</w:t>")
            && endnotes.contains(r#"<w:tag w:val="project-name"/>"#)
            && endnotes.contains("<w:t>Roadmap</w:t>")
            && endnotes.contains("<w:t></w:t>"),
        "endnote template fields should be filled and stale content-control runs cleared: {endnotes}"
    );
    assert!(
        !footnotes.contains("Old Foot") && !endnotes.contains("Old End"),
        "old note template values leaked after fill: {footnotes} {endnotes}"
    );

    let reopened = Document::open(&saved).expect("reopen edited notes");
    assert!(reopened.footnote_text().contains("Acme & Co"));
    assert!(reopened.footnote_text().contains("Roadmap"));
    assert!(reopened.endnote_text().contains("Acme & Co"));
    assert!(reopened.endnote_text().contains("Roadmap"));
}

#[test]
fn fill_template_fields_writes_markers_in_header_footer_and_note_stories() {
    let client = "Client\tValue\nNext";
    let project = "Project\tValue\nNext";

    let header_fixture = header_footer_merge_template_docx();
    let header_before = unzip_parts(&header_fixture);
    let mut header_doc = Document::open(&header_fixture).expect("header fixture opens");
    assert_eq!(
        header_doc
            .fill_template_fields([("client-name", client), ("project-name", project)])
            .expect("header/footer template fields filled with markers"),
        2
    );
    assert_eq!(
        header_doc.edited_parts(),
        ["word/footer1.xml", "word/header1.xml"]
    );
    let header_saved = header_doc.save().expect("save header/footer template");
    let header_parts = unzip_parts(&header_saved);
    let header = String::from_utf8(header_parts["word/header1.xml"].clone()).unwrap();
    let footer = String::from_utf8(header_parts["word/footer1.xml"].clone()).unwrap();
    assert!(
        header.contains("<w:t>Client</w:t><w:tab/><w:t>Value</w:t><w:br/><w:t>Next</w:t>"),
        "header MERGEFIELD did not use WML markers: {header}"
    );
    assert!(
        footer.contains("<w:t>Project</w:t><w:tab/><w:t>Value</w:t><w:br/><w:t>Next</w:t>"),
        "footer MERGEFIELD did not use WML markers: {footer}"
    );
    assert_eq!(
        header_parts["word/document.xml"], header_before["word/document.xml"],
        "story-only fill rewrote the body part"
    );
    assert_eq!(
        header_parts["word/header2.xml"], header_before["word/header2.xml"],
        "story-only fill rewrote an unreferenced header"
    );
    let mut reopened_header =
        Document::open(&header_saved).expect("reopen marker-filled header/footer template");
    assert!(reopened_header.header_text().contains(client));
    assert!(reopened_header.header_text().contains(project));
    assert_eq!(
        reopened_header
            .fill_template_fields([
                ("client-name", "Reopened client"),
                ("project-name", "Reopened project"),
            ])
            .expect("refill reopened header/footer template"),
        2
    );
    assert_eq!(
        reopened_header.edited_parts(),
        ["word/footer1.xml", "word/header1.xml"]
    );
    let header_refilled = reopened_header
        .save()
        .expect("save refilled header/footer template");
    let header_refilled_parts = unzip_parts(&header_refilled);
    assert_eq!(
        header_refilled_parts["word/document.xml"],
        header_before["word/document.xml"]
    );
    assert_eq!(
        header_refilled_parts["word/header2.xml"],
        header_before["word/header2.xml"]
    );
    for part in ["word/header1.xml", "word/footer1.xml"] {
        let xml = String::from_utf8(header_refilled_parts[part].clone()).unwrap();
        assert!(
            !xml.contains("<w:tab") && !xml.contains("<w:br"),
            "post-reopen {part} refill retained stale markers: {xml}"
        );
    }

    let mut note_doc = Document::open(&note_template_docx()).expect("note fixture opens");
    assert_eq!(
        note_doc
            .fill_template_fields([("client-name", client), ("project-name", project)])
            .expect("note template fields filled with markers"),
        4
    );
    assert_eq!(
        note_doc.edited_parts(),
        ["word/endnotes.xml", "word/footnotes.xml"]
    );
    let note_saved = note_doc.save().expect("save marker-filled notes");
    let note_parts = unzip_parts(&note_saved);
    for (part, expected_count) in [("word/footnotes.xml", 2), ("word/endnotes.xml", 2)] {
        let xml = String::from_utf8(note_parts[part].clone()).unwrap();
        assert_eq!(
            xml.matches("<w:tab/>").count(),
            expected_count,
            "{part} did not preserve every template tab: {xml}"
        );
        assert_eq!(
            xml.matches("<w:br/>").count(),
            expected_count,
            "{part} did not preserve every template break: {xml}"
        );
    }
    let mut reopened_notes = Document::open(&note_saved).expect("reopen marker-filled notes");
    assert!(reopened_notes.footnote_text().contains(client));
    assert!(reopened_notes.footnote_text().contains(project));
    assert!(reopened_notes.endnote_text().contains(client));
    assert!(reopened_notes.endnote_text().contains(project));
    assert_eq!(
        reopened_notes
            .fill_template_fields([
                ("client-name", "Reopened client"),
                ("project-name", "Reopened project"),
            ])
            .expect("refill reopened note template"),
        4
    );
    assert_eq!(
        reopened_notes.edited_parts(),
        ["word/endnotes.xml", "word/footnotes.xml"]
    );
    let notes_refilled = reopened_notes.save().expect("save refilled note template");
    let notes_refilled_parts = unzip_parts(&notes_refilled);
    for part in ["word/footnotes.xml", "word/endnotes.xml"] {
        let xml = String::from_utf8(notes_refilled_parts[part].clone()).unwrap();
        assert!(
            !xml.contains("<w:tab") && !xml.contains("<w:br"),
            "post-reopen {part} refill retained stale markers: {xml}"
        );
    }
}

#[test]
fn fill_template_fields_only_edits_selected_alternate_content_branches() {
    let mut doc = Document::open(&alternate_content_template_docx()).expect("fixture opens");

    let changed = doc
        .fill_template_fields([
            ("body-name", "Body updated"),
            ("foot-name", "Foot updated"),
            ("end-name", "End updated"),
            ("header-name", "Header updated"),
        ])
        .expect("selected template fields filled");

    assert_eq!(changed, 4);
    assert_eq!(
        doc.edited_parts(),
        [
            "word/document.xml",
            "word/endnotes.xml",
            "word/footnotes.xml",
            "word/header1.xml"
        ]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>Body updated</w:t>"), "{body}");
    assert!(
        body.contains(r#"<compat:Fallback><w:sdt><w:sdtPr><w:tag w:val="body-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Body fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></compat:Fallback>"#),
        "{body}"
    );
    assert!(
        footnotes.contains("<w:t>Foot updated</w:t>")
            && footnotes.contains(r#"<mcx:Fallback><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></mcx:Fallback>"#)
            && footnotes.contains(r#"<mcx:Fallback><w:footnote w:id="9"><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot entry fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></w:footnote></mcx:Fallback>"#)
            && footnotes.contains(r#"<w:footnote w:type="separator" w:id="-1"><w:sdt><w:sdtPr><w:tag w:val="foot-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot separator</w:t></w:r></w:p></w:sdtContent></w:sdt></w:footnote>"#),
        "{footnotes}"
    );
    assert!(
        endnotes.contains("<w:t>End updated</w:t>")
            && endnotes.contains(r#"<alt:Fallback><w:p><w:fldSimple w:instr=" MERGEFIELD end-name "><w:r><w:t>End fallback hidden</w:t></w:r></w:fldSimple></w:p></alt:Fallback>"#)
            && endnotes.contains("<w:t>End selected keep</w:t>")
            && endnotes.contains(r#"<w:endnote w:type="continuationSeparator" w:id="0"><w:p><w:fldSimple w:instr=" MERGEFIELD end-name "><w:r><w:t>End separator</w:t></w:r></w:fldSimple></w:p></w:endnote>"#),
        "{endnotes}"
    );
    assert!(
        header.contains("<w:t>Header updated</w:t>")
            && header.contains(r#"<m:Fallback><w:sdt><w:sdtPr><w:tag w:val="header-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Header fallback</w:t></w:r></w:p></w:sdtContent></w:sdt></m:Fallback>"#),
        "{header}"
    );
    assert_eq!(
        parts.get("word/header2.xml"),
        unzip_parts(&alternate_content_template_docx()).get("word/header2.xml"),
        "unreferenced header should remain byte-preserved"
    );
}

#[test]
fn fill_template_fields_treats_foreign_alternate_content_as_ordinary_xml() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:foreign="urn:not-markup-compatibility"><w:body><foreign:AlternateContent><foreign:Choice><w:p><w:fldSimple w:instr=" MERGEFIELD keep-name "><w:r><w:t>Keep choice</w:t></w:r></w:fldSimple></w:p></foreign:Choice><foreign:Fallback><w:p><w:fldSimple w:instr=" MERGEFIELD client-name "><w:r><w:t>Foreign client</w:t></w:r></w:fldSimple></w:p></foreign:Fallback></foreign:AlternateContent><w:p><w:fldSimple w:instr=" MERGEFIELD client-name "><w:r><w:t>Visible client</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(
        doc.fill_template_fields([("client-name", "Updated client")])
            .expect("foreign-wrapper fields filled"),
        2
    );

    let body = String::from_utf8(
        unzip_parts(&doc.save().expect("save edited docx"))["word/document.xml"].clone(),
    )
    .unwrap();
    assert_eq!(
        body.matches("<w:t>Updated client</w:t>").count(),
        2,
        "{body}"
    );
    assert!(body.contains("<w:t>Keep choice</w:t>"), "{body}");
}

#[test]
fn fill_template_fields_missing_header_footer_names_are_noop() {
    let fixture = header_footer_merge_template_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(doc.fill_template_fields([("missing", "Value")]).unwrap(), 0);
    assert!(doc.edited_parts().is_empty());

    let after = unzip_parts(&doc.save().expect("save noop"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "missing header/footer template field should not canonicalize document.xml"
    );
    assert_eq!(
        before.get("word/header1.xml"),
        after.get("word/header1.xml"),
        "missing header/footer template field should not canonicalize header1.xml"
    );
    assert_eq!(
        before.get("word/footer1.xml"),
        after.get("word/footer1.xml"),
        "missing header/footer template field should not canonicalize footer1.xml"
    );
}

#[test]
fn fill_template_fields_rejects_header_footer_merge_field_without_cached_result_without_mutation() {
    let fixture = header_footer_merge_template_missing_result_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    let err = doc
        .fill_template_fields([("client-name", "Acme")])
        .expect_err("header/footer merge field without cached text rejected");

    assert!(
        err.to_string()
            .contains("merge field \"client-name\" has no cached result text"),
        "{err}"
    );
    assert!(doc.edited_parts().is_empty());
    let after = unzip_parts(&doc.save().expect("save after rejected edit"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "rejected header/footer template fill should not mutate document.xml"
    );
    assert_eq!(
        before.get("word/header1.xml"),
        after.get("word/header1.xml"),
        "rejected header/footer template fill should not mutate header1.xml"
    );
}

#[test]
fn fill_template_fields_rejects_empty_name() {
    let mut doc = Document::open(&merge_template_docx()).expect("fixture opens");

    let err = doc
        .fill_template_fields([("", "Value")])
        .expect_err("empty template field name rejected");

    assert!(
        err.to_string().contains("field name must not be empty"),
        "{err}"
    );
    assert!(doc.edited_parts().is_empty());
}

#[test]
fn fill_template_fields_rejects_duplicate_name() {
    let mut doc = Document::open(&merge_template_docx()).expect("fixture opens");

    let err = doc
        .fill_template_fields([("client-name", "First"), ("client-name", "Second")])
        .expect_err("duplicate template field name rejected");

    assert!(err.to_string().contains("duplicate field name"), "{err}");
    assert!(doc.edited_parts().is_empty());
}

#[test]
fn docx_revision_text_preserves_supported_symbols() {
    let doc = Document::open(&symbol_tracked_revisions_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 4);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Insert • keep");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Delete • drop");
    assert_eq!(revisions[2].kind, RevisionKind::MoveFrom);
    assert_eq!(revisions[2].text, "From • old");
    assert_eq!(revisions[3].kind, RevisionKind::MoveTo);
    assert_eq!(revisions[3].text, "To • new");

    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Before • base Insert • keep To • new After"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Before • base Delete • drop From • old After"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "Before • base [+Insert • keep] [-Delete • drop] [~From • old->] [~->To • new] After"
    );
}

#[test]
fn docx_revision_text_preserves_expanded_supported_symbols() {
    let doc = Document::open(&expanded_symbol_tracked_revisions_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 4);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Insert • keep");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Delete • drop");
    assert_eq!(revisions[2].kind, RevisionKind::MoveFrom);
    assert_eq!(revisions[2].text, "From • old");
    assert_eq!(revisions[3].kind, RevisionKind::MoveTo);
    assert_eq!(revisions[3].text, "To • new");
}

#[test]
fn accept_all_revisions_accepts_body_tracked_changes() {
    let mut doc = Document::open(&tracked_revisions_docx()).expect("fixture opens");

    let changed = doc.accept_all_revisions().expect("body revisions accepted");

    assert_eq!(changed, 6);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    for marker in [
        "<w:ins",
        "<w:del",
        "<w:moveFrom",
        "<w:moveTo",
        "<w:pPrChange",
        "<w:rPrChange",
        "<w:delText",
    ] {
        assert!(
            !body.contains(marker),
            "accepted document still contains {marker}: {body}"
        );
    }
    assert!(
        body.contains("<w:t>Before</w:t>")
            && body.contains("<w:t> added</w:t>")
            && body.contains("<w:t> to</w:t>")
            && body.contains("<w:t> After</w:t>")
            && body.contains("<w:t>Props</w:t>"),
        "accepted text was not preserved: {body}"
    );
    assert!(
        !body.contains("removed") && !body.contains(" from"),
        "rejected revision text leaked into accepted document: {body}"
    );
    assert!(
        body.contains(r#"<w:jc w:val="left"/>"#) && body.contains("<w:b/>"),
        "current properties should be preserved while property-change history is removed: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen accepted docx");
    assert!(reopened.revisions().is_empty());
    assert!(reopened.text().contains("Before added to After"));
    assert!(reopened.text().contains("Props"));
}

#[test]
fn accept_all_revisions_accepts_note_tracked_changes() {
    let mut doc = Document::open(&tracked_note_revisions_docx()).expect("fixture opens");

    assert_eq!(doc.revisions().len(), 4);
    let changed = doc.accept_all_revisions().expect("note revisions accepted");

    assert_eq!(changed, 4);
    assert_eq!(
        doc.edited_parts(),
        ["word/endnotes.xml", "word/footnotes.xml"]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();
    for marker in ["<w:ins", "<w:del", "<w:moveFrom", "<w:moveTo", "<w:delText"] {
        assert!(
            !footnotes.contains(marker) && !endnotes.contains(marker),
            "accepted note revisions still contain {marker}: {footnotes} {endnotes}"
        );
    }
    assert!(footnotes.contains("Foot added") && !footnotes.contains("Foot removed"));
    assert!(endnotes.contains("End to") && !endnotes.contains("End from"));

    let reopened = Document::open(&saved).expect("reopen accepted notes");
    assert!(reopened.revisions().is_empty());
    assert_eq!(reopened.footnote_text(), "Foot added");
    assert_eq!(reopened.endnote_text(), "End to");
}

#[test]
fn accept_all_revisions_accepts_header_footer_tracked_changes() {
    let mut doc = Document::open(&tracked_header_footer_revisions_docx()).expect("fixture opens");

    assert_eq!(doc.revisions().len(), 4);
    let changed = doc
        .accept_all_revisions()
        .expect("header/footer revisions accepted");

    assert_eq!(changed, 4);
    assert_eq!(doc.edited_parts(), ["word/footer1.xml", "word/header1.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    for marker in ["<w:ins", "<w:del", "<w:moveFrom", "<w:moveTo", "<w:delText"] {
        assert!(
            !header.contains(marker) && !footer.contains(marker),
            "accepted header/footer revisions still contain {marker}: {header} {footer}"
        );
    }
    assert!(header.contains("Header added") && !header.contains("Header removed"));
    assert!(footer.contains("Footer to") && !footer.contains("Footer from"));

    let reopened = Document::open(&saved).expect("reopen accepted header/footer");
    assert!(reopened.revisions().is_empty());
    assert!(reopened.header_text().contains("Header added"));
    assert!(reopened.header_text().contains("Footer to"));
}

#[test]
fn accept_all_revisions_without_revisions_is_noop() {
    let fixture = content_control_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(doc.accept_all_revisions().unwrap(), 0);
    assert!(doc.edited_parts().is_empty());

    let after = unzip_parts(&doc.save().expect("save noop"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "no-op revision acceptance should not canonicalize document.xml"
    );
}

#[test]
fn reject_all_revisions_rejects_body_tracked_changes() {
    let mut doc = Document::open(&tracked_revisions_docx()).expect("fixture opens");

    let changed = doc.reject_all_revisions().expect("body revisions rejected");

    assert_eq!(changed, 8);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    for marker in [
        "<w:ins",
        "<w:del",
        "<w:moveFrom",
        "<w:moveTo",
        "<w:pPrChange",
        "<w:rPrChange",
        "<w:delText",
    ] {
        assert!(
            !body.contains(marker),
            "rejected document still contains {marker}: {body}"
        );
    }
    assert!(
        body.contains("<w:t>Before</w:t>")
            && body.contains("<w:t> removed</w:t>")
            && body.contains("<w:t> from</w:t>")
            && body.contains("<w:t> After</w:t>")
            && body.contains("<w:t>Props</w:t>"),
        "original text was not restored: {body}"
    );
    assert!(
        !body.contains(" added") && !body.contains(" to"),
        "accepted revision text leaked into rejected document: {body}"
    );
    assert!(
        body.contains(r#"<w:jc w:val="left"/>"#) && body.contains("<w:b/>"),
        "current properties should be preserved while property-change history is removed: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen rejected docx");
    assert!(reopened.revisions().is_empty());
    assert!(reopened.text().contains("Before removed from After"));
    assert!(reopened.text().contains("Props"));
}

#[test]
fn reject_all_revisions_rejects_note_tracked_changes() {
    let mut doc = Document::open(&tracked_note_revisions_docx()).expect("fixture opens");

    assert_eq!(doc.revisions().len(), 4);
    let changed = doc.reject_all_revisions().expect("note revisions rejected");

    assert_eq!(changed, 6);
    assert_eq!(
        doc.edited_parts(),
        ["word/endnotes.xml", "word/footnotes.xml"]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();
    for marker in ["<w:ins", "<w:del", "<w:moveFrom", "<w:moveTo", "<w:delText"] {
        assert!(
            !footnotes.contains(marker) && !endnotes.contains(marker),
            "rejected note revisions still contain {marker}: {footnotes} {endnotes}"
        );
    }
    assert!(footnotes.contains("Foot removed") && !footnotes.contains("Foot added"));
    assert!(endnotes.contains("End from") && !endnotes.contains("End to"));

    let reopened = Document::open(&saved).expect("reopen rejected notes");
    assert!(reopened.revisions().is_empty());
    assert_eq!(reopened.footnote_text(), "Foot removed");
    assert_eq!(reopened.endnote_text(), "End from");
}

#[test]
fn reject_all_revisions_rejects_header_footer_tracked_changes() {
    let mut doc = Document::open(&tracked_header_footer_revisions_docx()).expect("fixture opens");

    assert_eq!(doc.revisions().len(), 4);
    let changed = doc
        .reject_all_revisions()
        .expect("header/footer revisions rejected");

    assert_eq!(changed, 6);
    assert_eq!(doc.edited_parts(), ["word/footer1.xml", "word/header1.xml"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    for marker in ["<w:ins", "<w:del", "<w:moveFrom", "<w:moveTo", "<w:delText"] {
        assert!(
            !header.contains(marker) && !footer.contains(marker),
            "rejected header/footer revisions still contain {marker}: {header} {footer}"
        );
    }
    assert!(header.contains("Header removed") && !header.contains("Header added"));
    assert!(footer.contains("Footer from") && !footer.contains("Footer to"));

    let reopened = Document::open(&saved).expect("reopen rejected header/footer");
    assert!(reopened.revisions().is_empty());
    assert!(reopened.header_text().contains("Header removed"));
    assert!(reopened.header_text().contains("Footer from"));
}

#[test]
fn reject_all_revisions_resolves_newly_active_old_header() {
    let fixture = revision_switched_header_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    let changed = doc.reject_all_revisions().expect("revisions rejected");

    assert_eq!(changed, 5);
    assert_eq!(
        doc.edited_parts(),
        ["word/document.xml", "word/header2.xml"]
    );
    let saved = doc.save().expect("save rejected docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/header1.xml"], before["word/header1.xml"]);
    let old_header = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    for marker in ["<w:ins", "<w:del", "<w:delText"] {
        assert!(
            !old_header.contains(marker),
            "newly active old header still contains {marker}: {old_header}"
        );
    }

    let reopened = Document::open(&saved).expect("reopen rejected docx");
    assert!(reopened.revisions().is_empty());
    assert_eq!(reopened.header_text(), "Old removed");
}

#[test]
fn reject_all_revisions_without_revisions_is_noop() {
    let fixture = content_control_docx();
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(doc.reject_all_revisions().unwrap(), 0);
    assert!(doc.edited_parts().is_empty());

    let after = unzip_parts(&doc.save().expect("save noop"));
    assert_eq!(
        before.get("word/document.xml"),
        after.get("word/document.xml"),
        "no-op revision rejection should not canonicalize document.xml"
    );
}

#[test]
fn set_hyperlink_target_updates_body_relationship_only() {
    let fixture = hyperlink_docx();
    let original_parts = unzip_parts(&fixture);
    let original_body = original_parts["word/document.xml"].clone();
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.set_hyperlink_target(1, "https://new.example/second?x=1&y=2")
        .expect("second hyperlink target updated");

    assert_eq!(doc.edited_parts(), ["word/_rels/document.xml.rels"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(
        parts["word/document.xml"], original_body,
        "body XML must stay byte-preserved"
    );
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(r#"Id="rIdFirst" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/first" TargetMode="External""#),
        "first hyperlink changed: {rels}"
    );
    assert!(
        rels.contains(r#"Id="rIdOrphan" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/orphan" TargetMode="External""#),
        "orphan relationship changed: {rels}"
    );
    assert!(
        rels.contains(r#"Id="rIdSecond" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://new.example/second?x=1&amp;y=2" TargetMode="External""#),
        "second hyperlink target not updated/escaped: {rels}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let html = reopened.to_html();
    assert!(
        html.contains(r#"<a href="https://new.example/second?x=1&amp;y=2">Second</a>"#),
        "reopened HTML should resolve the edited relationship: {html}"
    );
}

#[test]
fn set_hyperlink_target_skips_deleted_body_hyperlinks() {
    let fixture = docx_fixture(&[
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
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDeleted" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/deleted" TargetMode="External"/><Relationship Id="rIdMovedFrom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/moved-from" TargetMode="External"/><Relationship Id="rIdCurrent" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/current" TargetMode="External"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:del w:id="1"><w:hyperlink r:id="rIdDeleted"><w:r><w:delText>Deleted</w:delText></w:r></w:hyperlink></w:del><w:moveFrom w:id="2"><w:hyperlink r:id="rIdMovedFrom"><w:r><w:delText>Moved</w:delText></w:r></w:hyperlink></w:moveFrom><w:hyperlink r:id="rIdCurrent"><w:r><w:t>Current</w:t></w:r></w:hyperlink></w:p></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.set_hyperlink_target(0, "https://new.example/current")
        .expect("visible hyperlink target updated");

    let parts = unzip_parts(&doc.save().expect("save edited docx"));
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(r#"Id="rIdDeleted" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/deleted" TargetMode="External""#),
        "deleted hyperlink relationship changed: {rels}"
    );
    assert!(
        rels.contains(r#"Id="rIdMovedFrom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://old.example/moved-from" TargetMode="External""#),
        "moved-from hyperlink relationship changed: {rels}"
    );
    assert!(
        rels.contains(r#"Id="rIdCurrent" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://new.example/current" TargetMode="External""#),
        "current hyperlink relationship not updated: {rels}"
    );
}

#[test]
fn set_hyperlink_target_rejects_invalid_index_without_mutation() {
    let mut doc = Document::open(&hyperlink_docx()).expect("fixture opens");

    let err = doc
        .set_hyperlink_target(2, "https://new.example/missing")
        .expect_err("missing hyperlink index rejected");

    assert!(
        err.to_string().contains("hyperlink index 2 out of range"),
        "{err}"
    );
    assert!(doc.edited_parts().is_empty());
}

#[test]
fn edited_parts_reports_package_parts_touched_by_preservation_edits() {
    let mut doc = Document::open(&header_footer_docx()).expect("fixture opens");
    assert!(
        doc.edited_parts().is_empty(),
        "freshly opened package should not report dirty parts"
    );

    assert_eq!(doc.replace_body_text("OLD", "BODY").unwrap(), 1);
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);

    assert_eq!(doc.replace_header_footer_text("OLD", "HF").unwrap(), 2);
    assert_eq!(
        doc.edited_parts(),
        ["word/document.xml", "word/footer1.xml", "word/header1.xml"]
    );

    let mut metadata = Document::open(&table_docx()).expect("fixture opens");
    metadata
        .set_core_property(CoreProperty::Creator, "Hyunjo Jung")
        .expect("creator property added");
    assert_eq!(
        metadata.edited_parts(),
        ["[Content_Types].xml", "_rels/.rels", "docProps/core.xml"]
    );
}

#[test]
fn replace_header_footer_text_edits_referenced_parts_only() {
    let mut doc = Document::open(&header_footer_docx()).expect("fixture opens");

    assert_eq!(doc.replace_header_footer_text("OLD", "NEW").unwrap(), 2);

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let orphan = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>OLD</w:t>"), "body changed: {body}");
    assert!(
        header.contains("<w:t>NEW</w:t>"),
        "header not edited: {header}"
    );
    assert!(
        header.contains("<a:t>OLD</a:t>"),
        "DrawingML text should not be edited: {header}"
    );
    assert!(
        footer.contains("<w:t>NEW</w:t>"),
        "footer not edited: {footer}"
    );
    assert!(
        orphan.contains("<w:t>OLD</w:t>"),
        "unreferenced header should not be edited: {orphan}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.header_text(), "NEW");
}

#[test]
fn replace_header_footer_text_skips_old_section_references() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdOld" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdCurrent" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/><Relationship Id="rIdChanged" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header3.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:del w:id="1"><w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rIdOld"/></w:sectPr></w:pPr></w:p></w:del><w:p><w:pPr><w:pPrChange w:id="2"><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rIdChanged"/></w:sectPr></w:pPr></w:pPrChange></w:pPr></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdCurrent"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header3.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:hdr>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(doc.replace_header_footer_text("OLD", "NEW").unwrap(), 1);

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let old_header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let current_header = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    let changed_header = String::from_utf8(parts["word/header3.xml"].clone()).unwrap();
    assert!(
        old_header.contains("<w:t>OLD</w:t>"),
        "old-only header should not be edited: {old_header}"
    );
    assert!(
        changed_header.contains("<w:t>OLD</w:t>"),
        "property-change header should not be edited: {changed_header}"
    );
    assert!(
        current_header.contains("<w:t>NEW</w:t>"),
        "current header not edited: {current_header}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.header_text(), "NEW");
}

#[test]
fn replace_body_text_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&header_footer_docx()).expect("fixture opens");

    assert_eq!(doc.replace_body_text("OLD", "Line 1\nLine\t2").unwrap(), 1);

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        body.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "body replacement did not use WML markers: {body}"
    );
    assert!(
        header.contains("<w:t>OLD</w:t>"),
        "header should not be edited by body replacement: {header}"
    );
    assert!(
        footer.contains("<w:t>OLD</w:t>"),
        "footer should not be edited by body replacement: {footer}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.main_text(), "Line 1\nLine\t2");
}

#[test]
fn docx_symbol_runs_are_exposed_in_main_text() {
    let doc = Document::open(&symbol_body_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Alpha • Beta");
    assert_eq!(doc.text(), "Alpha • Beta");
    let Block::Paragraph(paragraph) = &doc.model().blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(paragraph.text(), "Alpha • Beta");
    assert_eq!(paragraph.runs.len(), 1);
    assert_eq!(paragraph.runs[0].text, "Alpha • Beta");
}

#[test]
fn docx_symbol_runs_use_selected_alternate_content_branch() {
    let doc = Document::open(&symbol_alternate_content_body_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Alpha • Beta");
    assert!(!doc.main_text().contains("fallback"));
}

#[test]
fn replace_header_footer_text_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&header_footer_docx()).expect("fixture opens");

    assert_eq!(
        doc.replace_header_footer_text("OLD", "Line 1\nLine\t2")
            .unwrap(),
        2
    );

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>OLD</w:t>"), "body changed: {body}");
    assert!(
        header.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "header replacement did not use WML markers: {header}"
    );
    assert!(
        footer.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "footer replacement did not use WML markers: {footer}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert!(
        reopened.header_text().contains("Line 1\nLine\t2"),
        "reopened header/footer text should include marker-expanded text"
    );
}

#[test]
fn replace_text_in_part_edits_one_existing_wml_part() {
    let mut doc = Document::open(&header_footer_docx()).expect("fixture opens");

    assert_eq!(
        doc.replace_text_in_part("word/header2.xml", "OLD", "NEW")
            .unwrap(),
        1
    );
    assert_eq!(doc.edited_parts(), ["word/header2.xml"]);

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let referenced_header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let unreferenced_header = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>OLD</w:t>"), "body changed: {body}");
    assert!(
        referenced_header.contains("<w:t>OLD</w:t>"),
        "referenced header changed: {referenced_header}"
    );
    assert!(
        unreferenced_header.contains("<w:t>NEW</w:t>"),
        "target part not edited: {unreferenced_header}"
    );
    assert!(
        footer.contains("<w:t>OLD</w:t>"),
        "footer changed: {footer}"
    );

    let mut reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(
        reopened
            .replace_text_in_part("word/header2.xml", "missing", "x")
            .unwrap(),
        0
    );
    assert!(
        reopened
            .replace_text_in_part("word/missing.xml", "OLD", "NEW")
            .is_err(),
        "missing target part should be an error"
    );
}

#[test]
fn replace_text_in_part_keeps_all_alternate_content_branches_explicitly_editable() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="w14"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></mc:Choice><mc:Fallback><w:p><w:r><w:t>OLD</w:t></w:r></w:p></mc:Fallback></mc:AlternateContent><mc:AlternateContent><w:p><w:r><w:t>OLD</w:t></w:r></w:p></mc:AlternateContent></w:hdr>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(
        doc.replace_text_in_part("word/header2.xml", "OLD", "NEW")
            .expect("all explicit part descendants edited"),
        3
    );

    let header = String::from_utf8(
        unzip_parts(&doc.save().expect("save edited docx"))["word/header2.xml"].clone(),
    )
    .unwrap();
    assert_eq!(header.matches("<w:t>NEW</w:t>").count(), 3, "{header}");
    assert!(!header.contains("<w:t>OLD</w:t>"), "{header}");
}

#[test]
fn replace_text_in_part_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&header_footer_docx()).expect("fixture opens");

    assert_eq!(
        doc.replace_text_in_part("word/header2.xml", "OLD", "Line 1\nLine\t2")
            .unwrap(),
        1
    );

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let referenced_header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let unreferenced_header = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();
    let footer = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>OLD</w:t>"), "body changed: {body}");
    assert!(
        referenced_header.contains("<w:t>OLD</w:t>"),
        "referenced header changed: {referenced_header}"
    );
    assert!(
        unreferenced_header.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "target part replacement did not use WML markers: {unreferenced_header}"
    );
    assert!(
        footer.contains("<w:t>OLD</w:t>"),
        "footer changed: {footer}"
    );
}

#[test]
fn set_table_cell_text_updates_one_body_table_cell() {
    let mut doc = Document::open(&table_docx()).expect("fixture opens");

    doc.set_table_cell_text(0, 0, 1, "B1-updated")
        .expect("table cell text updates");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        body.contains("<w:t>BEFORE</w:t>"),
        "body prefix changed: {body}"
    );
    assert!(body.contains("<w:t>A1</w:t>"), "other cell changed: {body}");
    assert!(
        body.contains("<w:t>B1-updated</w:t>"),
        "target cell not updated: {body}"
    );
    assert!(
        body.contains("<w:t></w:t>"),
        "second run in target cell should be cleared: {body}"
    );
    assert!(body.contains("<w:t>B2</w:t>"), "other row changed: {body}");
    assert!(
        body.contains("<w:t>AFTER</w:t>"),
        "body suffix changed: {body}"
    );

    let mut reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(
        reopened.main_text(),
        "BEFORE\nA1\tB1-updated\nA2\tB2\nAFTER"
    );
    assert!(
        reopened.set_table_cell_text(0, 10, 0, "missing").is_err(),
        "out-of-range row should be an error"
    );
}

#[test]
fn set_table_cell_text_edits_accepted_revision_wrapped_tables() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:ins w:id="1"><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Inserted table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:ins><w:moveTo w:id="2"><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Moved table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:moveTo></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.set_table_cell_text(0, 0, 0, "Inserted updated")
        .expect("inserted table cell updates");
    doc.set_table_cell_text(1, 0, 0, "Moved updated")
        .expect("moved-to table cell updates");

    let saved = doc.save().expect("save edited docx");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains(r#"<w:ins w:id="1"><w:tbl>"#) && body.contains("<w:t>Inserted updated</w:t>"),
        "inserted table was not updated in place: {body}"
    );
    assert!(
        body.contains(r#"<w:moveTo w:id="2"><w:tbl>"#) && body.contains("<w:t>Moved updated</w:t>"),
        "moved-to table was not updated in place: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.main_text(), "Inserted updated\nMoved updated");
}

#[test]
fn set_table_cell_text_ignores_nested_tables_in_deleted_content() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Visible</w:t></w:r></w:p><w:del w:id="1"><w:tbl><w:tr><w:tc><w:p><w:r><w:delText>Deleted nested</w:delText></w:r></w:p></w:tc></w:tr></w:tbl></w:del></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.set_table_cell_text(0, 0, 0, "Updated")
        .expect("deleted nested table does not block the visible cell edit");
    assert_eq!(doc.edited_parts(), ["word/document.xml"]);

    let saved = doc.save().expect("save edited docx");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:t>Updated</w:t>"),
        "cell not updated: {body}"
    );
    assert!(
        body.contains(r#"<w:del w:id="1"><w:tbl>"#)
            && body.contains("<w:delText>Deleted nested</w:delText>"),
        "deleted nested table markup changed: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.main_text(), "Updated");
}

#[test]
fn set_table_cell_text_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&table_docx()).expect("fixture opens");

    doc.set_table_cell_text(0, 0, 1, "Line 1\nLine\t2")
        .expect("table cell text updates");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>A1</w:t>"), "other cell changed: {body}");
    assert!(
        body.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "table cell replacement did not use WML markers: {body}"
    );
    assert!(
        body.contains("<w:t></w:t>"),
        "second run in target cell should still be cleared: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(
        reopened.main_text(),
        "BEFORE\nA1\tLine 1\nLine\t2\nA2\tB2\nAFTER"
    );
}

#[test]
fn set_table_cell_text_uses_grid_span_logical_columns() {
    let mut doc = Document::open(&grid_span_table_docx()).expect("fixture opens");

    doc.set_table_cell_text(0, 0, 1, "Merged updated")
        .expect("logical column inside gridSpan edits merged cell");
    doc.set_table_cell_text(0, 0, 2, "C1-updated")
        .expect("logical column after gridSpan edits following cell");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        body.contains("<w:gridSpan w:val=\" 2 \"/>"),
        "gridSpan markup should be preserved: {body}"
    );
    assert!(
        body.contains("<w:t>Merged updated</w:t>"),
        "spanned cell was not updated: {body}"
    );
    assert!(
        !body.contains("<w:t>Merged AB</w:t>"),
        "old spanned cell text remains: {body}"
    );
    assert!(
        body.contains("<w:t>C1-updated</w:t>"),
        "following logical cell was not updated: {body}"
    );
    assert!(body.contains("<w:t>A2</w:t>"), "other row changed: {body}");
    assert!(body.contains("<w:t>B2</w:t>"), "other row changed: {body}");
    assert!(body.contains("<w:t>C2</w:t>"), "other row changed: {body}");
}

#[test]
fn set_table_cell_text_uses_vmerge_logical_rows() {
    let mut doc = Document::open(&vmerge_table_docx()).expect("fixture opens");

    doc.set_table_cell_text(0, 1, 0, "A merged updated")
        .expect("logical row inside vMerge edits restart cell");
    doc.set_table_cell_text(0, 1, 1, "B2-updated")
        .expect("normal cell beside vMerge continuation edits in-place");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        body.contains(r#"<w:vMerge w:val=" restart "/>"#),
        "vMerge restart markup should be preserved: {body}"
    );
    assert!(
        body.contains("<w:vMerge/>"),
        "vMerge continuation markup should be preserved: {body}"
    );
    assert!(
        body.contains("<w:t>A merged updated</w:t>"),
        "restart cell was not updated through continuation coordinate: {body}"
    );
    assert!(
        !body.contains("<w:t>A merged</w:t>"),
        "old restart cell text remains: {body}"
    );
    assert!(body.contains("<w:t>B1</w:t>"), "other row changed: {body}");
    assert!(
        body.contains("<w:t>B2-updated</w:t>"),
        "normal cell beside continuation was not updated: {body}"
    );
}

#[test]
fn set_table_cell_text_edits_parent_text_without_touching_nested_table() {
    let mut doc = Document::open(&nested_table_docx()).expect("fixture opens");

    doc.set_table_cell_text(0, 0, 0, "Updated")
        .expect("direct parent text should remain editable");

    let saved = doc.save().expect("save edited nested-table document");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:t>Updated</w:t>"),
        "parent cell text was not updated: {body}"
    );
    assert!(
        !body.contains("<w:t>Outer</w:t>"),
        "old parent cell text remains: {body}"
    );
    assert!(
        body.contains("<w:t>Inner</w:t>"),
        "nested table text was changed or dropped: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited nested-table document");
    assert_eq!(reopened.main_text(), "Updated Inner");
}

#[test]
fn docx_text_boxes_are_exposed_as_side_table_once() {
    let doc = Document::open(&text_box_docx()).expect("fixture opens");

    assert_eq!(doc.text().matches("BOX TEXT").count(), 1);
    assert_eq!(doc.text_box_text(), "BOX TEXT");
    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].id, "docx-text-box-0");
    assert_eq!(text_boxes[0].text, "BOX TEXT");
    assert_eq!(text_boxes[0].anchor, None);
}

#[test]
fn docx_header_footer_text_boxes_are_exposed_as_side_table() {
    let doc = Document::open(&header_footer_text_box_docx()).expect("fixture opens");

    assert_eq!(doc.header_text(), "HEADER BOX");
    assert_eq!(doc.text_box_text(), "HEADER BOX");
    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].id, "word/header1.xml#default-text-box-0");
    assert_eq!(text_boxes[0].text, "HEADER BOX");
    assert_eq!(text_boxes[0].anchor, None);
}

#[test]
fn docx_note_text_boxes_are_exposed_as_side_table() {
    let doc = Document::open(&note_text_box_docx()).expect("fixture opens");

    assert_eq!(doc.text_box_text(), "Foot box\nEnd box");
    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 2);
    assert_eq!(text_boxes[0].id, "word/footnotes.xml-text-box-0");
    assert_eq!(text_boxes[0].text, "Foot box");
    assert_eq!(text_boxes[0].anchor, None);
    assert_eq!(text_boxes[1].id, "word/endnotes.xml-text-box-0");
    assert_eq!(text_boxes[1].text, "End box");
    assert_eq!(text_boxes[1].anchor, None);
}

#[test]
fn docx_anchored_text_box_exposes_containing_body_anchor() {
    let doc = Document::open(&anchored_text_box_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "BOX TEXT");
    let anchor = text_boxes[0].anchor.as_ref().expect("text box anchor");
    assert_eq!(anchor.id, "7");
    assert_eq!(anchor.text, "Before After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_computed_simple_field_text() {
    let doc = Document::open(&anchored_text_box_simple_field_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "BOX TEXT");
    let anchor = text_boxes[0].anchor.as_ref().expect("text box anchor");
    assert_eq!(anchor.id, "9");
    assert_eq!(anchor.text, "Fresh anchor After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_computed_complex_field_text() {
    let doc =
        Document::open(&anchored_text_box_complex_field_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "BOX TEXT");
    let anchor = text_boxes[0].anchor.as_ref().expect("text box anchor");
    assert_eq!(anchor.id, "10");
    assert_eq!(anchor.text, "Fresh anchor After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_computed_dynamic_field_text() {
    let doc =
        Document::open(&anchored_text_box_dynamic_field_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 2);
    assert_eq!(text_boxes[0].text, "SIMPLE BOX");
    let simple = text_boxes[0].anchor.as_ref().expect("simple anchor");
    assert_eq!(simple.id, "11");
    assert_eq!(simple.text, "Fresh simple After");
    assert_eq!(text_boxes[1].text, "COMPLEX BOX");
    let complex = text_boxes[1].anchor.as_ref().expect("complex anchor");
    assert_eq!(complex.id, "12");
    assert_eq!(complex.text, "Fresh complex After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_local_field_bookmarks_in_dynamic_field_text() {
    let doc = Document::open(&anchored_text_box_local_field_bookmark_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "LOCAL BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("local state anchor");
    assert_eq!(anchor.id, "13");
    assert_eq!(anchor.text, "Fresh anchor After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_supported_display_and_action_field_text() {
    let doc = Document::open(&anchored_text_box_display_action_field_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "ACTION DISPLAY BOX");
    let anchor = text_boxes[0]
        .anchor
        .as_ref()
        .expect("display action anchor");
    assert_eq!(anchor.id, "14");
    assert_eq!(anchor.text, "• Fresh anchor After");
}

#[test]
fn docx_anchored_text_box_anchor_hides_supported_toc_and_index_marker_field_text() {
    let doc = Document::open(&anchored_text_box_marker_field_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "MARKER BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("marker anchor");
    assert_eq!(anchor.id, "15");
    assert_eq!(anchor.text, "Before After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_supported_source_order_numbering_field_text() {
    let doc =
        Document::open(&anchored_text_box_numbering_field_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "NUMBERING BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("numbering anchor");
    assert_eq!(anchor.id, "16");
    assert_eq!(anchor.text, "1 2 After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_supported_document_info_field_text() {
    let doc = Document::open(&anchored_text_box_document_info_field_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "DOCUMENT INFO BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("document info anchor");
    assert_eq!(anchor.id, "17");
    assert_eq!(anchor.text, "Side Table Plan After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_supported_revision_number_field_text() {
    let doc = Document::open(&anchored_text_box_revision_number_field_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "REVISION BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("revision anchor");
    assert_eq!(anchor.id, "18");
    assert_eq!(anchor.text, "17 After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_document_bookmark_formula_field_text() {
    let doc = Document::open(&anchored_text_box_document_bookmark_formula_field_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "FORMULA BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("formula anchor");
    assert_eq!(anchor.id, "19");
    assert_eq!(anchor.text, "50 After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_document_bookmark_merge_control_field_text() {
    let doc = Document::open(&anchored_text_box_document_bookmark_merge_control_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "MERGE CONTROL BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("merge control anchor");
    assert_eq!(anchor.id, "20");
    assert_eq!(anchor.text, "Before After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_document_bookmark_ref_field_text() {
    let doc = Document::open(&anchored_text_box_document_bookmark_ref_anchor_docx())
        .expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "REF BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("ref anchor");
    assert_eq!(anchor.id, "21");
    assert_eq!(anchor.text, "21st After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_document_note_ref_field_text() {
    let doc = Document::open(&anchored_text_box_note_ref_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "NOTE BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("note ref anchor");
    assert_eq!(anchor.id, "22");
    assert_eq!(anchor.text, "1 After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_document_toc_field_text() {
    let doc = Document::open(&anchored_text_box_toc_anchor_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "TOC BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("toc anchor");
    assert_eq!(anchor.id, "23");
    assert_eq!(anchor.text, "Executive Summary After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_legacy_form_dropdown_field_text() {
    let doc = Document::open(&anchored_text_box_legacy_form_anchor_docx()).expect("fixture opens");

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Direct B")
            && main_text.contains("Anchor B")
            && !main_text.contains("stale anchor option"),
        "main text should use computed legacy-form text: {main_text:?}"
    );
    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "LEGACY FORM BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("legacy form anchor");
    assert_eq!(anchor.id, "24");
    assert_eq!(anchor.text, "Anchor B After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_text_form_current_field_text() {
    let doc = Document::open(&anchored_text_box_text_form_anchor_docx()).expect("fixture opens");

    let main_text = doc.main_text();
    assert!(
        main_text.contains("ANCHOR TYPED")
            && !main_text.contains("Anchor default")
            && !main_text.contains("Anchor typed"),
        "main text should use formatted text-form current text: {main_text:?}"
    );
    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "TEXT FORM BOX");
    let anchor = text_boxes[0].anchor.as_ref().expect("text form anchor");
    assert_eq!(anchor.id, "25");
    assert_eq!(anchor.text, "ANCHOR TYPED After");
}

#[test]
fn docx_anchored_text_box_anchor_uses_complex_text_form_current_field_text() {
    let doc =
        Document::open(&anchored_text_box_complex_text_form_anchor_docx()).expect("fixture opens");

    let main_text = doc.main_text();
    assert!(
        main_text.contains("ANCHOR TYPED")
            && !main_text.contains("Anchor default")
            && !main_text.contains("Anchor typed"),
        "main text should use formatted complex text-form current text: {main_text:?}"
    );
    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 1);
    assert_eq!(text_boxes[0].text, "COMPLEX TEXT FORM BOX");
    let anchor = text_boxes[0]
        .anchor
        .as_ref()
        .expect("complex text form anchor");
    assert_eq!(anchor.id, "26");
    assert_eq!(anchor.text, "ANCHOR TYPED After");
}

#[test]
fn docx_duplicate_anchored_text_boxes_keep_distinct_body_anchors() {
    let doc = Document::open(&duplicate_anchored_text_box_docx()).expect("fixture opens");

    let text_boxes = doc.text_boxes();
    assert_eq!(text_boxes.len(), 2);
    assert_eq!(text_boxes[0].text, "BOX TEXT");
    assert_eq!(text_boxes[1].text, "BOX TEXT");
    let first = text_boxes[0].anchor.as_ref().expect("first anchor");
    assert_eq!(first.id, "7");
    assert_eq!(first.text, "First done");
    let second = text_boxes[1].anchor.as_ref().expect("second anchor");
    assert_eq!(second.id, "8");
    assert_eq!(second.text, "Second done");
}

#[test]
fn docx_text_boxes_follow_accepted_revision_view() {
    let doc = Document::open(&revision_wrapped_text_box_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Direct box\nInserted box\nMoved-to box");
    let text_boxes = doc.text_boxes();
    let texts: Vec<_> = text_boxes
        .iter()
        .map(|text_box| text_box.text.as_str())
        .collect();
    assert_eq!(texts, vec!["Direct box", "Inserted box", "Moved-to box"]);
}

#[test]
fn docx_notes_are_exposed_as_note_side_table() {
    let doc = Document::open(&notes_docx()).expect("fixture opens");

    assert_eq!(doc.footnote_text(), "OLD foot");
    assert_eq!(doc.endnote_text(), "OLD");
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "1");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(notes[0].text, "OLD foot");
    assert_eq!(notes[0].anchor.as_ref().map(|a| a.id.as_str()), Some("1"));
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("BODY")
    );
    assert_eq!(notes[1].id, "2");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(notes[1].text, "OLD");
    assert_eq!(notes[1].anchor.as_ref().map(|a| a.id.as_str()), Some("2"));
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("BODY")
    );
}

#[test]
fn docx_note_entries_use_single_alternate_content_branch() {
    let doc = Document::open(&alternate_content_note_entries_docx()).expect("fixture opens");

    assert_eq!(doc.footnote_text(), "Choice footnote");
    assert_eq!(doc.endnote_text(), "Choice endnote");
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "1");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(notes[0].text, "Choice footnote");
    assert_eq!(
        notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
        Some("Body")
    );
    assert_eq!(notes[1].id, "2");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(notes[1].text, "Choice endnote");
    assert_eq!(
        notes[1].anchor.as_ref().map(|anchor| anchor.text.as_str()),
        Some("Body")
    );
}

#[test]
fn docx_notes_ignore_blank_ids() {
    let doc = Document::open(&notes_with_blank_ids_docx()).expect("fixture opens");

    assert_eq!(doc.footnote_text(), "Valid note");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "1");
    assert_eq!(notes[0].text, "Valid note");
    assert_eq!(
        notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
        Some("Valid")
    );
}

#[test]
fn docx_note_reference_anchors_include_containing_body_text() {
    let doc = Document::open(&notes_with_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Foot before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("End before end after")
    );
}

#[test]
fn docx_note_reference_anchors_use_computed_simple_field_text() {
    let doc = Document::open(&notes_with_simple_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Fresh foot before foot after\nFresh end before end after"
    );
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh foot before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh end before end after")
    );
}

#[test]
fn docx_note_reference_anchors_use_computed_complex_field_text() {
    let doc = Document::open(&notes_with_complex_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Fresh foot before foot after\nFresh end before end after"
    );
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh foot before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh end before end after")
    );
}

#[test]
fn docx_note_reference_anchors_use_computed_style_ref_field_text() {
    let doc = Document::open(&notes_with_style_ref_anchor_text_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Intro Heading\nAnchor Heading\nAnchor Heading before foot after\nAnchor Heading before end after"
    );
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Anchor Heading before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Anchor Heading before end after")
    );
}

#[test]
fn docx_note_reference_anchor_ignores_deleted_style_ref_cursor() {
    let doc =
        Document::open(&notes_with_deleted_style_ref_before_anchor_docx()).expect("fixture opens");

    assert_eq!(
        doc.notes()[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Live Heading")
    );
}

#[test]
fn docx_note_reference_anchors_use_computed_dynamic_field_text() {
    let doc = Document::open(&notes_with_dynamic_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Fresh foot before foot after\nFresh end before end after"
    );
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh foot before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh end before end after")
    );
}

#[test]
fn docx_note_reference_anchors_use_local_field_bookmarks_in_dynamic_field_text() {
    let doc =
        Document::open(&notes_with_local_field_bookmark_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Fresh foot before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh foot before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_supported_display_and_action_field_text() {
    let doc =
        Document::open(&notes_with_display_action_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "• Fresh foot before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("• Fresh foot before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_hide_supported_toc_and_index_marker_field_text() {
    let doc = Document::open(&notes_with_marker_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Foot before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Foot before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_supported_source_order_numbering_field_text() {
    let doc =
        Document::open(&notes_with_numbering_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "1 2 before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("1 2 before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_supported_document_info_field_text() {
    let doc =
        Document::open(&notes_with_document_info_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Side Table Plan before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Side Table Plan before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_supported_revision_number_field_text() {
    let doc = Document::open(&notes_with_revision_number_field_anchor_text_docx())
        .expect("fixture opens");

    assert_eq!(doc.main_text(), "17 before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("17 before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_computed_section_field_text() {
    let doc = Document::open(&notes_with_section_field_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Second section 2 before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_legacy_form_dropdown_field_text() {
    let doc =
        Document::open(&notes_with_legacy_form_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Direct B\nAnchor B before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Anchor B before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_text_form_current_field_text() {
    let doc =
        Document::open(&notes_with_text_form_field_anchor_text_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "ANCHOR TYPED before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("ANCHOR TYPED before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_complex_text_form_current_field_text() {
    let doc = Document::open(&notes_with_complex_text_form_field_anchor_text_docx())
        .expect("fixture opens");

    assert_eq!(doc.main_text(), "ANCHOR TYPED before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("ANCHOR TYPED before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_alternate_content_text_form_current_field_text() {
    let doc = Document::open(&notes_with_alternate_content_text_form_field_anchor_text_docx())
        .expect("fixture opens");

    assert_eq!(doc.main_text(), "ANCHOR TYPED before foot after");
    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("ANCHOR TYPED before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_document_bookmark_formula_field_text() {
    let doc = Document::open(&notes_with_document_bookmark_formula_field_anchor_text_docx())
        .expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("50 before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_document_bookmark_merge_control_field_text() {
    let doc = Document::open(&notes_with_document_bookmark_merge_control_anchor_text_docx())
        .expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_document_bookmark_ref_field_text() {
    let doc = Document::open(&notes_with_document_bookmark_ref_anchor_text_docx())
        .expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("CLAUSE ONE before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_document_note_ref_field_text() {
    let doc = Document::open(&notes_with_note_ref_field_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    let note = notes.iter().find(|note| note.id == "7").expect("note 7");
    assert_eq!(note.kind, NoteKind::Footnote);
    assert_eq!(
        note.anchor.as_ref().map(|a| a.text.as_str()),
        Some("1 before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_ref_note_mark_field_text() {
    let doc = Document::open(&notes_with_ref_note_mark_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    let note = notes.iter().find(|note| note.id == "7").expect("note 7");
    assert_eq!(note.kind, NoteKind::Footnote);
    assert_eq!(
        note.anchor.as_ref().map(|a| a.text.as_str()),
        Some("1 before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_use_document_toc_field_text() {
    let doc = Document::open(&notes_with_toc_field_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Executive Summary before foot after")
    );
}

#[test]
fn docx_note_reference_anchors_preserve_supported_symbols() {
    let doc = Document::open(&notes_with_symbol_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Foot • before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("End • before end after")
    );
}

#[test]
fn docx_note_reference_anchors_preserve_expanded_supported_symbols() {
    let doc =
        Document::open(&notes_with_expanded_symbol_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Foot • before foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("End • before end after")
    );
}

#[test]
fn docx_note_reference_anchors_preserve_expanded_inline_markers() {
    let doc =
        Document::open(&notes_with_expanded_marker_anchor_text_docx()).expect("fixture opens");

    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Foot\tTab\nLine-Hard\u{00ad}Soft foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("End\tTab\nPage-Hard\u{00ad}Soft end after")
    );
}

#[test]
fn docx_note_reference_anchors_survive_accepted_revision_wrappers() {
    let doc =
        Document::open(&notes_with_revision_wrapped_anchor_text_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Inserted foot before inserted foot after\nMoved end before moved end after"
    );
    let notes = doc.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].id, "7");
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(
        notes[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Inserted foot before inserted foot after")
    );
    assert_eq!(notes[1].id, "8");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        notes[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Moved end before moved end after")
    );
}

#[test]
fn replace_note_text_edits_footnotes_and_endnotes_only() {
    let mut doc = Document::open(&notes_docx()).expect("fixture opens");

    assert_eq!(doc.replace_note_text("OLD", "NEW").unwrap(), 2);
    assert_eq!(
        doc.edited_parts(),
        ["word/endnotes.xml", "word/footnotes.xml"]
    );

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();

    assert!(body.contains("<w:t>BODY</w:t>"), "body changed: {body}");
    assert!(
        footnotes
            .contains(r#"<w:footnote w:type=" separator " w:id="-1"><w:p><w:r><w:t>OLD</w:t>"#),
        "separator footnote should not be edited: {footnotes}"
    );
    assert!(
        footnotes.contains(r#"<w:footnote w:id=" 1 "><w:p><w:r><w:t>NEW</w:t>"#),
        "real footnote not edited: {footnotes}"
    );
    assert!(
        footnotes.contains("<w:t> foot</w:t>"),
        "other footnote run should remain: {footnotes}"
    );
    assert!(
        endnotes.contains(r#"<w:endnote w:id=" 2 "><w:p><w:r><w:t>NEW</w:t>"#),
        "endnote not edited: {endnotes}"
    );

    let mut reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.text(), "BODY\nNEW foot\nNEW");
    assert_eq!(reopened.replace_note_text("missing", "x").unwrap(), 0);
}

#[test]
fn replace_note_text_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&notes_docx()).expect("fixture opens");

    assert_eq!(doc.replace_note_text("OLD", "Line 1\nLine\t2").unwrap(), 2);

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();

    assert!(
        footnotes.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "footnote replacement did not use WML markers: {footnotes}"
    );
    assert!(
        endnotes.contains("<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"),
        "endnote replacement did not use WML markers: {endnotes}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.footnote_text(), "Line 1\nLine\t2 foot");
    assert_eq!(reopened.endnote_text(), "Line 1\nLine\t2");
}

#[test]
fn add_footnote_on_text_creates_part_relationship_anchor_and_note() {
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    let id = doc
        .add_footnote_on_text("BODY", "Foot <one> & two")
        .expect("footnote added");

    assert_eq!(id, "1");
    assert_eq!(
        doc.edited_parts(),
        [
            "[Content_Types].xml",
            "word/_rels/document.xml.rels",
            "word/document.xml",
            "word/footnotes.xml"
        ]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    let anchor = body.find(r#"<w:t>BODY</w:t>"#).unwrap_or(usize::MAX);
    let reference = body.find(r#"<w:footnoteReference"#).unwrap_or(usize::MAX);
    assert!(
        anchor < reference && body.contains(r#"<w:footnoteReference w:id="1"/>"#),
        "footnote reference missing or misplaced: {body}"
    );
    assert!(
        footnotes.contains(r#"<w:footnote w:type="separator" w:id="-1">"#)
            && footnotes.contains(r#"<w:footnote w:type="continuationSeparator" w:id="0">"#)
            && footnotes.contains(r#"w:id="1""#),
        "footnotes skeleton or note missing: {footnotes}"
    );
    assert!(
        footnotes.contains(r#"<w:t>Foot &lt;one&gt; &amp; two</w:t>"#),
        "footnote text not escaped: {footnotes}"
    );
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml""#
        ),
        "footnotes relationship missing: {rels}"
    );
    assert!(
        ct.contains(r#"PartName="/word/footnotes.xml""#)
            && ct.contains("wordprocessingml.footnotes+xml"),
        "footnotes content type missing: {ct}"
    );

    let mut reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(
        reopened
            .replace_note_text("Foot <one> & two", "Updated")
            .unwrap(),
        1
    );
}

#[test]
fn add_footnote_on_text_skips_deleted_anchor_text() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:del w:id="1"><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:del><w:moveFrom w:id="2"><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:moveFrom><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.add_footnote_on_text("BODY", "Source note")
        .expect("footnote anchored to visible body text");

    let body = String::from_utf8(
        unzip_parts(&doc.save().expect("save edited docx"))["word/document.xml"].clone(),
    )
    .unwrap();
    assert!(
        body.contains(r#"<w:del w:id="1"><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:del>"#),
        "deleted anchor text changed: {body}"
    );
    assert!(
        body.contains(r#"<w:moveFrom w:id="2"><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:moveFrom>"#),
        "moved-from anchor text changed: {body}"
    );
    let anchor = body.rfind(r#"<w:t>BODY</w:t>"#).unwrap_or(usize::MAX);
    let reference = body.find(r#"<w:footnoteReference"#).unwrap_or(usize::MAX);
    assert!(
        anchor < reference && body.contains(r#"<w:footnoteReference w:id="1"/>"#),
        "footnote reference missing from current body text: {body}"
    );
}

#[test]
fn add_notes_on_text_preserve_edge_whitespace() {
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_footnote_on_text("BODY", " Foot note ")
        .expect("footnote added");
    doc.add_endnote_on_text("BODY", " End note ")
        .expect("endnote added");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();

    assert!(
        footnotes.contains(r#"<w:t xml:space="preserve"> Foot note </w:t>"#),
        "footnote text should preserve edge whitespace: {footnotes}"
    );
    assert!(
        endnotes.contains(r#"<w:t xml:space="preserve"> End note </w:t>"#),
        "endnote text should preserve edge whitespace: {endnotes}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let notes = reopened.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(notes[0].text, "Foot note");
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(notes[1].text, "End note");
}

#[test]
fn add_notes_on_text_write_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_footnote_on_text("BODY", "Foot\nLine\t2")
        .expect("footnote added");
    doc.add_endnote_on_text("BODY", "End\nLine\t3")
        .expect("endnote added");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();

    assert!(
        footnotes.contains(r#"<w:t>Foot</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"#),
        "footnote text should encode tabs and breaks as WML markers: {footnotes}"
    );
    assert!(
        endnotes.contains(r#"<w:t>End</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>3</w:t>"#),
        "endnote text should encode tabs and breaks as WML markers: {endnotes}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let notes = reopened.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].text, "Foot\nLine\t2");
    assert_eq!(notes[1].text, "End\nLine\t3");
}

#[test]
fn add_footnote_on_text_uses_next_id_and_rejects_missing_anchor_without_mutation() {
    let mut doc = Document::open(&notes_docx()).expect("fixture opens");

    let id = doc
        .add_footnote_on_text("BODY", "Second footnote")
        .expect("footnote added");

    assert_eq!(id, "2");
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    assert!(
        body.contains(r#"<w:footnoteReference w:id=" 1 "/>"#)
            && body.contains(r#"<w:footnoteReference w:id="2"/>"#),
        "existing and new footnote references should be present: {body}"
    );
    assert!(
        footnotes.contains(r#"w:id=" 1 ""#)
            && footnotes.contains(r#"w:id="2""#)
            && footnotes.contains(r#"<w:t>Second footnote</w:t>"#),
        "existing and new footnotes should be present: {footnotes}"
    );

    let before_missing = saved;
    let before_parts = unzip_parts(&before_missing);
    let mut missing = Document::open(&before_missing).expect("reopen edited docx");
    assert!(
        missing
            .add_footnote_on_text("Missing anchor", "Nope")
            .is_err(),
        "missing anchor text should be an error"
    );
    let after_missing = missing.save().expect("save after rejected edit");
    let after_parts = unzip_parts(&after_missing);
    assert_eq!(
        after_parts["word/document.xml"], before_parts["word/document.xml"],
        "rejected footnote edit mutated document.xml"
    );
    assert_eq!(
        after_parts["word/footnotes.xml"], before_parts["word/footnotes.xml"],
        "rejected footnote edit mutated footnotes.xml"
    );
}

#[test]
fn add_notes_on_text_can_anchor_across_adjacent_runs() {
    let mut doc = Document::open(&split_run_no_notes_docx()).expect("fixture opens");

    let footnote_id = doc
        .add_footnote_on_text("BODY", "Split footnote")
        .expect("footnote added across split runs");
    let endnote_id = doc
        .add_endnote_on_text("BODY", "Split endnote")
        .expect("endnote added across split runs");

    assert_eq!(footnote_id, "1");
    assert_eq!(endnote_id, "1");
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();

    let first = body.find(r#"<w:t>BO</w:t>"#).unwrap_or(usize::MAX);
    let second = body.find(r#"<w:t>DY</w:t>"#).unwrap_or(usize::MAX);
    let footnote_ref = body
        .find(r#"<w:footnoteReference w:id="1"/>"#)
        .unwrap_or(usize::MAX);
    let endnote_ref = body
        .find(r#"<w:endnoteReference w:id="1"/>"#)
        .unwrap_or(usize::MAX);
    let tail = body.find(r#"<w:t> tail</w:t>"#).unwrap_or(usize::MAX);
    assert!(
        first < second
            && second < footnote_ref
            && second < endnote_ref
            && footnote_ref < tail
            && endnote_ref < tail,
        "split-run note references missing or misplaced: {body}"
    );
    assert!(
        footnotes.contains(r#"<w:t>Split footnote</w:t>"#)
            && endnotes.contains(r#"<w:t>Split endnote</w:t>"#),
        "split-run notes missing: footnotes={footnotes} endnotes={endnotes}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert!(reopened.text().contains("Split footnote"));
    assert!(reopened.text().contains("Split endnote"));
}

#[test]
fn add_notes_on_text_anchor_across_accepted_revision_wrappers() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hel</w:t></w:r><w:ins w:id="1"><w:r><w:t>l</w:t></w:r></w:ins><w:moveTo w:id="2"><w:r><w:t>o</w:t></w:r></w:moveTo><w:r><w:t>!</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.add_footnote_on_text("Hello", "Revision footnote")
        .expect("footnote added across accepted revision wrappers");
    doc.add_endnote_on_text("Hello", "Revision endnote")
        .expect("endnote added across accepted revision wrappers");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let third = body.find("<w:t>o</w:t>").unwrap_or(usize::MAX);
    let footnote_ref = body.find("<w:footnoteReference").unwrap_or(usize::MAX);
    let endnote_ref = body.find("<w:endnoteReference").unwrap_or(usize::MAX);
    let tail = body.find("<w:t>!</w:t>").unwrap_or(usize::MAX);
    assert!(
        third < footnote_ref && third < endnote_ref && footnote_ref < tail && endnote_ref < tail,
        "revision-spanning note references missing or misplaced: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.main_text(), "Hello!");
    assert!(reopened.text().contains("Revision footnote"));
    assert!(reopened.text().contains("Revision endnote"));
}

#[test]
fn add_note_on_text_does_not_bridge_deleted_revision_wrappers() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hel</w:t></w:r><w:del w:id="1"><w:r><w:delText>deleted</w:delText></w:r></w:del><w:r><w:t>lo</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ]);
    let before = unzip_parts(&fixture);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    assert!(
        doc.add_footnote_on_text("Hello", "Must not be added")
            .is_err(),
        "deleted content must remain a hard anchor boundary"
    );
    assert!(doc.edited_parts().is_empty());
    assert_eq!(
        unzip_parts(&doc.save().expect("save unchanged docx")),
        before
    );
}

#[test]
fn add_endnote_on_text_creates_part_relationship_anchor_and_note() {
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    let id = doc
        .add_endnote_on_text("BODY", "End <one> & two")
        .expect("endnote added");

    assert_eq!(id, "1");
    assert_eq!(
        doc.edited_parts(),
        [
            "[Content_Types].xml",
            "word/_rels/document.xml.rels",
            "word/document.xml",
            "word/endnotes.xml"
        ]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    let anchor = body.find(r#"<w:t>BODY</w:t>"#).unwrap_or(usize::MAX);
    let reference = body.find(r#"<w:endnoteReference"#).unwrap_or(usize::MAX);
    assert!(
        anchor < reference && body.contains(r#"<w:endnoteReference w:id="1"/>"#),
        "endnote reference missing or misplaced: {body}"
    );
    assert!(
        endnotes.contains(r#"<w:endnote w:type="separator" w:id="-1">"#)
            && endnotes.contains(r#"<w:endnote w:type="continuationSeparator" w:id="0">"#)
            && endnotes.contains(r#"w:id="1""#),
        "endnotes skeleton or note missing: {endnotes}"
    );
    assert!(
        endnotes.contains(r#"<w:t>End &lt;one&gt; &amp; two</w:t>"#),
        "endnote text not escaped: {endnotes}"
    );
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml""#
        ),
        "endnotes relationship missing: {rels}"
    );
    assert!(
        ct.contains(r#"PartName="/word/endnotes.xml""#)
            && ct.contains("wordprocessingml.endnotes+xml"),
        "endnotes content type missing: {ct}"
    );

    let mut reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(
        reopened
            .replace_note_text("End <one> & two", "Updated")
            .unwrap(),
        1
    );
}

#[test]
fn add_endnote_on_text_uses_next_id_and_rejects_missing_anchor_without_mutation() {
    let mut doc = Document::open(&notes_docx()).expect("fixture opens");

    let id = doc
        .add_endnote_on_text("BODY", "Second endnote")
        .expect("endnote added");

    assert_eq!(id, "3");
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let endnotes = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();
    assert!(
        body.contains(r#"<w:endnoteReference w:id=" 2 "/>"#)
            && body.contains(r#"<w:endnoteReference w:id="3"/>"#),
        "existing and new endnote references should be present: {body}"
    );
    assert!(
        endnotes.contains(r#"w:id=" 2 ""#)
            && endnotes.contains(r#"w:id="3""#)
            && endnotes.contains(r#"<w:t>Second endnote</w:t>"#),
        "existing and new endnotes should be present: {endnotes}"
    );

    let before_missing = saved;
    let before_parts = unzip_parts(&before_missing);
    let mut missing = Document::open(&before_missing).expect("reopen edited docx");
    assert!(
        missing
            .add_endnote_on_text("Missing anchor", "Nope")
            .is_err(),
        "missing anchor text should be an error"
    );
    let after_missing = missing.save().expect("save after rejected edit");
    let after_parts = unzip_parts(&after_missing);
    assert_eq!(
        after_parts["word/document.xml"], before_parts["word/document.xml"],
        "rejected endnote edit mutated document.xml"
    );
    assert_eq!(
        after_parts["word/endnotes.xml"], before_parts["word/endnotes.xml"],
        "rejected endnote edit mutated endnotes.xml"
    );
}

#[test]
fn replace_image_png_updates_existing_media_part_only() {
    let original_png = tiny_png();
    let replacement_png = tiny_png_with_marker(b"rwml replacement");
    let mut doc = Document::open(&image_docx(&original_png)).expect("fixture opens");

    assert_eq!(
        doc.images()[0].bytes.as_deref(),
        Some(original_png.as_slice())
    );

    doc.replace_image_png(&replacement_png, "image1.png")
        .expect("replace existing image");

    assert_eq!(doc.edited_parts(), ["word/media/image1.png"]);
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/image1.png"], replacement_png);
    assert_eq!(
        parts["word/document.xml"],
        unzip_parts(&image_docx(&original_png))["word/document.xml"]
    );
    assert_eq!(
        parts["word/_rels/document.xml.rels"],
        unzip_parts(&image_docx(&original_png))["word/_rels/document.xml.rels"]
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.text(), "BEFORE");
    assert_eq!(
        reopened.images()[0].bytes.as_deref(),
        Some(replacement_png.as_slice())
    );
}

#[test]
fn replace_image_png_rejects_missing_or_invalid_inputs_without_mutation() {
    let original_png = tiny_png();
    let before = image_docx(&original_png);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert!(doc.replace_image_png(b"not png", "image1.png").is_err());
    assert!(doc.replace_image_png(&tiny_png(), "missing.png").is_err());
    assert!(doc.replace_image_png(&tiny_png(), "../image1.png").is_err());

    let after = doc.save().expect("save after failed edits");
    let before_parts = unzip_parts(&before);
    let after_parts = unzip_parts(&after);
    assert_eq!(after_parts["word/media/image1.png"], original_png);
    assert_eq!(
        after_parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        after_parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );
}

#[test]
fn add_image_jpeg_inserts_media_relationship_and_content_type() {
    let jpeg = tiny_jpeg();
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_image_jpeg(&jpeg, "photo.jpg")
        .expect("insert jpeg image");

    assert_eq!(
        doc.edited_parts(),
        [
            "[Content_Types].xml",
            "word/_rels/document.xml.rels",
            "word/document.xml",
            "word/media/photo.jpg"
        ]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/photo.jpg"], jpeg);

    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        ct.contains(r#"PartName="/word/media/photo.jpg""#)
            && ct.contains(r#"ContentType="image/jpeg""#),
        "jpeg content type missing: {ct}"
    );
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image""#
        ) && rels.contains(r#"Target="media/photo.jpg""#),
        "jpeg relationship missing: {rels}"
    );
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:drawing") && body.contains("r:embed"),
        "drawing missing: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/jpeg"));
    assert_eq!(images[0].bytes.as_deref(), Some(jpeg.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn replace_image_jpeg_updates_existing_media_part_only() {
    let original_jpeg = tiny_jpeg();
    let replacement_jpeg = tiny_jpeg_with_comment(b"rwml replacement");
    let before = jpeg_image_docx(&original_jpeg);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert_eq!(
        doc.images()[0].bytes.as_deref(),
        Some(original_jpeg.as_slice())
    );

    doc.replace_image_jpeg(&replacement_jpeg, "photo.jpeg")
        .expect("replace existing jpeg");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/photo.jpeg"], replacement_jpeg);
    assert_eq!(
        parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.images()[0].mime.as_deref(), Some("image/jpeg"));
    assert_eq!(
        reopened.images()[0].bytes.as_deref(),
        Some(replacement_jpeg.as_slice())
    );
}

#[test]
fn replace_image_jpeg_rejects_missing_or_invalid_inputs_without_mutation() {
    let original_jpeg = tiny_jpeg();
    let before = jpeg_image_docx(&original_jpeg);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert!(doc.replace_image_jpeg(b"not jpeg", "photo.jpeg").is_err());
    assert!(doc
        .replace_image_jpeg(&tiny_jpeg(), "missing.jpeg")
        .is_err());
    assert!(doc
        .replace_image_jpeg(&tiny_jpeg(), "../photo.jpeg")
        .is_err());

    let after = doc.save().expect("save after failed edits");
    let after_parts = unzip_parts(&after);
    assert_eq!(after_parts["word/media/photo.jpeg"], original_jpeg);
    assert_eq!(
        after_parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        after_parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );
}

#[test]
fn add_image_gif_inserts_media_relationship_and_content_type() {
    let gif = tiny_gif();
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_image_gif(&gif, "anim.gif")
        .expect("insert gif image");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/anim.gif"], gif);

    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        ct.contains(r#"PartName="/word/media/anim.gif""#)
            && ct.contains(r#"ContentType="image/gif""#),
        "gif content type missing: {ct}"
    );
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image""#
        ) && rels.contains(r#"Target="media/anim.gif""#),
        "gif relationship missing: {rels}"
    );
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:drawing") && body.contains("r:embed"),
        "drawing missing: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/gif"));
    assert_eq!(images[0].bytes.as_deref(), Some(gif.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn replace_image_gif_updates_existing_media_part_only() {
    let original_gif = tiny_gif();
    let replacement_gif = tiny_gif_with_comment(b"rwml replacement");
    let before = gif_image_docx(&original_gif);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert_eq!(
        doc.images()[0].bytes.as_deref(),
        Some(original_gif.as_slice())
    );

    doc.replace_image_gif(&replacement_gif, "anim.gif")
        .expect("replace existing gif");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/anim.gif"], replacement_gif);
    assert_eq!(
        parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.images()[0].mime.as_deref(), Some("image/gif"));
    assert_eq!(
        reopened.images()[0].bytes.as_deref(),
        Some(replacement_gif.as_slice())
    );
}

#[test]
fn replace_image_gif_rejects_missing_or_invalid_inputs_without_mutation() {
    let original_gif = tiny_gif();
    let before = gif_image_docx(&original_gif);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert!(doc.replace_image_gif(b"not gif", "anim.gif").is_err());
    assert!(doc.replace_image_gif(&tiny_gif(), "missing.gif").is_err());
    assert!(doc.replace_image_gif(&tiny_gif(), "../anim.gif").is_err());

    let after = doc.save().expect("save after failed edits");
    let after_parts = unzip_parts(&after);
    assert_eq!(after_parts["word/media/anim.gif"], original_gif);
    assert_eq!(
        after_parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        after_parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );
}

#[test]
fn add_image_bmp_inserts_media_relationship_and_content_type() {
    let bmp = tiny_bmp();
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_image_bmp(&bmp, "bitmap.bmp")
        .expect("insert bmp image");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/bitmap.bmp"], bmp);

    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        ct.contains(r#"PartName="/word/media/bitmap.bmp""#)
            && ct.contains(r#"ContentType="image/bmp""#),
        "bmp content type missing: {ct}"
    );
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image""#
        ) && rels.contains(r#"Target="media/bitmap.bmp""#),
        "bmp relationship missing: {rels}"
    );
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:drawing") && body.contains("r:embed"),
        "drawing missing: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/bmp"));
    assert_eq!(images[0].bytes.as_deref(), Some(bmp.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn replace_image_bmp_updates_existing_media_part_only() {
    let original_bmp = tiny_bmp();
    let replacement_bmp = tiny_bmp_with_marker(b"rwml replacement");
    let before = bmp_image_docx(&original_bmp);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert_eq!(
        doc.images()[0].bytes.as_deref(),
        Some(original_bmp.as_slice())
    );

    doc.replace_image_bmp(&replacement_bmp, "bitmap.bmp")
        .expect("replace existing bmp");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/bitmap.bmp"], replacement_bmp);
    assert_eq!(
        parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.images()[0].mime.as_deref(), Some("image/bmp"));
    assert_eq!(
        reopened.images()[0].bytes.as_deref(),
        Some(replacement_bmp.as_slice())
    );
}

#[test]
fn replace_image_bmp_rejects_missing_or_invalid_inputs_without_mutation() {
    let original_bmp = tiny_bmp();
    let before = bmp_image_docx(&original_bmp);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert!(doc.replace_image_bmp(b"not bmp", "bitmap.bmp").is_err());
    assert!(doc.replace_image_bmp(&tiny_bmp(), "missing.bmp").is_err());
    assert!(doc.replace_image_bmp(&tiny_bmp(), "../bitmap.bmp").is_err());

    let after = doc.save().expect("save after failed edits");
    let after_parts = unzip_parts(&after);
    assert_eq!(after_parts["word/media/bitmap.bmp"], original_bmp);
    assert_eq!(
        after_parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        after_parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );
}

#[test]
fn add_image_webp_inserts_media_relationship_and_content_type() {
    let webp = tiny_webp();
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_image_webp(&webp, "picture.webp")
        .expect("insert webp image");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/picture.webp"], webp);

    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        ct.contains(r#"PartName="/word/media/picture.webp""#)
            && ct.contains(r#"ContentType="image/webp""#),
        "webp content type missing: {ct}"
    );
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image""#
        ) && rels.contains(r#"Target="media/picture.webp""#),
        "webp relationship missing: {rels}"
    );
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:drawing") && body.contains("r:embed"),
        "drawing missing: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/webp"));
    assert_eq!(images[0].bytes.as_deref(), Some(webp.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn replace_image_webp_updates_existing_media_part_only() {
    let original_webp = tiny_webp();
    let replacement_webp = tiny_webp_with_marker(b"rwml replacement");
    let before = webp_image_docx(&original_webp);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert_eq!(
        doc.images()[0].bytes.as_deref(),
        Some(original_webp.as_slice())
    );

    doc.replace_image_webp(&replacement_webp, "picture.webp")
        .expect("replace existing webp");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/picture.webp"], replacement_webp);
    assert_eq!(
        parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.images()[0].mime.as_deref(), Some("image/webp"));
    assert_eq!(
        reopened.images()[0].bytes.as_deref(),
        Some(replacement_webp.as_slice())
    );
}

#[test]
fn replace_image_webp_rejects_missing_or_invalid_inputs_without_mutation() {
    let original_webp = tiny_webp();
    let before = webp_image_docx(&original_webp);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert!(doc.replace_image_webp(b"not webp", "picture.webp").is_err());
    assert!(doc
        .replace_image_webp(&tiny_webp(), "missing.webp")
        .is_err());
    assert!(doc
        .replace_image_webp(&tiny_webp(), "../picture.webp")
        .is_err());

    let after = doc.save().expect("save after failed edits");
    let after_parts = unzip_parts(&after);
    assert_eq!(after_parts["word/media/picture.webp"], original_webp);
    assert_eq!(
        after_parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        after_parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );
}

#[test]
fn add_image_tiff_inserts_media_relationship_and_content_type() {
    let tiff = tiny_tiff();
    let mut doc = Document::open(&no_notes_docx()).expect("fixture opens");

    doc.add_image_tiff(&tiff, "scan.tiff")
        .expect("insert tiff image");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/scan.tiff"], tiff);

    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        ct.contains(r#"PartName="/word/media/scan.tiff""#)
            && ct.contains(r#"ContentType="image/tiff""#),
        "tiff content type missing: {ct}"
    );
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image""#
        ) && rels.contains(r#"Target="media/scan.tiff""#),
        "tiff relationship missing: {rels}"
    );
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        body.contains("<w:drawing") && body.contains("r:embed"),
        "drawing missing: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/tiff"));
    assert_eq!(images[0].bytes.as_deref(), Some(tiff.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn replace_image_tiff_updates_existing_media_part_only() {
    let original_tiff = tiny_tiff();
    let replacement_tiff = tiny_tiff_with_marker(b"rwml replacement");
    let before = tiff_image_docx(&original_tiff);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert_eq!(
        doc.images()[0].bytes.as_deref(),
        Some(original_tiff.as_slice())
    );

    doc.replace_image_tiff(&replacement_tiff, "scan.tiff")
        .expect("replace existing tiff");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    assert_eq!(parts["word/media/scan.tiff"], replacement_tiff);
    assert_eq!(
        parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.images()[0].mime.as_deref(), Some("image/tiff"));
    assert_eq!(
        reopened.images()[0].bytes.as_deref(),
        Some(replacement_tiff.as_slice())
    );
}

#[test]
fn replace_image_tiff_rejects_missing_or_invalid_inputs_without_mutation() {
    let original_tiff = tiny_tiff();
    let before = tiff_image_docx(&original_tiff);
    let before_parts = unzip_parts(&before);
    let mut doc = Document::open(&before).expect("fixture opens");

    assert!(doc.replace_image_tiff(b"not tiff", "scan.tiff").is_err());
    assert!(doc
        .replace_image_tiff(&tiny_tiff(), "missing.tiff")
        .is_err());
    assert!(doc
        .replace_image_tiff(&tiny_tiff(), "../scan.tiff")
        .is_err());

    let after = doc.save().expect("save after failed edits");
    let after_parts = unzip_parts(&after);
    assert_eq!(after_parts["word/media/scan.tiff"], original_tiff);
    assert_eq!(
        after_parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    assert_eq!(
        after_parts["word/_rels/document.xml.rels"],
        before_parts["word/_rels/document.xml.rels"]
    );
}

#[test]
fn set_core_property_updates_existing_core_properties_part_only() {
    let before = core_properties_docx();
    let mut doc = Document::open(&before).expect("fixture opens");

    doc.set_core_property(CoreProperty::Title, "New <Title> & Co")
        .expect("title updates");
    doc.set_core_property(CoreProperty::Category, "Operations")
        .expect("category updates");
    doc.set_core_property(CoreProperty::ContentStatus, "Final")
        .expect("content status updates");
    doc.set_core_property(CoreProperty::Revision, "13")
        .expect("revision updates");
    doc.set_core_property(CoreProperty::Version, "2.0")
        .expect("version updates");
    doc.set_core_property(CoreProperty::Created, "2026-06-01T02:03:04Z")
        .expect("created timestamp updates");
    doc.set_core_property(CoreProperty::Modified, "2026-06-02T03:04:05Z")
        .expect("modified timestamp updates");
    doc.set_core_property(CoreProperty::LastPrinted, "2026-06-03T04:05:06Z")
        .expect("last-printed timestamp updates");

    assert_eq!(doc.edited_parts(), ["docProps/core.xml"]);
    let saved = doc.save().expect("save edited docx");
    let before_parts = unzip_parts(&before);
    let parts = unzip_parts(&saved);
    let core = String::from_utf8(parts["docProps/core.xml"].clone()).unwrap();

    assert!(core.contains("<dc:title>New &lt;Title&gt; &amp; Co</dc:title>"));
    assert!(core.contains("<dc:creator>Old Author</dc:creator>"));
    assert!(core.contains("category") && core.contains(">Operations</cp:category>"));
    assert!(core.contains("contentStatus") && core.contains(">Final</cp:contentStatus>"));
    assert!(core.contains("revision") && core.contains(">13</cp:revision>"));
    assert!(core.contains("version") && core.contains(">2.0</cp:version>"));
    assert!(
        core.contains(r#"xsi:type="dcterms:W3CDTF">2026-06-01T02:03:04Z</dcterms:created>"#),
        "created timestamp missing W3CDTF type: {core}"
    );
    assert!(
        core.contains(r#"xsi:type="dcterms:W3CDTF">2026-06-02T03:04:05Z</dcterms:modified>"#),
        "modified timestamp missing W3CDTF type: {core}"
    );
    assert!(
        core.contains("lastPrinted") && core.contains(">2026-06-03T04:05:06Z</cp:lastPrinted>")
    );
    assert_eq!(
        parts["word/document.xml"], before_parts["word/document.xml"],
        "body should not be rewritten"
    );
    let reopened = Document::open(&saved).expect("saved core metadata reopens");
    let props = reopened.core_properties();
    assert_eq!(props.category.as_deref(), Some("Operations"));
    assert_eq!(props.content_status.as_deref(), Some("Final"));
    assert_eq!(props.revision.as_deref(), Some("13"));
    assert_eq!(props.version.as_deref(), Some("2.0"));
    assert_eq!(props.created.as_deref(), Some("2026-06-01T02:03:04Z"));
    assert_eq!(props.modified.as_deref(), Some("2026-06-02T03:04:05Z"));
    assert_eq!(props.last_printed.as_deref(), Some("2026-06-03T04:05:06Z"));
}

#[test]
fn set_core_property_creates_missing_part_relationship_and_content_type() {
    let before = table_docx();
    let mut doc = Document::open(&before).expect("fixture opens");

    doc.set_core_property(CoreProperty::Creator, "Hyunjo Jung")
        .expect("creator property added");
    doc.set_core_property(CoreProperty::Created, "2026-06-01T02:03:04Z")
        .expect("created timestamp added");
    doc.set_core_property(CoreProperty::Modified, "2026-06-02T03:04:05Z")
        .expect("modified timestamp added");

    let saved = doc.save().expect("save edited docx");
    let before_parts = unzip_parts(&before);
    let parts = unzip_parts(&saved);
    let core = String::from_utf8(parts["docProps/core.xml"].clone()).unwrap();
    let root_rels = String::from_utf8(parts["_rels/.rels"].clone()).unwrap();
    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    assert!(
        core.contains("<dc:creator") && core.contains(">Hyunjo Jung</dc:creator>"),
        "creator property missing: {core}"
    );
    assert!(
        core.contains(
            r#"<dcterms:created xsi:type="dcterms:W3CDTF">2026-06-01T02:03:04Z</dcterms:created>"#
        ) && core.contains(
            r#"<dcterms:modified xsi:type="dcterms:W3CDTF">2026-06-02T03:04:05Z</dcterms:modified>"#
        ),
        "timestamp properties missing: {core}"
    );
    assert!(
        root_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml""#
        ),
        "core properties relationship missing: {root_rels}"
    );
    assert!(
        ct.contains(r#"PartName="/docProps/core.xml""#)
            && ct.contains("application/vnd.openxmlformats-package.core-properties+xml"),
        "core properties content type missing: {ct}"
    );
    assert_eq!(
        parts["word/document.xml"],
        before_parts["word/document.xml"]
    );
    let reopened = Document::open(&saved).expect("saved created core metadata reopens");
    let props = reopened.core_properties();
    assert_eq!(props.creator.as_deref(), Some("Hyunjo Jung"));
    assert_eq!(props.created.as_deref(), Some("2026-06-01T02:03:04Z"));
    assert_eq!(props.modified.as_deref(), Some("2026-06-02T03:04:05Z"));
}
