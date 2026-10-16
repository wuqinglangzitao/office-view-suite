#![cfg(feature = "docx")]

use rwml::{
    CoreProperties, Document, DocumentFormat, DocumentWarning, EditReadOnlyReason,
    FieldEvaluationReason, FieldEvaluationReasonCount, FieldKind, FieldKindCount, MetafileFormat,
};
use std::io::Write;

#[test]
fn blank_docx_report_exposes_format_and_stats() {
    let doc = Document::new();
    let report = doc.report();

    assert_eq!(report.format, DocumentFormat::Docx);
    assert_eq!(report.stats, doc.model().meta.stats);
    assert!(report.edit.package_preserving);
    assert!(report.edit.read_only_reasons.is_empty());
    assert_eq!(doc.edit_capability(), report.edit);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[cfg(feature = "docx")]
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

#[cfg(feature = "docx")]
fn field_kind_count(kind: FieldKind, count: usize) -> FieldKindCount {
    FieldKindCount { kind, count }
}

#[cfg(feature = "docx")]
fn field_reason_count(reason: FieldEvaluationReason, count: usize) -> FieldEvaluationReasonCount {
    FieldEvaluationReasonCount { reason, count }
}

#[cfg(feature = "docx")]
fn assert_report_field_diagnostics(
    fixture: Vec<u8>,
    field_count: usize,
    field_kinds: Vec<FieldKindCount>,
    unsupported_field_kinds: Vec<FieldKindCount>,
    unsupported_field_reasons: Vec<FieldEvaluationReasonCount>,
) {
    let doc = Document::open(&fixture).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, field_count);
    assert_eq!(report.features.field_kinds, field_kinds);
    assert_eq!(
        report.features.unsupported_field_kinds,
        unsupported_field_kinds
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        unsupported_field_reasons
    );
}

#[cfg(feature = "docx")]
fn docx_fixture_bytes(parts: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let opt = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(name, opt).unwrap();
            zip.write_all(&body).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

#[cfg(feature = "docx")]
fn put_u16le(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "docx")]
fn put_i16le(out: &mut [u8], offset: usize, value: i16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "docx")]
fn put_u32le(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "docx")]
fn put_i32le(out: &mut [u8], offset: usize, value: i32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "docx")]
fn sample_emf(width: i32, height: i32) -> Vec<u8> {
    let mut bytes = vec![0u8; 88];
    put_u32le(&mut bytes, 0, 1); // EMR_HEADER
    put_u32le(&mut bytes, 4, 88);
    put_i32le(&mut bytes, 8, 0);
    put_i32le(&mut bytes, 12, 0);
    put_i32le(&mut bytes, 16, width - 1);
    put_i32le(&mut bytes, 20, height - 1);
    bytes[40..44].copy_from_slice(b" EMF");
    put_u32le(&mut bytes, 44, 0x0001_0000);
    bytes
}

#[cfg(feature = "docx")]
fn sample_bgr24_dib() -> Vec<u8> {
    let mut dib = vec![0u8; 40];
    put_u32le(&mut dib, 0, 40); // BITMAPINFOHEADER
    put_i32le(&mut dib, 4, 2);
    put_i32le(&mut dib, 8, 2); // bottom-up
    put_u16le(&mut dib, 12, 1);
    put_u16le(&mut dib, 14, 24);
    put_u32le(&mut dib, 16, 0); // BI_RGB
    put_u32le(&mut dib, 20, 16);
    // bottom row: blue, white; top row: red, green; rows padded to 4 bytes.
    dib.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
    dib.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    dib
}

#[cfg(feature = "docx")]
fn sample_palette8_dib() -> Vec<u8> {
    let mut dib = vec![0u8; 40];
    put_u32le(&mut dib, 0, 40);
    put_i32le(&mut dib, 4, 2);
    put_i32le(&mut dib, 8, 2);
    put_u16le(&mut dib, 12, 1);
    put_u16le(&mut dib, 14, 8);
    put_u32le(&mut dib, 20, 8);
    put_u32le(&mut dib, 32, 4);
    dib.extend_from_slice(&[
        0, 0, 255, 0, // red
        0, 255, 0, 0, // green
        255, 0, 0, 0, // blue
        255, 255, 255, 0, // white
    ]);
    dib.extend_from_slice(&[2, 3, 0, 0]);
    dib.extend_from_slice(&[0, 1, 0, 0]);
    dib
}

#[cfg(feature = "docx")]
fn sample_rgb565_dib() -> Vec<u8> {
    let mut dib = vec![0u8; 40];
    put_u32le(&mut dib, 0, 40);
    put_i32le(&mut dib, 4, 2);
    put_i32le(&mut dib, 8, 2);
    put_u16le(&mut dib, 12, 1);
    put_u16le(&mut dib, 14, 16);
    put_u32le(&mut dib, 16, 3);
    put_u32le(&mut dib, 20, 8);
    dib.extend_from_slice(&0xF800u32.to_le_bytes());
    dib.extend_from_slice(&0x07E0u32.to_le_bytes());
    dib.extend_from_slice(&0x001Fu32.to_le_bytes());
    dib.extend_from_slice(&[0x1F, 0x00, 0xFF, 0xFF]);
    dib.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07]);
    dib
}

#[cfg(feature = "docx")]
fn expected_rgba_2x2_pixels() -> Vec<u8> {
    vec![
        255, 0, 0, 255, // red
        0, 255, 0, 255, // green
        0, 0, 255, 255, // blue
        255, 255, 255, 255, // white
    ]
}

#[cfg(feature = "docx")]
fn sample_emf_with_stretchdibits() -> Vec<u8> {
    let dib = sample_bgr24_dib();
    sample_emf_with_dib(&dib, 40)
}

#[cfg(feature = "docx")]
fn sample_emf_with_dib_and_vector_overlay() -> Vec<u8> {
    let mut bytes = sample_emf_with_stretchdibits();
    let eof = bytes.split_off(bytes.len() - 20);
    let start = bytes.len();
    bytes.resize(start + 20, 0);
    put_u32le(&mut bytes, start, 15); // EMR_SETPIXELV
    put_u32le(&mut bytes, start + 4, 20);
    put_u32le(&mut bytes, start + 16, 0x0000_00FF);
    bytes.extend_from_slice(&eof);
    let byte_len = bytes.len() as u32;
    put_u32le(&mut bytes, 48, byte_len);
    put_u32le(&mut bytes, 52, 4);
    bytes
}

#[cfg(feature = "docx")]
fn sample_emf_with_dib(dib: &[u8], bmi_len: usize) -> Vec<u8> {
    let bits_len = dib.len() - bmi_len;
    let record_size = 80 + dib.len();
    let mut bytes = sample_emf(2, 2);
    let start = bytes.len();
    bytes.resize(start + record_size, 0);
    put_u32le(&mut bytes, start, 81); // EMR_STRETCHDIBITS
    put_u32le(&mut bytes, start + 4, record_size as u32);
    put_i32le(&mut bytes, start + 16, 1);
    put_i32le(&mut bytes, start + 20, 1);
    put_u32le(&mut bytes, start + 48, 80);
    put_u32le(&mut bytes, start + 52, bmi_len as u32);
    put_u32le(&mut bytes, start + 56, (80 + bmi_len) as u32);
    put_u32le(&mut bytes, start + 60, bits_len as u32);
    put_i32le(&mut bytes, start + 40, 2);
    put_i32le(&mut bytes, start + 44, 2);
    put_u32le(&mut bytes, start + 68, 0x00CC_0020);
    put_i32le(&mut bytes, start + 72, 2);
    put_i32le(&mut bytes, start + 76, 2);
    bytes[start + 80..start + 80 + dib.len()].copy_from_slice(dib);
    let eof = bytes.len();
    bytes.resize(eof + 20, 0);
    put_u32le(&mut bytes, eof, 14);
    put_u32le(&mut bytes, eof + 4, 20);
    put_u32le(&mut bytes, eof + 16, 20);
    let byte_len = bytes.len() as u32;
    put_u32le(&mut bytes, 48, byte_len);
    put_u32le(&mut bytes, 52, 3);
    bytes
}

#[cfg(feature = "docx")]
fn sample_placeable_wmf(width_units: i16, height_units: i16, units_per_inch: u16) -> Vec<u8> {
    let mut bytes = vec![0u8; 40];
    put_u32le(&mut bytes, 0, 0x9AC6CDD7);
    put_i16le(&mut bytes, 6, 0);
    put_i16le(&mut bytes, 8, 0);
    put_i16le(&mut bytes, 10, width_units);
    put_i16le(&mut bytes, 12, height_units);
    put_u16le(&mut bytes, 14, units_per_inch);
    put_u16le(&mut bytes, 22, 1); // standard WMF header follows placeable header
    put_u16le(&mut bytes, 24, 9);
    put_u16le(&mut bytes, 26, 0x0300);
    let checksum = (0..20).step_by(2).fold(0u16, |value, offset| {
        value ^ u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    });
    put_u16le(&mut bytes, 20, checksum);
    bytes
}

#[cfg(feature = "docx")]
fn sample_placeable_wmf_with_stretchdib() -> Vec<u8> {
    let dib = sample_bgr24_dib();
    sample_placeable_wmf_with_dib(&dib)
}

#[cfg(feature = "docx")]
fn sample_wmf_with_dib_and_vector_overlay() -> Vec<u8> {
    let mut bytes = sample_placeable_wmf_with_stretchdib();
    let eof = bytes.split_off(bytes.len() - 6);
    let start = bytes.len();
    bytes.resize(start + 14, 0);
    put_u32le(&mut bytes, start, 7);
    put_u16le(&mut bytes, start + 4, 0x041F); // META_SETPIXEL
    put_u32le(&mut bytes, start + 6, 0x0000_00FF);
    bytes.extend_from_slice(&eof);
    let file_words = ((bytes.len() - 22) / 2) as u32;
    put_u32le(&mut bytes, 28, file_words);
    bytes
}

#[cfg(feature = "docx")]
fn sample_placeable_wmf_with_dib(dib: &[u8]) -> Vec<u8> {
    let mut bytes = sample_placeable_wmf(2, 2, 96);
    let record_size = 6 + 22 + dib.len();
    let start = bytes.len();
    bytes.resize(start + record_size, 0);
    put_u32le(&mut bytes, start, (record_size / 2) as u32);
    put_u16le(&mut bytes, start + 4, 0x0F43); // META_STRETCHDIB
    put_u32le(&mut bytes, start + 6, 0x00CC_0020);
    put_i16le(&mut bytes, start + 12, 2);
    put_i16le(&mut bytes, start + 14, 2);
    put_i16le(&mut bytes, start + 20, 2);
    put_i16le(&mut bytes, start + 22, 2);
    bytes[start + 28..start + 28 + dib.len()].copy_from_slice(dib);
    let eof = bytes.len();
    bytes.resize(eof + 6, 0);
    put_u32le(&mut bytes, eof, 3);
    let file_words = ((bytes.len() - 22) / 2) as u32;
    put_u32le(&mut bytes, 28, file_words);
    put_u16le(&mut bytes, 32, 0);
    put_u32le(&mut bytes, 34, (record_size / 2) as u32);
    put_u16le(&mut bytes, 38, 0);
    bytes
}

#[cfg(feature = "docx")]
fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

#[cfg(feature = "docx")]
fn sample_compressed_emf() -> Vec<u8> {
    gzip_bytes(&sample_emf(320, 240))
}

#[cfg(feature = "docx")]
fn sample_compressed_placeable_wmf() -> Vec<u8> {
    gzip_bytes(&sample_placeable_wmf(1920, 960, 960))
}

#[cfg(feature = "docx")]
fn complex_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="emf" ContentType="image/x-emf"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/pic1.emf"/><Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/" TargetMode="External"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Hello</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p><w:p><w:hyperlink r:id="rIdLink"><w:r><w:t>Link</w:t></w:r></w:hyperlink></w:p><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p><w:ins><w:r><w:t>new</w:t></w:r></w:ins><w:del><w:r><w:delText>old</w:delText></w:r></w:del><w:moveFrom><w:r><w:delText>moved</w:delText></w:r></w:moveFrom><w:p><w:pPr><w:pPrChange w:id="9" w:author="Reviewer"><w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange></w:pPr><w:r><w:t>changed props</w:t></w:r></w:p><w:sdt><w:sdtContent><w:p><w:r><w:t>control</w:t></w:r></w:p></w:sdtContent></w:sdt><w:tbl><w:tr><w:tc><w:p><w:r><w:t>outer</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl><w:r><w:drawing><wp:anchor/></w:drawing></w:r><w:r><w:object/></w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing/></mc:Choice></mc:AlternateContent></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="0"><w:p><w:r><w:t>note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
        (
            "word/charts/chart1.xml",
            r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>"#,
        ),
        ("word/media/pic1.emf", "fake"),
    ])
}

#[cfg(feature = "docx")]
fn metafile_docx() -> Vec<u8> {
    docx_fixture_bytes(vec![
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="emf" ContentType="image/x-emf"/><Default Extension="wmf" ContentType="image/x-wmf"/><Default Extension="emz" ContentType="image/x-emz"/><Default Extension="wmz" ContentType="image/x-wmz"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
        ),
        (
            "word/_rels/document.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdEmf" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/pic1.emf"/><Relationship Id="rIdWmf" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/pic2.wmf"/><Relationship Id="rIdEmz" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/pic3.emz"/><Relationship Id="rIdWmz" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/pic4.wmz"/></Relationships>"#.to_vec(),
        ),
        (
            "word/document.xml",
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>metafiles</w:t></w:r></w:p></w:body></w:document>"#.to_vec(),
        ),
        ("word/media/pic1.emf", sample_emf(640, 480)),
        (
            "word/media/pic2.wmf",
            sample_placeable_wmf(1440, 720, 1440),
        ),
        ("word/media/pic3.emz", sample_compressed_emf()),
        ("word/media/pic4.wmz", sample_compressed_placeable_wmf()),
    ])
}

#[cfg(feature = "docx")]
fn metafile_image_docx(media_name: &str, rid: &str, media: Vec<u8>) -> Vec<u8> {
    let rel_target = format!("media/{media_name}");
    let media_path = format!("word/media/{media_name}");
    let rels = format!(
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{rel_target}"/></Relationships>"#
    );
    let document = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="19050"/><a:blip r:embed="{rid}"/></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#
    );
    docx_fixture_bytes(vec![
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="emf" ContentType="image/x-emf"/><Default Extension="wmf" ContentType="image/x-wmf"/><Default Extension="emz" ContentType="image/x-emz"/><Default Extension="wmz" ContentType="image/x-wmz"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
        ),
        ("word/_rels/document.xml.rels", rels.into_bytes()),
        ("word/document.xml", document.into_bytes()),
        (media_path.as_str(), media),
    ])
}

