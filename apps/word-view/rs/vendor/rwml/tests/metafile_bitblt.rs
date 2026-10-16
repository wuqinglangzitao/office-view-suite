#![cfg(feature = "docx")]

use std::io::Write;

use rwml::{Document, DocumentWarning};

const SRCCOPY: u32 = 0x00CC_0020;

fn put_u16le(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32le(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_i32le(out: &mut [u8], offset: usize, value: i32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn rgb32_dib() -> Vec<u8> {
    let mut dib = vec![0u8; 48];
    put_u32le(&mut dib, 0, 40);
    put_i32le(&mut dib, 4, 2);
    put_i32le(&mut dib, 8, -1);
    put_u16le(&mut dib, 12, 1);
    put_u16le(&mut dib, 14, 32);
    put_u32le(&mut dib, 20, 8);
    dib[40..].copy_from_slice(&[0x33, 0x22, 0x11, 0x00, 0x66, 0x55, 0x44, 0x00]);
    dib
}

fn append_emf_eof(bytes: &mut Vec<u8>, record_count: u32) {
    let start = bytes.len();
    bytes.resize(start + 20, 0);
    put_u32le(bytes, start, 14);
    put_u32le(bytes, start + 4, 20);
    put_u32le(bytes, start + 16, 20);
    let byte_len = bytes.len() as u32;
    put_u32le(bytes, 48, byte_len);
    put_u32le(bytes, 52, record_count);
}

fn emf_source_blt(record_type: u32, raster_operation: u32) -> Vec<u8> {
    let fixed_size = match record_type {
        76 => 100,
        77 => 108,
        _ => panic!("unsupported test record"),
    };
    let dib = rgb32_dib();
    let mut bytes = vec![0u8; 88];
    put_u32le(&mut bytes, 0, 1);
    put_u32le(&mut bytes, 4, 88);
    put_i32le(&mut bytes, 16, 1);
    bytes[40..44].copy_from_slice(b" EMF");
    put_u32le(&mut bytes, 44, 0x0001_0000);

    let start = bytes.len();
    let record_size = fixed_size + dib.len();
    bytes.resize(start + record_size, 0);
    put_u32le(&mut bytes, start, record_type);
    put_u32le(&mut bytes, start + 4, record_size as u32);
    put_i32le(&mut bytes, start + 16, 1);
    put_i32le(&mut bytes, start + 32, 2);
    put_i32le(&mut bytes, start + 36, 1);
    put_u32le(&mut bytes, start + 40, raster_operation);
    put_u32le(&mut bytes, start + 52, 1.0f32.to_bits());
    put_u32le(&mut bytes, start + 64, 1.0f32.to_bits());
    put_u32le(&mut bytes, start + 84, fixed_size as u32);
    put_u32le(&mut bytes, start + 88, 40);
    put_u32le(&mut bytes, start + 92, (fixed_size + 40) as u32);
    put_u32le(&mut bytes, start + 96, 8);
    if record_type == 77 {
        put_i32le(&mut bytes, start + 100, 2);
        put_i32le(&mut bytes, start + 104, 1);
    }
    bytes[start + fixed_size..start + record_size].copy_from_slice(&dib);
    append_emf_eof(&mut bytes, 3);
    bytes
}

fn finalize_wmf(bytes: &mut [u8], max_record_words: usize) {
    put_u16le(bytes, 22, 1);
    put_u16le(bytes, 24, 9);
    put_u16le(bytes, 26, 0x0300);
    put_u32le(bytes, 28, ((bytes.len() - 22) / 2) as u32);
    put_u32le(bytes, 34, max_record_words as u32);
    let checksum = (0..20).step_by(2).fold(0u16, |value, offset| {
        value ^ u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    });
    put_u16le(bytes, 20, checksum);
}

fn wmf_source_dib_blt(function: u16, raster_operation: u32) -> Vec<u8> {
    let fixed_size = match function {
        0x0940 => 22,
        0x0B41 => 26,
        _ => panic!("unsupported test function"),
    };
    let dib = rgb32_dib();
    let mut bytes = vec![0u8; 40];
    put_u32le(&mut bytes, 0, 0x9AC6_CDD7);
    put_u16le(&mut bytes, 10, 2);
    put_u16le(&mut bytes, 12, 1);
    put_u16le(&mut bytes, 14, 96);

    let start = bytes.len();
    let record_size = fixed_size + dib.len();
    bytes.resize(start + record_size, 0);
    put_u32le(&mut bytes, start, (record_size / 2) as u32);
    put_u16le(&mut bytes, start + 4, function);
    put_u32le(&mut bytes, start + 6, raster_operation);
    match function {
        0x0940 => {
            put_u16le(&mut bytes, start + 14, 1);
            put_u16le(&mut bytes, start + 16, 2);
        }
        0x0B41 => {
            put_u16le(&mut bytes, start + 10, 1);
            put_u16le(&mut bytes, start + 12, 2);
            put_u16le(&mut bytes, start + 18, 1);
            put_u16le(&mut bytes, start + 20, 2);
        }
        _ => unreachable!(),
    }
    bytes[start + fixed_size..start + record_size].copy_from_slice(&dib);
    let eof = bytes.len();
    bytes.resize(eof + 6, 0);
    put_u32le(&mut bytes, eof, 3);
    finalize_wmf(&mut bytes, record_size / 2);
    bytes
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn metafile_docx(media_name: &str, media: &[u8]) -> Vec<u8> {
    let rels = format!(
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdMeta" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{media_name}"/></Relationships>"#
    );
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="9525"/><wp:docPr id="1" name="Picture 1"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="bitblt"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdMeta"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="19050" cy="9525"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
    let parts = [
        (
            "[Content_Types].xml".to_string(),
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="emf" ContentType="image/x-emf"/><Default Extension="wmf" ContentType="image/x-wmf"/><Default Extension="emz" ContentType="image/x-emz"/><Default Extension="wmz" ContentType="image/x-wmz"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels".to_string(),
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
        ),
        (
            "word/_rels/document.xml.rels".to_string(),
            rels.into_bytes(),
        ),
        ("word/document.xml".to_string(), document.as_bytes().to_vec()),
        (format!("word/media/{media_name}"), media.to_vec()),
    ];

    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(name, options).unwrap();
            zip.write_all(&body).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

#[test]
fn source_bearing_bitblt_metafiles_extract_through_document_and_report_apis() {
    let raw_cases = [
        ("bitblt.emf", emf_source_blt(76, SRCCOPY)),
        ("stretchblt.emf", emf_source_blt(77, SRCCOPY)),
        ("dibbitblt.wmf", wmf_source_dib_blt(0x0940, SRCCOPY)),
        ("dibstretchblt.wmf", wmf_source_dib_blt(0x0B41, SRCCOPY)),
    ];
    let compressed_cases = [
        ("bitblt.emz", gzip(&raw_cases[0].1)),
        ("stretchblt.emz", gzip(&raw_cases[1].1)),
        ("dibbitblt.wmz", gzip(&raw_cases[2].1)),
        ("dibstretchblt.wmz", gzip(&raw_cases[3].1)),
    ];

    for (name, bytes, compressed) in raw_cases
        .into_iter()
        .map(|(name, bytes)| (name, bytes, false))
        .chain(
            compressed_cases
                .into_iter()
                .map(|(name, bytes)| (name, bytes, true)),
        )
    {
        let doc = Document::open(&metafile_docx(name, &bytes)).expect("synthetic DOCX opens");
        let images = doc.images();
        assert_eq!(images.len(), 1, "{name}");
        assert_eq!(images[0].width_px, Some(2), "{name}");
        assert_eq!(images[0].height_px, Some(1), "{name}");
        assert_eq!(
            images[0].bytes.as_deref(),
            Some([0x11, 0x22, 0x33, 0xFF, 0x44, 0x55, 0x66, 0xFF,].as_slice()),
            "{name}"
        );

        let report = doc.report();
        assert_eq!(report.features.metafiles.len(), 1, "{name}");
        assert_eq!(report.features.unsupported_metafiles, 0, "{name}");
        assert_eq!(
            report.features.metafiles[0].compressed, compressed,
            "{name}"
        );
        assert_eq!(report.features.metafiles[0].width_px, Some(2), "{name}");
        assert_eq!(report.features.metafiles[0].height_px, Some(1), "{name}");
        assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
    }
}

#[cfg(feature = "render")]
#[test]
fn source_bearing_bitblt_rasters_render_without_metafile_warnings() {
    for (name, bytes) in [
        ("bitblt.emf", emf_source_blt(76, SRCCOPY)),
        ("dibstretchblt.wmf", wmf_source_dib_blt(0x0B41, SRCCOPY)),
    ] {
        let doc = Document::open(&metafile_docx(name, &bytes)).expect("synthetic DOCX opens");
        let rendered = doc.to_pdf_with_report();
        assert!(rendered.pdf.starts_with(b"%PDF"), "{name}");
        assert!(
            rendered.report.warnings.iter().all(|warning| !matches!(
                warning,
                rwml::RenderWarning::UnsupportedMetafileImages { .. }
            )),
            "{name}: {:?}",
            rendered.report.warnings
        );
    }
}

#[test]
fn unsupported_source_blt_semantics_remain_reported_instead_of_extracted() {
    let mut composed_emf = emf_source_blt(76, SRCCOPY);
    let eof = composed_emf.split_off(composed_emf.len() - 20);
    let start = composed_emf.len();
    composed_emf.resize(start + 20, 0);
    put_u32le(&mut composed_emf, start, 15);
    put_u32le(&mut composed_emf, start + 4, 20);
    composed_emf.extend_from_slice(&eof);
    let byte_len = composed_emf.len() as u32;
    put_u32le(&mut composed_emf, 48, byte_len);
    put_u32le(&mut composed_emf, 52, 4);

    for (name, bytes) in [
        ("composed.emf", composed_emf),
        ("non-srccopy.wmf", wmf_source_dib_blt(0x0940, 0x0066_0046)),
    ] {
        let doc = Document::open(&metafile_docx(name, &bytes)).expect("synthetic DOCX opens");
        assert!(doc.images().is_empty(), "{name}");
        let report = doc.report();
        assert_eq!(report.features.metafiles.len(), 1, "{name}");
        assert_eq!(report.features.unsupported_metafiles, 1, "{name}");
        assert!(report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::UnsupportedMetafileImages { count: 1 }
        )));
    }
}