#[cfg(feature = "docx")]
fn lossy_metadata_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn editable_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>Quarter &lt;One&gt; &amp; Co</dc:title><dc:subject>Pipeline</dc:subject><dc:creator>Analyst</dc:creator><dc:description>Diagnostics summary</dc:description><cp:keywords>rwml,metadata</cp:keywords><cp:category>Operations</cp:category><cp:contentStatus>Draft</cp:contentStatus><cp:lastModifiedBy>Reviewer</cp:lastModifiedBy><dcterms:created>2026-06-01T02:03:04Z</dcterms:created><dcterms:modified>2026-06-02T03:04:05Z</dcterms:modified><cp:lastPrinted>2026-06-03T04:05:06Z</cp:lastPrinted><cp:revision>12</cp:revision><cp:version>1.2</cp:version></cp:coreProperties>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn custom_properties_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCustom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "docProps/custom.xml",
            r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Client Name"><vt:lpwstr>ACME &lt;Launch&gt;</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="Phase"><vt:lpwstr>Review &amp; Ship</vt:lpwstr></property></Properties>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FILENAME \p "><w:r><w:t>report.docx</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HYPERLINK &quot;https://example.com&quot; "><w:r><w:t>Example</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-3&quot; "><w:r><w:t>Contents</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 "><w:r><w:t>Figure 1</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" CUSTOM value "><w:r><w:t>custom</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn hyperlink_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" HYPERLINK &quot;https://example.com&quot; \o &quot;tip&quot; "><w:r><w:t>Example</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HYPERLINK &quot;https://example.com/portal&quot; \o Client portal tooltip \t NewWindow "><w:r><w:t>Unquoted tip</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HYPERLINK &quot;https://example.com/no-history&quot; \m \n "><w:r><w:t>No history</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HYPERLINK &quot;https://example.com "><w:r><w:t>Cached link</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HYPERLINK \o &quot;tip&quot; "><w:r><w:t>Cached tooltip-only link</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HYPERLINK &quot;https://example.com&quot; extra "><w:r><w:t>Cached trailing link</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn mixed_case_hyperlink_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" hYpErLiNk &quot;https://example.com/mixed&quot; "><w:r><w:t>Mixed link</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_unsupported_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" PAGE \* Unknown "><w:r><w:t>cached bad page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_unsupported_section_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="2" w:fmt="chicago"/></w:sectPr></w:pPr></w:p><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>stale unsupported current page</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGE \* Arabic "><w:r><w:t>stale explicit current page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_trusted_current_context_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="4" w:fmt="decimalZero"/></w:sectPr></w:pPr></w:p><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>stale restart decimal zero</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGE \* Arabic "><w:r><w:t>stale restart arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGE \* CardText \* Upper "><w:r><w:t>stale restart card upper</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Visible before ambiguous break.</w:t><w:br w:type="page"/></w:r></w:p><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>cached ambiguous page</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Rendered page lead.</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE \* roman </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale rendered roman page</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_accepted_current_context_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Page one text.</w:t></w:r></w:p><w:ins><w:p><w:r><w:lastRenderedPageBreak/><w:t>Inserted page two.</w:t></w:r></w:p></w:ins><w:ins><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE \* Arabic </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale inserted page</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:ins><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:lastRenderedPageBreak/></mc:Choice><mc:Fallback><w:lastRenderedPageBreak/></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>Alternate page three.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGE \* Ordinal "><w:r><w:t>stale alternate page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn unknown_field_gap_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" CUSTOM Field "><w:r><w:t>cached unknown field</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn section_pages_unsupported_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SECTIONPAGES \x "><w:r><w:t>cached bad section pages</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn revnum_unsupported_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" REVNUM \x "><w:r><w:t>cached bad revision</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn filename_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FILENAME \p "><w:r><w:t>report.docx</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FILENAME \x "><w:r><w:t>cached filename</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn document_info_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" DATE \@ &quot;yyyy-MM-dd&quot; "><w:r><w:t>2026-06-24</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TIME \@ &quot;HH:mm&quot; "><w:r><w:t>14:35</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTHOR \* MERGEFORMAT "><w:r><w:t>Hyunjo Jung</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DOCPROPERTY Company "><w:r><w:t>Example Co.</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NUMPAGES "><w:r><w:t>12</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EDITTIME "><w:r><w:t>42</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DOCPROPERTY &quot;Broken Name "><w:r><w:t>cached broken property</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn document_info_compact_date_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dcterms="http://purl.org/dc/terms/"><dcterms:created>2026-06-01T02:03:04Z</dcterms:created></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" CREATEDATE \@&quot;yyyy-MM-dd&quot; "><w:r><w:t>stale compact date</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn document_info_unquoted_date_picture_diagnostics_docx() -> Vec<u8> {
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dcterms="http://purl.org/dc/terms/"><dcterms:created>2026-06-01T02:03:04Z</dcterms:created></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" CREATEDATE \@ MMMM d, yyyy \* Upper "><w:r><w:t>stale unquoted date picture</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn user_info_compact_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" USERNAME &quot;Casey Reviewer&quot; \*Upper "><w:r><w:t>stale compact user</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn document_info_compact_property_format_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCustom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/></Relationships>"#,
        ),
        (
            "docProps/custom.xml",
            r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name=" Client Name "><vt:lpwstr>acme launch</vt:lpwstr></property></Properties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" DOCPROPERTY &quot;Client Name&quot; \*Caps "><w:r><w:t>stale compact property</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn document_info_compact_variable_format_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/settings.xml",
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docVars><w:docVar w:name=" ClientCode " w:val="alpha-42"/></w:docVars></w:settings>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" DOCVARIABLE ClientCode \*Upper "><w:r><w:t>stale compact variable</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn display_layout_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" ADVANCE \r 2 "><w:r><w:t>offset</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \s\up8(A)\ai4(B) "><w:r><w:t>cached broader script</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \s\up8(A "><w:r><w:t>cached malformed script</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \d \fo10(A) "><w:r><w:t>cached broader displace</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \d \fo10(A "><w:r><w:t>cached malformed displace</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SYMBOL 65 \f Wingdings "><w:r><w:t>cached unmapped wingdings</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SYMBOL 74 \f Wingdings "><w:r><w:t>cached smile wingdings</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SYMBOL 65 \f &quot;Wingdings "><w:r><w:t>cached malformed symbol</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn deterministic_eq_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" EQ \f(1,\f(2,3)) "><w:r><w:t>stale nested fraction</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \r(\f(1,4)) "><w:r><w:t>stale nested radical</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \b \lc\[ \rc\] (&quot;Range&quot;) "><w:r><w:t>stale explicit bracket</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \x \to \bo(\f(5,8)) \* Upper "><w:r><w:t>stale box</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \l(A,&quot;B, C&quot;,\r(4,16)) "><w:r><w:t>stale list</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \a \al \co2 \vs3 \hs3(Axy,Bxy,A,B) "><w:r><w:t>stale array</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \i \su \fcS(0,1,\f(3,4)) "><w:r><w:t>stale integral</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EQ \o \ac(&quot;A&quot;,/) "><w:r><w:t>stale overstrike</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn advance_compact_quoted_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" ADVANCE \r&quot;2&quot; "><w:r><w:t>stale compact right</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ADVANCE \u &quot;3&quot; \d4 "><w:r><w:t>stale quoted up down</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ADVANCE \l2 \y &quot;5&quot; \* Upper "><w:r><w:t>stale compact left vertical</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn symbol_compact_unicode_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SYMBOL 183 \fSymbol \s12 "><w:r><w:t>stale compact symbol</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SYMBOL 0x03BB \u \f Symbol "><w:r><w:t>stale unicode font</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SYMBOL 0x0041 \u \s &quot;10&quot; "><w:r><w:t>stale quoted size</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SYMBOL 0x0042 \u \s&quot;11&quot; "><w:r><w:t>stale compact quoted size</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn symbol_multi_token_font_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SYMBOL 0x03BB \u \f Times New Roman \* Upper "><w:r><w:t>stale multi-token font</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn action_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" GOTOBUTTON TargetBookmark &quot;Jump&quot; "><w:r><w:t>stale jump</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" GOTOBUTTON TargetBookmark Jump Now \* Upper "><w:r><w:t>stale jump upper</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MACROBUTTON RunReport &quot;Run report&quot; "><w:r><w:t>stale run</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MACROBUTTON RunReport Run \* Upper Again "><w:r><w:t>cached malformed action</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MACROBUTTON RunReport \* MERGEFORMAT "><w:r><w:t>cached target-only action</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT &quot;page \p&quot; "><w:r><w:t>Print instruction</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT status "><w:r><w:t>Unquoted print instruction</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT status ready \* MERGEFORMAT "><w:r><w:t>Multi-token print instruction</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT \p ReportBox &quot;0 0 moveto&quot; \* MERGEFORMAT "><w:r><w:t>PostScript instruction</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT \pReportBox &quot;compact moveto&quot; "><w:r><w:t>Compact PostScript instruction</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT \z &quot;bad&quot; "><w:r><w:t>cached unsupported print</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn action_compact_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" PRINT status ready \*MERGEFORMAT "><w:r><w:t>cached compact print</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn action_button_compact_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" GOTOBUTTON TargetBookmark Jump Now \*Upper "><w:r><w:t>stale compact jump</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MACROBUTTON RunReport \*MERGEFORMAT "><w:r><w:t>cached compact target-only action</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn action_print_group_unquoted_code_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" PRINT \p ReportBox 0 0 moveto \* MERGEFORMAT "><w:r><w:t>cached unquoted print group</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRINT \pReportBox compact moveto "><w:r><w:t>cached compact unquoted print group</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn action_accepted_current_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><w:del><w:p><w:fldSimple w:instr=" GOTOBUTTON HiddenTarget &quot;Deleted Jump&quot; "><w:r><w:t>deleted cached jump</w:t></w:r></w:fldSimple></w:p></w:del><mc:AlternateContent><mc:Choice Requires="wps"><w:p><w:fldSimple w:instr=" GOTOBUTTON TargetBookmark &quot;Choice Jump&quot; "><w:r><w:t>stale choice jump</w:t></w:r></w:fldSimple></w:p></mc:Choice><mc:Fallback><w:p><w:fldSimple w:instr=" GOTOBUTTON FallbackTarget &quot;Fallback Jump&quot; "><w:r><w:t>fallback cached jump</w:t></w:r></w:fldSimple></w:p></mc:Fallback></mc:AlternateContent><w:moveFrom><w:p><w:fldSimple w:instr=" PRINT moved status "><w:r><w:t>moved print cache</w:t></w:r></w:fldSimple></w:p></w:moveFrom><w:p><w:fldSimple w:instr=" PRINT status ready "><w:r><w:t>visible print cache</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn inserted_content_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" INCLUDETEXT &quot;appendix.docx&quot; "><w:r><w:t>Appendix text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INCLUDEPICTURE &quot;chart.png "><w:r><w:t>cached malformed include picture</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" LINK \* "><w:r><w:t>cached dangling format switch</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INCLUDETEXT &quot;chapter.docx&quot; \* BadFormat "><w:r><w:t>cached bad include format</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn mail_merge_helper_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" ADDRESSBLOCK "><w:r><w:t>Acme Corp</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" GREETINGLINE &quot;Dear "><w:r><w:t>cached malformed greeting</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn reference_index_generated_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" BIBLIOGRAPHY \l 1033 "><w:r><w:t>Works cited</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INDEX &quot;bad "><w:r><w:t>cached malformed index</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn compatibility_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" PRIVATE legacy-data "><w:r><w:t>cached private payload</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DATA legacy-data "><w:r><w:t>cached data payload</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" GLOSSARY AutoTextName "><w:r><w:t>cached glossary payload</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" HTMLACTIVEX LegacyControl "><w:r><w:t>cached activex payload</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ADDIN &quot;bad "><w:r><w:t>cached malformed addin</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn protected_form_field_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/settings.xml",
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:documentProtection w:edit="forms" w:enforcement="1"/></w:settings>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMCHECKBOX "><w:ffData><w:checkBox><w:checked w:val="true"/></w:checkBox></w:ffData><w:r><w:t>cached protected checked</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="First"/><w:listEntry w:val="Second"/></w:ddList></w:ffData><w:r><w:t>cached protected dropdown</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FORMTEXT "><w:r><w:t>cached protected text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FORMTEXT &quot;bad "><w:r><w:t>cached malformed form text</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn compact_form_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN \*Upper "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="first"/><w:listEntry w:val="chosen option"/></w:ddList></w:ffData><w:r><w:t>stale compact dropdown</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:r><w:t>alice</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn legacy_form_spaced_dropdown_index_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:default w:val=" 0 "/><w:result w:val=" 1 "/><w:listEntry w:val="Default A"/><w:listEntry w:val="Chosen B"/></w:ddList></w:ffData><w:r><w:t>stale spaced dropdown</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn legacy_form_alternate_content_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p/></mc:Choice><mc:Fallback><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:checkBox><w:checked w:val="true"/></w:checkBox></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMCHECKBOX </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>fallback checked</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></mc:Fallback></mc:AlternateContent><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><mc:AlternateContent><mc:Choice Requires="wps"><w:ddList><w:result w:val="0"/><w:listEntry w:val="Choice option"/></w:ddList></mc:Choice><mc:Fallback><w:ddList><w:result w:val="1"/><w:listEntry w:val="Fallback option"/></w:ddList></mc:Fallback></mc:AlternateContent></w:ffData><w:r><w:t>stale option</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn legacy_form_deleted_context_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:del><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:checkBox><w:checked w:val="true"/></w:checkBox></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMCHECKBOX </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>deleted checked</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:del><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:checkBox><w:checked w:val="false"/></w:checkBox></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMCHECKBOX </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale visible unchecked</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn non_body_form_field_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:fldSimple w:instr=" FORMCHECKBOX "><w:ffData><w:checkBox><w:checked w:val="true"/></w:checkBox></w:ffData><w:r><w:t>stale header checkbox</w:t></w:r></w:fldSimple></w:p></w:hdr>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:id="1"><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Option A"/><w:listEntry w:val="Option B"/></w:ddList></w:ffData><w:r><w:t>stale footnote option</w:t></w:r></w:fldSimple></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn numbering_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale autonum one</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \* MERGEFORMAT "><w:r><w:t>stale autonum two</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \* roman "><w:r><w:t>stale autonum roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \* Unknown "><w:r><w:t>cached unsupported autonum</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale autonum after unsupported</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \s. "><w:r><w:t>stale autonum separator</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \s &quot;)&quot; "><w:r><w:t>stale quoted autonum separator</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUMLGL "><w:r><w:t>cached legal number</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUMLGL \* roman "><w:r><w:t>cached legal roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUMOUT "><w:r><w:t>cached outline number</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUMOUT \* roman "><w:r><w:t>cached outline roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" LISTNUM LegalDefault \l 2 "><w:r><w:t>cached list number</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" BIDIOUTLINE "><w:r><w:t>cached bidi outline</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" BIDIOUTLINE \x "><w:r><w:t>cached malformed bidi outline</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn dynamic_control_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" IF CustomerTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>cached data if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" IF 1 = "><w:r><w:t>cached broken if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" COMPARE CustomerTier = &quot;Gold&quot; "><w:r><w:t>cached data compare</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" COMPARE \o = &quot;Gold&quot; "><w:r><w:t>cached switch compare</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = CustomerTotal \# &quot;0.00&quot; "><w:r><w:t>cached data formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 1 \# &quot;0.00 "><w:r><w:t>cached broken formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FILLIN &quot;Client?&quot; "><w:r><w:t>cached fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ASK ClientCode &quot;Client code?&quot; "><w:r><w:t>cached ask</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FILLIN &quot;broken prompt "><w:r><w:t>cached broken fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SET ClientName Client 42 "><w:r><w:t>cached unquoted set</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SET ClientName &quot;Acme "><w:r><w:t>cached broken set</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" QUOTE &quot;literal&quot; "><w:r><w:t>cached quote</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" QUOTE &quot;bad "><w:r><w:t>cached broken quote</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NEXT "><w:r><w:t>cached next</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SKIPIF 1 = 0 "><w:r><w:t>cached skipif</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NEXTIF City = &quot;Tokyo&quot; "><w:r><w:t>cached data nextif</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NEXTIF 1 = "><w:r><w:t>cached broken nextif</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_body_syntax_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" = CustomerTotal \# &quot;0.00&quot; "><w:r><w:t>cached data formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 1 + "><w:r><w:t>cached trailing operator</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = (1 + 2 "><w:r><w:t>cached open paren</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_numeric_picture_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" = 1234.5 \# &quot;$#,##0.00&quot; "><w:r><w:t>stale currency formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 33 \# &quot;##%&quot; "><w:r><w:t>stale percent formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 111053 + 111439 \# x## "><w:r><w:t>stale dropped formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 1 / 8 \# 0.00x "><w:r><w:t>stale precision formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 3 / 4 \# .x "><w:r><w:t>stale rounded formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 5 \# &quot;0 units&quot; "><w:r><w:t>stale spaced suffix formula</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_unquoted_multi_token_numeric_picture_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" = 5 \# 0 units \* MERGEFORMAT "><w:r><w:t>stale unquoted units formula</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_defined_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" = DEFINED(SUM(1; 2; 3)) "><w:r><w:t>stale defined expression</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = DEFINED(UnknownBookmark) "><w:r><w:t>stale undefined name</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = DEFINED(1 / 0) "><w:r><w:t>stale error expression</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = IF(DEFINED(2 + 3), 7, 9) "><w:r><w:t>stale nested defined</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = DEFINED() "><w:r><w:t>cached empty defined</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_neutral_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" = 2 + 3 \* MERGEFORMAT "><w:r><w:t>stale neutral formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = SUM(1; 2; 3) \*CHARFORMAT "><w:r><w:t>stale compact neutral formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 10.25 \* DollarText "><w:r><w:t>stale dollar formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 31 \* Hex "><w:r><w:t>stale hex formula</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = 2 + 3 \* Upper "><w:r><w:t>cached unsupported formula format</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_literal_function_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" = ABS(-22) "><w:r><w:t>stale abs</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = SUM(1, 2, 3) "><w:r><w:t>stale sum</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = PRODUCT(2, 3, 4) "><w:r><w:t>stale product</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = ROUND(123.456, 2) "><w:r><w:t>stale round</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = AVERAGE(2, 4, 6) "><w:r><w:t>stale average</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = IF(OR(0, TRUE), SUM(1, 2), 9) "><w:r><w:t>stale nested if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = SUM(1; 2; 3) "><w:r><w:t>stale semicolon sum</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" = SUM(1, 2; 3) "><w:r><w:t>cached mixed separators</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_reference_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>stale left sum</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = AVERAGE(LEFT) \# &quot;0.0&quot; "><w:r><w:t>stale left average</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(BELOW) "><w:r><w:t>stale below sum</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>9</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = SUM(RIGHT) "><w:r><w:t>stale right sum</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>7</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>0</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>0</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(ABOVE) "><w:r><w:t>stale above sum</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>n/a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>cached nonnumeric left</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_alternate_content_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p/></mc:Choice><mc:Fallback><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>fallback formula</w:t></w:r></w:fldSimple></w:p></mc:Fallback></mc:AlternateContent><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>stale visible sum</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_deleted_context_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:del><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>deleted formula</w:t></w:r></w:fldSimple></w:p></w:del><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>stale visible sum</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_combined_reference_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT,ABOVE) "><w:r><w:t>stale left above sum</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = SUM(RIGHT;ABOVE) "><w:r><w:t>stale right above sum</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>7</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = COUNT(LEFT,RIGHT) "><w:r><w:t>stale side count</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT,RIGHT;ABOVE) "><w:r><w:t>cached mixed positional separators</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_nested_expression_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = IF(SUM(LEFT)&gt;=10,10,0) "><w:r><w:t>stale nested if</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = AND(SUM(LEFT)&lt;10,SUM(ABOVE)&gt;=2) "><w:r><w:t>stale nested and</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = ROUND(AVERAGE(A1:B2),1) "><w:r><w:t>stale nested round</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = IF(SUM(LEFT,RIGHT;ABOVE)&gt;0,1,0) "><w:r><w:t>cached mixed nested table expression</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_ragged_reference_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = SUM(A1:C2) "><w:r><w:t>stale ragged range</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(C) "><w:r><w:t>stale ragged column</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(ABOVE) "><w:r><w:t>stale ragged above</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>0</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(ABOVE) "><w:r><w:t>cached absent above</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_general_number_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>10.25</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) \* DollarText "><w:r><w:t>stale table dollar</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>31</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) \* Hex "><w:r><w:t>stale table hex</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>21</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) \* OrdText "><w:r><w:t>stale table ordinal text</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_cell_reference_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>7</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = SUM(A1:B2) "><w:r><w:t>stale a1 range</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = PRODUCT(R1C2:R2C3) "><w:r><w:t>stale rncn range</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(A1,C2) "><w:r><w:t>stale a1 list</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(R) "><w:r><w:t>stale current row</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>7</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>9</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(C) "><w:r><w:t>stale current column</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>7</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>9</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>7</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(R2C1:R2C3) "><w:r><w:t>stale explicit row range</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = SUM(R1C1:R3C1) "><w:r><w:t>stale explicit column range</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = SUM(A1,B1;C1) "><w:r><w:t>cached mixed cell separators</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn formula_table_direct_cell_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>n/a</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:fldSimple w:instr=" = A1 + R1C2 \# &quot;0&quot; "><w:r><w:t>stale direct expression</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = B1 "><w:r><w:t>stale direct cell</w:t></w:r></w:fldSimple></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=" = C1 "><w:r><w:t>cached nonnumeric direct cell</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn set_backed_dynamic_control_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SET ClientTier &quot;Gold&quot; "><w:r><w:t>cached set</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" IF ClientTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale set if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" COMPARE ClientTier = &quot;Gold&quot; "><w:r><w:t>stale set compare</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" IF MissingTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>cached missing if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NEXTIF ClientTier = &quot;Gold&quot; "><w:r><w:t>cached nextif</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SKIPIF ClientTier &lt;&gt; &quot;Gold&quot; "><w:r><w:t>cached skipif</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn bookmark_backed_dynamic_control_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="InvoiceTier"/><w:r><w:t>Gold</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="InvoiceTotal"/><w:r><w:t>42</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" IF InvoiceTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale bookmark if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" COMPARE InvoiceTier = &quot;G*&quot; "><w:r><w:t>stale bookmark compare</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" IF InvoiceTotal &gt;= 40 &quot;match&quot; &quot;miss&quot; "><w:r><w:t>stale numeric bookmark if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" COMPARE InvoiceTotal &lt; 40 "><w:r><w:t>stale numeric bookmark compare</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" IF MissingTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>cached missing bookmark if</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NEXTIF InvoiceTier = &quot;Gold&quot; "><w:r><w:t>cached bookmark nextif</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SKIPIF InvoiceTotal &lt; 40 "><w:r><w:t>cached bookmark skipif</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn set_backed_direct_ref_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SET ClientCode Client-42 "><w:r><w:t>cached set</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ClientCode \* Upper "><w:r><w:t>stale direct set ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SET ClientName Client 42 "><w:r><w:t>cached multi-token set</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ClientName \* Upper "><w:r><w:t>stale direct multi-token ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn set_backed_direct_ref_gap_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SET ClientName Client 42 "><w:r><w:t>cached set</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ClientName \f "><w:r><w:t>cached field-bookmark note ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn compact_prompt_default_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FILLIN &quot;Client?&quot; \d&quot;Acme&quot; "><w:r><w:t>stale compact fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FILLIN &quot;Department?&quot; \dops \* Upper "><w:r><w:t>stale compact unquoted fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ASK ClientCode &quot;Client code?&quot; \d&quot;ac-42&quot; "><w:r><w:t>cached compact ask</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF ClientCode \* Upper "><w:r><w:t>stale compact ask ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FILLIN &quot;Project?&quot; \d Client 42 \* Upper "><w:r><w:t>stale multi-token fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ASK ClientName &quot;Client name?&quot; \d Client 42 "><w:r><w:t>cached multi-token ask</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF ClientName \* Upper "><w:r><w:t>stale multi-token ask ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn prompt_unquoted_multi_token_text_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FILLIN Client display prompt \d Acme Corp \* Upper "><w:r><w:t>stale unquoted prompt fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ASK ClientName Client name prompt \d Acme Corp "><w:r><w:t>cached unquoted prompt ask</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF ClientName \* Upper "><w:r><w:t>stale unquoted prompt ask ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn prompt_unquoted_multi_token_text_no_default_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FILLIN Client display prompt "><w:r><w:t>cached unquoted prompt fillin</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ASK ClientName Client name prompt "><w:r><w:t>cached unquoted prompt ask</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF ClientName \* Upper "><w:r><w:t>stale unquoted prompt ask ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn document_structure_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"><cp:revision>12</cp:revision></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" REVNUM "><w:r><w:t>4</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>2</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SECTIONPAGES "><w:r><w:t>5</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;Heading 1&quot; \n "><w:r><w:t>Executive Summary</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;Heading 1 "><w:r><w:t>cached broken style ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" STYLEREF \p &quot;Heading 1&quot; "><w:r><w:t>cached switch-first style ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn style_ref_supported_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="CustomCallout"><w:name w:val="Custom Heading"/></w:style><w:style w:type="character" w:styleId="LastName"><w:name w:val="Last Name"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; \* Upper "><w:r><w:t>stale heading style</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;Last Name&quot; "><w:r><w:t>stale forward last</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:rPr><w:rStyle w:val="LastName"/></w:rPr><w:t>Ackerman</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="CustomCallout"/></w:pPr><w:r><w:t>Forward Finding</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> STYLEREF &quot;Custom Heading&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale custom style</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn style_ref_unquoted_multi_token_style_name_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/></w:style><w:style w:type="paragraph" w:styleId="CustomCallout"><w:name w:val="Custom Heading"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF Heading 1 \* Upper "><w:r><w:t>stale unquoted heading</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:pStyle w:val="CustomCallout"/></w:pPr><w:r><w:t>Forward Finding</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> STYLEREF Custom Heading \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale unquoted custom style</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn style_ref_numbering_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="NumberedTarget"><w:name w:val="Numbered Target"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="91"/></w:numPr></w:pPr><w:r><w:t>Top 4</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="91"/></w:numPr></w:pPr><w:r><w:t>Child 4.3</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="91"/></w:numPr></w:pPr><w:fldSimple w:instr=" STYLEREF NumberedTarget \r "><w:r><w:t>stale relative style number</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:fldSimple w:instr=" STYLEREF NumberedTarget \r \t "><w:r><w:t>stale relative numeric style number</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="91"/></w:numPr></w:pPr><w:r><w:t>Child 4.4</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="91"/></w:numPr></w:pPr><w:r><w:t>Child 4.5</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="NumberedTarget"/><w:numPr><w:ilvl w:val="2"/><w:numId w:val="91"/></w:numPr></w:pPr><w:r><w:t>Target number</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF NumberedTarget \n "><w:r><w:t>stale style number</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" STYLEREF NumberedTarget \n\t "><w:r><w:t>stale numeric style number</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> STYLEREF &quot;Numbered Target&quot; \w \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale full style number</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="15"><w:lvl w:ilvl="0"><w:start w:val="4"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%2."/></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="Part %3."/></w:lvl></w:abstractNum><w:num w:numId="91"><w:abstractNumId w:val="15"/></w:num></w:numbering>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn style_ref_alternate_content_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Choice </w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:t>Heading</w:t></mc:Choice><mc:Fallback><w:t>Fallback Inline</w:t></mc:Fallback></mc:AlternateContent></w:r></w:p></mc:Choice><mc:Fallback><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Fallback Heading</w:t></w:r></w:p></mc:Fallback></mc:AlternateContent><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale style ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn style_ref_deleted_heading_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Visible Heading</w:t></w:r></w:p><w:del><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Deleted Heading</w:t></w:r></w:p></w:del><w:moveFrom><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Moved Heading</w:t></w:r></w:p></w:moveFrom><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale style ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn section_field_text_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SECTION \* ROMAN "><w:r><w:t>stale roman section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:fldSimple w:instr=" SECTION \* CardText \* Upper "><w:r><w:t>stale card section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:fldSimple w:instr=" SECTION \* Ordinal "><w:r><w:t>stale ordinal section</w:t></w:r></w:fldSimple></w:p><w:sectPr/></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn section_alternate_content_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p><w:pPr><w:sectPr/></w:pPr></w:p></mc:Choice><mc:Fallback><w:p><w:pPr><w:sectPr/></w:pPr></w:p></mc:Fallback></mc:AlternateContent><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale alternate section</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn section_pages_text_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SECTIONPAGES "></w:fldSimple></w:p><w:p><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:fldSimple w:instr=" SECTIONPAGES \* ROMAN "></w:fldSimple></w:p><w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:fldSimple w:instr=" SECTIONPAGES \* CardText \* Upper "></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SECTIONPAGES \* Ordinal </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn revision_number_text_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"><cp:revision>draft REVISION</cp:revision></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" REVNUM \* Upper "><w:r><w:t>stale upper revision</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REVNUM \*Lower "><w:r><w:t>stale lower revision</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REVNUM \* Caps "><w:r><w:t>stale caps revision</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REVNUM \* FirstCap "><w:r><w:t>stale first-cap revision</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn merge_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \* MERGEFORMAT "><w:r><w:t>Acme</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MERGEFIELD &quot;project-name&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Roadmap</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:p><w:fldSimple w:instr=" MERGEFIELD display-name \b &quot;Before &quot; \f &quot; After&quot; "><w:r><w:t>Display</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEFIELD &quot;client-name "><w:r><w:t>Cached client</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEFIELD \* MERGEFORMAT ClientName "><w:r><w:t>Cached missing name before format</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \b "><w:r><w:t>Cached missing prefix</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEFIELD client-name \x "><w:r><w:t>Cached unknown switch</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

fn split_complex_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MERGE</w:instrText></w:r><w:r><w:instrText>FIELD &quot;client-name&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Acme</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \h "><w:r><w:t>3</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF MissingBookmark \h "><w:r><w:t>missing page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_gap_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="PlainText"/><w:r><w:t>Plain target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF PlainText \h "><w:r><w:t>cached page ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF MissingPage \h "><w:r><w:t>cached missing page ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_text_format_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \* Upper "><w:r><w:t>3</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF MissingBookmark \* Lower "><w:r><w:t>missing page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_non_current_section_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><w:del><w:p><w:pPr><w:sectPr><w:pgNumType w:fmt="bullet"/></w:sectPr></w:pPr></w:p></w:del><mc:AlternateContent><mc:Choice Requires="wps"><w:p/></mc:Choice><mc:Fallback><w:p><w:pPr><w:sectPr><w:pgNumType w:fmt="bullet"/></w:sectPr></w:pPr></w:p></mc:Fallback></mc:AlternateContent><w:p><w:r><w:t>Intro text.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="CurrentTarget"/><w:r><w:t>Current target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF CurrentTarget \h "><w:r><w:t>cached current page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_content_paragraph_section_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:pgNumType w:fmt="bullet"/></w:sectPr></w:pPr><w:bookmarkStart w:id="7" w:name="BeforeFormatBreak"/><w:r><w:t>Before format break</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="AfterFormatBreak"/><w:r><w:t>After format break</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF BeforeFormatBreak \p "><w:r><w:t>stale before</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF AfterFormatBreak \p "><w:r><w:t>stale after</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_content_paragraph_section_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr><w:bookmarkStart w:id="7" w:name="BeforeSectionBreak"/><w:r><w:t>Before break</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:t>After break</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF BeforeSectionBreak \h "><w:r><w:t>stale before break</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_unsupported_section_format_override_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="2" w:fmt="chicago"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name=" UnsupportedFmt "/><w:r><w:t>Unsupported format target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF UnsupportedFmt \h \* Arabic "><w:r><w:t>stale override</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_relative_unsupported_section_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="2" w:fmt="chicago"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name=" UnsupportedFmt "/><w:r><w:t>Unsupported format target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Later page lead.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF UnsupportedFmt \p "><w:r><w:t>stale relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_relative_unsupported_section_format_override_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="2" w:fmt="chicago"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name=" UnsupportedFmt "/><w:r><w:t>Unsupported format target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Later page lead.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF UnsupportedFmt \p \* Arabic "><w:r><w:t>stale relative override</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_relative_unsupported_even_odd_section_formats_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="evenPage"/><w:pgNumType w:fmt="chicago"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name="EvenUnsupportedFmt"/><w:r><w:t>Even unsupported format target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>After even target.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF EvenUnsupportedFmt \p "><w:r><w:t>stale even relative</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:lastRenderedPageBreak/></w:r></w:p><w:p><w:pPr><w:sectPr><w:type w:val="oddPage"/><w:pgNumType w:fmt="chicago"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="8" w:name="OddUnsupportedFmt"/><w:r><w:t>Odd unsupported format target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>After odd target.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF OddUnsupportedFmt \p "><w:r><w:t>stale odd relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_manual_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \h "><w:r><w:t>99</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGEREF &quot;Figure1&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>old page</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \* CardText "><w:r><w:t>stale cardtext</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \* OrdText "><w:r><w:t>stale ordtext</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \p "><w:r><w:t>above</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_rendered_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Page one text can be long enough to paginate.</w:t></w:r></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="FigureTwo"/><w:r><w:t>Figure 2</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF FigureTwo \h "><w:r><w:t>99</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGEREF &quot;FigureTwo&quot; \* CHARFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>old rendered page</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF FigureTwo \p "><w:r><w:t>below</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_rendered_then_manual_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Page one text.</w:t></w:r></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t><w:br w:type="page"/></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="PageThree"/><w:r><w:t>Page three target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF PageThree \h "><w:r><w:t>99</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF PageThree \p "><w:r><w:t>old relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_section_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name="NextSection"/><w:r><w:t>Next section target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page three lead.</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:type w:val="evenPage"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="8" w:name="EvenSection"/><w:r><w:t>Even section target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:pPr><w:sectPr><w:type w:val="oddPage"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="9" w:name="OddSection"/><w:r><w:t>Odd section target</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF NextSection \h "><w:r><w:t>stale next</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF EvenSection \* roman "><w:r><w:t>stale even</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF OddSection \p "><w:r><w:t>stale odd relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_default_section_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr/></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name="DefaultSection"/><w:r><w:t>Default section target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF DefaultSection \h "><w:r><w:t>stale default</w:t></w:r></w:fldSimple></w:p><w:sectPr/></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_page_break_before_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:bookmarkStart w:id="7" w:name="BreakBefore"/><w:r><w:t>Break-before target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:bookmarkStart w:id="8" w:name="RenderedBreakBefore"/><w:r><w:t>Rendered break-before target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF BreakBefore \h "><w:r><w:t>stale break-before</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF RenderedBreakBefore \* Ordinal "><w:r><w:t>stale rendered break-before</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF RenderedBreakBefore \p "><w:r><w:t>stale relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_section_page_number_restart_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="7"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name="Restarted"/><w:r><w:t>Restarted target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Restarted next page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="8" w:name="RestartedNext"/><w:r><w:t>Restarted next target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF Restarted \h "><w:r><w:t>stale restart</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF RestartedNext \* ROMAN "><w:r><w:t>stale restart roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF RestartedNext \p "><w:r><w:t>stale restart relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_visible_intro_section_page_number_restart_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Intro text can auto-paginate before the restart.</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="7"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name="RestartedAfterIntro"/><w:r><w:t>Restarted target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF RestartedAfterIntro \h "><w:r><w:t>stale restarted page</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF RestartedAfterIntro \p "><w:r><w:t>stale restarted relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_final_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="FinalSection"/><w:r><w:t>Final-section target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGE \* Arabic "><w:r><w:t>stale final current</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF FinalSection \h "><w:r><w:t>stale final roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF FinalSection \* Arabic "><w:r><w:t>stale final arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF FinalSection \p "><w:r><w:t>stale final relative</w:t></w:r></w:fldSimple></w:p><w:sectPr><w:pgNumType w:start="5" w:fmt="lowerRoman"/></w:sectPr></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_decimal_zero_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="4" w:fmt="decimalZero"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Decimal zero page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="8" w:name="DecimalZeroSection"/><w:r><w:t>Decimal zero target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF DecimalZeroSection \h "><w:r><w:t>stale decimal zero</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF DecimalZeroSection \* Arabic "><w:r><w:t>stale decimal arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_number_in_dash_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="5" w:fmt="numberInDash"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="9" w:name="DashedSection"/><w:r><w:t>Dashed target</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF DashedSection \h "><w:r><w:t>stale dashed</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF DashedSection \* Arabic "><w:r><w:t>stale dashed arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_textual_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="2" w:fmt="lowerLetter"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="7" w:name="LowerLetterSection"/><w:r><w:t>Lower letter target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF LowerLetterSection \h "><w:r><w:t>stale lower letter</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF LowerLetterSection \* Arabic "><w:r><w:t>stale lower letter arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="3" w:fmt="upperLetter"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Upper letter page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="8" w:name="UpperLetterSection"/><w:r><w:t>Upper letter target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF UpperLetterSection \h "><w:r><w:t>stale upper letter</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF UpperLetterSection \* Arabic "><w:r><w:t>stale upper letter arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="6" w:fmt="upperRoman"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="9" w:name="UpperRomanSection"/><w:r><w:t>Upper roman target</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF UpperRomanSection \h "><w:r><w:t>stale upper roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF UpperRomanSection \* Arabic "><w:r><w:t>stale upper roman arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="4" w:fmt="cardinalText"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Cardinal page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="10" w:name="CardinalTextSection"/><w:r><w:t>Cardinal text target</w:t></w:r><w:bookmarkEnd w:id="10"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF CardinalTextSection \h "><w:r><w:t>stale cardinal text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF CardinalTextSection \* Arabic "><w:r><w:t>stale cardinal text arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="5" w:fmt="ordinalText"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="11" w:name="OrdinalTextSection"/><w:r><w:t>Ordinal text target</w:t></w:r><w:bookmarkEnd w:id="11"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF OrdinalTextSection \h "><w:r><w:t>stale ordinal text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF OrdinalTextSection \* Arabic "><w:r><w:t>stale ordinal text arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_decimal_full_width_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="12" w:fmt="decimalFullWidth"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="10" w:name="FullWidthSection"/><w:r><w:t>Full-width target</w:t></w:r><w:bookmarkEnd w:id="10"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF FullWidthSection \h "><w:r><w:t>stale fullwidth</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF FullWidthSection \* Arabic "><w:r><w:t>stale fullwidth arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_decimal_enclosed_circle_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="12" w:fmt="decimalEnclosedCircle"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="11" w:name="CircleSection"/><w:r><w:t>Circle target</w:t></w:r><w:bookmarkEnd w:id="11"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF CircleSection \h "><w:r><w:t>stale circle</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF CircleSection \* Arabic "><w:r><w:t>stale circle arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_decimal_enclosed_punctuation_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="12" w:fmt="decimalEnclosedFullstop"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="12" w:name="FullstopSection"/><w:r><w:t>Fullstop target</w:t></w:r><w:bookmarkEnd w:id="12"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF FullstopSection \h "><w:r><w:t>stale fullstop</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF FullstopSection \* Arabic "><w:r><w:t>stale fullstop arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="11" w:fmt="decimalEnclosedParen"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Paren page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="13" w:name="ParenSection"/><w:r><w:t>Paren target</w:t></w:r><w:bookmarkEnd w:id="13"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF ParenSection \h "><w:r><w:t>stale paren</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF ParenSection \* Arabic "><w:r><w:t>stale paren arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_decimal_width_variant_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="12" w:fmt="decimalHalfWidth"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="14" w:name="HalfWidthSection"/><w:r><w:t>Half-width target</w:t></w:r><w:bookmarkEnd w:id="14"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF HalfWidthSection \h "><w:r><w:t>stale halfwidth</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF HalfWidthSection \* ArabicDash "><w:r><w:t>stale halfwidth dash</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="12" w:fmt="decimalFullWidth2"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Full-width alternate page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="15" w:name="FullWidthAltSection"/><w:r><w:t>Full-width alternate target</w:t></w:r><w:bookmarkEnd w:id="15"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF FullWidthAltSection \h "><w:r><w:t>stale fullwidth alt</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF FullWidthAltSection \* Arabic "><w:r><w:t>stale fullwidth alt arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_korean_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="1" w:fmt="ganada"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="16" w:name="GanadaSection"/><w:r><w:t>Ganada target</w:t></w:r><w:bookmarkEnd w:id="16"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF GanadaSection \h "><w:r><w:t>stale ganada</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF GanadaSection \* Arabic "><w:r><w:t>stale ganada arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="1" w:fmt="chosung"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Chosung page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="17" w:name="ChosungSection"/><w:r><w:t>Chosung target</w:t></w:r><w:bookmarkEnd w:id="17"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF ChosungSection \h "><w:r><w:t>stale chosung</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF ChosungSection \* Arabic "><w:r><w:t>stale chosung arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_korean_numeric_section_page_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="1" w:fmt="koreanDigital"/></w:sectPr></w:pPr></w:p><w:p><w:bookmarkStart w:id="18" w:name="KoreanDigitalSection"/><w:r><w:t>Korean digital target</w:t></w:r><w:bookmarkEnd w:id="18"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanDigitalSection \h "><w:r><w:t>stale korean digital</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanDigitalSection \* Arabic "><w:r><w:t>stale korean digital arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="1" w:fmt="koreanCounting"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Korean counting page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="19" w:name="KoreanCountingSection"/><w:r><w:t>Korean counting target</w:t></w:r><w:bookmarkEnd w:id="19"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanCountingSection \h "><w:r><w:t>stale korean counting</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanCountingSection \* Arabic "><w:r><w:t>stale korean counting arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="9" w:fmt="koreanLegal"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Korean legal page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="20" w:name="KoreanLegalSection"/><w:r><w:t>Korean legal target</w:t></w:r><w:bookmarkEnd w:id="20"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanLegalSection \h "><w:r><w:t>stale korean legal</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanLegalSection \* Arabic "><w:r><w:t>stale korean legal arabic</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgNumType w:start="1" w:fmt="koreanDigital2"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Korean digital2 page lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="21" w:name="KoreanDigital2Section"/><w:r><w:t>Korean digital2 target</w:t></w:r><w:bookmarkEnd w:id="21"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanDigital2Section \h "><w:r><w:t>stale korean digital2</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF KoreanDigital2Section \* Arabic "><w:r><w:t>stale korean digital2 arabic</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_visible_manual_break_before_rendered_hint_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Cover text can auto-paginate before the hard break.</w:t><w:br w:type="page"/></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="AmbiguousTarget"/><w:r><w:t>Ambiguous target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Later rendered hint.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF AmbiguousTarget \h "><w:r><w:t>99</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_rendered_break_page_one_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Cover"/><w:r><w:t>Cover title</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" PAGEREF Cover \h "><w:r><w:t>99</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_deleted_rendered_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:del><w:r><w:lastRenderedPageBreak/></w:r></w:del></w:p><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF Figure1 \h "><w:r><w:t>99</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_rendered_break_no_cached_result_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Page one text.</w:t></w:r></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="FigureTwo"/><w:r><w:t>Figure 2</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF FigureTwo \h "></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGEREF &quot;FigureTwo&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_wrapped_complex_rendered_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Page one text.</w:t></w:r></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="FigureTwo"/><w:r><w:t>Figure 2</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:ins><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGEREF &quot;FigureTwo&quot; \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>old wrapped page</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn page_ref_alternate_content_rendered_break_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>Page one text.</w:t></w:r></w:p><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:lastRenderedPageBreak/></mc:Choice><mc:Fallback><w:lastRenderedPageBreak/></mc:Fallback></mc:AlternateContent><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="AltPage"/><w:r><w:t>Alternate target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" PAGEREF AltPage \h "><w:r><w:t>stale alternate page</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="9" w:name="_Ref123456789"/><w:r><w:t>Table 2</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p><w:p><w:bookmarkStart w:id="10" w:name="EmptyTarget"/><w:bookmarkEnd w:id="10"/></w:p><w:p><w:fldSimple w:instr=" REF Figure1 "><w:r><w:t>stale cached text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF _Ref123456789 "><w:r><w:t>stale hidden ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \p "><w:r><w:t>above</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \f "><w:r><w:t>note mark</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \d- "><w:r><w:t>sequence separator</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF EmptyTarget "><w:r><w:t>empty target</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF MissingNote \f "><w:r><w:t>missing note mark</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF MissingRomanNote \f \* roman "><w:r><w:t>missing roman note mark</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF MissingBookmark "><w:r><w:t>Missing</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_accepted_current_bookmark_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><w:p><w:bookmarkStart w:id="7" w:name="ClauseText"/><w:r><w:t>Visible clause</w:t></w:r><w:del><w:r><w:t> deleted clause</w:t></w:r></w:del><w:moveFrom><w:r><w:t> moved clause</w:t></w:r></w:moveFrom><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="AltText"/><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:t>Choice clause</w:t></mc:Choice><mc:Fallback><w:t>Fallback clause</w:t></mc:Fallback></mc:AlternateContent></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" REF ClauseText "><w:r><w:t>stale deleted ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF AltText "><w:r><w:t>stale alternate ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_gap_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="PlainText"/><w:r><w:t>Plain target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" REF PlainText \f "><w:r><w:t>cached non-note ref mark</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF PlainText \d- "><w:r><w:t>cached ref separator</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF MissingRef "><w:r><w:t>cached missing ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_note_reference_mark_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" REF FootOne \f "><w:r><w:t>stale explicit note mark</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FootOne \f "><w:r><w:t>stale direct note mark</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_comment_reference_mark_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="CommentOne"/><w:r><w:commentReference w:id="0"/></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="CommentTwo"/><w:r><w:commentReference w:id="1"/></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" REF CommentOne \f "><w:r><w:t>stale comment one</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" CommentTwo \f \* ROMAN "><w:r><w:t>stale comment two</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_comment_range_reference_mark_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="CommentedRange"/><w:commentRangeStart w:id="0"/><w:r><w:t>Commented text</w:t></w:r><w:commentRangeEnd w:id="0"/><w:bookmarkEnd w:id="7"/><w:r><w:commentReference w:id="0"/></w:r></w:p><w:p><w:fldSimple w:instr=" REF CommentedRange \f "><w:r><w:t>stale range comment</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_inside_comment_range_reference_mark_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Comment starts </w:t></w:r><w:bookmarkStart w:id="7" w:name="InsideComment"/><w:r><w:t>inside comment</w:t></w:r><w:bookmarkEnd w:id="7"/><w:r><w:t> comment ends</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p><w:p><w:fldSimple w:instr=" REF InsideComment \f "><w:r><w:t>stale inside comment</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_non_current_bookmark_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><w:del><w:bookmarkStart w:id="10" w:name="DeletedOnly"/><w:r><w:t>old target</w:t></w:r><w:bookmarkEnd w:id="10"/></w:del><mc:AlternateContent><mc:Choice Requires="wps"><w:p/></mc:Choice><mc:Fallback><w:p><w:bookmarkStart w:id="11" w:name="FallbackOnly"/><w:r><w:t>fallback target</w:t></w:r><w:bookmarkEnd w:id="11"/></w:p></mc:Fallback></mc:AlternateContent><w:p><w:fldSimple w:instr=" REF DeletedOnly \f "><w:r><w:t>deleted note mark</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF FallbackOnly \d- "><w:r><w:t>fallback sequence separator</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn malformed_ref_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" REF &quot;MissingBookmark "><w:r><w:t>cached malformed ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF &quot;Figure List&quot; "><w:r><w:t>cached whitespace ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PAGEREF &quot;Figure List&quot; "><w:r><w:t>cached whitespace page ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF &quot;Figure List&quot; "><w:r><w:t>cached whitespace note ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \b &quot;Figure List&quot; "><w:r><w:t>cached whitespace toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn note_ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="PlainText"/><w:r><w:t>Not a note mark</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>stale note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" FTNREF FootOne "><w:r><w:t>stale legacy note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne \p "><w:r><w:t>stale relative note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF PlainText "><w:r><w:t>plain bookmark note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF MissingNote "><w:r><w:t>missing note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF MissingFormattedNote \* Upper "><w:r><w:t>missing formatted note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF MissingRomanNote \* roman "><w:r><w:t>missing roman note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne \x "><w:r><w:t>unsupported note switch</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn note_ref_number_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="LaterNote"/><w:r><w:footnoteReference w:id="2"/></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" NOTEREF LaterNote \* roman "><w:r><w:t>stale roman note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne \* OrdText \* Upper "><w:r><w:t>stale ordinal note</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn note_ref_alternate_content_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p/></mc:Choice><mc:Fallback><w:p><w:r><w:bookmarkStart w:id="50" w:name="FallbackNote"/><w:footnoteReference w:id="50"/><w:bookmarkEnd w:id="50"/></w:r></w:p></mc:Fallback></mc:AlternateContent><w:p><w:r><w:t>Target note</w:t></w:r><w:bookmarkStart w:id="7" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>stale alternate note</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF FootOne \p "><w:r><w:t>stale alternate relative</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:id="1"><w:p><w:r><w:t>First footnote.</w:t></w:r></w:p></w:footnote><w:footnote w:id="50"><w:p><w:r><w:t>Fallback footnote.</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn note_ref_gap_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="PlainText"/><w:r><w:t>Plain target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" NOTEREF PlainText "><w:r><w:t>cached plain note ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF MissingNote "><w:r><w:t>cached missing note ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" NOTEREF PlainText \x "><w:r><w:t>cached bad note ref switch</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
struct FieldDiagnosticInventoryCase {
    row: &'static str,
    instruction: &'static str,
    cached: &'static str,
    kind: FieldKind,
    reason: FieldEvaluationReason,
}

#[cfg(feature = "docx")]
fn field_diagnostic_inventory_case(
    row: &'static str,
    instruction: &'static str,
    cached: &'static str,
    kind: FieldKind,
    reason: FieldEvaluationReason,
) -> FieldDiagnosticInventoryCase {
    FieldDiagnosticInventoryCase {
        row,
        instruction,
        cached,
        kind,
        reason,
    }
}

#[cfg(feature = "docx")]
fn field_diagnostic_inventory_cases() -> Vec<FieldDiagnosticInventoryCase> {
    vec![
        field_diagnostic_inventory_case(
            "D1",
            r"REF PlainText \f",
            "cached ref note mark",
            FieldKind::Ref,
            FieldEvaluationReason::NoComputedResult,
        ),
        field_diagnostic_inventory_case(
            "D3",
            "REF MissingRef",
            "cached missing ref",
            FieldKind::Ref,
            FieldEvaluationReason::UnresolvedBookmark,
        ),
        field_diagnostic_inventory_case(
            "D4",
            r"PAGEREF PlainText \h",
            "cached page ref",
            FieldKind::PageRef,
            FieldEvaluationReason::NoComputedResult,
        ),
        field_diagnostic_inventory_case(
            "D5",
            r"PAGEREF MissingPage \h",
            "cached missing page ref",
            FieldKind::PageRef,
            FieldEvaluationReason::UnresolvedBookmark,
        ),
        field_diagnostic_inventory_case(
            "D6",
            "NOTEREF PlainText",
            "cached plain note ref",
            FieldKind::NoteRef,
            FieldEvaluationReason::NoComputedResult,
        ),
        field_diagnostic_inventory_case(
            "D7",
            "NOTEREF MissingNote",
            "cached missing note ref",
            FieldKind::NoteRef,
            FieldEvaluationReason::UnresolvedBookmark,
        ),
        field_diagnostic_inventory_case(
            "D8",
            r"NOTEREF PlainText \x",
            "cached bad note ref switch",
            FieldKind::NoteRef,
            FieldEvaluationReason::UnsupportedSwitch,
        ),
        field_diagnostic_inventory_case(
            "D10",
            r"TOC \b MissingScope",
            "cached missing toc scope",
            FieldKind::Toc,
            FieldEvaluationReason::UnresolvedBookmark,
        ),
        field_diagnostic_inventory_case(
            "D11",
            r"TOC \q",
            "cached bad toc switch",
            FieldKind::Toc,
            FieldEvaluationReason::UnsupportedSwitch,
        ),
        field_diagnostic_inventory_case(
            "D12",
            "CUSTOM Field",
            "cached unknown field",
            FieldKind::Unknown("CUSTOM".to_string()),
            FieldEvaluationReason::UnknownField,
        ),
    ]
}

#[cfg(feature = "docx")]
fn xml_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(feature = "docx")]
fn field_kind_counts_from_inventory(cases: &[FieldDiagnosticInventoryCase]) -> Vec<FieldKindCount> {
    let mut counts: Vec<FieldKindCount> = Vec::new();
    for case in cases {
        if let Some(count) = counts.iter_mut().find(|count| count.kind == case.kind) {
            count.count += 1;
        } else {
            counts.push(field_kind_count(case.kind.clone(), 1));
        }
    }
    counts
}

#[cfg(feature = "docx")]
fn field_reason_counts_from_inventory(
    cases: &[FieldDiagnosticInventoryCase],
) -> Vec<FieldEvaluationReasonCount> {
    let mut counts: Vec<FieldEvaluationReasonCount> = Vec::new();
    for case in cases {
        if let Some(count) = counts.iter_mut().find(|count| count.reason == case.reason) {
            count.count += 1;
        } else {
            counts.push(field_reason_count(case.reason, 1));
        }
    }
    counts
}

#[cfg(feature = "docx")]
fn field_diagnostic_inventory_docx() -> Vec<u8> {
    let mut document = String::from(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="PlainText"/><w:r><w:t>Plain target</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
    );
    for case in field_diagnostic_inventory_cases() {
        document.push_str(&format!(
            r#"<w:p><w:fldSimple w:instr=" {} "><w:r><w:t>{}</w:t></w:r></w:fldSimple></w:p>"#,
            xml_escape(case.instruction),
            xml_escape(case.cached)
        ));
    }
    document.push_str("</w:body></w:document>");

    docx_fixture_bytes(vec![
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
        ),
        ("word/document.xml", document.into_bytes()),
    ])
}

#[cfg(feature = "docx")]
fn direct_bookmark_ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" Figure1 "><w:r><w:t>stale direct ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn complex_direct_bookmark_ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>figure one</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> Figure1 \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex direct ref</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn direct_bookmark_ref_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>figure one</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" Figure1 \* Upper "><w:r><w:t>stale direct upper</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" Figure1 \*FirstCap "><w:r><w:t>stale direct first-cap</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" Figure1 \h "><w:r><w:t>stale direct hyperlink</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" Figure1 \d- "><w:r><w:t>direct sequence separator</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" Figure1 \f "><w:r><w:t>direct note mark</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn direct_relative_ref_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" LaterBookmark \p "><w:r><w:t>stale direct below</w:t></w:r></w:fldSimple></w:p><w:p><w:bookmarkStart w:id="8" w:name="LaterBookmark"/><w:r><w:t>Later target</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" Figure1 \p "><w:r><w:t>stale direct above</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn numbered_ref_switch_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="42"/></w:numPr></w:pPr><w:bookmarkStart w:id="7" w:name="Clause"/><w:r><w:t>Numbered clause</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" REF Clause \n "><w:r><w:t>stale number</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Clause \n \p "><w:r><w:t>stale number relative</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" Clause \n "><w:r><w:t>stale direct number</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="9"><w:lvl w:ilvl="0"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="42"><w:abstractNumId w:val="9"/></w:num></w:numbering>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn numbered_ref_suppress_text_switch_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="77"/></w:numPr></w:pPr><w:bookmarkStart w:id="9" w:name="SectionClause"/><w:r><w:t>Section clause</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p><w:p><w:fldSimple w:instr=" REF SectionClause \n \t "><w:r><w:t>stale numeric text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF SectionClause \n \t \p "><w:r><w:t>stale numeric relative</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SectionClause \n \t "><w:r><w:t>stale direct numeric text</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="11"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="Section %1.01,"/></w:lvl></w:abstractNum><w:num w:numId="77"><w:abstractNumId w:val="11"/></w:num></w:numbering>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn full_context_ref_switch_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="88"/></w:numPr></w:pPr><w:r><w:t>Top clause</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="88"/></w:numPr></w:pPr><w:r><w:t>Child clause</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="88"/></w:numPr></w:pPr><w:bookmarkStart w:id="12" w:name="DeepClause"/><w:r><w:t>Deep clause</w:t></w:r><w:bookmarkEnd w:id="12"/></w:p><w:p><w:fldSimple w:instr=" REF DeepClause \w "><w:r><w:t>stale full context</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF DeepClause \w \p "><w:r><w:t>stale full relative</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DeepClause \w "><w:r><w:t>stale direct full</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="12"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2."/></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="lowerRoman"/><w:lvlText w:val="%3."/></w:lvl></w:abstractNum><w:num w:numId="88"><w:abstractNumId w:val="12"/></w:num></w:numbering>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn full_context_ref_suppress_text_switch_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="89"/></w:numPr></w:pPr><w:r><w:t>Top clause</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="89"/></w:numPr></w:pPr><w:r><w:t>Child clause</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="89"/></w:numPr></w:pPr><w:bookmarkStart w:id="13" w:name="DeepClause"/><w:r><w:t>Deep clause</w:t></w:r><w:bookmarkEnd w:id="13"/></w:p><w:p><w:fldSimple w:instr=" REF DeepClause \w \t "><w:r><w:t>stale full numeric text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF DeepClause \w \t \p "><w:r><w:t>stale full numeric relative</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DeepClause \w \t "><w:r><w:t>stale direct full numeric</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="13"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="Section %1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="Article %2."/></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="lowerRoman"/><w:lvlText w:val="Part %3."/></w:lvl></w:abstractNum><w:num w:numId="89"><w:abstractNumId w:val="13"/></w:num></w:numbering>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn relative_context_ref_switch_diagnostics_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="90"/></w:numPr></w:pPr><w:r><w:t>Top 4</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="90"/></w:numPr></w:pPr><w:r><w:t>Child 4.3</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="90"/></w:numPr></w:pPr><w:fldSimple w:instr=" REF LaterClause \r "><w:r><w:t>stale relative context</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:fldSimple w:instr=" REF LaterClause \r \p "><w:r><w:t>stale relative context position</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:fldSimple w:instr=" LaterClause \r \t "><w:r><w:t>stale direct relative context</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="90"/></w:numPr></w:pPr><w:r><w:t>Child 4.4</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="90"/></w:numPr></w:pPr><w:r><w:t>Child 4.5</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="90"/></w:numPr></w:pPr><w:r><w:t>Target sibling 4.5.1</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="2"/><w:numId w:val="90"/></w:numPr></w:pPr><w:bookmarkStart w:id="14" w:name="LaterClause"/><w:r><w:t>Target 4.5.2</w:t></w:r><w:bookmarkEnd w:id="14"/></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="14"><w:lvl w:ilvl="0"><w:start w:val="4"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%2."/></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%3."/></w:lvl></w:abstractNum><w:num w:numId="90"><w:abstractNumId w:val="14"/></w:num></w:numbering>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn ref_text_format_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="Figure1"/><w:r><w:t>figure one</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \* Upper "><w:r><w:t>stale upper ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \* Caps "><w:r><w:t>stale caps ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \*FirstCap "><w:r><w:t>stale first-cap ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF Figure1 \p "><w:r><w:t>above</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF MissingBookmark \*Lower "><w:r><w:t>missing lower ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn multi_paragraph_ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="11" w:name="ClauseText"/><w:r><w:t>First paragraph.</w:t></w:r></w:p><w:p><w:r><w:t>Second paragraph.</w:t></w:r><w:bookmarkEnd w:id="11"/></w:p><w:p><w:fldSimple w:instr=" REF ClauseText "><w:r><w:t>stale multi ref</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" REF MissingBookmark "><w:r><w:t>Missing</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn inline_break_ref_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="12" w:name="InlineText"/><w:r><w:t>Alpha</w:t><w:tab/><w:t>Beta</w:t><w:br/><w:t>Gamma</w:t><w:noBreakHyphen/><w:t>Delta</w:t></w:r><w:bookmarkEnd w:id="12"/></w:p><w:p><w:fldSimple w:instr=" REF InlineText "><w:r><w:t>stale inline ref</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_field_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; "><w:r><w:t>stale toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \t &quot;CustomHeading,1&quot; "><w:r><w:t>stale advanced toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" CUSTOM value "><w:r><w:t>custom</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn complex_toc_field_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> TOC \o &quot;1-2&quot; \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex toc</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_bad_switch_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TOC \q "><w:r><w:t>cached bad toc switch</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_neutral_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \h \z \w \x "><w:r><w:t>stale neutral toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \t &quot;CustomHeading,1&quot; "><w:r><w:t>stale advanced toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_tc_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TC &quot;Manual Entry&quot; \f m \l 2 "/></w:p><w:p><w:fldSimple w:instr=" TOC \f m "><w:r><w:t>stale tc toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_tc_unquoted_multi_token_text_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TC Manual Appendix Entry \f m \l 2 "><w:r><w:t>cached unquoted tc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \f m "><w:r><w:t>stale unquoted tc toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_deleted_tc_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:del><w:p><w:fldSimple w:instr=" TC &quot;Deleted Entry&quot; \f m \l 1 "/></w:p></w:del><w:moveFrom><w:p><w:fldSimple w:instr=" TC &quot;Moved Entry&quot; \f m \l 1 "/></w:p></w:moveFrom><w:p><w:fldSimple w:instr=" TC &quot;Visible Entry&quot; \f m \l 1 "><w:r><w:t>stale visible marker</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \f m "><w:r><w:t>stale deleted tc toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_alternate_content_heading_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wps"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Choice </w:t><mc:AlternateContent><mc:Choice Requires="wps"><w:t>Inline</w:t></mc:Choice><mc:Fallback><w:t>Fallback Inline</w:t></mc:Fallback></mc:AlternateContent></w:r></w:p></mc:Choice><mc:Fallback><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Fallback Heading</w:t></w:r></w:p></mc:Fallback></mc:AlternateContent><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; "><w:r><w:t>stale alternate toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_sequence_caption_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>1</w:t></w:r></w:fldSimple><w:r><w:t>: Mercury</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \c &quot;Figure&quot; "><w:r><w:t>stale figures toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_dirty_sequence_caption_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>9</w:t></w:r></w:fldSimple><w:r><w:t>: Mercury</w:t></w:r></w:p><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>99</w:t></w:r></w:fldSimple><w:r><w:t>: Venus</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \c Figure "><w:r><w:t>stale dirty figures toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_dirty_complex_sequence_caption_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Figure </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SEQ Figure </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>9</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>: Mercury</w:t></w:r></w:p><w:p><w:r><w:t>Figure </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SEQ Figure </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>99</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>: Venus</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \c Figure "><w:r><w:t>stale dirty complex figures toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_sequence_caption_text_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>8</w:t></w:r></w:fldSimple><w:r><w:t>: Mercury</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \a Figure "><w:r><w:t>stale caption-text toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_sequence_heading_reset_caption_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter One</w:t></w:r></w:p><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure \s 1 "><w:r><w:t>9</w:t></w:r></w:fldSimple><w:r><w:t>: Mercury</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter Two</w:t></w:r></w:p><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure \s1 "><w:r><w:t>99</w:t></w:r></w:fldSimple><w:r><w:t>: Venus</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \c Figure "><w:r><w:t>stale heading-reset figures toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \a Figure "><w:r><w:t>stale heading-reset captions toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn sequence_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>stale figure</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SEQ Figure \r -1 "><w:r><w:t>cached invalid reset</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SEQ Figure \s 1 "><w:r><w:t>cached heading reset</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SEQ Figure \x "><w:r><w:t>bad sequence</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn sequence_numbering_text_format_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SEQ Figure \* CardText \* Upper "><w:r><w:t>stale sequence card</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SEQ Figure \* roman \* Upper "><w:r><w:t>stale sequence roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" SEQ Invoice \r 21 \* DollarText "><w:r><w:t>stale sequence dollars</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \* CardText \* Upper "><w:r><w:t>stale autonum card</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTONUM \* roman \* Upper "><w:r><w:t>stale autonum roman</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" LISTNUM NumberDefault \* CardText \* Upper "><w:r><w:t>stale listnum card</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" LISTNUM NumberDefault \* roman \* Upper "><w:r><w:t>stale listnum roman</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_general_format_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>executive summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>risk review</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \* Upper "><w:r><w:t>stale upper toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \* Caps "><w:r><w:t>stale caps toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \* MERGEFORMAT "><w:r><w:t>stale mergeformat toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_custom_style_match_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="CustomCallout"><w:name w:val="CustomHeading"/></w:style><w:style w:type="paragraph" w:styleId="CustomSpaced"><w:name w:val="Custom Heading"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="CustomCallout"/></w:pPr><w:r><w:t>Custom Finding</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="CustomSpaced"/></w:pPr><w:r><w:t>Spaced Custom Finding</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; \t &quot;CustomHeading,2&quot; "><w:r><w:t>stale custom toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; \t &quot;Custom Heading,2&quot; "><w:r><w:t>stale spaced custom toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_unquoted_custom_style_match_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="CustomCallout"><w:name w:val="Custom Heading"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="CustomCallout"/></w:pPr><w:r><w:t>Custom Finding</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; \t Custom Heading,2 \* Upper "><w:r><w:t>stale unquoted custom toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_quoted_custom_style_no_result_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="BodyText"><w:name w:val="Body Text"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="BodyText"/></w:pPr><w:r><w:t>Body paragraph</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \t &quot;Custom Heading,2&quot; "><w:r><w:t>stale quoted custom toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \s chapter \d &quot;-&quot; "><w:r><w:t>stale sequence prefix toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \c &quot;Figure List&quot; "><w:r><w:t>stale malformed caption toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \t &quot;Custom Heading,2 "><w:r><w:t>stale malformed style toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_no_page_numbers_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \n &quot;1-2&quot; "><w:r><w:t>stale no-page toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \t &quot;CustomHeading,1&quot; "><w:r><w:t>stale advanced toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_entry_page_separator_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \p &quot;-&quot; "><w:r><w:t>stale separator toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \t &quot;CustomHeading,1&quot; "><w:r><w:t>stale advanced toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_sequence_page_separator_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \s chapter \d &quot;-&quot; "><w:r><w:t>stale sequence separator toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_outline_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Style Heading</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Outline Heading</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \u "><w:r><w:t>stale outline toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; \t &quot;CustomHeading,1&quot; "><w:r><w:t>stale advanced toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_bookmark_scope_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="ScopedToc"/><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Scoped Heading</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:bookmarkStart w:id="8" w:name="EmptyScope"/><w:r><w:t>No headings here</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; \b ScopedToc "><w:r><w:t>stale scoped toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \b EmptyScope "><w:r><w:t>stale empty scope toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; \b MissingScope "><w:r><w:t>stale missing scope toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_bookmark_scoped_tc_field_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TC &quot;Outside Manual&quot; \f m \l 1 "><w:r><w:t>stale outside tc</w:t></w:r></w:fldSimple></w:p><w:p><w:bookmarkStart w:id="7" w:name="ScopedToc"/><w:fldSimple w:instr=" TC &quot;Scoped Manual&quot; \f m \l 2 "><w:r><w:t>stale scoped tc</w:t></w:r></w:fldSimple><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" TOC \b ScopedToc \f m "><w:r><w:t>stale scoped tc toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_scope_gap_diagnostics_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="PlainText"/><w:r><w:t>Plain target</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" TOC \b PlainText "><w:r><w:t>cached empty toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \b MissingScope "><w:r><w:t>cached missing toc scope</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_bookmark_only_scope_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7" w:name="ScopedToc"/><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Scoped Heading</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Scoped Detail</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" TOC \b ScopedToc "><w:r><w:t>stale bookmark-only toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn compact_toc_operand_switch_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Outside Heading</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="7" w:name="ScopedToc"/><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Scoped Heading</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Scoped Detail</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:fldSimple w:instr=" TC &quot;Manual Entry&quot; \fm \l2 "/></w:p><w:p><w:fldSimple w:instr=" TOC \fm \l2-3 "><w:r><w:t>stale compact tc toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOC \o&quot;1-2&quot; \bScopedToc \*Upper "><w:r><w:t>stale compact scoped toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn toc_outline_without_range_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading4"/></w:pPr><w:r><w:t>Appendix Detail</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC \o "><w:r><w:t>stale open-outline toc</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
fn bare_toc_field_diagnostics_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="2"/></w:pPr><w:r><w:t>Mitigation</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" TOC "><w:r><w:t>stale bare toc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" CUSTOM value "><w:r><w:t>custom</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_docx_feature_markers() {
    let doc = Document::open(&complex_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.comments, 1);
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Page,
            count: 1
        }]
    );
    assert_eq!(report.features.hyperlinks, 1);
    assert_eq!(report.features.tracked_insertions, 1);
    assert_eq!(report.features.tracked_deletions, 1);
    assert_eq!(report.features.tracked_moves, 1);
    assert_eq!(report.features.tracked_property_changes, 1);
    assert_eq!(report.features.content_controls, 1);
    assert_eq!(report.features.nested_tables, 1);
    assert_eq!(report.features.floating_shapes, 2);
    assert_eq!(report.features.charts, 1);
    assert_eq!(report.features.ole_objects, 1);
    assert_eq!(report.features.unsupported_metafiles, 1);
    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        DocumentWarning::IncompleteRevisionView {
            property_changes: 1
        }
    )));
    assert_eq!(report.warnings.len(), 6);
    assert!(
        report.to_json().contains(r#""nested_tables":1"#),
        "{}",
        report.to_json()
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_table_cell_revision_markers() {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:cellIns w:id="1" w:author="A" w:date="2024-01-01T00:00:00Z"/></w:tcPr><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:cellDel w:id="2" w:author="A" w:date="2024-01-01T00:00:00Z"/><w:cellMerge w:vMerge="rest" w:id="3" w:author="A" w:date="2024-01-01T00:00:00Z"/></w:tcPr><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&fixture).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.tracked_property_changes, 3);
    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        DocumentWarning::IncompleteRevisionView {
            property_changes: 3
        }
    )));
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_feature_markers_in_docx_story_parts() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:sdt><w:sdtPr><w:tag w:val="body"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:sdt><w:sdtPr><w:tag w:val="foot"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Foot</w:t></w:r></w:p></w:sdtContent></w:sdt></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="2"><w:sdt><w:sdtPr><w:tag w:val="end"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>End</w:t></w:r></w:p></w:sdtContent></w:sdt><w:tbl><w:tr><w:tc><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Nested</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote></w:endnotes>"#,
        ),
        (
            "word/header1.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:sdt><w:sdtPr><w:tag w:val="header"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Header</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:pPr><w:pPrChange w:id="9"><w:pPr><w:jc w:val="right"/></w:pPr></w:pPrChange></w:pPr><w:r><w:object/><w:drawing><c:chart/></w:drawing></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing/></mc:Choice></mc:AlternateContent></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:commentReference w:id="5"/></w:r></w:p></w:hdr>"#,
        ),
        (
            "word/header2.xml",
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:tag w:val="orphan"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Orphan</w:t></w:r></w:p></w:sdtContent></w:sdt></w:hdr>"#,
        ),
    ]))
    .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.content_controls, 4);
    assert_eq!(report.features.nested_tables, 1);
    assert_eq!(report.features.ole_objects, 1);
    assert_eq!(report.features.charts, 1);
    assert_eq!(report.features.floating_shapes, 1);
    assert_eq!(report.features.fields, 1);
    assert_eq!(report.features.tracked_property_changes, 1);
    assert_eq!(report.features.comments, 1);
    assert!(
        report.to_json().contains(r#""content_controls":4"#),
        "{}",
        report.to_json()
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_alternate_content_floating_shape_once() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor relativeHeight="51"><wp:extent cx="914400" cy="457200"/><wp:docPr id="51" name="Choice report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Choice body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:drawing><wp:anchor relativeHeight="52"><wp:extent cx="914400" cy="457200"/><wp:docPr id="52" name="Fallback report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Fallback body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></mc:Fallback></mc:AlternateContent></w:r></w:p></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");

    assert_eq!(doc.floating_shapes().len(), 1);
    assert_eq!(doc.report().features.floating_shapes, 1);
}

#[cfg(feature = "docx")]
#[test]
fn report_floating_shapes_follow_accepted_revision_view() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:drawing><wp:anchor relativeHeight="61"><wp:extent cx="914400" cy="457200"/><wp:docPr id="61" name="Direct report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Direct body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p><w:ins w:id="62" w:author="Editor"><w:p><w:r><w:drawing><wp:anchor relativeHeight="62"><wp:extent cx="914400" cy="457200"/><wp:docPr id="62" name="Inserted report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Inserted body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:ins><w:moveTo w:id="63" w:author="Editor"><w:p><w:r><w:drawing><wp:anchor relativeHeight="63"><wp:extent cx="914400" cy="457200"/><wp:docPr id="63" name="Moved-to report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Moved-to body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:moveTo><w:del w:id="64" w:author="Editor"><w:p><w:r><w:drawing><wp:anchor relativeHeight="64"><wp:extent cx="914400" cy="457200"/><wp:docPr id="64" name="Deleted report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Deleted body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:del><w:moveFrom w:id="65" w:author="Editor"><w:p><w:r><w:drawing><wp:anchor relativeHeight="65"><wp:extent cx="914400" cy="457200"/><wp:docPr id="65" name="Moved-from report float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Moved-from body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:moveFrom></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");
    let report = doc.report();

    assert_eq!(doc.floating_shapes().len(), 3);
    assert_eq!(report.features.floating_shapes, 3);
    assert_eq!(report.features.tracked_insertions, 1);
    assert_eq!(report.features.tracked_deletions, 1);
    assert_eq!(report.features.tracked_moves, 2);
}

#[cfg(feature = "docx")]
#[test]
fn report_chart_and_ole_markers_follow_accepted_revision_view() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><w:body><w:p><w:r><w:drawing><c:chart/></w:drawing><w:object/></w:r></w:p><w:del w:id="71" w:author="Editor"><w:r><w:drawing><c:chart/></w:drawing><w:object/></w:r></w:del><w:moveFrom w:id="72" w:author="Editor"><w:r><w:drawing><c:chart/></w:drawing><w:object/></w:r></w:moveFrom></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.charts, 1);
    assert_eq!(report.features.ole_objects, 1);
    assert_eq!(report.features.tracked_deletions, 1);
    assert_eq!(report.features.tracked_moves, 1);
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_chart_payload_parts_without_style_companions() {
    let doc = Document::open(&docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/><Override PartName="/word/charts/chartEx1.xml" ContentType="application/vnd.ms-office.chartex+xml"/><Override PartName="/word/charts/style1.xml" ContentType="application/vnd.ms-office.chartstyle+xml"/><Override PartName="/word/charts/colors1.xml" ContentType="application/vnd.ms-office.chartcolorstyle+xml"/><Override PartName="/word/charts/chartStyle1.xml" ContentType="application/vnd.ms-office.chartstyle+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><w:body><w:p><w:r><w:drawing><c:chart/></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
        ("word/charts/chart1.xml", r#"<c:chartSpace/>"#),
        ("word/charts/chartEx1.xml", r#"<cx:chartSpace/>"#),
        ("word/charts/style1.xml", r#"<cs:chartStyle/>"#),
        ("word/charts/colors1.xml", r#"<cs:colorStyle/>"#),
        ("word/charts/chartStyle1.xml", r#"<cs:chartStyle/>"#),
    ]))
    .expect("fixture opens");

    assert_eq!(doc.report().features.charts, 2);
}

#[cfg(feature = "docx")]
#[test]
fn report_field_markers_follow_accepted_revision_view() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p><w:del w:id="73" w:author="Editor"><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>deleted page</w:t></w:r></w:fldSimple></w:p></w:del><w:moveFrom w:id="74" w:author="Editor"><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>moved-from page</w:t></w:r></w:fldSimple></w:p></w:moveFrom></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");
    let report = doc.report();

    assert_eq!(doc.fields().len(), 1);
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Page,
            count: 1
        }]
    );
    assert_eq!(report.features.tracked_deletions, 1);
    assert_eq!(report.features.tracked_moves, 1);
}

#[cfg(feature = "docx")]
#[test]
fn report_omits_old_only_alternate_content_floating_shape_marker() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing/></mc:Choice></mc:AlternateContent></w:r></w:p><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:del w:id="70" w:author="Editor"><w:r><w:drawing/></w:r></w:del></mc:Choice></mc:AlternateContent></w:r></w:p></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.floating_shapes, 1);
    assert_eq!(report.features.tracked_deletions, 1);
}

#[cfg(feature = "docx")]
#[test]
fn report_exposes_metafile_metadata() {
    let doc = Document::open(&metafile_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.unsupported_metafiles, 4);
    assert_eq!(report.features.metafiles.len(), 4);

    let emf = report
        .features
        .metafiles
        .iter()
        .find(|info| info.path == "word/media/pic1.emf")
        .expect("EMF metadata");
    assert_eq!(emf.format, MetafileFormat::Emf);
    assert!(!emf.compressed);
    assert_eq!(emf.bytes, 88);
    assert_eq!(emf.width_px, Some(640));
    assert_eq!(emf.height_px, Some(480));

    let wmf = report
        .features
        .metafiles
        .iter()
        .find(|info| info.path == "word/media/pic2.wmf")
        .expect("WMF metadata");
    assert_eq!(wmf.format, MetafileFormat::Wmf);
    assert!(!wmf.compressed);
    assert_eq!(wmf.width_px, Some(96));
    assert_eq!(wmf.height_px, Some(48));

    let emz = report
        .features
        .metafiles
        .iter()
        .find(|info| info.path == "word/media/pic3.emz")
        .expect("compressed EMF metadata");
    assert_eq!(emz.format, MetafileFormat::Emf);
    assert!(emz.compressed);
    assert_eq!(emz.bytes, 51);
    assert_eq!(emz.width_px, Some(320));
    assert_eq!(emz.height_px, Some(240));

    let wmz = report
        .features
        .metafiles
        .iter()
        .find(|info| info.path == "word/media/pic4.wmz")
        .expect("compressed WMF metadata");
    assert_eq!(wmz.format, MetafileFormat::Wmf);
    assert!(wmz.compressed);
    assert_eq!(wmz.bytes, 45);
    assert_eq!(wmz.width_px, Some(192));
    assert_eq!(wmz.height_px, Some(96));

    let json = report.to_json();
    assert!(
        json.contains(r#""metafiles":[{"path":"word/media/pic1.emf","format":"EMF","bytes":88,"compressed":false,"width_px":640,"height_px":480}"#),
        "{json}"
    );
    assert!(
        json.contains(r#"{"path":"word/media/pic2.wmf","format":"WMF","bytes":40,"compressed":false,"width_px":96,"height_px":48}"#),
        "{json}"
    );
    assert!(
        json.contains(r#"{"path":"word/media/pic3.emz","format":"EMF","bytes":51,"compressed":true,"width_px":320,"height_px":240}"#),
        "{json}"
    );
    assert!(
        json.contains(r#"{"path":"word/media/pic4.wmz","format":"WMF","bytes":45,"compressed":true,"width_px":192,"height_px":96}"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_extracts_single_dib_metafiles_as_images() {
    for (name, bytes) in [
        ("pic.emf", sample_emf_with_stretchdibits()),
        ("pic.wmf", sample_placeable_wmf_with_stretchdib()),
        ("pic.emz", gzip_bytes(&sample_emf_with_stretchdibits())),
        (
            "pic.wmz",
            gzip_bytes(&sample_placeable_wmf_with_stretchdib()),
        ),
    ] {
        let doc = Document::open(&metafile_image_docx(name, "rIdMeta", bytes))
            .expect("metafile image fixture opens");
        let images = doc.images();
        assert_eq!(images.len(), 1, "{name} should extract a raster image");
        assert_eq!(images[0].width_px, Some(2), "{name}");
        assert_eq!(images[0].height_px, Some(2), "{name}");
        assert_eq!(
            images[0].bytes.as_deref(),
            Some(expected_rgba_2x2_pixels().as_slice()),
            "{name} pixels"
        );

        let report = doc.report();
        assert_eq!(report.features.metafiles.len(), 1, "{name}");
        assert_eq!(report.features.unsupported_metafiles, 0, "{name}");
        assert!(
            report.warnings.iter().all(|warning| !matches!(
                warning,
                DocumentWarning::UnsupportedMetafileImages { .. }
            )),
            "{name}: {:?}",
            report.warnings
        );
    }
}

#[cfg(feature = "docx")]
#[test]
fn report_extracts_palette_and_bitfield_metafiles_as_images() {
    let palette = sample_palette8_dib();
    let bitfields = sample_rgb565_dib();
    for (name, bytes) in [
        ("palette.emf", sample_emf_with_dib(&palette, 56)),
        ("palette.wmf", sample_placeable_wmf_with_dib(&palette)),
        ("bitfields.emf", sample_emf_with_dib(&bitfields, 52)),
        ("bitfields.wmf", sample_placeable_wmf_with_dib(&bitfields)),
    ] {
        let doc = Document::open(&metafile_image_docx(name, "rIdMeta", bytes))
            .expect("metafile image fixture opens");
        let images = doc.images();
        assert_eq!(images.len(), 1, "{name}");
        assert_eq!(images[0].width_px, Some(2), "{name}");
        assert_eq!(images[0].height_px, Some(2), "{name}");
        assert_eq!(
            images[0].bytes.as_deref(),
            Some(expected_rgba_2x2_pixels().as_slice()),
            "{name} pixels"
        );
        assert_eq!(doc.report().features.unsupported_metafiles, 0, "{name}");
    }
}

#[cfg(feature = "docx")]
#[test]
fn report_rejects_single_dib_metafiles_with_vector_composition() {
    for (name, bytes) in [
        ("composed.emf", sample_emf_with_dib_and_vector_overlay()),
        ("composed.wmf", sample_wmf_with_dib_and_vector_overlay()),
    ] {
        let doc = Document::open(&metafile_image_docx(name, "rIdMeta", bytes))
            .expect("composed metafile fixture opens");
        assert!(doc.images().is_empty(), "{name}");
        let report = doc.report();
        assert_eq!(report.features.unsupported_metafiles, 1, "{name}");
        assert!(report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::UnsupportedMetafileImages { count: 1 }
        )));
    }
}

#[cfg(all(feature = "docx", feature = "render"))]
#[test]
fn render_report_omits_unsupported_warning_for_extracted_metafile_raster() {
    let doc = Document::open(&metafile_image_docx(
        "pic.emf",
        "rIdMeta",
        sample_emf_with_stretchdibits(),
    ))
    .expect("metafile image fixture opens");

    let rendered = doc.to_pdf_with_report();

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert!(
        rendered.report.warnings.iter().all(|warning| !matches!(
            warning,
            rwml::RenderWarning::UnsupportedMetafileImages { .. }
        )),
        "{:?}",
        rendered.report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_keeps_vector_only_metafiles_unsupported() {
    let doc = Document::open(&metafile_image_docx("pic.emf", "rIdMeta", sample_emf(2, 2)))
        .expect("vector-only metafile fixture opens");

    assert!(doc.images().is_empty());
    let report = doc.report();
    assert_eq!(report.features.metafiles.len(), 1);
    assert_eq!(report.features.unsupported_metafiles, 1);
    assert!(report.warnings.iter().any(|warning| {
        matches!(
            warning,
            DocumentWarning::UnsupportedMetafileImages { count: 1 }
        )
    }));
}

#[cfg(all(feature = "docx", feature = "render"))]
#[test]
fn render_report_keeps_vector_only_metafile_warning() {
    let doc = Document::open(&metafile_image_docx("pic.emf", "rIdMeta", sample_emf(2, 2)))
        .expect("vector-only metafile fixture opens");

    let rendered = doc.to_pdf_with_report();

    assert!(rendered.report.warnings.iter().any(|warning| {
        matches!(
            warning,
            rwml::RenderWarning::UnsupportedMetafileImages { count: 1 }
        )
    }));
}

#[cfg(feature = "docx")]
#[test]
fn malformed_metafile_fixtures_do_not_panic_or_extract() {
    for (name, bytes) in [
        ("truncated.emf", vec![1, 0, 0, 0, 88]),
        ("truncated.wmf", vec![0xD7, 0xCD, 0xC6, 0x9A, 0]),
    ] {
        let doc = Document::open(&metafile_image_docx(name, "rIdMeta", bytes))
            .expect("malformed metafile fixture still opens");
        assert!(doc.images().is_empty(), "{name}");
        let report = doc.report();
        assert_eq!(report.features.metafiles.len(), 1, "{name}");
        assert_eq!(report.features.unsupported_metafiles, 1, "{name}");
    }
}

#[cfg(feature = "docx")]
#[test]
fn report_field_evaluation_warning_only_lists_unsupported_kinds() {
    let doc = Document::open(&field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 6);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Page,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Filename,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Hyperlink,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Ref,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Unknown("CUSTOM".to_string()),
                count: 1,
            },
        ]
    );

    let warning = report
        .warnings
        .iter()
        .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
        .expect("unsupported field warning");
    assert_eq!(
        warning,
        &DocumentWarning::UnsupportedFieldEvaluation {
            count: 3,
            field_kinds: vec![
                FieldKindCount {
                    kind: FieldKind::Toc,
                    count: 1,
                },
                FieldKindCount {
                    kind: FieldKind::Ref,
                    count: 1,
                },
                FieldKindCount {
                    kind: FieldKind::Unknown("CUSTOM".to_string()),
                    count: 1,
                },
            ],
        }
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnknownField,
                count: 1,
            },
        ]
    );

    let json = report.to_json();
    assert!(json.contains(r#""field_kinds":[{"kind":"PAGE","count":1},{"kind":"FILENAME","count":1},{"kind":"HYPERLINK","count":1},{"kind":"TOC","count":1},{"kind":"REF","count":1},{"kind":"CUSTOM","count":1}]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":1},{"reason":"UnknownField","count":1}]"#), "{json}");
    assert!(json.contains(r#""kind":"UnsupportedFieldEvaluation","count":3,"field_kinds":[{"kind":"TOC","count":1},{"kind":"REF","count":1},{"kind":"CUSTOM","count":1}]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_hyperlink_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&hyperlink_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 6);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Hyperlink));

    let report = doc.report();

    assert_eq!(report.features.fields, 6);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Hyperlink,
            count: 6,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Hyperlink,
            count: 3,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 3,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_mixed_case_hyperlink_field_is_supported() {
    let doc = Document::open(&mixed_case_hyperlink_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Hyperlink);

    let report = doc.report();
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_page_field_warning_distinguishes_unsupported_switches() {
    let doc = Document::open(&page_unsupported_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Page,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_field_warning_distinguishes_unsupported_section_format() {
    let doc =
        Document::open(&page_unsupported_section_format_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Page);
    assert_eq!(fields[0].instruction, "PAGE");
    assert_eq!(fields[0].result, "stale unsupported current page");
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(fields[1].kind, FieldKind::Page);
    assert_eq!(fields[1].instruction, "PAGE \\* Arabic");
    assert_eq!(fields[1].result, "stale explicit current page");
    assert_eq!(fields[1].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Page, 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::UnsupportedSwitch,
            1
        )]
    );
}

#[cfg(feature = "render")]
#[test]
fn report_page_unsupported_section_format_model_render_report_matches_document_reason_bucket() {
    let doc =
        Document::open(&page_unsupported_section_format_diagnostics_docx()).expect("fixture opens");
    let expected_kinds = doc.report().features.unsupported_field_kinds;
    let expected_reasons = doc.report().features.unsupported_field_reasons;

    assert_eq!(expected_kinds, vec![field_kind_count(FieldKind::Page, 1)]);
    assert_eq!(
        expected_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::UnsupportedSwitch,
            1
        )]
    );

    let rendered = rwml::render_pdf_with_report(&doc.model());

    assert_eq!(rendered.report.unsupported.fields, 1);
    assert_eq!(rendered.report.unsupported.field_kinds, expected_kinds);
    assert_eq!(
        rendered.report.unsupported.unsupported_field_reasons,
        expected_reasons
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_fields_split_trusted_and_ambiguous_current_contexts() {
    let doc =
        Document::open(&page_trusted_current_context_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("PAGE", Some("04")),
        ("PAGE \\* Arabic", Some("4")),
        ("PAGE \\* CardText \\* Upper", Some("FOUR")),
        ("PAGE", None),
        ("PAGE \\* roman", Some("v")),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(field.kind, FieldKind::Page);
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), computed);
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Page, 5)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Page, 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![field_kind_count(FieldKind::Page, 1)],
        })
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("04")
            && main_text.contains("FOUR")
            && main_text.contains("cached ambiguous page")
            && main_text.contains("v"),
        "trusted PAGE fields should use computed text while the ambiguous field keeps its cached text: {main_text:?}"
    );
    assert!(
        !main_text.contains("stale restart decimal zero")
            && !main_text.contains("stale restart arabic")
            && !main_text.contains("stale restart card upper")
            && !main_text.contains("stale rendered roman page"),
        "computed PAGE fields should replace stale cached text in report fixtures: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_fields_follow_accepted_wrappers_and_single_alternate_branch() {
    let doc =
        Document::open(&page_accepted_current_context_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [("PAGE \\* Arabic", "2"), ("PAGE \\* Ordinal", "3rd")];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(field.kind, FieldKind::Page);
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Page, 2)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("2") && main_text.contains("3rd"),
        "accepted PAGE fields should be materialized in report fixtures: {main_text:?}"
    );
    assert!(
        !main_text.contains("stale inserted page") && !main_text.contains("stale alternate page"),
        "accepted PAGE report diagnostics should use computed text: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_sectionpages_unsupported_switch_reports_unsupported_switch() {
    let doc = Document::open(&section_pages_unsupported_switch_diagnostics_docx())
        .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::DocumentStructure("SECTIONPAGES".to_string()),
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_revnum_unsupported_switch_reports_unsupported_switch() {
    let doc = Document::open(&revnum_unsupported_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::DocumentStructure("REVNUM".to_string()),
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_filename_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&filename_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Filename));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Filename,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Filename,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_document_info_fields_split_cached_display_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
        document_info_field_diagnostics_docx(),
        7,
        vec![
            field_kind_count(FieldKind::DocumentInfo("DATE".to_string()), 1),
            field_kind_count(FieldKind::DocumentInfo("TIME".to_string()), 1),
            field_kind_count(FieldKind::DocumentInfo("AUTHOR".to_string()), 1),
            field_kind_count(FieldKind::DocumentInfo("DOCPROPERTY".to_string()), 2),
            field_kind_count(FieldKind::DocumentInfo("NUMPAGES".to_string()), 1),
            field_kind_count(FieldKind::DocumentInfo("EDITTIME".to_string()), 1),
        ],
        vec![FieldKindCount {
            kind: FieldKind::DocumentInfo("DOCPROPERTY".to_string()),
            count: 1,
        }],
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_document_info_compact_date_format_switch_is_supported() {
    assert_report_field_diagnostics(
        document_info_compact_date_format_diagnostics_docx(),
        1,
        vec![field_kind_count(
            FieldKind::DocumentInfo("CREATEDATE".to_string()),
            1,
        )],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_document_info_unquoted_date_picture_is_supported() {
    assert_report_field_diagnostics(
        document_info_unquoted_date_picture_diagnostics_docx(),
        1,
        vec![field_kind_count(
            FieldKind::DocumentInfo("CREATEDATE".to_string()),
            1,
        )],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_document_info_user_override_compact_format_switch_is_supported() {
    assert_report_field_diagnostics(
        user_info_compact_format_diagnostics_docx(),
        1,
        vec![field_kind_count(
            FieldKind::DocumentInfo("USERNAME".to_string()),
            1,
        )],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_document_info_property_compact_format_switch_is_supported() {
    assert_report_field_diagnostics(
        document_info_compact_property_format_diagnostics_docx(),
        1,
        vec![field_kind_count(
            FieldKind::DocumentInfo("DOCPROPERTY".to_string()),
            1,
        )],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_document_info_variable_compact_format_switch_is_supported() {
    assert_report_field_diagnostics(
        document_info_compact_variable_format_diagnostics_docx(),
        1,
        vec![field_kind_count(
            FieldKind::DocumentInfo("DOCVARIABLE".to_string()),
            1,
        )],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_display_layout_fields_split_cached_and_malformed_diagnostics() {
    let doc = Document::open(&display_layout_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 8);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Display("ADVANCE".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Display("EQ".to_string()),
                count: 4,
            },
            FieldKindCount {
                kind: FieldKind::Display("SYMBOL".to_string()),
                count: 3,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Display("EQ".to_string()),
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Display("SYMBOL".to_string()),
                count: 1,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 3,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_eq_fields_accept_deterministic_display_subset() {
    assert_report_field_diagnostics(
        deterministic_eq_diagnostics_docx(),
        8,
        vec![field_kind_count(FieldKind::Display("EQ".to_string()), 8)],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_advance_fields_accept_compact_and_quoted_point_switches() {
    let doc = Document::open(&advance_compact_quoted_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        r#"ADVANCE \r"2""#,
        r#"ADVANCE \u "3" \d4"#,
        r#"ADVANCE \l2 \y "5" \* Upper"#,
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, instruction) in fields.iter().zip(expected) {
        assert_eq!(field.kind, FieldKind::Display("ADVANCE".to_string()));
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(""));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Display("ADVANCE".to_string()),
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_symbol_fields_accept_compact_and_unicode_switches() {
    let doc = Document::open(&symbol_compact_unicode_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    for field in &fields {
        assert_eq!(field.kind, FieldKind::Display("SYMBOL".to_string()));
    }
    assert_eq!(fields[0].instruction, "SYMBOL 183 \\fSymbol \\s12");
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{2022}"));
    assert_eq!(fields[1].instruction, "SYMBOL 0x03BB \\u \\f Symbol");
    assert_eq!(fields[1].computed_result.as_deref(), Some("\u{03bb}"));
    assert_eq!(fields[2].instruction, "SYMBOL 0x0041 \\u \\s \"10\"");
    assert_eq!(fields[2].computed_result.as_deref(), Some("A"));
    assert_eq!(fields[3].instruction, "SYMBOL 0x0042 \\u \\s\"11\"");
    assert_eq!(fields[3].computed_result.as_deref(), Some("B"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Display("SYMBOL".to_string()),
            count: 4,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_symbol_fields_accept_multi_token_font_switches() {
    let doc = Document::open(&symbol_multi_token_font_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Display("SYMBOL".to_string()));
    assert_eq!(
        fields[0].instruction,
        "SYMBOL 0x03BB \\u \\f Times New Roman \\* Upper"
    );
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{039B}"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Display("SYMBOL".to_string()),
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_action_fields_split_cached_and_malformed_diagnostics() {
    let doc = Document::open(&action_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 11);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Action("GOTOBUTTON".to_string()),
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Action("MACROBUTTON".to_string()),
                count: 3,
            },
            FieldKindCount {
                kind: FieldKind::Action("PRINT".to_string()),
                count: 6,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Action("MACROBUTTON".to_string()),
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Action("PRINT".to_string()),
                count: 1,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 2,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
        ]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_action_print_groups_accept_unquoted_multi_token_code() {
    assert_report_field_diagnostics(
        action_print_group_unquoted_code_diagnostics_docx(),
        2,
        vec![field_kind_count(FieldKind::Action("PRINT".to_string()), 2)],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_action_field_accepts_compact_neutral_format_switch() {
    assert_report_field_diagnostics(
        action_compact_format_diagnostics_docx(),
        1,
        vec![field_kind_count(FieldKind::Action("PRINT".to_string()), 1)],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_button_action_fields_accept_compact_format_switches() {
    assert_report_field_diagnostics(
        action_button_compact_format_diagnostics_docx(),
        2,
        vec![
            field_kind_count(FieldKind::Action("GOTOBUTTON".to_string()), 1),
            field_kind_count(FieldKind::Action("MACROBUTTON".to_string()), 1),
        ],
        vec![field_kind_count(
            FieldKind::Action("MACROBUTTON".to_string()),
            1,
        )],
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1,
        )],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_action_fields_follow_accepted_current_view() {
    assert_report_field_diagnostics(
        action_accepted_current_diagnostics_docx(),
        2,
        vec![
            field_kind_count(FieldKind::Action("GOTOBUTTON".to_string()), 1),
            field_kind_count(FieldKind::Action("PRINT".to_string()), 1),
        ],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_inserted_content_fields_split_cached_and_malformed_diagnostics() {
    let doc = Document::open(&inserted_content_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    assert_eq!(
        fields[0].kind,
        FieldKind::InsertedContent("INCLUDETEXT".to_string())
    );
    assert_eq!(fields[0].instruction, r#"INCLUDETEXT "appendix.docx""#);
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(
        fields[1].kind,
        FieldKind::InsertedContent("INCLUDEPICTURE".to_string())
    );
    assert_eq!(fields[1].instruction, r#"INCLUDEPICTURE "chart.png "#);
    assert_eq!(fields[1].computed_result, None);
    assert_eq!(
        fields[2].kind,
        FieldKind::InsertedContent("LINK".to_string())
    );
    assert_eq!(fields[2].instruction, r#"LINK \*"#);
    assert_eq!(fields[2].computed_result, None);
    assert_eq!(
        fields[3].kind,
        FieldKind::InsertedContent("INCLUDETEXT".to_string())
    );
    assert_eq!(
        fields[3].instruction,
        r#"INCLUDETEXT "chapter.docx" \* BadFormat"#
    );
    assert_eq!(fields[3].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::InsertedContent("INCLUDETEXT".to_string()), 2),
            field_kind_count(FieldKind::InsertedContent("INCLUDEPICTURE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("LINK".to_string()), 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        report.features.field_kinds
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 1),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 3),
        ]
    );
    assert!(report
        .warnings
        .iter()
        .any(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(main_text.contains("Appendix text"), "{main_text:?}");
    assert!(
        main_text.contains("cached malformed include picture"),
        "{main_text:?}"
    );
    assert!(
        main_text.contains("cached dangling format switch"),
        "{main_text:?}"
    );
    assert!(
        main_text.contains("cached bad include format"),
        "{main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_mail_merge_helpers_split_cached_and_malformed_diagnostics() {
    let doc = Document::open(&mail_merge_helper_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].kind,
        FieldKind::MailMerge("ADDRESSBLOCK".to_string())
    );
    assert_eq!(fields[0].instruction, "ADDRESSBLOCK");
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(
        fields[1].kind,
        FieldKind::MailMerge("GREETINGLINE".to_string())
    );
    assert_eq!(fields[1].instruction, r#"GREETINGLINE "Dear "#);
    assert_eq!(fields[1].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::MailMerge("ADDRESSBLOCK".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("GREETINGLINE".to_string()), 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        report.features.field_kinds
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 1),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ]
    );
    assert!(report
        .warnings
        .iter()
        .any(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(main_text.contains("Acme Corp"), "{main_text:?}");
    assert!(
        main_text.contains("cached malformed greeting"),
        "{main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_reference_index_generated_fields_split_cached_and_malformed_diagnostics() {
    let doc = Document::open(&reference_index_generated_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].kind,
        FieldKind::ReferenceIndex("BIBLIOGRAPHY".to_string())
    );
    assert_eq!(fields[0].instruction, r#"BIBLIOGRAPHY \l 1033"#);
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(
        fields[1].kind,
        FieldKind::ReferenceIndex("INDEX".to_string())
    );
    assert_eq!(fields[1].instruction, r#"INDEX "bad "#);
    assert_eq!(fields[1].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::ReferenceIndex("BIBLIOGRAPHY".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("INDEX".to_string()), 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        report.features.field_kinds
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 1),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ]
    );
    assert!(report
        .warnings
        .iter()
        .any(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(main_text.contains("Works cited"), "{main_text:?}");
    assert!(
        main_text.contains("cached malformed index"),
        "{main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_reference_index_markers_accept_unquoted_multi_token_switch_operands() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" TA \l Case v. Example \c 1 "><w:r><w:t>cached unquoted ta marker</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" XE &quot;Mercury&quot; \t See planets \* FirstCap "><w:r><w:t>cached unquoted xe marker</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
        ),
    ]))
    .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::ReferenceIndex("TA".to_string()));
    assert_eq!(fields[0].instruction, "TA \\l Case v. Example \\c 1");
    assert_eq!(fields[0].computed_result.as_deref(), Some(""));
    assert_eq!(fields[1].kind, FieldKind::ReferenceIndex("XE".to_string()));
    assert_eq!(
        fields[1].instruction,
        "XE \"Mercury\" \\t See planets \\* FirstCap"
    );
    assert_eq!(fields[1].computed_result.as_deref(), Some(""));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::ReferenceIndex("TA".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("XE".to_string()), 1),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(
        !main_text.contains("cached unquoted ta marker")
            && !main_text.contains("cached unquoted xe marker"),
        "{main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_compatibility_fields_split_cached_and_malformed_diagnostics() {
    let doc = Document::open(&compatibility_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Compatibility("PRIVATE".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("DATA".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("GLOSSARY".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("HTMLACTIVEX".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("ADDIN".to_string()),
                count: 1,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Compatibility("PRIVATE".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("DATA".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("GLOSSARY".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("HTMLACTIVEX".to_string()),
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Compatibility("ADDIN".to_string()),
                count: 1,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 4,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 1,
            },
        ]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_opaque_fields_accept_compact_format_tails() {
    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" INCLUDETEXT &quot;appendix.docx&quot; \*MERGEFORMAT "><w:r><w:t>Appendix text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ADDRESSBLOCK \*CHARFORMAT "><w:r><w:t>Acme Corp</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INDEX \c &quot;2&quot; \*Lower "><w:r><w:t>Index preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" PRIVATE payload \*Upper "><w:r><w:t>cached private payload</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" ADDIN payload \*BadFormat "><w:r><w:t>cached bad addin payload</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        5,
        vec![
            field_kind_count(FieldKind::InsertedContent("INCLUDETEXT".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("ADDRESSBLOCK".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("INDEX".to_string()), 1),
            field_kind_count(FieldKind::Compatibility("PRIVATE".to_string()), 1),
            field_kind_count(FieldKind::Compatibility("ADDIN".to_string()), 1),
        ],
        vec![
            field_kind_count(FieldKind::InsertedContent("INCLUDETEXT".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("ADDRESSBLOCK".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("INDEX".to_string()), 1),
            field_kind_count(FieldKind::Compatibility("PRIVATE".to_string()), 1),
            field_kind_count(FieldKind::Compatibility("ADDIN".to_string()), 1),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 4),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_field_category_matrix_splits_cached_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" INCLUDETEXT &quot;appendix.docx&quot; "><w:r><w:t>Appendix text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" LINK Excel.Sheet.12 &quot;book.xlsx&quot; &quot;Sheet1!R1C1&quot; "><w:r><w:t>42</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" EMBED Excel.Sheet.12 "><w:r><w:t>Embedded object</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DATABASE \d &quot;source.accdb&quot; "><w:r><w:t>Rows</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DDE Excel &quot;book.xlsx&quot; &quot;R1C1&quot; "><w:r><w:t>DDE value</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DDEAUTO Excel &quot;book.xlsx&quot; &quot;R2C1&quot; "><w:r><w:t>Auto DDE value</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" IMPORT &quot;legacy.wmf&quot; "><w:r><w:t>Imported object</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INCLUDE &quot;legacy.doc&quot; "><w:r><w:t>Included text</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTOTEXT Signature "><w:r><w:t>AutoText signature</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" AUTOTEXTLIST &quot;Choose clause&quot; \s Legal "><w:r><w:t>AutoText list</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INCLUDEPICTURE &quot;chart.png "><w:r><w:t>cached malformed include picture</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        11,
        vec![
            field_kind_count(FieldKind::InsertedContent("INCLUDETEXT".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("LINK".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("EMBED".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("DATABASE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("DDE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("DDEAUTO".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("IMPORT".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("INCLUDE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("AUTOTEXT".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("AUTOTEXTLIST".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("INCLUDEPICTURE".to_string()), 1),
        ],
        vec![
            field_kind_count(FieldKind::InsertedContent("INCLUDETEXT".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("LINK".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("EMBED".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("DATABASE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("DDE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("DDEAUTO".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("IMPORT".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("INCLUDE".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("AUTOTEXT".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("AUTOTEXTLIST".to_string()), 1),
            field_kind_count(FieldKind::InsertedContent("INCLUDEPICTURE".to_string()), 1),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 10),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ],
    );

    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" ADDRESSBLOCK "><w:r><w:t>Acme Corp</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" GREETINGLINE "><w:r><w:t>Dear Hyunjo,</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEREC "><w:r><w:t>7</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGESEQ "><w:r><w:t>3</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" GREETINGLINE &quot;Dear "><w:r><w:t>cached malformed greeting</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        5,
        vec![
            field_kind_count(FieldKind::MailMerge("ADDRESSBLOCK".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("GREETINGLINE".to_string()), 2),
            field_kind_count(FieldKind::MailMerge("MERGEREC".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("MERGESEQ".to_string()), 1),
        ],
        vec![
            field_kind_count(FieldKind::MailMerge("ADDRESSBLOCK".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("GREETINGLINE".to_string()), 2),
            field_kind_count(FieldKind::MailMerge("MERGEREC".to_string()), 1),
            field_kind_count(FieldKind::MailMerge("MERGESEQ".to_string()), 1),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 4),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ],
    );

    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" BIBLIOGRAPHY \l 1033 "><w:r><w:t>Works cited</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" CITATION Smith2026 \l 1033 "><w:r><w:t>(Smith, 2026)</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" INDEX \e &quot; - &quot; "><w:r><w:t>Index preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TOA \c &quot;1&quot; "><w:r><w:t>Authorities</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TA \l &quot;Case v. Example&quot; \c 1 "><w:r><w:t>Case v. Example</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" XE &quot;Term&quot; "><w:r><w:t>Term</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" RD &quot;appendix.docx&quot; "><w:r><w:t>Referenced doc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TA \l&quot;Compact Case&quot; \c2 "><w:r><w:t>Compact Case</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TA \sShortEntry \c3 "><w:r><w:t>Short Entry</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" XE &quot;See Term&quot; \t&quot;See Also&quot; "><w:r><w:t>See Term</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" RD &quot;formatted-appendix.docx&quot; \*MERGEFORMAT "><w:r><w:t>Formatted referenced doc</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TA \l &quot;Formatted Case&quot; \c 1 \*CHARFORMAT "><w:r><w:t>Formatted Case</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" XE &quot;Formatted Term&quot; \*MERGEFORMAT "><w:r><w:t>Formatted Term</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" XE &quot;Duplicate Format&quot; \* Upper \* Lower "><w:r><w:t>Duplicate Format</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" TA \l &quot;Broken Case&quot; \c 99 "><w:r><w:t>Broken Case</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        15,
        vec![
            field_kind_count(FieldKind::ReferenceIndex("BIBLIOGRAPHY".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("CITATION".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("INDEX".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("TOA".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("TA".to_string()), 5),
            field_kind_count(FieldKind::ReferenceIndex("XE".to_string()), 4),
            field_kind_count(FieldKind::ReferenceIndex("RD".to_string()), 2),
        ],
        vec![
            field_kind_count(FieldKind::ReferenceIndex("BIBLIOGRAPHY".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("CITATION".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("INDEX".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("TOA".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("XE".to_string()), 1),
            field_kind_count(FieldKind::ReferenceIndex("TA".to_string()), 1),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 4),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 2),
        ],
    );

    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \q H "><w:r><w:t>QR preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEBARCODE CustomerId CODE128 \t "><w:r><w:t>Merge barcode preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" BARCODE &quot;9781234567890&quot; "><w:r><w:t>Legacy barcode preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \q "><w:r><w:t>cached missing quality operand</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        4,
        vec![
            field_kind_count(FieldKind::Barcode("DISPLAYBARCODE".to_string()), 2),
            field_kind_count(FieldKind::Barcode("MERGEBARCODE".to_string()), 1),
            field_kind_count(FieldKind::Barcode("BARCODE".to_string()), 1),
        ],
        vec![
            field_kind_count(FieldKind::Barcode("DISPLAYBARCODE".to_string()), 2),
            field_kind_count(FieldKind::Barcode("MERGEBARCODE".to_string()), 1),
            field_kind_count(FieldKind::Barcode("BARCODE".to_string()), 1),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 3),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ],
    );

    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;12345&quot; QR \qH "><w:r><w:t>Compact QR preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" MERGEBARCODE Zip JPPOST \h1440 \s100 \r1 \f0x000000 \bFFFFFF \t \a "><w:r><w:t>Compact merge barcode preview</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        2,
        vec![
            field_kind_count(FieldKind::Barcode("DISPLAYBARCODE".to_string()), 1),
            field_kind_count(FieldKind::Barcode("MERGEBARCODE".to_string()), 1),
        ],
        vec![
            field_kind_count(FieldKind::Barcode("DISPLAYBARCODE".to_string()), 1),
            field_kind_count(FieldKind::Barcode("MERGEBARCODE".to_string()), 1),
        ],
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            2,
        )],
    );

    assert_report_field_diagnostics(
        protected_form_field_diagnostics_docx(),
        4,
        vec![
            field_kind_count(FieldKind::FormField("FORMCHECKBOX".to_string()), 1),
            field_kind_count(FieldKind::FormField("FORMDROPDOWN".to_string()), 1),
            field_kind_count(FieldKind::FormField("FORMTEXT".to_string()), 2),
        ],
        vec![
            field_kind_count(FieldKind::FormField("FORMCHECKBOX".to_string()), 1),
            field_kind_count(FieldKind::FormField("FORMDROPDOWN".to_string()), 1),
            field_kind_count(FieldKind::FormField("FORMTEXT".to_string()), 2),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 3),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ],
    );

    assert_report_field_diagnostics(
        compact_form_field_diagnostics_docx(),
        2,
        vec![
            field_kind_count(FieldKind::FormField("FORMDROPDOWN".to_string()), 1),
            field_kind_count(FieldKind::FormField("FORMTEXT".to_string()), 1),
        ],
        vec![],
        vec![],
    );
}

#[test]
fn report_barcode_broader_syntax_splits_valid_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
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
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \q H "><w:r><w:t>QR preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \* Upper "><w:r><w:t>formatted QR preview</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; \q H "><w:r><w:t>cached missing barcode type</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \q "><w:r><w:t>cached missing quality operand</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com QR "><w:r><w:t>cached malformed barcode</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \* "><w:r><w:t>cached dangling barcode format</w:t></w:r></w:fldSimple></w:p><w:p><w:fldSimple w:instr=" DISPLAYBARCODE &quot;https://example.com&quot; QR \* BadFormat "><w:r><w:t>cached bad barcode format</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#,
            ),
        ]),
        7,
        vec![field_kind_count(
            FieldKind::Barcode("DISPLAYBARCODE".to_string()),
            7,
        )],
        vec![field_kind_count(
            FieldKind::Barcode("DISPLAYBARCODE".to_string()),
            7,
        )],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 2),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 5),
        ],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_legacy_form_spaced_dropdown_index_is_supported() {
    let doc = Document::open(&legacy_form_spaced_dropdown_index_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].kind,
        FieldKind::FormField("FORMDROPDOWN".to_string())
    );
    assert_eq!(fields[0].instruction, "FORMDROPDOWN");
    assert_eq!(fields[0].result, "stale spaced dropdown");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Chosen B"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::FormField("FORMDROPDOWN".to_string()),
            1
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Chosen B") && !main_text.contains("stale spaced dropdown"),
        "whitespace-padded dropdown indexes should stay supported in report diagnostics: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_legacy_form_alternate_content_uses_single_branch() {
    let doc =
        Document::open(&legacy_form_alternate_content_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].kind,
        FieldKind::FormField("FORMDROPDOWN".to_string())
    );
    assert_eq!(fields[0].instruction, "FORMDROPDOWN");
    assert_eq!(fields[0].result, "stale option");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Choice option"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::FormField("FORMDROPDOWN".to_string()),
            1
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Choice option")
            && !main_text.contains("Fallback option")
            && !main_text.contains("fallback checked"),
        "legacy form report diagnostics should use one AlternateContent branch: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_legacy_form_deleted_context_ignores_deleted_fields() {
    let doc =
        Document::open(&legacy_form_deleted_context_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].kind,
        FieldKind::FormField("FORMCHECKBOX".to_string())
    );
    assert_eq!(fields[0].instruction, "FORMCHECKBOX");
    assert_eq!(fields[0].result, "stale visible unchecked");
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{2610}"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::FormField("FORMCHECKBOX".to_string()),
            1
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(
        main_text.contains('\u{2610}') && !main_text.contains("deleted checked"),
        "deleted legacy form fields should stay out of report diagnostics: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_non_body_legacy_form_fields_are_supported_when_unprotected() {
    let doc = Document::open(&non_body_form_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().any(|field| {
        field.kind == FieldKind::FormField("FORMCHECKBOX".to_string())
            && field.result == "stale header checkbox"
            && field.computed_result.as_deref() == Some("\u{2612}")
    }));
    assert!(fields.iter().any(|field| {
        field.kind == FieldKind::FormField("FORMDROPDOWN".to_string())
            && field.result == "stale footnote option"
            && field.computed_result.as_deref() == Some("Option B")
    }));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::FormField("FORMDROPDOWN".to_string()), 1),
            field_kind_count(FieldKind::FormField("FORMCHECKBOX".to_string()), 1),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
    assert!(doc.header_text().contains('\u{2612}'));
    assert!(doc.footnote_text().contains("Option B"));
}

#[cfg(feature = "docx")]
#[test]
fn report_numbering_fields_split_cached_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
        numbering_field_diagnostics_docx(),
        14,
        vec![
            field_kind_count(FieldKind::Numbering("AUTONUM".to_string()), 7),
            field_kind_count(FieldKind::Numbering("AUTONUMLGL".to_string()), 2),
            field_kind_count(FieldKind::Numbering("AUTONUMOUT".to_string()), 2),
            field_kind_count(FieldKind::Numbering("LISTNUM".to_string()), 1),
            field_kind_count(FieldKind::Numbering("BIDIOUTLINE".to_string()), 2),
        ],
        vec![
            field_kind_count(FieldKind::Numbering("AUTONUM".to_string()), 1),
            field_kind_count(FieldKind::Numbering("LISTNUM".to_string()), 1),
            field_kind_count(FieldKind::Numbering("BIDIOUTLINE".to_string()), 1),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 2),
            field_reason_count(FieldEvaluationReason::NoComputedResult, 1),
        ],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_dynamic_control_fields_split_cached_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
        dynamic_control_field_diagnostics_docx(),
        17,
        vec![
            field_kind_count(FieldKind::Dynamic("IF".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("COMPARE".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("=".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("SET".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("QUOTE".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("NEXT".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("SKIPIF".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("NEXTIF".to_string()), 2),
        ],
        vec![
            field_kind_count(FieldKind::Dynamic("IF".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("COMPARE".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("=".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("SET".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("QUOTE".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("NEXTIF".to_string()), 2),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 6),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 7),
        ],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_body_syntax_splits_data_and_malformed_diagnostics() {
    let doc = Document::open(&formula_body_syntax_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(fields[1].computed_result, None);
    assert_eq!(fields[2].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 3)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 3)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 1),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 2),
        ]
    );
}

#[cfg(feature = "render")]
#[test]
fn report_dynamic_control_model_render_report_matches_document_reason_buckets() {
    let doc = Document::open(&dynamic_control_field_diagnostics_docx()).expect("fixture opens");
    let expected_kinds = doc.report().features.unsupported_field_kinds;
    let expected_reasons = doc.report().features.unsupported_field_reasons;
    assert_eq!(
        expected_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("IF".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("COMPARE".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("=".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("SET".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("QUOTE".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("NEXTIF".to_string()), 2),
        ]
    );
    assert_eq!(
        expected_reasons,
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 6),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 7),
        ]
    );

    let rendered = rwml::render_pdf_with_report(&doc.model());

    assert_eq!(rendered.report.unsupported.fields, 13);
    assert_eq!(rendered.report.unsupported.field_kinds, expected_kinds);
    assert_eq!(
        rendered.report.unsupported.unsupported_field_reasons,
        expected_reasons
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_prompt_fields_accept_unquoted_multi_token_prompt_text() {
    let doc = Document::open(&prompt_unquoted_multi_token_text_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(fields[0].computed_result.as_deref(), Some("ACME CORP"));
    assert_eq!(fields[1].kind, FieldKind::Dynamic("ASK".to_string()));
    assert_eq!(fields[1].computed_result.as_deref(), Some(""));
    assert_eq!(fields[2].kind, FieldKind::Ref);
    assert_eq!(fields[2].computed_result.as_deref(), Some("ACME CORP"));

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
            field_kind_count(FieldKind::Ref, 1),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_prompt_fields_accept_unquoted_multi_token_prompt_text_without_defaults() {
    let doc = Document::open(&prompt_unquoted_multi_token_text_no_default_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(fields[1].kind, FieldKind::Dynamic("ASK".to_string()));
    assert_eq!(fields[1].computed_result, None);
    assert_eq!(fields[2].kind, FieldKind::Ref);
    assert_eq!(fields[2].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
            field_kind_count(FieldKind::Ref, 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
            field_kind_count(FieldKind::Ref, 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 2),
            field_reason_count(FieldEvaluationReason::UnresolvedBookmark, 1),
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 3,
            field_kinds: vec![
                field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 1),
                field_kind_count(FieldKind::Dynamic("ASK".to_string()), 1),
                field_kind_count(FieldKind::Ref, 1),
            ],
        })
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_unquoted_multi_token_numeric_picture_is_supported() {
    let doc = Document::open(&formula_unquoted_multi_token_numeric_picture_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("=".to_string()));
    assert_eq!(fields[0].computed_result.as_deref(), Some("5 units"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_numeric_picture_fields_are_supported() {
    let doc = Document::open(&formula_numeric_picture_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 6);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("$1,234.50"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("33%"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("492"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("0.125"));
    assert_eq!(fields[4].computed_result.as_deref(), Some(".8"));
    assert_eq!(fields[5].computed_result.as_deref(), Some("5 units"));

    let report = doc.report();
    assert_eq!(report.features.fields, 6);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 6)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_defined_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&formula_defined_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 5);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("0"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("0"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("7"));
    assert_eq!(fields[4].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 5)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_neutral_format_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&formula_neutral_format_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 5);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("5"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("ten and 25/100"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("1F"));
    assert_eq!(fields[4].computed_result.as_deref(), Some("5"));

    let report = doc.report();
    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 5)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        0,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_literal_function_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&formula_literal_function_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 8);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("22"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("24"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("123.46"));
    assert_eq!(fields[4].computed_result.as_deref(), Some("4"));
    assert_eq!(fields[5].computed_result.as_deref(), Some("3"));
    assert_eq!(fields[6].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[7].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 8);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 8)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_reference_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&formula_table_reference_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 6);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("5"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("7.5"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("11"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("15"));
    assert_eq!(fields[4].computed_result.as_deref(), Some("29"));
    assert_eq!(fields[5].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 6);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 6)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_deleted_context_ignores_deleted_fields() {
    let doc =
        Document::open(&formula_table_deleted_context_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("=".to_string()));
    assert_eq!(fields[0].instruction, r#"= SUM(LEFT)"#);
    assert_eq!(fields[0].result, "stale visible sum");
    assert_eq!(fields[0].computed_result.as_deref(), Some("5"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("5") && !main_text.contains("deleted formula"),
        "deleted formulas must not shift visible report diagnostics: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_alternate_content_uses_single_branch() {
    let doc =
        Document::open(&formula_table_alternate_content_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("=".to_string()));
    assert_eq!(fields[0].instruction, r#"= SUM(LEFT)"#);
    assert_eq!(fields[0].result, "stale visible sum");
    assert_eq!(fields[0].computed_result.as_deref(), Some("5"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("5") && !main_text.contains("fallback formula"),
        "AlternateContent fallback formulas must not shift visible report diagnostics: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_combined_reference_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&formula_table_combined_reference_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("16"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("2"));
    assert_eq!(fields[3].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 4)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_nested_expression_fields_split_supported_and_malformed_diagnostics() {
    let doc =
        Document::open(&formula_table_nested_expression_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("10"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("5"));
    assert_eq!(fields[3].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 4)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_ragged_reference_fields_split_supported_and_malformed_diagnostics() {
    let doc =
        Document::open(&formula_table_ragged_reference_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("15"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("4"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[3].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 4)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_general_number_fields_are_supported() {
    let doc =
        Document::open(&formula_table_general_number_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("ten and 25/100"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("1F"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("twenty-first"));

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 3)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_cell_reference_fields_split_supported_and_malformed_diagnostics() {
    let doc =
        Document::open(&formula_table_cell_reference_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 8);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("16"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("504"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("9"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("10"));
    assert_eq!(fields[4].computed_result.as_deref(), Some("10"));
    assert_eq!(fields[5].computed_result.as_deref(), Some("15"));
    assert_eq!(fields[6].computed_result.as_deref(), Some("12"));
    assert_eq!(fields[7].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 8);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 8)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_formula_table_direct_cell_fields_split_supported_and_malformed_diagnostics() {
    let doc = Document::open(&formula_table_direct_cell_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::Dynamic("=".to_string())));
    assert_eq!(fields[0].computed_result.as_deref(), Some("5"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("3"));
    assert_eq!(fields[2].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 3)]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("=".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1
        )]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .filter(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. }))
            .count(),
        1,
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_set_backed_dynamic_controls_only_flags_unresolved_operands() {
    let doc =
        Document::open(&set_backed_dynamic_control_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 6);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("SET".to_string()));
    assert_eq!(fields[0].instruction, r#"SET ClientTier "Gold""#);
    assert_eq!(fields[0].computed_result.as_deref(), Some(""));
    assert_eq!(fields[1].kind, FieldKind::Dynamic("IF".to_string()));
    assert_eq!(
        fields[1].instruction,
        r#"IF ClientTier = "Gold" "ship" "hold""#
    );
    assert_eq!(fields[1].computed_result.as_deref(), Some("ship"));
    assert_eq!(fields[2].kind, FieldKind::Dynamic("COMPARE".to_string()));
    assert_eq!(fields[2].instruction, r#"COMPARE ClientTier = "Gold""#);
    assert_eq!(fields[2].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[3].kind, FieldKind::Dynamic("IF".to_string()));
    assert_eq!(
        fields[3].instruction,
        r#"IF MissingTier = "Gold" "ship" "hold""#
    );
    assert_eq!(fields[3].computed_result, None);
    assert_eq!(fields[4].kind, FieldKind::Dynamic("NEXTIF".to_string()));
    assert_eq!(fields[4].instruction, r#"NEXTIF ClientTier = "Gold""#);
    assert_eq!(fields[4].computed_result.as_deref(), Some(""));
    assert_eq!(fields[5].kind, FieldKind::Dynamic("SKIPIF".to_string()));
    assert_eq!(fields[5].instruction, r#"SKIPIF ClientTier <> "Gold""#);
    assert_eq!(fields[5].computed_result.as_deref(), Some(""));

    let report = doc.report();
    assert_eq!(report.features.fields, 6);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("SET".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("IF".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("COMPARE".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("NEXTIF".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("SKIPIF".to_string()), 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("IF".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1,
        )]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_bookmark_backed_dynamic_controls_only_flag_unresolved_operands() {
    let doc =
        Document::open(&bookmark_backed_dynamic_control_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 7);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("IF".to_string()));
    assert_eq!(
        fields[0].instruction,
        r#"IF InvoiceTier = "Gold" "ship" "hold""#
    );
    assert_eq!(fields[0].computed_result.as_deref(), Some("ship"));
    assert_eq!(fields[1].kind, FieldKind::Dynamic("COMPARE".to_string()));
    assert_eq!(fields[1].instruction, r#"COMPARE InvoiceTier = "G*""#);
    assert_eq!(fields[1].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[2].kind, FieldKind::Dynamic("IF".to_string()));
    assert_eq!(
        fields[2].instruction,
        r#"IF InvoiceTotal >= 40 "match" "miss""#
    );
    assert_eq!(fields[2].computed_result.as_deref(), Some("match"));
    assert_eq!(fields[3].kind, FieldKind::Dynamic("COMPARE".to_string()));
    assert_eq!(fields[3].instruction, "COMPARE InvoiceTotal < 40");
    assert_eq!(fields[3].computed_result.as_deref(), Some("0"));
    assert_eq!(fields[4].kind, FieldKind::Dynamic("IF".to_string()));
    assert_eq!(
        fields[4].instruction,
        r#"IF MissingTier = "Gold" "ship" "hold""#
    );
    assert_eq!(fields[4].computed_result, None);
    assert_eq!(fields[5].kind, FieldKind::Dynamic("NEXTIF".to_string()));
    assert_eq!(fields[5].instruction, r#"NEXTIF InvoiceTier = "Gold""#);
    assert_eq!(fields[5].computed_result.as_deref(), Some(""));
    assert_eq!(fields[6].kind, FieldKind::Dynamic("SKIPIF".to_string()));
    assert_eq!(fields[6].instruction, "SKIPIF InvoiceTotal < 40");
    assert_eq!(fields[6].computed_result.as_deref(), Some(""));

    let report = doc.report();
    assert_eq!(report.features.fields, 7);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("IF".to_string()), 3),
            field_kind_count(FieldKind::Dynamic("COMPARE".to_string()), 2),
            field_kind_count(FieldKind::Dynamic("NEXTIF".to_string()), 1),
            field_kind_count(FieldKind::Dynamic("SKIPIF".to_string()), 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Dynamic("IF".to_string()), 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1,
        )]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_set_backed_direct_bookmark_refs_are_supported() {
    let doc = Document::open(&set_backed_direct_ref_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("SET".to_string()));
    assert_eq!(fields[0].instruction, "SET ClientCode Client-42");
    assert_eq!(fields[0].computed_result.as_deref(), Some(""));
    assert_eq!(fields[1].kind, FieldKind::Ref);
    assert_eq!(fields[1].instruction, r#"ClientCode \* Upper"#);
    assert_eq!(fields[1].computed_result.as_deref(), Some("CLIENT-42"));
    assert_eq!(fields[2].kind, FieldKind::Dynamic("SET".to_string()));
    assert_eq!(fields[2].instruction, "SET ClientName Client 42");
    assert_eq!(fields[2].computed_result.as_deref(), Some(""));
    assert_eq!(fields[3].kind, FieldKind::Ref);
    assert_eq!(fields[3].instruction, r#"ClientName \* Upper"#);
    assert_eq!(fields[3].computed_result.as_deref(), Some("CLIENT 42"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("SET".to_string()), 2),
            field_kind_count(FieldKind::Ref, 2),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
}

#[cfg(feature = "docx")]
#[test]
fn report_set_backed_direct_bookmark_ref_gap_uses_known_target_reason() {
    let doc = Document::open(&set_backed_direct_ref_gap_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("SET".to_string()));
    assert_eq!(fields[0].instruction, "SET ClientName Client 42");
    assert_eq!(fields[0].computed_result.as_deref(), Some(""));
    assert_eq!(fields[1].kind, FieldKind::Ref);
    assert_eq!(fields[1].instruction, "ClientName \\f");
    assert_eq!(fields[1].result, "cached field-bookmark note ref");
    assert_eq!(fields[1].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("SET".to_string()), 1),
            field_kind_count(FieldKind::Ref, 1),
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![field_kind_count(FieldKind::Ref, 1)]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![field_reason_count(
            FieldEvaluationReason::NoComputedResult,
            1,
        )]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_compact_prompt_defaults_are_supported() {
    let doc = Document::open(&compact_prompt_default_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 7);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(fields[0].instruction, r#"FILLIN "Client?" \d"Acme""#);
    assert_eq!(fields[0].computed_result.as_deref(), Some("Acme"));
    assert_eq!(fields[1].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(
        fields[1].instruction,
        r#"FILLIN "Department?" \dops \* Upper"#
    );
    assert_eq!(fields[1].computed_result.as_deref(), Some("OPS"));
    assert_eq!(fields[2].kind, FieldKind::Dynamic("ASK".to_string()));
    assert_eq!(
        fields[2].instruction,
        r#"ASK ClientCode "Client code?" \d"ac-42""#
    );
    assert_eq!(fields[2].computed_result.as_deref(), Some(""));
    assert_eq!(fields[3].kind, FieldKind::Ref);
    assert_eq!(fields[3].instruction, r#"REF ClientCode \* Upper"#);
    assert_eq!(fields[3].computed_result.as_deref(), Some("AC-42"));
    assert_eq!(fields[4].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(
        fields[4].instruction,
        r#"FILLIN "Project?" \d Client 42 \* Upper"#
    );
    assert_eq!(fields[4].computed_result.as_deref(), Some("CLIENT 42"));
    assert_eq!(fields[5].kind, FieldKind::Dynamic("ASK".to_string()));
    assert_eq!(
        fields[5].instruction,
        r#"ASK ClientName "Client name?" \d Client 42"#
    );
    assert_eq!(fields[5].computed_result.as_deref(), Some(""));
    assert_eq!(fields[6].kind, FieldKind::Ref);
    assert_eq!(fields[6].instruction, r#"REF ClientName \* Upper"#);
    assert_eq!(fields[6].computed_result.as_deref(), Some("CLIENT 42"));

    let report = doc.report();
    assert_eq!(report.features.fields, 7);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Dynamic("FILLIN".to_string()), 3),
            field_kind_count(FieldKind::Dynamic("ASK".to_string()), 2),
            field_kind_count(FieldKind::Ref, 2),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_document_structure_fields_split_computed_cached_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
        document_structure_field_diagnostics_docx(),
        6,
        vec![
            field_kind_count(FieldKind::DocumentStructure("REVNUM".to_string()), 1),
            field_kind_count(FieldKind::DocumentStructure("SECTION".to_string()), 1),
            field_kind_count(FieldKind::DocumentStructure("SECTIONPAGES".to_string()), 1),
            field_kind_count(FieldKind::DocumentStructure("STYLEREF".to_string()), 3),
        ],
        vec![
            field_kind_count(FieldKind::DocumentStructure("SECTIONPAGES".to_string()), 1),
            field_kind_count(FieldKind::DocumentStructure("STYLEREF".to_string()), 3),
        ],
        vec![
            field_reason_count(FieldEvaluationReason::NoComputedResult, 3),
            field_reason_count(FieldEvaluationReason::UnsupportedSwitch, 1),
        ],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_style_ref_supported_fields_stay_out_of_unsupported_diagnostics() {
    let doc = Document::open(&style_ref_supported_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("STYLEREF \"heading 1\" \\* Upper", "EXECUTIVE SUMMARY"),
        ("STYLEREF \"Last Name\"", "Ackerman"),
        (
            "STYLEREF \"Custom Heading\" \\* MERGEFORMAT",
            "Forward Finding",
        ),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("STYLEREF".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("STYLEREF".to_string()),
            3,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_style_ref_accepts_unquoted_multi_token_style_names() {
    let doc = Document::open(&style_ref_unquoted_multi_token_style_name_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("STYLEREF Heading 1 \\* Upper", "EXECUTIVE SUMMARY"),
        ("STYLEREF Custom Heading \\* MERGEFORMAT", "Forward Finding"),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("STYLEREF".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("STYLEREF".to_string()),
            2,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_style_ref_number_switches_are_supported() {
    let doc = Document::open(&style_ref_numbering_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("STYLEREF NumberedTarget \\r", "5.1"),
        ("STYLEREF NumberedTarget \\r \\t", "5.1"),
        ("STYLEREF NumberedTarget \\n", "Part 1"),
        ("STYLEREF NumberedTarget \\n\\t", "1"),
        ("STYLEREF \"Numbered Target\" \\w \\* MERGEFORMAT", "4.5.1"),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("STYLEREF".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, expected.len());
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("STYLEREF".to_string()),
            expected.len(),
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(main_text.contains("5.1"), "{main_text:?}");
    assert!(main_text.contains("Part 1"), "{main_text:?}");
    assert!(main_text.contains("4.5.1"), "{main_text:?}");
    assert!(
        !main_text.contains("stale relative style number"),
        "{main_text:?}"
    );
    assert!(
        !main_text.contains("stale numeric style number"),
        "{main_text:?}"
    );
    assert!(
        !main_text.contains("stale full style number"),
        "{main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_style_ref_alternate_content_uses_single_branch() {
    let doc =
        Document::open(&style_ref_alternate_content_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].kind,
        FieldKind::DocumentStructure("STYLEREF".to_string())
    );
    assert_eq!(fields[0].instruction, "STYLEREF \"heading 1\"");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Choice Heading"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("STYLEREF".to_string()),
            1,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Choice Heading")
            && !main_text.contains("Fallback Heading")
            && !main_text.contains("Fallback Inline"),
        "STYLEREF report diagnostics must use one AlternateContent branch: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_style_ref_deleted_context_uses_current_heading() {
    let doc = Document::open(&style_ref_deleted_heading_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].kind,
        FieldKind::DocumentStructure("STYLEREF".to_string())
    );
    assert_eq!(fields[0].instruction, "STYLEREF \"heading 1\"");
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("Visible Heading")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("STYLEREF".to_string()),
            1,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Visible Heading")
            && !main_text.contains("Deleted Heading")
            && !main_text.contains("Moved Heading"),
        "STYLEREF report diagnostics must follow accepted-current headings: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_section_text_format_switches_are_supported() {
    let doc = Document::open(&section_field_text_format_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("SECTION \\* ROMAN", "I"),
        ("SECTION \\* CardText \\* Upper", "TWO"),
        ("SECTION \\* Ordinal", "3rd"),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("SECTION".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("SECTION".to_string()),
            3,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_section_alternate_content_uses_single_branch() {
    let doc = Document::open(&section_alternate_content_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].kind,
        FieldKind::DocumentStructure("SECTION".to_string())
    );
    assert_eq!(fields[0].instruction, "SECTION");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("SECTION".to_string()),
            1,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("2") && !main_text.contains("stale alternate section"),
        "SECTION report diagnostics must use one AlternateContent branch: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_section_pages_text_format_switches_are_supported() {
    let doc = Document::open(&section_pages_text_format_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("SECTIONPAGES", "3"),
        ("SECTIONPAGES \\* ROMAN", "III"),
        ("SECTIONPAGES \\* CardText \\* Upper", "THREE"),
        ("SECTIONPAGES \\* Ordinal", "1st"),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("SECTIONPAGES".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("SECTIONPAGES".to_string()),
            4,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_revnum_text_format_switches_are_supported() {
    let doc =
        Document::open(&revision_number_text_format_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    let expected = [
        ("REVNUM \\* Upper", "DRAFT REVISION"),
        ("REVNUM \\*Lower", "draft revision"),
        ("REVNUM \\* Caps", "Draft REVISION"),
        ("REVNUM \\* FirstCap", "Draft REVISION"),
    ];

    assert_eq!(fields.len(), expected.len());
    for (field, (instruction, computed)) in fields.iter().zip(expected) {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("REVNUM".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(
            FieldKind::DocumentStructure("REVNUM".to_string()),
            4,
        )]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_merge_fields_split_supported_and_malformed_diagnostics() {
    assert_report_field_diagnostics(
        merge_field_diagnostics_docx(),
        7,
        vec![field_kind_count(FieldKind::MergeField, 7)],
        vec![field_kind_count(FieldKind::MergeField, 4)],
        vec![field_reason_count(
            FieldEvaluationReason::UnsupportedSwitch,
            4,
        )],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_split_complex_field_instruction_once() {
    let doc = Document::open(&split_complex_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(doc.fields().len(), 1);
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::MergeField,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_field_warning_is_named_unsupported_evaluation() {
    let doc = Document::open(&page_ref_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 1,
            },
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 2,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::PageRef,
                count: 2,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"PAGEREF","count":2}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"PAGEREF","count":2}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":1}]"#),
        "{json}"
    );
    assert!(!json.contains(r#""reason":"UnknownField""#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_field_warning_reports_gap_cases() {
    let doc = Document::open(&page_ref_gap_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::PageRef));
    assert_eq!(fields[0].instruction, "PAGEREF PlainText \\h");
    assert_eq!(fields[0].result, "cached page ref");
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(fields[1].instruction, "PAGEREF MissingPage \\h");
    assert_eq!(fields[1].result, "cached missing page ref");
    assert_eq!(fields[1].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 1,
            },
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 2,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::PageRef,
                count: 2,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"PAGEREF","count":2}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_unknown_field_warning_reports_gap_case() {
    let doc = Document::open(&unknown_field_gap_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Unknown("CUSTOM".to_string()));
    assert_eq!(fields[0].instruction, "CUSTOM Field");
    assert_eq!(fields[0].result, "cached unknown field");
    assert_eq!(fields[0].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Unknown("CUSTOM".to_string()),
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Unknown("CUSTOM".to_string()),
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnknownField,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Unknown("CUSTOM".to_string()),
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"CUSTOM","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"UnknownField","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_text_format_switches_keep_page_ref_reasons() {
    let doc =
        Document::open(&page_ref_text_format_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 1,
            },
        ]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_section_format_scan_uses_current_single_branch_view() {
    let doc = Document::open(&page_ref_non_current_section_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].computed_result, None);

    let report = doc.report();
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_defers_content_paragraph_section_format_until_paragraph_end() {
    let doc = Document::open(&page_ref_content_paragraph_section_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF BeforeFormatBreak \\p");
    assert_eq!(fields[0].result, "stale before");
    assert_eq!(fields[0].computed_result.as_deref(), Some("on page 1"));
    assert_eq!(fields[1].kind, FieldKind::PageRef);
    assert_eq!(fields[1].instruction, "PAGEREF AfterFormatBreak \\p");
    assert_eq!(fields[1].result, "stale after");
    assert_eq!(fields[1].computed_result, None);

    let report = doc.report();
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_content_paragraph_section_break_target_is_supported() {
    let doc = Document::open(&page_ref_content_paragraph_section_break_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF BeforeSectionBreak \\h");
    assert_eq!(fields[0].computed_result.as_deref(), Some("1"));

    let report = doc.report();
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());

    let main_text = doc.main_text();
    assert!(
        main_text.contains("1") && !main_text.contains("stale before break"),
        "content-paragraph section-break PAGEREF should use computed text: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_display_only_restart_relative_is_supported() {
    let doc =
        Document::open(&page_ref_visible_intro_section_page_number_restart_diagnostics_docx())
            .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF RestartedAfterIntro \\h");
    assert_eq!(fields[0].computed_result.as_deref(), Some("7"));
    assert_eq!(fields[1].kind, FieldKind::PageRef);
    assert_eq!(fields[1].instruction, "PAGEREF RestartedAfterIntro \\p");
    assert_eq!(fields[1].computed_result.as_deref(), Some("above"));

    let report = doc.report();
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_relative_unsupported_section_format_is_supported() {
    let doc = Document::open(&page_ref_relative_unsupported_section_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF UnsupportedFmt \\p");
    assert_eq!(fields[0].result, "stale relative");
    assert_eq!(fields[0].computed_result.as_deref(), Some("on page 2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_relative_unsupported_section_format_explicit_override_is_supported() {
    let doc =
        Document::open(&page_ref_relative_unsupported_section_format_override_diagnostics_docx())
            .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(
        fields[0].instruction,
        "PAGEREF UnsupportedFmt \\p \\* Arabic"
    );
    assert_eq!(fields[0].result, "stale relative override");
    assert_eq!(fields[0].computed_result.as_deref(), Some("on page 2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_relative_unsupported_even_odd_section_formats_are_supported() {
    let doc =
        Document::open(&page_ref_relative_unsupported_even_odd_section_formats_diagnostics_docx())
            .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF EvenUnsupportedFmt \\p");
    assert_eq!(fields[0].result, "stale even relative");
    assert_eq!(fields[0].computed_result.as_deref(), Some("on page 2"));
    assert_eq!(fields[1].kind, FieldKind::PageRef);
    assert_eq!(fields[1].instruction, "PAGEREF OddUnsupportedFmt \\p");
    assert_eq!(fields[1].result, "stale odd relative");
    assert_eq!(fields[1].computed_result.as_deref(), Some("on page 5"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_unsupported_section_format_explicit_override_is_supported() {
    let doc = Document::open(&page_ref_unsupported_section_format_override_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(
        fields[0].instruction,
        "PAGEREF UnsupportedFmt \\h \\* Arabic"
    );
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_manual_break_targets_are_supported_when_unambiguous() {
    let doc = Document::open(&page_ref_manual_break_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 5,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"PAGEREF","count":5}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_rendered_break_targets_are_supported_when_source_hints_exist() {
    let doc = Document::open(&page_ref_rendered_break_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"PAGEREF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_rendered_context_manual_break_targets_are_supported() {
    let doc = Document::open(&page_ref_rendered_then_manual_break_diagnostics_docx())
        .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "computed rendered-context hard-break PAGEREF should not warn: {:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_section_break_targets_are_supported_when_structural() {
    let doc = Document::open(&page_ref_section_break_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("iv"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("above"));

    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"PAGEREF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_default_section_break_target_is_supported() {
    let doc =
        Document::open(&page_ref_default_section_break_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_page_break_before_targets_are_supported_when_structural() {
    let doc =
        Document::open(&page_ref_page_break_before_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("4th"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("above"));

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_section_page_number_restart_targets_are_supported() {
    let doc = Document::open(&page_ref_section_page_number_restart_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("7"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("VIII"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("above"));

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_final_section_page_number_format_is_supported() {
    let doc = Document::open(&page_ref_final_section_page_number_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("vi"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("6"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("above"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::Page, 1),
            field_kind_count(FieldKind::PageRef, 3),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_decimal_zero_section_page_number_format_is_supported() {
    let doc = Document::open(&page_ref_decimal_zero_section_page_number_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("05"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("5"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_number_in_dash_section_page_number_format_is_supported() {
    let doc =
        Document::open(&page_ref_number_in_dash_section_page_number_format_diagnostics_docx())
            .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("- 5 -"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("5"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_textual_section_page_number_formats_are_supported() {
    let doc = Document::open(&page_ref_textual_section_page_number_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let expected = ["b", "2", "C", "3", "VI", "6", "five", "5", "fifth", "5"];
    assert_eq!(fields.len(), expected.len());
    for (field, computed) in fields.iter().zip(expected) {
        assert_eq!(field.kind, FieldKind::PageRef);
        assert_eq!(field.computed_result.as_deref(), Some(computed));
    }

    let report = doc.report();
    assert_eq!(report.features.fields, expected.len());
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: expected.len(),
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_decimal_full_width_section_page_number_format_is_supported() {
    let doc =
        Document::open(&page_ref_decimal_full_width_section_page_number_format_diagnostics_docx())
            .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("１２"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("12"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_decimal_enclosed_circle_section_page_number_format_is_supported() {
    let doc = Document::open(
        &page_ref_decimal_enclosed_circle_section_page_number_format_diagnostics_docx(),
    )
    .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{246b}"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("12"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_decimal_enclosed_punctuation_section_page_number_formats_are_supported() {
    let doc = Document::open(
        &page_ref_decimal_enclosed_punctuation_section_page_number_format_diagnostics_docx(),
    )
    .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{2493}"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("12"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("\u{2480}"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("13"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 4,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_decimal_width_variant_section_page_number_formats_are_supported() {
    let doc = Document::open(
        &page_ref_decimal_width_variant_section_page_number_format_diagnostics_docx(),
    )
    .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("12"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("- 12 -"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("１３"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("13"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 4,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_korean_section_page_number_formats_are_supported() {
    let doc = Document::open(&page_ref_korean_section_page_number_format_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{ac00}"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("\u{3134}"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 4,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_korean_numeric_section_page_number_formats_are_supported() {
    let doc =
        Document::open(&page_ref_korean_numeric_section_page_number_format_diagnostics_docx())
            .expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{c77c}"));
    assert_eq!(fields[1].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("\u{b458}"));
    assert_eq!(fields[3].computed_result.as_deref(), Some("2"));
    assert_eq!(fields[4].computed_result.as_deref(), Some("\u{c2ed}"));
    assert_eq!(fields[5].computed_result.as_deref(), Some("10"));
    assert_eq!(fields[6].computed_result.as_deref(), Some("\u{c774}"));
    assert_eq!(fields[7].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 8);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 8,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_ambiguous_pre_marker_manual_break_remains_unsupported() {
    let doc =
        Document::open(&page_ref_visible_manual_break_before_rendered_hint_diagnostics_docx())
            .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 1,
        }]
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "ambiguous pre-marker hard-break PAGEREF should remain unsupported: {:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_rendered_break_page_one_targets_are_supported() {
    let doc = Document::open(&page_ref_rendered_break_page_one_diagnostics_docx())
        .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "computed rendered page-one PAGEREF should not warn: {:?}",
        report.warnings
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_deleted_rendered_break_marker_remains_unsupported() {
    let doc =
        Document::open(&page_ref_deleted_rendered_break_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 1,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_no_cached_rendered_break_fields_are_supported() {
    let doc = Document::open(&page_ref_rendered_break_no_cached_result_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF FigureTwo \\h");
    assert_eq!(fields[0].result, "");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));
    assert_eq!(fields[1].kind, FieldKind::PageRef);
    assert_eq!(
        fields[1].instruction,
        "PAGEREF \"FigureTwo\" \\* MERGEFORMAT"
    );
    assert_eq!(fields[1].result, "");
    assert_eq!(fields[1].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("2\n2"),
        "computed PAGEREF report diagnostics should not depend on cached result runs: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_wrapped_complex_rendered_break_field_is_supported() {
    let doc = Document::open(&page_ref_wrapped_complex_rendered_break_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(
        fields[0].instruction,
        "PAGEREF \"FigureTwo\" \\* MERGEFORMAT"
    );
    assert_eq!(fields[0].result, "old wrapped page");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("2") && !main_text.contains("old wrapped page"),
        "wrapped complex PAGEREF report diagnostics should use computed visible text: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_page_ref_alternate_content_rendered_break_uses_single_branch() {
    let doc = Document::open(&page_ref_alternate_content_rendered_break_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF AltPage \\h");
    assert_eq!(fields[0].result, "stale alternate page");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("2") && !main_text.contains("stale alternate page"),
        "PAGEREF report diagnostics must not double-count AlternateContent page markers: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_direct_bookmark_field_counts_as_supported_ref_when_bookmark_exists() {
    let doc = Document::open(&direct_bookmark_ref_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].computed_result.as_deref(), Some("Figure 1"));
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":1}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_complex_direct_bookmark_field_counts_as_supported_ref_when_bookmark_exists() {
    let doc = Document::open(&complex_direct_bookmark_ref_field_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].instruction, "Figure1 \\* Upper");
    assert_eq!(fields[0].computed_result.as_deref(), Some("FIGURE ONE"));
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_direct_bookmark_field_switches_count_as_supported_refs() {
    let doc =
        Document::open(&direct_bookmark_ref_switch_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 5);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("FIGURE ONE"),
            Some("Figure one"),
            Some("figure one"),
            Some("figure one"),
            None
        ]
    );
    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 5,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Ref,
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":5}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"REF","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_direct_bookmark_p_fields_count_as_supported_refs() {
    let doc =
        Document::open(&direct_relative_ref_switch_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("below"), Some("above")]
    );
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":2}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_n_fields_count_as_supported_refs() {
    let doc = Document::open(&numbered_ref_switch_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("3"), Some("3 above"), Some("3")]
    );
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_n_t_fields_count_as_supported_refs() {
    let doc = Document::open(&numbered_ref_suppress_text_switch_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("1.01"), Some("1.01 above"), Some("1.01")]
    );
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_w_fields_count_as_supported_refs() {
    let doc = Document::open(&full_context_ref_switch_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("1.a.i"), Some("1.a.i above"), Some("1.a.i")]
    );
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_w_t_fields_count_as_supported_refs() {
    let doc = Document::open(&full_context_ref_suppress_text_switch_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("1.a.i"), Some("1.a.i above"), Some("1.a.i")]
    );
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_r_fields_count_as_supported_refs() {
    let doc =
        Document::open(&relative_context_ref_switch_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(
        fields
            .iter()
            .map(|field| field.computed_result.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("5.2"), Some("5.2 below"), Some("5.2")]
    );
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_field_warning_ignores_computed_bookmark_refs() {
    let doc = Document::open(&ref_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 9);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 9,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 4,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 3,
            },
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 4,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Ref,
                count: 4,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"REF","count":9}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"REF","count":4}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":3}]"#),
        "{json}"
    );
    assert!(json.contains(r#""kind":"UnsupportedFieldEvaluation","count":4,"field_kinds":[{"kind":"REF","count":4}]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_bookmark_targets_follow_accepted_current_view() {
    let doc =
        Document::open(&ref_accepted_current_bookmark_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(fields[0].instruction, "REF ClauseText");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Visible clause"));
    assert_eq!(fields[1].instruction, "REF AltText");
    assert_eq!(fields[1].computed_result.as_deref(), Some("Choice clause"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Ref, 2)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(main_text.contains("Visible clause"), "{main_text:?}");
    assert!(main_text.contains("Choice clause"), "{main_text:?}");
    assert!(!main_text.contains("deleted clause"), "{main_text:?}");
    assert!(!main_text.contains("moved clause"), "{main_text:?}");
    assert!(!main_text.contains("Fallback clause"), "{main_text:?}");
    assert!(!main_text.contains("stale deleted ref"), "{main_text:?}");
    assert!(!main_text.contains("stale alternate ref"), "{main_text:?}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_field_warning_reports_gap_cases() {
    let doc = Document::open(&ref_gap_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert_eq!(fields[0].instruction, "REF PlainText \\f");
    assert_eq!(fields[0].result, "cached non-note ref mark");
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(fields[1].instruction, "REF PlainText \\d-");
    assert_eq!(fields[1].result, "cached ref separator");
    assert_eq!(fields[1].computed_result.as_deref(), Some("Plain target"));
    assert_eq!(fields[2].instruction, "REF MissingRef");
    assert_eq!(fields[2].result, "cached missing ref");
    assert_eq!(fields[2].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 3,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 1,
            },
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 2,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Ref,
                count: 2,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"REF","count":2}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_f_note_reference_marks_count_as_supported_refs() {
    let doc = Document::open(&ref_note_reference_mark_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert!(fields.iter().all(|field| field.computed_result.is_some()));
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Ref, 2)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_f_comment_reference_marks_count_as_supported_refs() {
    let doc =
        Document::open(&ref_comment_reference_mark_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert!(fields.iter().all(|field| field.computed_result.is_some()));
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Ref, 2)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_f_comment_range_reference_marks_count_as_supported_refs() {
    let doc = Document::open(&ref_comment_range_reference_mark_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].instruction, "REF CommentedRange \\f");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Ref, 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_f_inside_comment_range_reference_marks_count_as_supported_refs() {
    let doc = Document::open(&ref_inside_comment_range_reference_mark_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let report = doc.report();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].instruction, "REF InsideComment \\f");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2"));
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Ref, 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        None
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_bookmark_names_follow_accepted_single_branch_view() {
    let doc = Document::open(&ref_non_current_bookmark_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Ref));
    assert!(fields.iter().all(|field| field.computed_result.is_none()));

    let report = doc.report();
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnresolvedBookmark,
            count: 2,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_malformed_ref_targets_are_unsupported_switches() {
    let doc = Document::open(&malformed_ref_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields[0].computed_result, None);

    let report = doc.report();
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 5,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_note_ref_field_warning_tracks_unresolved_and_unsupported_cases() {
    let doc = Document::open(&note_ref_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 8);
    assert!(fields.iter().all(|field| field.kind == FieldKind::NoteRef));
    assert_eq!(fields[0].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[1].instruction, "FTNREF FootOne");
    assert_eq!(fields[1].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[2].computed_result.as_deref(), Some("above"));
    assert_eq!(fields[3].computed_result, None);
    assert_eq!(fields[4].computed_result, None);
    assert_eq!(fields[5].computed_result, None);
    assert_eq!(fields[6].computed_result, None);
    assert_eq!(fields[7].computed_result, None);

    let report = doc.report();

    assert_eq!(report.features.fields, 8);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::NoteRef,
            count: 8,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::NoteRef,
            count: 5,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 3,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 1,
            },
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 5,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::NoteRef,
                count: 5,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"NOTEREF","count":8}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"NOTEREF","count":5}]"#),
        "{json}"
    );
    assert!(json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":3},{"reason":"UnsupportedSwitch","count":1}]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_note_ref_number_format_switches_are_supported() {
    let doc = Document::open(&note_ref_number_format_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::NoteRef));
    assert_eq!(fields[0].instruction, "NOTEREF LaterNote \\* roman");
    assert_eq!(fields[0].computed_result.as_deref(), Some("ii"));
    assert_eq!(
        fields[1].instruction,
        "NOTEREF FootOne \\* OrdText \\* Upper"
    );
    assert_eq!(fields[1].computed_result.as_deref(), Some("FIRST"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::NoteRef, 2)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_note_ref_alternate_content_uses_single_branch() {
    let doc =
        Document::open(&note_ref_alternate_content_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::NoteRef));
    assert_eq!(fields[0].instruction, "NOTEREF FootOne");
    assert_eq!(fields[0].computed_result.as_deref(), Some("1"));
    assert_eq!(fields[1].instruction, "NOTEREF FootOne \\p");
    assert_eq!(fields[1].computed_result.as_deref(), Some("above"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::NoteRef, 2)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("1") && main_text.contains("above"),
        "accepted NOTEREF fields should be materialized in report fixtures: {main_text:?}"
    );
    assert!(
        !main_text.contains("stale alternate"),
        "NOTEREF AlternateContent diagnostics should use computed text: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_note_ref_field_warning_reports_gap_cases() {
    let doc = Document::open(&note_ref_gap_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.kind == FieldKind::NoteRef));
    assert_eq!(fields[0].instruction, "NOTEREF PlainText");
    assert_eq!(fields[0].result, "cached plain note ref");
    assert_eq!(fields[0].computed_result, None);
    assert_eq!(fields[1].instruction, "NOTEREF MissingNote");
    assert_eq!(fields[1].result, "cached missing note ref");
    assert_eq!(fields[1].computed_result, None);
    assert_eq!(fields[2].instruction, "NOTEREF PlainText \\x");
    assert_eq!(fields[2].result, "cached bad note ref switch");
    assert_eq!(fields[2].computed_result, None);

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::NoteRef,
            count: 3,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::NoteRef,
            count: 3,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnresolvedBookmark,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 1,
            },
        ]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 3,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::NoteRef,
                count: 3,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"NOTEREF","count":3}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":1},{"reason":"UnresolvedBookmark","count":1},{"reason":"UnsupportedSwitch","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_field_diagnostic_inventory_tracks_active_gap_buckets() {
    let cases = field_diagnostic_inventory_cases();
    let expected_rows = [
        "D1", "D3", "D4", "D5", "D6", "D7", "D8", "D10", "D11", "D12",
    ];
    let rows = cases.iter().map(|case| case.row).collect::<Vec<_>>();
    assert_eq!(rows, expected_rows);
    let expected_field_kinds = field_kind_counts_from_inventory(&cases);
    let expected_reasons = field_reason_counts_from_inventory(&cases);

    let doc = Document::open(&field_diagnostic_inventory_docx()).expect("fixture opens");
    let fields = doc.fields();
    assert_eq!(fields.len(), cases.len());
    for (field, case) in fields.iter().zip(&cases) {
        assert_eq!(field.instruction, case.instruction, "{}", case.row);
        assert_eq!(field.kind, case.kind, "{}", case.row);
        assert_eq!(field.result, case.cached, "{}", case.row);
        assert_eq!(field.computed_result, None, "{}", case.row);
    }

    let report = doc.report();

    assert_eq!(report.features.fields, cases.len());
    assert_eq!(report.features.field_kinds, expected_field_kinds);
    assert_eq!(
        report.features.unsupported_field_kinds,
        expected_field_kinds
    );
    assert_eq!(report.features.unsupported_field_reasons, expected_reasons);
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: cases.len(),
            field_kinds: expected_field_kinds.clone(),
        })
    );

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":3},{"reason":"UnresolvedBookmark","count":4},{"reason":"UnsupportedSwitch","count":2},{"reason":"UnknownField","count":1}]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_field_warning_ignores_supported_text_format_switch_refs() {
    let doc = Document::open(&ref_text_format_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 5);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 5,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnresolvedBookmark,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Ref,
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"UnresolvedBookmark","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_field_warning_ignores_multi_paragraph_bookmark_refs() {
    let doc = Document::open(&multi_paragraph_ref_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Ref,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnresolvedBookmark,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Ref,
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"REF","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_ref_bookmark_text_preserves_inline_tabs_and_breaks() {
    let doc = Document::open(&inline_break_ref_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].instruction, "REF InlineText");
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("Alpha\tBeta\nGamma-Delta")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Ref, 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Alpha\tBeta\nGamma-Delta"),
        "{main_text:?}"
    );
    assert!(!main_text.contains("stale inline ref"), "{main_text:?}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_computed_heading_tocs() {
    let doc = Document::open(&toc_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Unknown("CUSTOM".to_string()),
                count: 1,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Unknown("CUSTOM".to_string()),
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnknownField,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Unknown("CUSTOM".to_string()),
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"CUSTOM","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"UnknownField","count":1}]"#),
        "{json}"
    );
    assert!(json.contains(r#""kind":"UnsupportedFieldEvaluation","count":1,"field_kinds":[{"kind":"CUSTOM","count":1}]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_complex_toc_field_counts_as_supported() {
    let doc = Document::open(&complex_toc_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Toc);
    assert_eq!(fields[0].instruction, "TOC \\o \"1-2\" \\* Upper");
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("EXECUTIVE SUMMARY\n  RISKS")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Toc, 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_bare_toc_field_warning_ignores_computed_default_tocs() {
    let doc = Document::open(&bare_toc_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Unknown("CUSTOM".to_string()),
                count: 1,
            },
        ]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Unknown("CUSTOM".to_string()),
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Unknown("CUSTOM".to_string()),
                count: 1,
            }],
        })
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_neutral_switch_tocs() {
    let doc = Document::open(&toc_neutral_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_general_format_switch_tocs() {
    let doc = Document::open(&toc_general_format_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 3,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_with_quoted_custom_style_switch_reports_no_computed_result() {
    let doc = Document::open(&toc_quoted_custom_style_no_result_diagnostics_docx())
        .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 4,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 4,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 2,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 2,
            },
        ]
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"NoComputedResult","count":2},{"reason":"UnsupportedSwitch","count":2}]"#),
        "{json}"
    );
}

#[cfg(feature = "render")]
#[test]
fn report_toc_gap_model_render_report_matches_document_reason_buckets() {
    let doc = Document::open(&toc_quoted_custom_style_no_result_diagnostics_docx())
        .expect("fixture opens");
    let expected_reasons = doc.report().features.unsupported_field_reasons;
    assert_eq!(
        expected_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 2,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 2,
            },
        ]
    );
    let model = doc.model();

    let rendered = rwml::render_pdf_with_report(&model);

    assert_eq!(rendered.report.unsupported.fields, 4);
    assert_eq!(
        rendered.report.unsupported.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 4,
        }]
    );
    assert_eq!(
        rendered.report.unsupported.unsupported_field_reasons,
        expected_reasons
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_accepts_unquoted_multi_token_custom_style_switch() {
    let doc =
        Document::open(&toc_unquoted_custom_style_match_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Toc);
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("EXECUTIVE SUMMARY\n  CUSTOM FINDING")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_matched_custom_style_tocs() {
    let doc = Document::open(&toc_custom_style_match_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_no_page_number_switch_tocs() {
    let doc =
        Document::open(&toc_no_page_numbers_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_entry_page_separator_switch_tocs() {
    let doc =
        Document::open(&toc_entry_page_separator_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_sequence_page_separator_switch_tocs() {
    let doc = Document::open(&toc_sequence_page_separator_switch_diagnostics_docx())
        .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_outline_switch_tocs() {
    let doc = Document::open(&toc_outline_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 2,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_supported_tc_entry_tocs() {
    let doc = Document::open(&toc_tc_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::TocEntry,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_entry_accepts_unquoted_multi_token_marker_text() {
    let doc = Document::open(&toc_tc_unquoted_multi_token_text_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::TocEntry);
    assert_eq!(
        fields[0].instruction,
        "TC Manual Appendix Entry \\f m \\l 2"
    );
    assert_eq!(fields[0].computed_result.as_deref(), Some(""));
    assert_eq!(fields[1].kind, FieldKind::Toc);
    assert_eq!(
        fields[1].computed_result.as_deref(),
        Some("  Manual Appendix Entry")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::TocEntry, 1),
            field_kind_count(FieldKind::Toc, 1),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_deleted_tc_entries_are_ignored() {
    let doc = Document::open(&toc_deleted_tc_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::TocEntry);
    assert_eq!(fields[0].instruction, "TC \"Visible Entry\" \\f m \\l 1");
    let toc = fields
        .iter()
        .find(|field| field.kind == FieldKind::Toc)
        .expect("TOC field is parsed");
    assert_eq!(toc.instruction, "TOC \\f m");
    assert_eq!(toc.computed_result.as_deref(), Some("Visible Entry"));

    let report = doc.report();
    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            field_kind_count(FieldKind::TocEntry, 1),
            field_kind_count(FieldKind::Toc, 1),
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Visible Entry")
            && !main_text.contains("Deleted Entry")
            && !main_text.contains("Moved Entry"),
        "TOC report diagnostics must follow accepted-current TC entries: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_alternate_content_entries_use_single_branch() {
    let doc =
        Document::open(&toc_alternate_content_heading_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Toc);
    assert_eq!(fields[0].instruction, "TOC \\o \"1-1\"");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Choice Inline"));

    let report = doc.report();
    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![field_kind_count(FieldKind::Toc, 1)]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        "{:?}",
        report.warnings
    );

    let main_text = doc.main_text();
    assert!(
        main_text.contains("Choice Inline")
            && !main_text.contains("Fallback Heading")
            && !main_text.contains("Fallback Inline"),
        "TOC report diagnostics must use one AlternateContent branch: {main_text:?}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_supported_sequence_caption_tocs() {
    let doc = Document::open(&toc_sequence_caption_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Sequence,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_dirty_sequence_caption_tocs() {
    let doc =
        Document::open(&toc_dirty_sequence_caption_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();
    let toc = fields
        .iter()
        .find(|field| field.kind == FieldKind::Toc)
        .expect("TOC field is parsed");

    assert_eq!(
        toc.computed_result.as_deref(),
        Some("Figure 1: Mercury\nFigure 2: Venus")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Sequence,
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_dirty_complex_sequence_caption_tocs() {
    let doc = Document::open(&toc_dirty_complex_sequence_caption_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let toc = fields
        .iter()
        .find(|field| field.kind == FieldKind::Toc)
        .expect("TOC field is parsed");

    assert_eq!(
        toc.computed_result.as_deref(),
        Some("Figure 1: Mercury\nFigure 2: Venus")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Sequence,
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_supported_sequence_caption_text_tocs() {
    let doc = Document::open(&toc_sequence_caption_text_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Sequence,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_sequence_heading_reset_caption_tocs() {
    let doc = Document::open(&toc_sequence_heading_reset_caption_diagnostics_docx())
        .expect("fixture opens");
    let fields = doc.fields();
    let caption_toc = fields
        .iter()
        .find(|field| field.instruction == "TOC \\c Figure")
        .expect("TOC \\c field is parsed");
    let caption_text_toc = fields
        .iter()
        .find(|field| field.instruction == "TOC \\a Figure")
        .expect("TOC \\a field is parsed");

    assert_eq!(
        caption_toc.computed_result.as_deref(),
        Some("Figure 1: Mercury\nFigure 1: Venus")
    );
    assert_eq!(
        caption_text_toc.computed_result.as_deref(),
        Some("Mercury\nVenus")
    );

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::Sequence,
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 2,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[cfg(feature = "docx")]
#[test]
fn report_sequence_fields_split_computed_cached_and_malformed_diagnostics() {
    let doc = Document::open(&sequence_field_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 4);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Sequence));
    assert_eq!(fields[2].instruction, "SEQ Figure \\s 1");
    assert_eq!(fields[2].computed_result.as_deref(), Some("1"));

    let report = doc.report();
    assert_eq!(report.features.fields, 4);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Sequence,
            count: 4,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Sequence,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 2,
        }]
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_sequence_and_numbering_text_format_switches_are_supported() {
    assert_report_field_diagnostics(
        sequence_numbering_text_format_diagnostics_docx(),
        7,
        vec![
            field_kind_count(FieldKind::Sequence, 3),
            field_kind_count(FieldKind::Numbering("AUTONUM".to_string()), 2),
            field_kind_count(FieldKind::Numbering("LISTNUM".to_string()), 2),
        ],
        vec![],
        vec![],
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_resolved_bookmark_scope_tocs() {
    let doc = Document::open(&toc_bookmark_scope_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 3,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnresolvedBookmark,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"TOC","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"UnresolvedBookmark","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_bookmark_scoped_tc_entry_tocs() {
    let doc =
        Document::open(&toc_bookmark_scoped_tc_field_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::TocEntry,
                count: 2,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_existing_empty_scope_tocs() {
    let doc = Document::open(&toc_scope_gap_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Toc);
    assert_eq!(fields[0].instruction, "TOC \\b PlainText");
    assert_eq!(fields[0].result, "cached empty toc");
    assert_eq!(fields[0].computed_result.as_deref(), Some(""));
    assert_eq!(fields[1].kind, FieldKind::Toc);
    assert_eq!(fields[1].instruction, "TOC \\b MissingScope");
    assert_eq!(fields[1].result, "cached missing toc scope");
    assert_eq!(fields[1].computed_result, None);

    let report = doc.report();

    assert_eq!(report.features.fields, 2);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 2,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnresolvedBookmark,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"TOC","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"UnresolvedBookmark","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_bookmark_only_scope_tocs() {
    let doc = Document::open(&toc_bookmark_only_scope_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_compact_operand_switch_tocs() {
    let doc =
        Document::open(&compact_toc_operand_switch_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 3);
    assert_eq!(
        report.features.field_kinds,
        vec![
            FieldKindCount {
                kind: FieldKind::TocEntry,
                count: 1,
            },
            FieldKindCount {
                kind: FieldKind::Toc,
                count: 2,
            },
        ]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_ignores_range_less_outline_tocs() {
    let doc = Document::open(&toc_outline_without_range_diagnostics_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert!(report.features.unsupported_field_kinds.is_empty());
    assert!(report.features.unsupported_field_reasons.is_empty());
    assert!(report
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));

    let json = report.to_json();
    assert!(json.contains(r#""unsupported_field_kinds":[]"#), "{json}");
    assert!(json.contains(r#""unsupported_field_reasons":[]"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_toc_field_warning_reports_unsupported_toc_switch() {
    let doc = Document::open(&toc_bad_switch_diagnostics_docx()).expect("fixture opens");
    let fields = doc.fields();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Toc);
    assert_eq!(fields[0].instruction, "TOC \\q");
    assert_eq!(fields[0].result, "cached bad toc switch");
    assert_eq!(fields[0].computed_result, None);

    let report = doc.report();

    assert_eq!(report.features.fields, 1);
    assert_eq!(
        report.features.field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_kinds,
        vec![FieldKindCount {
            kind: FieldKind::Toc,
            count: 1,
        }]
    );
    assert_eq!(
        report.features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
    assert_eq!(
        report
            .warnings
            .iter()
            .find(|warning| matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })),
        Some(&DocumentWarning::UnsupportedFieldEvaluation {
            count: 1,
            field_kinds: vec![FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            }],
        })
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"TOC","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_reasons":[{"reason":"UnsupportedSwitch","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_includes_core_properties_for_diagnostics_json() {
    let doc = Document::open(&core_properties_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(
        report.core_properties,
        CoreProperties {
            title: Some("Quarter <One> & Co".to_string()),
            subject: Some("Pipeline".to_string()),
            creator: Some("Analyst".to_string()),
            description: Some("Diagnostics summary".to_string()),
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
    );

    let json = report.to_json();
    assert!(
        json.contains(r#""core_properties":{"title":"Quarter <One> & Co""#),
        "{json}"
    );
    assert!(json.contains(r#""subject":"Pipeline""#), "{json}");
    assert!(json.contains(r#""creator":"Analyst""#), "{json}");
    assert!(
        json.contains(r#""description":"Diagnostics summary""#),
        "{json}"
    );
    assert!(json.contains(r#""keywords":"rwml,metadata""#), "{json}");
    assert!(json.contains(r#""category":"Operations""#), "{json}");
    assert!(json.contains(r#""content_status":"Draft""#), "{json}");
    assert!(json.contains(r#""last_modified_by":"Reviewer""#), "{json}");
    assert!(
        json.contains(r#""created":"2026-06-01T02:03:04Z""#),
        "{json}"
    );
    assert!(
        json.contains(r#""modified":"2026-06-02T03:04:05Z""#),
        "{json}"
    );
    assert!(
        json.contains(r#""last_printed":"2026-06-03T04:05:06Z""#),
        "{json}"
    );
    assert!(json.contains(r#""revision":"12""#), "{json}");
    assert!(json.contains(r#""version":"1.2""#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_includes_custom_properties_in_json() {
    let doc = Document::open(&custom_properties_docx()).expect("fixture opens");
    let report = doc.report();

    assert_eq!(
        report
            .custom_properties
            .get("Client Name")
            .map(String::as_str),
        Some("ACME <Launch>")
    );
    assert_eq!(
        report.custom_properties.get("Phase").map(String::as_str),
        Some("Review & Ship")
    );

    let json = report.to_json();
    assert!(
        json.contains(
            r#""custom_properties":{"Client Name":"ACME <Launch>","Phase":"Review & Ship"}"#
        ),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_counts_docx_note_and_text_box_records() {
    let doc = Document::open(&docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:t>BODY</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p><w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>BOX</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>BOX</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:id="1"><w:p><w:r><w:t>FOOT</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="2"><w:p><w:r><w:t>END</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ]))
    .expect("fixture opens");
    let report = doc.report();

    assert_eq!(report.features.footnotes, 1);
    assert_eq!(report.features.endnotes, 1);
    assert_eq!(report.features.text_boxes, 1);
    let json = report.to_json();
    assert!(json.contains(r#""footnotes":1"#), "{json}");
    assert!(json.contains(r#""endnotes":1"#), "{json}");
    assert!(json.contains(r#""text_boxes":1"#), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn report_exposes_package_read_only_reason_for_lossy_opc_metadata() {
    let mut doc = Document::open(&lossy_metadata_docx()).expect("fixture opens");
    let report = doc.report();

    assert!(!report.edit.package_preserving);
    assert_eq!(
        report.edit.read_only_reasons,
        vec![EditReadOnlyReason::LossyOpcMetadata]
    );
    assert_eq!(doc.edit_capability(), report.edit);
    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        DocumentWarning::PackageReadOnly { reasons }
            if reasons == &vec![EditReadOnlyReason::LossyOpcMetadata]
    )));
    assert!(
        doc.replace_body_text("OLD", "NEW").is_err(),
        "lossy OPC metadata should make preservation editing read-only"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_can_be_serialized_as_compact_json() {
    let doc = Document::open(&complex_docx()).expect("fixture opens");
    let json = doc.report().to_json();

    assert!(json.starts_with(r#"{"format":"docx","#), "{json}");
    assert!(
        json.contains(r#""edit":{"package_preserving":true,"read_only_reasons":[]}"#),
        "{json}"
    );
    assert!(json.contains(r#""comments":1"#), "{json}");
    assert!(json.contains(r#""tracked_insertions":1"#), "{json}");
    assert!(json.contains(r#""tracked_property_changes":1"#), "{json}");
    assert!(json.contains(r#""fields":1"#), "{json}");
    assert!(json.contains(r#""hyperlinks":1"#), "{json}");
    assert!(
        json.contains(r#""field_kinds":[{"kind":"PAGE","count":1}]"#),
        "{json}"
    );
    assert!(
        !json.contains(r#""kind":"UnsupportedFieldEvaluation""#),
        "{json}"
    );
    assert!(
        json.contains(r#"{"kind":"TrackedChangesPresent","insertions":1,"deletions":1,"moves":1}"#),
        "{json}"
    );
    assert!(
        json.contains(r#"{"kind":"IncompleteRevisionView","property_changes":1}"#),
        "{json}"
    );
    assert!(json.ends_with("]}"), "{json}");
}

#[cfg(feature = "docx")]
#[test]
fn read_only_edit_capability_is_serialized_as_compact_json() {
    let doc = Document::open(&lossy_metadata_docx()).expect("fixture opens");
    let json = doc.report().to_json();

    assert!(
        json.contains(
            r#""edit":{"package_preserving":false,"read_only_reasons":["lossy_opc_metadata"]}"#
        ),
        "{json}"
    );
    assert!(
        json.contains(r#"{"kind":"PackageReadOnly","reasons":["lossy_opc_metadata"]}"#),
        "{json}"
    );
}

#[cfg(feature = "docx")]
#[test]
fn report_includes_edited_parts_after_preservation_edit() {
    let mut doc = Document::open(&editable_docx()).expect("fixture opens");

    assert_eq!(doc.replace_body_text("OLD", "NEW").unwrap(), 1);

    let report = doc.report();
    assert_eq!(report.edited_parts, vec!["word/document.xml"]);
    assert_eq!(report.edited_parts, doc.edited_parts());
    assert!(
        report
            .to_json()
            .contains(r#""edited_parts":["word/document.xml"]"#),
        "{}",
        report.to_json()
    );
}
