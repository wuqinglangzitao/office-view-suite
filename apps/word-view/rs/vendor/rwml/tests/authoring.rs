#![cfg(feature = "docx")]

//! Integration coverage of the public authoring + rendering entry points: a model
//! built from data must serialize to an Office-openable `.docx` (and re-open
//! through the reader) and render to a valid PDF.

use std::io::Read;

use rwml::{
    Align, Block, Cell, CellBuilder, CellMargins, CharProps, ChartBuilder, ChartKind, ChartShape,
    Color, CommentBuilder, ContentControlBuilder, DocBuilder, DocGridType, DocModel, DocSetup,
    Document, DocumentWarning, FieldKind, FieldRole, ImageBuilder, NoteKind, PageNumberFormat,
    PageSetup, ParaProps, Paragraph, ParagraphBuilder, ParagraphStyleBuilder, RevisionBuilder,
    RevisionKind, RevisionView, Row, RunBuilder, SectionBreakKind, Table, TableBorderSide,
    TableBorderStyle, TableBuilder, TextDirection, VCell,
};

fn run(text: &str, props: CharProps) -> rwml::Run {
    rwml::Run {
        text: text.to_string(),
        props,
        ..rwml::Run::default()
    }
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

fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36,
        0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x60,
        0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

fn tiny_jpeg() -> Vec<u8> {
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x03, 0x00, 0x02, 0x03,
        0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00, 0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01,
        0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x3F, 0x00, 0x00, 0xFF, 0xD9,
    ]
}

fn tiny_gif() -> Vec<u8> {
    vec![
        b'G', b'I', b'F', b'8', b'9', b'a', 0x02, 0x00, 0x03, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xff, 0xff, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x02,
        0x02, 0x44, 0x01, 0x00, 0x3b,
    ]
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

fn tiny_webp() -> Vec<u8> {
    vec![
        b'R', b'I', b'F', b'F', 22, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b'X', 10, 0,
        0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0,
    ]
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

fn single_paragraph_text(blocks: &[Block]) -> String {
    let [Block::Paragraph(paragraph)] = blocks else {
        panic!("expected exactly one paragraph block, got {blocks:?}");
    };
    paragraph.text()
}

fn plain_paragraph(text: &str) -> Paragraph {
    Paragraph {
        runs: vec![rwml::Run {
            text: text.to_string(),
            ..rwml::Run::default()
        }],
        ..Paragraph::default()
    }
}

fn report() -> DocModel {
    let navy = Color {
        r: 0x1F,
        g: 0x38,
        b: 0x64,
    };
    let white = Color {
        r: 0xFF,
        g: 0xFF,
        b: 0xFF,
    };
    let title = Block::Paragraph(Paragraph {
        props: ParaProps {
            heading_level: Some(1),
            align: Align::Center,
            ..ParaProps::default()
        },
        runs: vec![run(
            "분기 운영 리포트",
            CharProps {
                color: Some(navy),
                ..CharProps::default()
            },
        )],
    });
    let hdr = |t: &str| Cell {
        blocks: vec![Block::Paragraph(Paragraph {
            props: ParaProps::default(),
            runs: vec![run(
                t,
                CharProps {
                    bold: true,
                    color: Some(white),
                    ..CharProps::default()
                },
            )],
        })],
        shading: Some(navy),
        valign: VCell::Center,
        ..Cell::default()
    };
    let cell = |t: &str| Cell {
        blocks: vec![Block::Paragraph(Paragraph {
            props: ParaProps::default(),
            runs: vec![run(t, CharProps::default())],
        })],
        ..Cell::default()
    };
    let table = Table {
        rows: vec![
            Row {
                cells: vec![hdr("작업"), hdr("담당 부서")],
            },
            Row {
                cells: vec![cell("문서 변환 점검"), cell("플랫폼팀")],
            },
        ],
        header_rows: 1,
        col_widths_pct: vec![0.7, 0.3],
        width_pct: None,
        fixed_layout: false,
        indent_twips: None,
        align: None,
        bidi_visual: false,
        border_color: None,
        border_colors: Default::default(),
        border_size_eighths: None,
        border_sizes: Default::default(),
        border_style: None,
        border_styles: Default::default(),
    };
    DocModel {
        blocks: vec![title, Block::Table(table)],
        setup: DocSetup {
            title: Some("리포트".to_string()),
            ..DocSetup::default()
        },
        ..DocModel::default()
    }
}

#[test]
fn doc_builder_adds_lists_links_and_images() {
    let png = tiny_png();
    let model = DocBuilder::new()
        .numbered_list(["First", "Second"])
        .bullet_list(["Check"])
        .hyperlink("rwml site", "https://example.com/")
        .image(png.clone(), "image/png")
        .build();

    assert_eq!(model.blocks.len(), 5);
    let Block::Paragraph(first) = &model.blocks[0] else {
        panic!("numbered item should be a paragraph");
    };
    let list = first.props.list.as_ref().expect("numbered item has list");
    assert!(list.ordered);
    assert_eq!(list.level, 0);
    assert_eq!(list.label, "");

    let Block::Paragraph(bullet) = &model.blocks[2] else {
        panic!("bullet item should be a paragraph");
    };
    assert_eq!(bullet.props.list.as_ref().map(|l| l.ordered), Some(false));

    let Block::Paragraph(link) = &model.blocks[3] else {
        panic!("hyperlink should be a paragraph");
    };
    assert_eq!(link.runs[0].text, "rwml site");
    assert!(matches!(
        &link.runs[0].field,
        FieldRole::Hyperlink { url } if url == "https://example.com/"
    ));

    let Block::Image(image) = &model.blocks[4] else {
        panic!("image should be a block image");
    };
    assert_eq!(image.mime.as_deref(), Some("image/png"));
    assert_eq!(image.bytes.as_deref(), Some(png.as_slice()));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    assert!(
        document_xml.contains("<w:numPr>") && document_xml.contains("<w:hyperlink r:id="),
        "expected list and hyperlink XML: {document_xml}"
    );
    assert!(
        rels.contains("relationships/hyperlink")
            && rels.contains(r#"Target="https://example.com/""#)
            && rels.contains(r#"TargetMode="External""#)
            && rels.contains("relationships/image"),
        "expected hyperlink and image rels: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="image/png""#),
        "expected PNG content type: {content_types}"
    );
    assert_eq!(parts["word/media/image1.png"], png);

    let reopened = Document::open(&bytes).expect("builder-authored .docx reopens");
    let text = reopened.text();
    assert!(
        text.contains("First")
            && text.contains("Second")
            && text.contains("Check")
            && text.contains("rwml site"),
        "authored text lost: {text:?}"
    );
    assert_eq!(reopened.images().len(), 1);
}

#[test]
fn doc_builder_adds_leveled_lists() {
    let model = DocBuilder::new()
        .numbered_list(["Top"])
        .numbered_list_level(2, ["Deep numbered"])
        .bullet_list_level(1, ["Nested bullet"])
        .list_level(["Clamped"], true, 42)
        .build();

    assert_eq!(model.blocks.len(), 4);
    for (index, expected) in [(0, 0), (1, 2), (2, 1), (3, 8)] {
        let Block::Paragraph(paragraph) = &model.blocks[index] else {
            panic!("list item should be a paragraph");
        };
        assert_eq!(
            paragraph.props.list.as_ref().map(|list| list.level),
            Some(expected),
            "unexpected list level at block {index}"
        );
    }
    let Block::Paragraph(bullet) = &model.blocks[2] else {
        panic!("third item should be a paragraph");
    };
    assert_eq!(
        bullet.props.list.as_ref().map(|list| list.ordered),
        Some(false)
    );

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:ilvl w:val="2"/><w:numId w:val="1"/>"#)
            && document_xml.contains(r#"<w:ilvl w:val="1"/><w:numId w:val="2"/>"#)
            && document_xml.contains(r#"<w:ilvl w:val="8"/><w:numId w:val="1"/>"#),
        "leveled list XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("leveled-list .docx reopens");
    let levels: Vec<_> = reopened
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => paragraph.props.list.as_ref().map(|list| {
                (
                    paragraph.text(),
                    list.level,
                    list.ordered,
                    list.label.clone(),
                )
            }),
            _ => None,
        })
        .collect();
    assert_eq!(levels.len(), 4);
    assert_eq!(levels[1].0, "Deep numbered");
    assert_eq!(levels[1].1, 2);
    assert!(levels[1].2);
    assert_eq!(levels[2].0, "Nested bullet");
    assert_eq!(levels[2].1, 1);
    assert!(!levels[2].2);
    assert_eq!(levels[3].1, 8);
}

#[test]
fn doc_builder_adds_page_breaks() {
    let model = DocBuilder::new()
        .paragraph("Cover")
        .page_break()
        .heading(1, "Detail")
        .build();

    assert!(matches!(model.blocks[1], Block::PageBreak));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:br w:type="page"/>"#),
        "page break XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("page-break .docx reopens");
    assert!(matches!(reopened.model().blocks[1], Block::PageBreak));
    assert_eq!(
        reopened.to_markdown(),
        "Cover\n\n\\pagebreak\n\n# **Detail**"
    );
    assert_eq!(
        reopened.to_html(),
        "<p>Cover</p><hr class=\"page-break\"><h1><strong>Detail</strong></h1>"
    );
}

#[test]
fn doc_builder_adds_section_breaks_with_section_setup() {
    let cover_page = PageSetup {
        margin_pt: 72.0,
        ..PageSetup::default()
    };
    let detail_page = PageSetup {
        width_pt: 792.0,
        height_pt: 612.0,
        margin_pt: 36.0,
        landscape: true,
        ..PageSetup::default()
    };

    let model = DocBuilder::new()
        .page_setup(cover_page)
        .columns(2)
        .page_number_start(3)
        .page_number_format(PageNumberFormat::UpperRoman)
        .text_direction(TextDirection::TopToBottomRightToLeft)
        .header("Cover header")
        .paragraph("Cover")
        .section_break()
        .clear_header()
        .page_setup(detail_page)
        .columns(3)
        .page_number_start(7)
        .page_number_format(PageNumberFormat::DecimalZero)
        .text_direction(TextDirection::LeftToRightTopToBottomVertical)
        .header("Detail header")
        .paragraph("Detail")
        .build();

    assert_eq!(model.blocks.len(), 3);
    let Block::SectionBreak(section) = &model.blocks[1] else {
        panic!("second block should be a section break");
    };
    assert_eq!(section.page.margin_pt, 72.0);
    assert_eq!(section.columns, Some(2));
    assert_eq!(section.page_number_start, Some(3));
    assert_eq!(
        section.page_number_format,
        Some(PageNumberFormat::UpperRoman)
    );
    assert_eq!(
        section.text_direction,
        Some(TextDirection::TopToBottomRightToLeft)
    );
    assert_eq!(section.header.len(), 1);
    assert_eq!(model.setup.page.width_pt, 792.0);
    assert_eq!(model.setup.columns, Some(3));
    assert_eq!(model.setup.page_number_start, Some(7));
    assert_eq!(
        model.setup.page_number_format,
        Some(PageNumberFormat::DecimalZero)
    );
    assert_eq!(
        model.setup.text_direction,
        Some(TextDirection::LeftToRightTopToBottomVertical)
    );
    assert_eq!(model.setup.header.len(), 1);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let header1_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let header2_xml = String::from_utf8(parts["word/header2.xml"].clone()).unwrap();

    assert_eq!(
        document_xml.matches("<w:sectPr>").count(),
        2,
        "expected one section break plus final section properties: {document_xml}"
    );
    assert!(
        document_xml.contains(r#"<w:type w:val="nextPage"/>"#)
            && document_xml.contains(r#"<w:pgSz w:w="11906" w:h="16838"/>"#)
            && document_xml.contains(r#"<w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>"#)
            && document_xml.contains(r#"<w:pgNumType w:start="3" w:fmt="upperRoman"/>"#)
            && document_xml.contains(r#"<w:pgNumType w:start="7" w:fmt="decimalZero"/>"#)
            && document_xml.contains(r#"<w:textDirection w:val="tbRl"/>"#)
            && document_xml.contains(r#"<w:textDirection w:val="lrTbV"/>"#)
            && document_xml.contains(r#"<w:cols w:num="2"/>"#)
            && document_xml.contains(r#"<w:cols w:num="3"/>"#),
        "section page setup missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="header1.xml""#) && rels.contains(r#"Target="header2.xml""#),
        "section header relationships missing: {rels}"
    );
    assert!(
        header1_xml.contains("Cover header") && header2_xml.contains("Detail header"),
        "distinct section headers missing: header1={header1_xml}, header2={header2_xml}"
    );

    let reopened = Document::open(&bytes).expect("multi-section .docx reopens");
    let reopened_model = reopened.model();
    assert!(
        reopened_model
            .blocks
            .iter()
            .any(|block| matches!(block, Block::SectionBreak(_))),
        "section break lost on reopen"
    );
    assert_eq!(reopened_model.setup.page.width_pt, 792.0);
    assert_eq!(reopened_model.setup.columns, Some(3));
    assert_eq!(reopened_model.setup.page_number_start, Some(7));
    assert_eq!(
        reopened_model.setup.page_number_format,
        Some(PageNumberFormat::DecimalZero)
    );
    assert_eq!(
        reopened_model.setup.text_direction,
        Some(TextDirection::LeftToRightTopToBottomVertical)
    );
    let Some(Block::SectionBreak(reopened_section)) = reopened_model
        .blocks
        .iter()
        .find(|block| matches!(block, Block::SectionBreak(_)))
    else {
        panic!("section break lost on reopen");
    };
    assert_eq!(reopened_section.columns, Some(2));
    assert_eq!(reopened_section.page_number_start, Some(3));
    assert_eq!(
        reopened_section.page_number_format,
        Some(PageNumberFormat::UpperRoman)
    );
    assert_eq!(
        reopened_section.text_direction,
        Some(TextDirection::TopToBottomRightToLeft)
    );
    let text = reopened.text();
    assert!(
        text.contains("Cover header") && text.contains("Detail header"),
        "section headers lost from text view: {text:?}"
    );
}

#[test]
fn doc_builder_adds_even_and_odd_page_section_breaks() {
    let model = DocBuilder::new()
        .paragraph("Cover")
        .section_break_even_page()
        .paragraph("Even target")
        .section_break_odd_page()
        .paragraph("Odd target")
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:type w:val="evenPage"/>"#)
            && document_xml.contains(r#"<w:type w:val="oddPage"/>"#),
        "even/odd section break types missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("even/odd section break .docx reopens");
    let sections: Vec<_> = reopened
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::SectionBreak(setup) => setup.section_break,
            _ => None,
        })
        .collect();
    assert_eq!(
        sections,
        vec![SectionBreakKind::EvenPage, SectionBreakKind::OddPage]
    );
}

#[test]
fn doc_builder_adds_number_in_dash_page_number_format() {
    let model = DocBuilder::new()
        .page_number_start(5)
        .page_number_format(PageNumberFormat::NumberInDash)
        .paragraph("Dashed numbering")
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:pgNumType w:start="5" w:fmt="numberInDash"/>"#),
        "numberInDash page-number format missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("numberInDash .docx reopens");
    assert_eq!(
        reopened.model().setup.page_number_format,
        Some(PageNumberFormat::NumberInDash)
    );
}

#[test]
fn doc_builder_adds_decimal_full_width_page_number_format() {
    let model = DocBuilder::new()
        .page_number_start(12)
        .page_number_format(PageNumberFormat::DecimalFullWidth)
        .paragraph("Full-width numbering")
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:pgNumType w:start="12" w:fmt="decimalFullWidth"/>"#),
        "decimalFullWidth page-number format missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("decimalFullWidth .docx reopens");
    assert_eq!(
        reopened.model().setup.page_number_format,
        Some(PageNumberFormat::DecimalFullWidth)
    );
}

#[test]
fn doc_builder_adds_decimal_enclosed_circle_page_number_format() {
    let model = DocBuilder::new()
        .page_number_start(12)
        .page_number_format(PageNumberFormat::DecimalEnclosedCircle)
        .paragraph("Circled numbering")
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:pgNumType w:start="12" w:fmt="decimalEnclosedCircle"/>"#),
        "decimalEnclosedCircle page-number format missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("decimalEnclosedCircle .docx reopens");
    assert_eq!(
        reopened.model().setup.page_number_format,
        Some(PageNumberFormat::DecimalEnclosedCircle)
    );
}

#[test]
fn doc_builder_adds_decimal_enclosed_punctuation_page_number_formats() {
    for (format, wml, text) in [
        (
            PageNumberFormat::DecimalEnclosedFullstop,
            "decimalEnclosedFullstop",
            "Fullstop numbering",
        ),
        (
            PageNumberFormat::DecimalEnclosedParen,
            "decimalEnclosedParen",
            "Parenthesized numbering",
        ),
    ] {
        let model = DocBuilder::new()
            .page_number_start(12)
            .page_number_format(format)
            .paragraph(text)
            .build();

        let bytes = rwml::write_docx(&model);
        let parts = unzip_parts(&bytes);
        let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
        assert!(
            document_xml.contains(&format!(r#"<w:pgNumType w:start="12" w:fmt="{wml}"/>"#)),
            "{wml} page-number format missing: {document_xml}"
        );

        let reopened = Document::open(&bytes).expect("punctuation page format .docx reopens");
        assert_eq!(reopened.model().setup.page_number_format, Some(format));
    }
}

#[test]
fn doc_builder_adds_decimal_width_variant_page_number_formats() {
    for (format, wml, text) in [
        (
            PageNumberFormat::DecimalHalfWidth,
            "decimalHalfWidth",
            "Half-width numbering",
        ),
        (
            PageNumberFormat::DecimalFullWidth2,
            "decimalFullWidth2",
            "Full-width alternate numbering",
        ),
    ] {
        let model = DocBuilder::new()
            .page_number_start(12)
            .page_number_format(format)
            .paragraph(text)
            .build();

        let bytes = rwml::write_docx(&model);
        let parts = unzip_parts(&bytes);
        let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
        assert!(
            document_xml.contains(&format!(r#"<w:pgNumType w:start="12" w:fmt="{wml}"/>"#)),
            "{wml} page-number format missing: {document_xml}"
        );

        let reopened = Document::open(&bytes).expect("decimal width variant .docx reopens");
        assert_eq!(reopened.model().setup.page_number_format, Some(format));
    }
}

#[test]
fn doc_builder_adds_korean_page_number_formats() {
    for (format, wml, text) in [
        (PageNumberFormat::Ganada, "ganada", "Ganada numbering"),
        (PageNumberFormat::Chosung, "chosung", "Chosung numbering"),
        (
            PageNumberFormat::KoreanDigital,
            "koreanDigital",
            "Korean digital numbering",
        ),
        (
            PageNumberFormat::KoreanCounting,
            "koreanCounting",
            "Korean counting numbering",
        ),
        (
            PageNumberFormat::KoreanLegal,
            "koreanLegal",
            "Korean legal numbering",
        ),
        (
            PageNumberFormat::KoreanDigital2,
            "koreanDigital2",
            "Korean digital alternate numbering",
        ),
    ] {
        let model = DocBuilder::new()
            .page_number_start(1)
            .page_number_format(format)
            .paragraph(text)
            .build();

        let bytes = rwml::write_docx(&model);
        let parts = unzip_parts(&bytes);
        let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
        assert!(
            document_xml.contains(&format!(r#"<w:pgNumType w:start="1" w:fmt="{wml}"/>"#)),
            "{wml} page-number format missing: {document_xml}"
        );

        let reopened = Document::open(&bytes).expect("Korean page format .docx reopens");
        assert_eq!(reopened.model().setup.page_number_format, Some(format));
    }
}

#[test]
fn write_docx_emits_first_even_header_footer_variants() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(plain_paragraph("Default header"))],
            first_header: vec![Block::Paragraph(plain_paragraph("First header"))],
            even_header: vec![Block::Paragraph(plain_paragraph("Even header"))],
            footer: vec![Block::Paragraph(plain_paragraph("Default footer"))],
            first_footer: vec![Block::Paragraph(plain_paragraph("First footer"))],
            even_footer: vec![Block::Paragraph(plain_paragraph("Even footer"))],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let settings_xml = String::from_utf8(parts["word/settings.xml"].clone()).unwrap();

    assert!(
        document_xml.contains("<w:titlePg/>")
            && document_xml.contains(r#"<w:headerReference w:type="first""#)
            && document_xml.contains(r#"<w:headerReference w:type="even""#)
            && document_xml.contains(r#"<w:footerReference w:type="first""#)
            && document_xml.contains(r#"<w:footerReference w:type="even""#),
        "first/even section references missing: {document_xml}"
    );
    assert!(
        settings_xml.contains("<w:evenAndOddHeaders/>"),
        "even/odd settings missing: {settings_xml}"
    );

    let reopened = Document::open(&bytes).expect("variant header/footer .docx reopens");
    let setup = &reopened.model().setup;
    assert_eq!(single_paragraph_text(&setup.header), "Default header");
    assert_eq!(single_paragraph_text(&setup.first_header), "First header");
    assert_eq!(single_paragraph_text(&setup.even_header), "Even header");
    assert_eq!(single_paragraph_text(&setup.footer), "Default footer");
    assert_eq!(single_paragraph_text(&setup.first_footer), "First footer");
    assert_eq!(single_paragraph_text(&setup.even_footer), "Even footer");
}

#[test]
fn doc_builder_adds_first_even_header_footer_variants() {
    let model = DocBuilder::new()
        .header("Default header")
        .first_header("First header")
        .even_header("Even header")
        .footer("Default footer")
        .first_footer("First footer")
        .even_footer("Even footer")
        .paragraph("Body")
        .build();

    assert_eq!(single_paragraph_text(&model.setup.header), "Default header");
    assert_eq!(
        single_paragraph_text(&model.setup.first_header),
        "First header"
    );
    assert_eq!(
        single_paragraph_text(&model.setup.even_header),
        "Even header"
    );
    assert_eq!(single_paragraph_text(&model.setup.footer), "Default footer");
    assert_eq!(
        single_paragraph_text(&model.setup.first_footer),
        "First footer"
    );
    assert_eq!(
        single_paragraph_text(&model.setup.even_footer),
        "Even footer"
    );

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let settings_xml = String::from_utf8(parts["word/settings.xml"].clone()).unwrap();

    assert!(
        document_xml.contains("<w:titlePg/>")
            && document_xml.contains(r#"<w:headerReference w:type="first""#)
            && document_xml.contains(r#"<w:headerReference w:type="even""#)
            && document_xml.contains(r#"<w:footerReference w:type="first""#)
            && document_xml.contains(r#"<w:footerReference w:type="even""#),
        "builder first/even references missing: {document_xml}"
    );
    assert!(
        settings_xml.contains("<w:evenAndOddHeaders/>"),
        "builder even/odd settings missing: {settings_xml}"
    );

    let reopened = Document::open(&bytes).expect("builder variant header/footer .docx reopens");
    let setup = &reopened.model().setup;
    assert_eq!(single_paragraph_text(&setup.first_header), "First header");
    assert_eq!(single_paragraph_text(&setup.even_header), "Even header");
    assert_eq!(single_paragraph_text(&setup.first_footer), "First footer");
    assert_eq!(single_paragraph_text(&setup.even_footer), "Even footer");
}

#[test]
fn doc_builder_clear_header_footer_removes_all_variants() {
    let model = DocBuilder::new()
        .header("Default header")
        .first_header("First header")
        .even_header("Even header")
        .footer("Default footer")
        .first_footer("First footer")
        .even_footer("Even footer")
        .clear_header_footer()
        .paragraph("Body")
        .build();

    assert!(model.setup.header.is_empty());
    assert!(model.setup.first_header.is_empty());
    assert!(model.setup.even_header.is_empty());
    assert!(model.setup.footer.is_empty());
    assert!(model.setup.first_footer.is_empty());
    assert!(model.setup.even_footer.is_empty());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert!(!parts.keys().any(|name| name.starts_with("word/header")));
    assert!(!parts.keys().any(|name| name.starts_with("word/footer")));

    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = parts
        .get("word/_rels/document.xml.rels")
        .map(|bytes| String::from_utf8(bytes.clone()).unwrap())
        .unwrap_or_default();
    assert!(
        !document_xml.contains("headerReference") && !document_xml.contains("footerReference"),
        "header/footer references should be absent: {document_xml}"
    );
    assert!(
        !rels.contains("relationships/header") && !rels.contains("relationships/footer"),
        "header/footer relationships should be absent: {rels}"
    );
}

#[test]
fn doc_builder_adds_section_columns() {
    let model = DocBuilder::new()
        .columns(2)
        .paragraph("Column body")
        .build();

    assert_eq!(model.setup.columns, Some(2));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<w:cols w:num="2"/>"#),
        "section columns missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("column section .docx reopens");
    assert_eq!(reopened.model().setup.columns, Some(2));
}

#[test]
fn doc_builder_adds_line_document_grid() {
    let model = DocBuilder::new()
        .doc_grid_lines(360)
        .paragraph("Grid body")
        .build();

    let grid = model.setup.doc_grid.expect("doc grid");
    assert_eq!(grid.grid_type, DocGridType::Lines);
    assert_eq!(grid.line_pitch, Some(360));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<w:docGrid w:type="lines" w:linePitch="360"/>"#),
        "document grid missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("document-grid section .docx reopens");
    assert_eq!(reopened.model().setup.doc_grid, Some(grid));
}

#[test]
fn doc_builder_adds_line_and_character_document_grid() {
    let model = DocBuilder::new()
        .doc_grid_lines_and_chars(360, 40960)
        .paragraph("Grid body")
        .build();

    let grid = model.setup.doc_grid.expect("doc grid");
    assert_eq!(grid.grid_type, DocGridType::LinesAndChars);
    assert_eq!(grid.line_pitch, Some(360));
    assert_eq!(grid.character_space, Some(40960));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<w:docGrid w:type="linesAndChars" w:linePitch="360" w:charSpace="40960"/>"#
        ),
        "line-and-character document grid missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("line-and-character grid .docx reopens");
    assert_eq!(reopened.model().setup.doc_grid, Some(grid));
}

#[test]
fn doc_builder_adds_character_document_grid() {
    let model = DocBuilder::new()
        .doc_grid_snap_to_chars(40960)
        .paragraph("Grid body")
        .build();

    let grid = model.setup.doc_grid.expect("doc grid");
    assert_eq!(grid.grid_type, DocGridType::SnapToChars);
    assert_eq!(grid.character_space, Some(40960));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<w:docGrid w:type="snapToChars" w:charSpace="40960"/>"#),
        "character document grid missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("character grid .docx reopens");
    assert_eq!(reopened.model().setup.doc_grid, Some(grid));
}

#[test]
fn doc_builder_adds_explicit_title_page_section_option() {
    let model = DocBuilder::new()
        .title_page()
        .header("Default header")
        .paragraph("Body")
        .build();

    assert!(model.setup.title_page);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        document_xml.contains("<w:titlePg/>"),
        "title page section option missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("title-page section .docx reopens");
    assert!(reopened.model().setup.title_page);
}

#[test]
fn run_builder_adds_simple_field_runs() {
    let model = DocBuilder::new()
        .paragraph_runs([
            RunBuilder::new("File: ").build(),
            RunBuilder::new("report.docx").field("FILENAME \\p").build(),
        ])
        .field("PAGE", "1")
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(
        paragraph.runs[1].field,
        FieldRole::Simple {
            instruction: "FILENAME \\p".to_string()
        }
    );

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:fldSimple w:instr=" FILENAME \p ">"#)
            && document_xml.contains(r#"<w:fldSimple w:instr=" PAGE ">"#)
            && document_xml.contains("report.docx"),
        "simple field XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("field-authored .docx reopens");
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Filename);
    assert_eq!(fields[0].instruction, "FILENAME \\p");
    assert_eq!(fields[0].result, "report.docx");
    assert_eq!(fields[1].kind, FieldKind::Page);
    assert_eq!(fields[1].instruction, "PAGE");
    assert_eq!(fields[1].result, "1");

    let Block::Paragraph(reopened_paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened paragraph");
    };
    assert!(matches!(
        &reopened_paragraph.runs[1].field,
        FieldRole::Simple { instruction } if instruction == "FILENAME \\p"
    ));
}

#[test]
fn field_helpers_skip_empty_instructions() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("raw").field(" \t\r\n ").build()])
        .field("   ", "cached")
        .build();

    let Block::Paragraph(run_paragraph) = &model.blocks[0] else {
        panic!("expected run paragraph");
    };
    assert!(matches!(run_paragraph.runs[0].field, FieldRole::None));

    let Block::Paragraph(doc_paragraph) = &model.blocks[1] else {
        panic!("expected doc field paragraph");
    };
    assert!(matches!(doc_paragraph.runs[0].field, FieldRole::None));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        !document_xml.contains("<w:fldSimple"),
        "empty field instructions should serialize as plain runs: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("empty-field .docx reopens");
    assert_eq!(reopened.fields().len(), 0);
    assert!(reopened.text().contains("raw"));
    assert!(reopened.text().contains("cached"));
}

#[test]
fn doc_builder_adds_dirty_toc_heading_range() {
    let model = DocBuilder::new()
        .heading(1, "Executive Summary")
        .heading(3, "Detail")
        .heading(4, "Appendix Detail")
        .toc_heading_range(1, 3)
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[3] else {
        panic!("expected TOC paragraph");
    };
    assert_eq!(
        paragraph.runs[0].field,
        FieldRole::Simple {
            instruction: "TOC \\o \"1-3\"".to_string()
        }
    );
    assert!(paragraph.runs[0].field_dirty);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:fldSimple w:instr=" TOC \o &quot;1-3&quot; " w:dirty="true">"#),
        "dirty TOC field XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("TOC-authored .docx reopens");
    let fields = reopened.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Toc);
    assert_eq!(fields[0].instruction, "TOC \\o \"1-3\"");
    assert_eq!(fields[0].result, "Contents");
    let computed = fields[0]
        .computed_result
        .as_deref()
        .expect("TOC should compute from authored headings");
    assert!(computed.contains("Executive Summary"), "{computed:?}");
    assert!(computed.contains("Detail"), "{computed:?}");
    assert!(!computed.contains("Appendix Detail"), "{computed:?}");
    assert!(reopened
        .report()
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[test]
fn doc_builder_adds_string_custom_properties() {
    let model = DocBuilder::new()
        .custom_property(" Client Name ", "ACME <Launch>")
        .field(r#"DOCPROPERTY "Client Name""#, "cached")
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let root_rels = String::from_utf8(parts["_rels/.rels"].clone()).unwrap();
    let custom_xml = String::from_utf8(parts["docProps/custom.xml"].clone()).unwrap();

    assert!(
        content_types.contains(
            r#"<Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/>"#
        ),
        "custom properties content type missing: {content_types}"
    );
    assert!(
        root_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml""#
        ),
        "custom properties relationship missing: {root_rels}"
    );
    assert!(
        custom_xml
            .contains(r#"pid="2" name="Client Name"><vt:lpwstr>ACME &lt;Launch&gt;</vt:lpwstr>"#),
        "custom property XML missing: {custom_xml}"
    );

    let reopened = Document::open(&bytes).expect("custom-property .docx reopens");
    assert_eq!(
        reopened
            .model()
            .custom_properties
            .get("Client Name")
            .map(String::as_str),
        Some("ACME <Launch>")
    );
    let fields = reopened.fields();
    assert_eq!(fields[0].computed_result.as_deref(), Some("ACME <Launch>"));
}

#[test]
fn doc_builder_ignores_blank_custom_property_names() {
    let model = DocBuilder::new().custom_property(" ", "ignored").build();

    assert!(model.custom_properties.is_empty());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert!(
        !parts.contains_key("docProps/custom.xml"),
        "blank custom property names should not emit a custom properties part"
    );
}

#[test]
fn doc_builder_adds_document_id_setting() {
    let model = DocBuilder::new()
        .document_id(" 6ECD4467 ")
        .paragraph("Body")
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let settings_xml = String::from_utf8(parts["word/settings.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    assert!(
        settings_xml.contains(r#"<w14:docId w14:val="6ECD4467"/>"#),
        "document id setting missing: {settings_xml}"
    );
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml""#
        ),
        "settings relationship missing: {rels}"
    );

    let reopened = Document::open(&bytes).expect("document-id .docx reopens");
    assert_eq!(
        reopened.model().setup.document_id.as_deref(),
        Some("6ECD4467")
    );
}

#[test]
fn doc_builder_adds_web_extension_task_pane() {
    let model = DocBuilder::new()
        .web_extension_task_pane(
            " {52811C31-4593-43B8-A697-EB873422D156} ",
            " af8fa5ba-4010-4bcc-9e03-a91ddadf6dd3 ",
            " 1.0.0.0 ",
            " EXCatalog ",
            " EXCatalog ",
        )
        .paragraph("Body")
        .build();

    assert_eq!(model.setup.web_extension_task_panes.len(), 1);
    let pane = &model.setup.web_extension_task_panes[0];
    assert_eq!(pane.extension_id, "{52811C31-4593-43B8-A697-EB873422D156}");
    assert_eq!(pane.reference_id, "af8fa5ba-4010-4bcc-9e03-a91ddadf6dd3");
    assert_eq!(pane.version, "1.0.0.0");
    assert_eq!(pane.store, "EXCatalog");
    assert_eq!(pane.store_type, "EXCatalog");

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let root_rels = String::from_utf8(parts["_rels/.rels"].clone()).unwrap();
    let taskpanes_xml =
        String::from_utf8(parts["word/webextensions/taskpanes.xml"].clone()).unwrap();
    let taskpanes_rels =
        String::from_utf8(parts["word/webextensions/_rels/taskpanes.xml.rels"].clone()).unwrap();
    let webextension_xml =
        String::from_utf8(parts["word/webextensions/webextension1.xml"].clone()).unwrap();

    assert!(
        content_types.contains(
            r#"<Override PartName="/word/webextensions/taskpanes.xml" ContentType="application/vnd.ms-office.webextensiontaskpanes+xml"/>"#
        ) && content_types.contains(
            r#"<Override PartName="/word/webextensions/webextension1.xml" ContentType="application/vnd.ms-office.webextension+xml"/>"#
        ),
        "web extension content types missing: {content_types}"
    );
    assert!(
        root_rels.contains(
            r#"Type="http://schemas.microsoft.com/office/2011/relationships/webextensiontaskpanes" Target="word/webextensions/taskpanes.xml""#
        ),
        "web extension taskpanes root relationship missing: {root_rels}"
    );
    assert!(
        taskpanes_rels.contains(
            r#"Type="http://schemas.microsoft.com/office/2011/relationships/webextension" Target="webextension1.xml""#
        ),
        "web extension relationship missing: {taskpanes_rels}"
    );
    assert!(
        taskpanes_xml.contains(r#"<wetp:taskpane dockstate="right" visibility="1" width="350" row="0">"#)
            && taskpanes_xml.contains(r#"<wetp:webextension xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rId1"/>"#),
        "taskpanes XML missing authored pane: {taskpanes_xml}"
    );
    assert!(
        webextension_xml.contains(
            r#"<we:webextension xmlns:we="http://schemas.microsoft.com/office/webextensions/webextension/2010/11" id="{52811C31-4593-43B8-A697-EB873422D156}">"#
        ) && webextension_xml.contains(
            r#"<we:reference id="af8fa5ba-4010-4bcc-9e03-a91ddadf6dd3" version="1.0.0.0" store="EXCatalog" storeType="EXCatalog"/>"#
        ) && webextension_xml.contains(
            r#"<we:property name="Office.AutoShowTaskpaneWithDocument" value="true"/>"#
        ),
        "webextension XML missing reference/properties: {webextension_xml}"
    );

    let reopened = Document::open(&bytes).expect("web-extension taskpane .docx reopens");
    assert_eq!(reopened.text(), "Body");
    let saved = reopened
        .save()
        .expect("web-extension taskpane package saves");
    let saved_parts = unzip_parts(&saved);
    assert_eq!(
        saved_parts["word/webextensions/taskpanes.xml"],
        parts["word/webextensions/taskpanes.xml"]
    );
    assert_eq!(
        saved_parts["word/webextensions/webextension1.xml"],
        parts["word/webextensions/webextension1.xml"]
    );
}

#[test]
fn doc_builder_ignores_blank_web_extension_task_panes() {
    let mut model = DocBuilder::new()
        .web_extension_task_pane(" ", "ref", "1.0.0.0", "store", "storeType")
        .paragraph("Body")
        .build();

    assert!(model.setup.web_extension_task_panes.is_empty());

    model
        .setup
        .web_extension_task_panes
        .push(rwml::WebExtensionTaskPane {
            extension_id: "{52811C31-4593-43B8-A697-EB873422D156}".to_string(),
            reference_id: " ".to_string(),
            version: "1.0.0.0".to_string(),
            store: "EXCatalog".to_string(),
            store_type: "EXCatalog".to_string(),
            ..rwml::WebExtensionTaskPane::default()
        });

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert!(!parts.contains_key("word/webextensions/taskpanes.xml"));
    assert!(!parts.contains_key("word/webextensions/webextension1.xml"));
    let root_rels = String::from_utf8(parts["_rels/.rels"].clone()).unwrap();
    assert!(
        !root_rels.contains("relationships/webextensiontaskpanes"),
        "blank web extension pane should not emit root relationship: {root_rels}"
    );
}

#[test]
fn run_builder_adds_bookmark_for_ref_fields() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Figure 1").bookmark(" Figure1 ").build()])
        .field("REF Figure1", "stale")
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected bookmark paragraph");
    };
    assert_eq!(paragraph.runs[0].bookmark.as_deref(), Some("Figure1"));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let start = document_xml
        .find(r#"<w:bookmarkStart w:id="0" w:name="Figure1"/>"#)
        .unwrap_or(usize::MAX);
    let text = document_xml
        .find(r#"<w:t xml:space="preserve">Figure 1</w:t>"#)
        .unwrap_or(usize::MAX);
    let end = document_xml
        .find(r#"<w:bookmarkEnd w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    let field = document_xml
        .find(r#"<w:fldSimple w:instr=" REF Figure1 ">"#)
        .unwrap_or(usize::MAX);
    assert!(
        start < text && text < end && end < field,
        "bookmark XML missing or out of order: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("bookmark-authored .docx reopens");
    let fields = reopened.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].instruction, "REF Figure1");
    assert_eq!(fields[0].result, "stale");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Figure 1"));
    assert!(
        !reopened.main_text().contains("stale"),
        "resolved REF should display computed bookmark text"
    );
    assert!(reopened
        .report()
        .warnings
        .iter()
        .all(|warning| !matches!(warning, DocumentWarning::UnsupportedFieldEvaluation { .. })));
}

#[test]
fn run_builder_skips_unreferenceable_bookmark_names() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Figure 1").bookmark("Figure 1").build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected bookmark paragraph");
    };
    assert_eq!(paragraph.runs[0].bookmark, None);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:t xml:space="preserve">Figure 1</w:t>"#)
            && !document_xml.contains("<w:bookmarkStart")
            && !document_xml.contains("<w:bookmarkEnd"),
        "invalid bookmark name should not be serialized: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("bookmark-sanitized .docx reopens");
    assert_eq!(reopened.main_text(), "Figure 1");
}

#[test]
fn run_builder_adds_page_ref_field() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Figure 1").bookmark("Figure1").build()])
        .paragraph_runs([RunBuilder::new("3").page_ref(" Figure1 ").build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[1] else {
        panic!("expected page-ref paragraph");
    };
    assert!(matches!(
        &paragraph.runs[0].field,
        FieldRole::Simple { instruction } if instruction == "PAGEREF Figure1 \\h"
    ));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:fldSimple w:instr=" PAGEREF Figure1 \h ">"#)
            && document_xml.contains(r#"<w:t xml:space="preserve">3</w:t>"#),
        "PAGEREF field missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("page-ref-authored .docx reopens");
    let fields = reopened.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PageRef);
    assert_eq!(fields[0].instruction, "PAGEREF Figure1 \\h");
    assert_eq!(fields[0].result, "3");
}

#[test]
fn run_builder_skips_unreferenceable_page_ref_targets() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("3").page_ref("Figure 1").build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected page-ref paragraph");
    };
    assert!(matches!(paragraph.runs[0].field, FieldRole::None));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:t xml:space="preserve">3</w:t>"#)
            && !document_xml.contains("<w:fldSimple"),
        "invalid PAGEREF target should not be serialized as a field: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("page-ref-sanitized .docx reopens");
    assert_eq!(reopened.fields().len(), 0);
    assert_eq!(reopened.main_text(), "3");
}

#[test]
fn run_builder_adds_inline_hyperlink_runs() {
    let model = DocBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new().runs([
                RunBuilder::new("See ").build(),
                RunBuilder::new("rwml")
                    .hyperlink("https://example.com/rwml?x=1&y=2")
                    .bold()
                    .build(),
                RunBuilder::new(" now").build(),
            ]),
        )
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(paragraph.text(), "See rwml now");
    assert!(matches!(
        &paragraph.runs[1].field,
        FieldRole::Hyperlink { url } if url == "https://example.com/rwml?x=1&y=2"
    ));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let see_pos = document_xml
        .find(r#"<w:t xml:space="preserve">See </w:t>"#)
        .expect("leading text run");
    let link_pos = document_xml
        .find(r#"<w:hyperlink r:id="rId"#)
        .expect("hyperlink wrapper");
    let text_pos = document_xml.find(">rwml</w:t>").expect("hyperlink text");
    let close_pos = document_xml
        .find("</w:hyperlink>")
        .expect("hyperlink close");
    let now_pos = document_xml
        .find(r#"<w:t xml:space="preserve"> now</w:t>"#)
        .expect("trailing text run");
    assert!(
        see_pos < link_pos
            && link_pos < text_pos
            && text_pos < close_pos
            && close_pos < now_pos
            && document_xml.contains("<w:b/>"),
        "inline hyperlink XML missing or not in run order: {document_xml}"
    );
    assert!(
        rels.contains("relationships/hyperlink")
            && rels.contains(r#"Target="https://example.com/rwml?x=1&amp;y=2""#)
            && rels.contains(r#"TargetMode="External""#),
        "hyperlink relationship missing: {rels}"
    );

    let reopened = Document::open(&bytes).expect("inline-hyperlink .docx reopens");
    assert_eq!(reopened.text(), "See rwml now");
    let Block::Paragraph(reopened_paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened paragraph");
    };
    assert!(matches!(
        &reopened_paragraph.runs[1].field,
        FieldRole::Hyperlink { url } if url == "https://example.com/rwml?x=1&y=2"
    ));
}

#[test]
fn run_builder_adds_internal_anchor_hyperlinks_without_relationships() {
    let model = DocBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new().runs([
                RunBuilder::new("Target").bookmark("SectionOne").build(),
                RunBuilder::new(" jump")
                    .hyperlink(" #SectionOne ")
                    .bold()
                    .build(),
                RunBuilder::new(" invalid").hyperlink("#Bad Target").build(),
            ]),
        )
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = parts
        .get("word/_rels/document.xml.rels")
        .map(|bytes| std::str::from_utf8(bytes).unwrap())
        .unwrap_or_default();
    assert!(
        document_xml.contains(r#"<w:bookmarkStart w:id="0" w:name="SectionOne"/>"#)
            && document_xml.contains(r#"<w:hyperlink w:anchor="SectionOne"><w:r>"#)
            && document_xml.contains("<w:b/>")
            && document_xml.contains("> jump</w:t></w:r></w:hyperlink>")
            && document_xml.contains("> invalid</w:t>")
            && !document_xml.contains(r#"w:anchor="Bad Target""#),
        "internal hyperlink XML missing or malformed target retained: {document_xml}"
    );
    assert_eq!(document_xml.matches("<w:hyperlink").count(), 1);
    assert!(
        !rels.contains("relationships/hyperlink") && !rels.contains("#SectionOne"),
        "internal hyperlinks must not create external relationships: {rels}"
    );

    let reopened = Document::open(&bytes).expect("internal-anchor .docx reopens");
    let Block::Paragraph(paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened paragraph");
    };
    assert_eq!(paragraph.runs[0].bookmark.as_deref(), Some("SectionOne"));
    assert!(matches!(
        &paragraph.runs[1].field,
        FieldRole::Hyperlink { url } if url == "#SectionOne"
    ));
    assert!(matches!(paragraph.runs[2].field, FieldRole::None));
}

#[test]
fn builders_ignore_blank_hyperlink_targets() {
    let model = DocBuilder::new()
        .hyperlink("doc link", " ")
        .rich_paragraph(
            ParagraphBuilder::new().runs([RunBuilder::new("inline link").hyperlink("\t").build()]),
        )
        .build();

    let Block::Paragraph(doc_link) = &model.blocks[0] else {
        panic!("expected doc-builder paragraph");
    };
    assert!(matches!(doc_link.runs[0].field, FieldRole::None));
    let Block::Paragraph(inline_link) = &model.blocks[1] else {
        panic!("expected run-builder paragraph");
    };
    assert!(matches!(inline_link.runs[0].field, FieldRole::None));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        !document_xml.contains("<w:hyperlink"),
        "blank hyperlink targets should serialize as plain text: {document_xml}"
    );
    assert!(!parts.contains_key("word/_rels/document.xml.rels"));

    let reopened = Document::open(&bytes).expect("blank-hyperlink .docx reopens");
    assert_eq!(reopened.text(), "doc link\ninline link");
    let reopened_model = reopened.model();
    assert!(reopened_model.blocks.iter().all(|block| match block {
        Block::Paragraph(paragraph) => paragraph
            .runs
            .iter()
            .all(|run| matches!(run.field, FieldRole::None)),
        _ => true,
    }));
}

#[test]
fn run_builder_adds_authored_comment() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Reviewed clause")
            .comment(
                CommentBuilder::new("Check <risk> & owner")
                    .author(" Reviewer ")
                    .initials(" ")
                    .date(" 2026-06-24T00:00:00Z "),
            )
            .build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    let comment = paragraph.runs[0]
        .comment
        .as_ref()
        .expect("run carries authored comment");
    assert_eq!(comment.text, "Check <risk> & owner");
    assert_eq!(comment.author.as_deref(), Some("Reviewer"));
    assert_eq!(comment.initials, None);
    assert_eq!(comment.date.as_deref(), Some("2026-06-24T00:00:00Z"));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let comments_xml = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    let start = document_xml
        .find(r#"<w:commentRangeStart w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    let anchor = document_xml
        .find(r#"<w:t xml:space="preserve">Reviewed clause</w:t>"#)
        .unwrap_or(usize::MAX);
    let end = document_xml
        .find(r#"<w:commentRangeEnd w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    let reference = document_xml
        .find(r#"<w:commentReference w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    assert!(
        start < anchor && anchor < end && end < reference,
        "comment markers missing or out of order: {document_xml}"
    );
    assert!(
        comments_xml
            .contains(r#"<w:comment w:id="0" w:author="Reviewer" w:date="2026-06-24T00:00:00Z">"#)
            && comments_xml.contains(r#"<w:t>Check &lt;risk&gt; &amp; owner</w:t>"#),
        "comments.xml missing authored metadata/text: {comments_xml}"
    );
    assert!(
        rels.contains("relationships/comments") && rels.contains(r#"Target="comments.xml""#),
        "comments relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"PartName="/word/comments.xml""#)
            && content_types.contains("wordprocessingml.comments+xml"),
        "comments content type missing: {content_types}"
    );

    let reopened = Document::open(&bytes).expect("comment-authored .docx reopens");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, "0");
    assert_eq!(comments[0].author.as_deref(), Some("Reviewer"));
    assert_eq!(comments[0].initials, None);
    assert_eq!(comments[0].date.as_deref(), Some("2026-06-24T00:00:00Z"));
    assert_eq!(comments[0].text, "Check <risk> & owner");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Reviewed clause")
    );
}

#[test]
fn run_builder_adds_comment_reply_parent_id() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Original clause")
            .comment(CommentBuilder::new("Original note").author("Reviewer"))
            .build()])
        .paragraph_runs([RunBuilder::new("Reply clause")
            .comment(
                CommentBuilder::new("Reply note")
                    .author("Approver")
                    .parent_comment_id(" 0 "),
            )
            .build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[1] else {
        panic!("expected reply paragraph");
    };
    assert_eq!(
        paragraph.runs[0]
            .comment
            .as_ref()
            .and_then(|comment| comment.parent_comment_id.as_deref()),
        Some("0")
    );

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let comments_xml = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    let comments_ex_xml = String::from_utf8(parts["word/commentsExtended.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        comments_xml.contains(r#"<w:comment w:id="1" w:author="Approver" w:parentId="0">"#)
            && comments_xml.contains(r#"<w:t>Reply note</w:t>"#),
        "reply parent id missing: {comments_xml}"
    );
    assert!(
        comments_xml.contains(r#"<w:p w14:paraId="00000001">"#)
            && comments_xml.contains(r#"<w:p w14:paraId="00000002">"#),
        "comment paragraphs should carry ids for reply threading: {comments_xml}"
    );
    assert!(
        comments_ex_xml.contains(r#"<w15:commentEx w15:paraId="00000001" w15:done="0"/>"#)
            && comments_ex_xml.contains(
                r#"<w15:commentEx w15:paraId="00000002" w15:paraIdParent="00000001" w15:done="0"/>"#
            ),
        "commentsExtended.xml missing reply threading metadata: {comments_ex_xml}"
    );
    assert!(
        rels.contains("relationships/commentsExtended")
            && rels.contains(r#"Target="commentsExtended.xml""#),
        "commentsExtended relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"PartName="/word/commentsExtended.xml""#)
            && content_types.contains("application/vnd.ms-word.commentsExt+xml"),
        "commentsExtended content type missing: {content_types}"
    );

    let reopened = Document::open(&bytes).expect("comment reply .docx reopens");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[1].id, "1");
    assert_eq!(comments[1].parent_comment_id.as_deref(), Some("0"));
}

#[test]
fn run_builder_ignores_blank_comment_reply_parent_id() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Reply clause")
            .comment(
                CommentBuilder::new("Reply note")
                    .author("Approver")
                    .parent_comment_id(" "),
            )
            .build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(paragraph.runs[0]
        .comment
        .as_ref()
        .and_then(|comment| comment.parent_comment_id.as_deref())
        .is_none());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert!(!parts.contains_key("word/commentsExtended.xml"));

    let reopened = Document::open(&bytes).expect("blank reply parent comment .docx reopens");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].parent_comment_id, None);
}

#[test]
fn run_builder_authored_comment_writes_tabs_and_breaks() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Reviewed clause")
            .comment(CommentBuilder::new("Line 1\nLine\t2").author("Reviewer"))
            .build()])
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let comments_xml = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();

    assert!(
        comments_xml.contains(r#"<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"#),
        "authored comment text should encode tabs and breaks as WML markers: {comments_xml}"
    );

    let reopened = Document::open(&bytes).expect("comment-authored .docx reopens");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Line 1\nLine\t2");
}

#[test]
fn run_builder_adds_authored_notes() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Clause")
            .footnote("Foot <one> & two")
            .build()])
        .paragraph_runs([RunBuilder::new("Appendix").endnote("End\nLine\t2").build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected first paragraph");
    };
    let Block::Paragraph(second_paragraph) = &model.blocks[1] else {
        panic!("expected second paragraph");
    };
    assert_eq!(
        paragraph.runs[0].note.as_ref().map(|note| note.kind),
        Some(NoteKind::Footnote)
    );
    assert_eq!(
        second_paragraph.runs[0].note.as_ref().map(|note| note.kind),
        Some(NoteKind::Endnote)
    );

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let footnotes_xml = String::from_utf8(parts["word/footnotes.xml"].clone()).unwrap();
    let endnotes_xml = String::from_utf8(parts["word/endnotes.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    let clause_pos = document_xml
        .find(r#"<w:t xml:space="preserve">Clause</w:t>"#)
        .unwrap_or(usize::MAX);
    let footnote_ref_pos = document_xml
        .find(r#"<w:footnoteReference w:id="1"/>"#)
        .unwrap_or(usize::MAX);
    let appendix_pos = document_xml
        .find(r#"<w:t xml:space="preserve">Appendix</w:t>"#)
        .unwrap_or(usize::MAX);
    let endnote_ref_pos = document_xml
        .find(r#"<w:endnoteReference w:id="1"/>"#)
        .unwrap_or(usize::MAX);
    assert!(
        clause_pos < footnote_ref_pos
            && footnote_ref_pos < appendix_pos
            && appendix_pos < endnote_ref_pos,
        "note references missing or out of order: {document_xml}"
    );
    assert!(
        footnotes_xml.contains(r#"<w:footnote w:type="separator" w:id="-1">"#)
            && footnotes_xml.contains(r#"<w:footnote w:id="1">"#)
            && footnotes_xml
                .contains(r#"<w:t xml:space="preserve">Foot &lt;one&gt; &amp; two</w:t>"#),
        "footnotes part missing authored note: {footnotes_xml}"
    );
    assert!(
        endnotes_xml.contains(r#"<w:endnote w:type="separator" w:id="-1">"#)
            && endnotes_xml.contains(r#"<w:endnote w:id="1">"#)
            && endnotes_xml.contains(r#"<w:t xml:space="preserve">End</w:t><w:br/><w:t xml:space="preserve">Line</w:t><w:tab/><w:t xml:space="preserve">2</w:t>"#),
        "endnotes part missing authored note: {endnotes_xml}"
    );
    assert!(
        rels.contains("relationships/footnotes")
            && rels.contains(r#"Target="footnotes.xml""#)
            && rels.contains("relationships/endnotes")
            && rels.contains(r#"Target="endnotes.xml""#),
        "note relationships missing: {rels}"
    );
    assert!(
        content_types.contains(r#"PartName="/word/footnotes.xml""#)
            && content_types.contains("wordprocessingml.footnotes+xml")
            && content_types.contains(r#"PartName="/word/endnotes.xml""#)
            && content_types.contains("wordprocessingml.endnotes+xml"),
        "note content types missing: {content_types}"
    );

    let reopened = Document::open(&bytes).expect("note-authored .docx reopens");
    let notes = reopened.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].kind, NoteKind::Footnote);
    assert_eq!(notes[0].id, "1");
    assert_eq!(notes[0].text, "Foot <one> & two");
    assert_eq!(
        notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
        Some("Clause")
    );
    assert_eq!(notes[1].kind, NoteKind::Endnote);
    assert_eq!(notes[1].id, "1");
    assert_eq!(notes[1].text, "End\nLine\t2");
    assert_eq!(
        notes[1].anchor.as_ref().map(|anchor| anchor.text.as_str()),
        Some("Appendix")
    );
}

#[test]
fn run_builder_adds_authored_revision_runs() {
    let model = DocBuilder::new()
        .paragraph_runs([
            RunBuilder::new("kept").build(),
            RunBuilder::new("added")
                .revision(
                    RevisionBuilder::insertion()
                        .author(" Alice ")
                        .date(" 2026-06-24T01:00:00Z "),
                )
                .bold()
                .build(),
            RunBuilder::new("removed")
                .revision(RevisionBuilder::deletion().author(" ").date("\n"))
                .italic()
                .build(),
        ])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(
        paragraph.runs[1].revision.as_ref().map(|r| r.kind),
        Some(RevisionKind::Insertion)
    );
    assert_eq!(
        paragraph.runs[2].revision.as_ref().map(|r| r.kind),
        Some(RevisionKind::Deletion)
    );
    let insertion = paragraph.runs[1].revision.as_ref().unwrap();
    let deletion = paragraph.runs[2].revision.as_ref().unwrap();
    assert_eq!(insertion.author.as_deref(), Some("Alice"));
    assert_eq!(insertion.date.as_deref(), Some("2026-06-24T01:00:00Z"));
    assert_eq!(deletion.author, None);
    assert_eq!(deletion.date, None);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    let ins_pos = document_xml
        .find(r#"<w:ins w:id="0""#)
        .unwrap_or(usize::MAX);
    let add_pos = document_xml
        .find(r#"<w:t xml:space="preserve">added</w:t>"#)
        .unwrap_or(usize::MAX);
    let del_pos = document_xml
        .find(r#"<w:del w:id="1""#)
        .unwrap_or(usize::MAX);
    let del_text_pos = document_xml
        .find(r#"<w:delText xml:space="preserve">removed</w:delText>"#)
        .unwrap_or(usize::MAX);
    assert!(
        ins_pos < add_pos && add_pos < del_pos && del_pos < del_text_pos,
        "revision XML missing or out of order: {document_xml}"
    );
    assert!(
        document_xml.contains(r#"<w:ins w:id="0" w:author="Alice" w:date="2026-06-24T01:00:00Z">"#)
            && document_xml.contains("<w:b/>")
            && document_xml.contains(r#"<w:del w:id="1">"#)
            && document_xml.contains("<w:i/>"),
        "revision metadata or run properties missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("revision-authored .docx reopens");
    let revisions = reopened.revisions();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].author.as_deref(), Some("Alice"));
    assert_eq!(revisions[0].date.as_deref(), Some("2026-06-24T01:00:00Z"));
    assert_eq!(revisions[0].text, "added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].author, None);
    assert_eq!(revisions[1].date, None);
    assert_eq!(revisions[1].text, "removed");
    assert_eq!(
        reopened.main_text_with_revision_view(RevisionView::Accepted),
        "kept added"
    );
    assert_eq!(
        reopened.main_text_with_revision_view(RevisionView::Original),
        "kept removed"
    );
}

#[test]
fn run_builder_adds_plain_text_content_control() {
    let model = DocBuilder::new()
        .paragraph_runs([
            RunBuilder::new("Locked clause")
                .content_control(
                    ContentControlBuilder::new()
                        .alias(" Clause title ")
                        .tag(" clause-001 "),
                )
                .bold()
                .build(),
            RunBuilder::new(" tail").build(),
        ])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    let control = paragraph.runs[0]
        .content_control
        .as_ref()
        .expect("run carries content-control metadata");
    assert_eq!(control.alias.as_deref(), Some("Clause title"));
    assert_eq!(control.tag.as_deref(), Some("clause-001"));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let sdt_pos = document_xml.find("<w:sdt>").unwrap_or(usize::MAX);
    let alias_pos = document_xml
        .find(r#"<w:alias w:val="Clause title"/>"#)
        .unwrap_or(usize::MAX);
    let tag_pos = document_xml
        .find(r#"<w:tag w:val="clause-001"/>"#)
        .unwrap_or(usize::MAX);
    let content_pos = document_xml.find("<w:sdtContent>").unwrap_or(usize::MAX);
    let text_pos = document_xml
        .find(r#"<w:t xml:space="preserve">Locked clause</w:t>"#)
        .unwrap_or(usize::MAX);
    assert!(
        sdt_pos < alias_pos
            && alias_pos < tag_pos
            && tag_pos < content_pos
            && content_pos < text_pos,
        "content-control XML missing or out of order: {document_xml}"
    );
    assert!(
        document_xml.contains("<w:b/>"),
        "content-control run formatting missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("content-control-authored .docx reopens");
    assert_eq!(reopened.text(), "Locked clause tail");
    assert_eq!(reopened.report().features.content_controls, 1);
    let Block::Paragraph(reopened_paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened paragraph");
    };
    assert_eq!(reopened_paragraph.text(), "Locked clause tail");
    assert!(reopened_paragraph.runs[0].props.bold);
    let reopened_control = reopened_paragraph.runs[0]
        .content_control
        .as_ref()
        .expect("reopened run carries content-control metadata");
    assert_eq!(reopened_control.alias.as_deref(), Some("Clause title"));
    assert_eq!(reopened_control.tag.as_deref(), Some("clause-001"));
}

#[test]
fn run_builder_does_not_emit_blank_content_control_metadata() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Loose text")
            .content_control(
                ContentControlBuilder::new()
                    .alias(" ")
                    .tag("\t")
                    .data_binding(" /root/client ", "\n"),
            )
            .build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    let control = paragraph.runs[0]
        .content_control
        .as_ref()
        .expect("run carries content-control metadata");
    assert_eq!(control.alias, None);
    assert_eq!(control.tag, None);
    assert_eq!(control.data_binding_xpath, None);
    assert_eq!(control.data_binding_store_item_id, None);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        !document_xml.contains("<w:sdt>"),
        "blank content-control metadata should not emit an SDT wrapper: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("blank content-control metadata .docx reopens");
    assert_eq!(reopened.text(), "Loose text");
    assert_eq!(reopened.report().features.content_controls, 0);
}

#[test]
fn content_control_builder_adds_data_binding_metadata() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("Bound value")
            .content_control(
                ContentControlBuilder::new()
                    .alias("Client")
                    .tag("client-name")
                    .data_binding(
                        r#" /root/client[@code="A&B"] "#,
                        " {11111111-2222-3333-4444-555555555555} ",
                    ),
            )
            .build()])
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph");
    };
    let control = paragraph.runs[0]
        .content_control
        .as_ref()
        .expect("run carries content-control metadata");
    assert_eq!(
        control.data_binding_xpath.as_deref(),
        Some(r#"/root/client[@code="A&B"]"#)
    );
    assert_eq!(
        control.data_binding_store_item_id.as_deref(),
        Some("{11111111-2222-3333-4444-555555555555}")
    );

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let tag_pos = document_xml
        .find(r#"<w:tag w:val="client-name"/>"#)
        .unwrap_or(usize::MAX);
    let binding_pos = document_xml
        .find(r#"<w:dataBinding w:xpath="/root/client[@code=&quot;A&amp;B&quot;]" w:storeItemID="{11111111-2222-3333-4444-555555555555}"/>"#)
        .unwrap_or(usize::MAX);
    let content_pos = document_xml.find("<w:sdtContent>").unwrap_or(usize::MAX);
    assert!(
        tag_pos < binding_pos && binding_pos < content_pos,
        "data binding XML missing or out of order: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("data-bound content-control .docx reopens");
    assert_eq!(reopened.text(), "Bound value");
    assert_eq!(reopened.report().features.content_controls, 1);
    let Block::Paragraph(reopened_paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened paragraph");
    };
    let reopened_control = reopened_paragraph.runs[0]
        .content_control
        .as_ref()
        .expect("reopened run carries data-binding metadata");
    assert_eq!(reopened_control.alias.as_deref(), Some("Client"));
    assert_eq!(reopened_control.tag.as_deref(), Some("client-name"));
    assert_eq!(
        reopened_control.data_binding_xpath.as_deref(),
        Some(r#"/root/client[@code="A&B"]"#)
    );
    assert_eq!(
        reopened_control.data_binding_store_item_id.as_deref(),
        Some("{11111111-2222-3333-4444-555555555555}")
    );
}

#[test]
fn doc_builder_adds_custom_xml_item() {
    let store_item_id = "{11111111-2222-3333-4444-555555555555}";
    let padded_store_item_id = " {11111111-2222-3333-4444-555555555555} ";
    let xml = r#"<root><client code="A&amp;B">ACME</client></root>"#;
    let model = DocBuilder::new()
        .custom_xml_item(padded_store_item_id, xml)
        .paragraph_runs([RunBuilder::new("Bound value")
            .content_control(
                ContentControlBuilder::new()
                    .tag("client-name")
                    .data_binding("/root/client", store_item_id),
            )
            .build()])
        .build();

    assert_eq!(model.custom_xml_items[0].store_item_id, store_item_id);
    assert_eq!(model.custom_xml_items[0].xml, xml);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert_eq!(
        String::from_utf8(parts["customXml/item1.xml"].clone()).unwrap(),
        xml
    );
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    assert!(
        content_types.contains(
            r#"<Override PartName="/customXml/itemProps1.xml" ContentType="application/vnd.openxmlformats-officedocument.customXmlProperties+xml"/>"#
        ),
        "custom XML properties content type missing: {content_types}"
    );
    let item_rels = String::from_utf8(parts["customXml/_rels/item1.xml.rels"].clone()).unwrap();
    assert!(
        item_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps" Target="itemProps1.xml""#
        ),
        "custom XML item relationship missing: {item_rels}"
    );
    let item_props = String::from_utf8(parts["customXml/itemProps1.xml"].clone()).unwrap();
    assert!(
        item_props
            .contains(r#"<ds:datastoreItem ds:itemID="{11111111-2222-3333-4444-555555555555}""#)
            && item_props.contains("<ds:schemaRefs/>"),
        "custom XML item properties missing: {item_props}"
    );

    let reopened = Document::open(&bytes).expect("custom XML .docx reopens");
    assert_eq!(reopened.text(), "Bound value");
    assert_eq!(reopened.report().features.content_controls, 1);
    assert_eq!(reopened.model().custom_xml_items, model.custom_xml_items);
}

#[test]
fn doc_builder_ignores_blank_custom_xml_item_ids() {
    let model = DocBuilder::new()
        .custom_xml_item(" ", "<root/>")
        .paragraph("Body")
        .build();

    assert!(model.custom_xml_items.is_empty());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert!(!parts.contains_key("customXml/item1.xml"));
    assert!(!parts.contains_key("customXml/itemProps1.xml"));
}

#[test]
fn run_builder_adds_styled_paragraph_and_heading_runs() {
    let mut model = DocBuilder::new()
        .paragraph_runs([
            RunBuilder::new("Status: ").bold().build(),
            RunBuilder::new("green")
                .italic()
                .underline()
                .font(" Arial ")
                .size_half_pt(28)
                .color(Color::rgb(0, 128, 0))
                .highlight(" yellow ")
                .build(),
        ])
        .heading_runs(
            2,
            [RunBuilder::new("Section")
                .small_caps()
                .caps()
                .font(" ")
                .highlight("\t")
                .build()],
        )
        .build();

    assert_eq!(model.blocks.len(), 2);
    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("first block should be a paragraph");
    };
    assert_eq!(paragraph.runs.len(), 2);
    assert!(paragraph.runs[0].props.bold);
    assert!(paragraph.runs[1].props.italic);
    assert!(paragraph.runs[1].props.underline);
    assert_eq!(paragraph.runs[1].props.font.as_deref(), Some("Arial"));
    assert_eq!(paragraph.runs[1].props.size_half_pt, Some(28));
    assert_eq!(paragraph.runs[1].props.color, Some(Color::rgb(0, 128, 0)));
    assert_eq!(paragraph.runs[1].props.highlight.as_deref(), Some("yellow"));

    let Block::Paragraph(heading) = &model.blocks[1] else {
        panic!("second block should be a heading");
    };
    assert_eq!(heading.props.heading_level, Some(2));
    assert!(heading.runs[0].props.small_caps);
    assert!(heading.runs[0].props.caps);
    assert_eq!(heading.runs[0].props.font, None);
    assert_eq!(heading.runs[0].props.highlight, None);

    let Block::Paragraph(heading) = &mut model.blocks[1] else {
        panic!("second block should be a heading");
    };
    heading.runs[0].props.font = Some(" ".to_string());
    heading.runs[0].props.highlight = Some("\t".to_string());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains("<w:b/>")
            && document_xml.contains("<w:i/>")
            && document_xml.contains(r#"<w:u w:val="single"/>"#)
            && document_xml.contains(r#"<w:rFonts w:ascii="Arial""#)
            && document_xml.contains(r#"<w:sz w:val="28"/>"#)
            && document_xml.contains(r#"<w:color w:val="008000"/>"#)
            && document_xml.contains(r#"<w:highlight w:val="yellow"/>"#)
            && document_xml.contains("<w:smallCaps/>")
            && document_xml.contains("<w:caps/>"),
        "styled run properties missing: {document_xml}"
    );
    assert!(!document_xml.contains(r#"<w:rFonts w:ascii="""#));
    assert!(!document_xml.contains(r#"<w:highlight w:val="""#));

    let reopened = Document::open(&bytes).expect("styled builder .docx reopens");
    let text = reopened.text();
    assert!(text.contains("Status: green"), "styled text lost: {text:?}");
}

#[test]
fn paragraph_builder_adds_layout_properties() {
    let shade = Color::rgb(0xEE, 0xF2, 0xF7);
    let model = DocBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .align(Align::Justify)
                .spacing_before_pt(12.0)
                .spacing_after_pt(6.0)
                .line_pct(1.5)
                .indent_left_pt(24.0)
                .indent_right_pt(12.0)
                .first_line_pt(18.0)
                .page_break_before()
                .shading(shade)
                .runs([
                    RunBuilder::new("Layout ").bold().build(),
                    RunBuilder::new("paragraph").italic().build(),
                ]),
        )
        .build();

    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected paragraph block");
    };
    assert_eq!(paragraph.props.align, Align::Justify);
    assert_eq!(paragraph.props.spacing.before_pt, Some(12.0));
    assert_eq!(paragraph.props.spacing.after_pt, Some(6.0));
    assert_eq!(paragraph.props.spacing.line_pct, Some(1.5));
    assert_eq!(paragraph.props.indent.left_pt, Some(24.0));
    assert_eq!(paragraph.props.indent.right_pt, Some(12.0));
    assert_eq!(paragraph.props.indent.first_line_pt, Some(18.0));
    assert!(paragraph.props.page_break_before);
    assert_eq!(paragraph.props.shading, Some(shade));
    assert!(paragraph.runs[0].props.bold);
    assert!(paragraph.runs[1].props.italic);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="EEF2F7"/>"#)
            && document_xml.contains(
                r#"<w:spacing w:before="240" w:after="120" w:line="360" w:lineRule="auto"/>"#
            )
            && document_xml.contains(r#"<w:ind w:left="480" w:right="240" w:firstLine="360"/>"#)
            && document_xml.contains(r#"<w:jc w:val="both"/>"#)
            && document_xml.contains("<w:pageBreakBefore/>")
            && document_xml.contains("<w:b/>")
            && document_xml.contains("<w:i/>"),
        "paragraph layout XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("paragraph builder .docx reopens");
    let Block::Paragraph(reopened_paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened paragraph");
    };
    assert_eq!(reopened_paragraph.props.align, Align::Justify);
    assert!(reopened_paragraph.props.page_break_before);
    assert_eq!(reopened_paragraph.props.shading, Some(shade));
    assert_eq!(reopened_paragraph.text(), "Layout paragraph");
}

#[test]
fn doc_builder_adds_custom_paragraph_style() {
    let accent = Color::rgb(0x7A, 0x1F, 0x1F);
    let model = DocBuilder::new()
        .paragraph_style(
            ParagraphStyleBuilder::new(" RiskCallout ", " Risk callout ")
                .based_on(" Normal ")
                .next(" Normal ")
                .q_format()
                .align(Align::Justify)
                .spacing_before_pt(6.0)
                .spacing_after_pt(12.0)
                .indent_left_pt(18.0)
                .shading(Color::rgb(0xFE, 0xF2, 0xF2))
                .run_bold()
                .run_font(" Aptos ")
                .run_color(accent)
                .run_highlight(" yellow ")
                .run_size_half_pt(24),
        )
        .rich_paragraph(
            ParagraphBuilder::text("Risk status")
                .style(" RiskCallout ")
                .push_run(RunBuilder::new(": review required").italic().build()),
        )
        .build();

    assert_eq!(model.setup.styles.len(), 1);
    assert_eq!(model.setup.styles[0].id, "RiskCallout");
    assert_eq!(model.setup.styles[0].name, "Risk callout");
    assert_eq!(model.setup.styles[0].based_on.as_deref(), Some("Normal"));
    assert_eq!(model.setup.styles[0].next.as_deref(), Some("Normal"));
    assert_eq!(model.setup.styles[0].run.font.as_deref(), Some("Aptos"));
    assert_eq!(
        model.setup.styles[0].run.highlight.as_deref(),
        Some("yellow")
    );
    let Block::Paragraph(paragraph) = &model.blocks[0] else {
        panic!("expected styled paragraph");
    };
    assert_eq!(paragraph.props.style_id.as_deref(), Some("RiskCallout"));
    assert_eq!(paragraph.text(), "Risk status: review required");

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let styles_xml = String::from_utf8(parts["word/styles.xml"].clone()).unwrap();
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    assert!(
        styles_xml.contains(r#"<w:style w:type="paragraph" w:styleId="RiskCallout">"#)
            && styles_xml.contains(r#"<w:name w:val="Risk callout"/>"#)
            && styles_xml.contains(r#"<w:basedOn w:val="Normal"/>"#)
            && styles_xml.contains(r#"<w:next w:val="Normal"/>"#)
            && styles_xml.contains("<w:qFormat/>")
            && styles_xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="FEF2F2"/>"#)
            && styles_xml.contains(r#"<w:spacing w:before="120" w:after="240"/>"#)
            && styles_xml.contains(r#"<w:ind w:left="360"/>"#)
            && styles_xml.contains(r#"<w:jc w:val="both"/>"#)
            && styles_xml.contains(r#"<w:rFonts w:ascii="Aptos""#)
            && styles_xml.contains("<w:b/>")
            && styles_xml.contains(r#"<w:color w:val="7A1F1F"/>"#)
            && styles_xml.contains(r#"<w:highlight w:val="yellow"/>"#)
            && styles_xml.contains(r#"<w:sz w:val="24"/>"#),
        "custom style XML missing: {styles_xml}"
    );
    assert!(
        document_xml.contains(r#"<w:pStyle w:val="RiskCallout"/>"#)
            && document_xml.contains("<w:i/>"),
        "styled paragraph XML missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/styles") && rels.contains(r#"Target="styles.xml""#),
        "styles relationship missing: {rels}"
    );

    let reopened = Document::open(&bytes).expect("custom style .docx reopens");
    let Block::Paragraph(reopened_paragraph) = &reopened.model().blocks[0] else {
        panic!("expected reopened styled paragraph");
    };
    assert_eq!(
        reopened_paragraph.props.style_id.as_deref(),
        Some("RiskCallout")
    );
    assert_eq!(
        reopened_paragraph.props.style_name.as_deref(),
        Some("Risk callout")
    );
    assert_eq!(reopened_paragraph.text(), "Risk status: review required");
}

#[test]
fn doc_builder_creates_basic_report_model() {
    let model = DocBuilder::new()
        .title(" Builder Report ")
        .subject(" Operations ")
        .creator(" rwml ")
        .description(" Quarterly <plan> & review ")
        .keywords(" rwml,metadata ")
        .category(" Operations ")
        .content_status(" Draft ")
        .last_modified_by(" Reviewer ")
        .revision(" 12 ")
        .version(" 1.2 ")
        .created(" 2026-06-01T02:03:04Z ")
        .modified(" 2026-06-02T03:04:05Z ")
        .last_printed(" 2026-06-03T04:05:06Z ")
        .page_setup(PageSetup {
            margin_pt: 54.0,
            ..PageSetup::default()
        })
        .heading(1, "Builder Report")
        .paragraph("Summary")
        .table_with_header([["Metric", "Value"], ["Openable", "Yes"]])
        .header("Builder Report")
        .footer("Confidential")
        .page_numbers()
        .build();

    assert_eq!(model.setup.title.as_deref(), Some("Builder Report"));
    assert_eq!(model.setup.subject.as_deref(), Some("Operations"));
    assert_eq!(model.setup.creator.as_deref(), Some("rwml"));
    assert_eq!(
        model.setup.description.as_deref(),
        Some("Quarterly <plan> & review")
    );
    assert_eq!(model.setup.keywords.as_deref(), Some("rwml,metadata"));
    assert_eq!(model.setup.category.as_deref(), Some("Operations"));
    assert_eq!(model.setup.content_status.as_deref(), Some("Draft"));
    assert_eq!(model.setup.last_modified_by.as_deref(), Some("Reviewer"));
    assert_eq!(model.setup.revision.as_deref(), Some("12"));
    assert_eq!(model.setup.version.as_deref(), Some("1.2"));
    assert_eq!(model.setup.created.as_deref(), Some("2026-06-01T02:03:04Z"));
    assert_eq!(
        model.setup.modified.as_deref(),
        Some("2026-06-02T03:04:05Z")
    );
    assert_eq!(
        model.setup.last_printed.as_deref(),
        Some("2026-06-03T04:05:06Z")
    );
    assert_eq!(model.setup.page.margin_pt, 54.0);
    assert_eq!(model.setup.header.len(), 1);
    assert_eq!(model.setup.footer.len(), 1);
    assert!(model.setup.page_numbers);
    assert_eq!(model.blocks.len(), 3);

    let Block::Paragraph(title) = &model.blocks[0] else {
        panic!("first block should be a heading");
    };
    assert_eq!(title.props.heading_level, Some(1));
    assert_eq!(title.text(), "Builder Report");

    let Block::Table(table) = &model.blocks[2] else {
        panic!("third block should be a table");
    };
    assert_eq!(table.header_rows, 1);
    assert!(table.rows[0].cells.iter().all(|cell| cell.is_header));
    assert_eq!(table.rows[1].cells[1].text(), "Yes");

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let root_rels = String::from_utf8(parts["_rels/.rels"].clone()).unwrap();
    let core_xml = String::from_utf8(parts["docProps/core.xml"].clone()).unwrap();
    assert!(
        content_types.contains(
            r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#
        ),
        "core properties content type missing: {content_types}"
    );
    assert!(
        root_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml""#
        ),
        "core properties relationship missing: {root_rels}"
    );
    assert!(
        core_xml.contains("<dc:title>Builder Report</dc:title>")
            && core_xml.contains("<dc:subject>Operations</dc:subject>")
            && core_xml.contains("<dc:creator>rwml</dc:creator>"),
        "core properties XML missing title/creator: {core_xml}"
    );
    assert!(
        core_xml.contains("<dc:description>Quarterly &lt;plan&gt; &amp; review</dc:description>")
            && core_xml.contains("<cp:keywords>rwml,metadata</cp:keywords>"),
        "core properties XML missing descriptive metadata: {core_xml}"
    );
    assert!(
        core_xml.contains("<cp:category>Operations</cp:category>")
            && core_xml.contains("<cp:contentStatus>Draft</cp:contentStatus>")
            && core_xml.contains("<cp:lastModifiedBy>Reviewer</cp:lastModifiedBy>")
            && core_xml.contains("<cp:revision>12</cp:revision>")
            && core_xml.contains("<cp:version>1.2</cp:version>"),
        "core properties XML missing package metadata: {core_xml}"
    );
    assert!(
        core_xml.contains(
            r#"<dcterms:created xsi:type="dcterms:W3CDTF">2026-06-01T02:03:04Z</dcterms:created>"#
        ) && core_xml.contains(
            r#"<dcterms:modified xsi:type="dcterms:W3CDTF">2026-06-02T03:04:05Z</dcterms:modified>"#
        ) && core_xml.contains("<cp:lastPrinted>2026-06-03T04:05:06Z</cp:lastPrinted>"),
        "core properties XML missing timestamp metadata: {core_xml}"
    );
    let reopened = Document::open(&bytes).expect("builder-authored .docx reopens");
    let core = reopened.core_properties();
    assert_eq!(core.title.as_deref(), Some("Builder Report"));
    assert_eq!(core.subject.as_deref(), Some("Operations"));
    assert_eq!(core.creator.as_deref(), Some("rwml"));
    assert_eq!(
        core.description.as_deref(),
        Some("Quarterly <plan> & review")
    );
    assert_eq!(core.keywords.as_deref(), Some("rwml,metadata"));
    assert_eq!(core.category.as_deref(), Some("Operations"));
    assert_eq!(core.content_status.as_deref(), Some("Draft"));
    assert_eq!(core.last_modified_by.as_deref(), Some("Reviewer"));
    assert_eq!(core.revision.as_deref(), Some("12"));
    assert_eq!(core.version.as_deref(), Some("1.2"));
    assert_eq!(core.created.as_deref(), Some("2026-06-01T02:03:04Z"));
    assert_eq!(core.modified.as_deref(), Some("2026-06-02T03:04:05Z"));
    assert_eq!(core.last_printed.as_deref(), Some("2026-06-03T04:05:06Z"));
    let text = reopened.text();
    assert!(text.contains("Builder Report"), "title lost: {text:?}");
    assert!(
        text.contains("Openable") && text.contains("Yes"),
        "table lost"
    );
}

#[test]
fn doc_builder_ignores_blank_core_metadata() {
    let mut model = DocBuilder::new()
        .title(" ")
        .subject("\r\n")
        .creator("\t")
        .description(" ")
        .keywords("\n")
        .category(" ")
        .content_status("\n")
        .last_modified_by("\t")
        .revision(" ")
        .version(" ")
        .created(" ")
        .modified("\n")
        .last_printed("\t")
        .paragraph("Body")
        .build();

    assert_eq!(model.setup.title, None);
    assert_eq!(model.setup.subject, None);
    assert_eq!(model.setup.creator, None);
    assert_eq!(model.setup.description, None);
    assert_eq!(model.setup.keywords, None);
    assert_eq!(model.setup.category, None);
    assert_eq!(model.setup.content_status, None);
    assert_eq!(model.setup.last_modified_by, None);
    assert_eq!(model.setup.revision, None);
    assert_eq!(model.setup.version, None);
    assert_eq!(model.setup.created, None);
    assert_eq!(model.setup.modified, None);
    assert_eq!(model.setup.last_printed, None);

    model.setup.title = Some(" \n ".to_string());
    model.setup.subject = Some(" ".to_string());
    model.setup.creator = Some("\t".to_string());
    model.setup.description = Some("\r".to_string());
    model.setup.keywords = Some("\n".to_string());
    model.setup.category = Some(" ".to_string());
    model.setup.content_status = Some("\n".to_string());
    model.setup.last_modified_by = Some("\t".to_string());
    model.setup.revision = Some(" ".to_string());
    model.setup.version = Some(" ".to_string());
    model.setup.created = Some(" ".to_string());
    model.setup.modified = Some("\n".to_string());
    model.setup.last_printed = Some("\t".to_string());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    assert!(!parts.contains_key("docProps/core.xml"));
    let root_rels = String::from_utf8(parts["_rels/.rels"].clone()).unwrap();
    assert!(
        !root_rels.contains("relationships/metadata/core-properties"),
        "blank core metadata should not emit root relationship: {root_rels}"
    );
}

#[test]
fn doc_builder_sets_page_geometry_with_helpers() {
    let model = DocBuilder::new()
        .page_size_pt(792.0, 612.0)
        .landscape()
        .margins_pt(36.0)
        .margins_each_pt(72.0, 54.0, 36.0, 18.0)
        .paragraph("geometry")
        .build();

    assert_eq!(model.setup.page.width_pt, 792.0);
    assert_eq!(model.setup.page.height_pt, 612.0);
    assert!(model.setup.page.landscape);
    assert_eq!(model.setup.page.margin_pt, 36.0);
    assert_eq!(model.setup.page.margin_top_pt, Some(72.0));
    assert_eq!(model.setup.page.margin_right_pt, Some(54.0));
    assert_eq!(model.setup.page.margin_bottom_pt, Some(36.0));
    assert_eq!(model.setup.page.margin_left_pt, Some(18.0));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>"#),
        "page size missing: {document_xml}"
    );
    assert!(
        document_xml
            .contains(r#"<w:pgMar w:top="1440" w:right="1080" w:bottom="720" w:left="360""#),
        "page margins missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("geometry .docx reopens");
    let page = reopened.model().setup.page;
    assert_eq!(page.width_pt, 792.0);
    assert_eq!(page.height_pt, 612.0);
    assert!(page.landscape);
    assert_eq!(page.margin_top_pt, Some(72.0));
    assert_eq!(page.margin_right_pt, Some(54.0));
    assert_eq!(page.margin_bottom_pt, Some(36.0));
    assert_eq!(page.margin_left_pt, Some(18.0));
}

#[test]
fn doc_builder_adds_styled_header_and_footer_blocks() {
    let footer_block = Block::Paragraph(Paragraph {
        props: ParaProps {
            align: Align::Center,
            ..ParaProps::default()
        },
        runs: vec![RunBuilder::new("Manual footer").underline().build()],
    });

    let model = DocBuilder::new()
        .header_runs([RunBuilder::new("Confidential").bold().build()])
        .push_footer_block(footer_block)
        .footer_runs([RunBuilder::new("Page ").italic().build()])
        .page_numbers()
        .paragraph("body")
        .build();

    assert_eq!(model.setup.header.len(), 1);
    assert_eq!(model.setup.footer.len(), 2);
    let Block::Paragraph(header) = &model.setup.header[0] else {
        panic!("header should be paragraph");
    };
    assert!(header.runs[0].props.bold);
    let Block::Paragraph(footer) = &model.setup.footer[0] else {
        panic!("footer should be paragraph");
    };
    assert_eq!(footer.props.align, Align::Center);
    assert!(footer.runs[0].props.underline);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    assert!(
        header_xml.contains("Confidential") && header_xml.contains("<w:b/>"),
        "styled header missing: {header_xml}"
    );
    assert!(
        footer_xml.contains("Manual footer")
            && footer_xml.contains(r#"<w:u w:val="single"/>"#)
            && footer_xml.contains("Page ")
            && footer_xml.contains("<w:i/>")
            && footer_xml.contains("PAGE"),
        "styled footer/page number missing: {footer_xml}"
    );
    assert!(
        rels.contains("relationships/header") && rels.contains("relationships/footer"),
        "header/footer rels missing: {rels}"
    );
}

#[test]
fn doc_builder_adds_styled_first_even_header_footer_blocks() {
    let styled_block = |text: &str, align| {
        Block::Paragraph(Paragraph {
            props: ParaProps {
                align,
                ..ParaProps::default()
            },
            runs: vec![RunBuilder::new(text).underline().build()],
        })
    };

    let model = DocBuilder::new()
        .first_header_runs([RunBuilder::new("First confidential").bold().build()])
        .push_first_header_block(styled_block("Manual first header", Align::Center))
        .even_header_runs([RunBuilder::new("Even confidential").italic().build()])
        .push_even_header_block(styled_block("Manual even header", Align::Right))
        .first_footer_runs([RunBuilder::new("First footer").bold().build()])
        .push_first_footer_block(styled_block("Manual first footer", Align::Center))
        .even_footer_runs([RunBuilder::new("Even footer").italic().build()])
        .push_even_footer_block(styled_block("Manual even footer", Align::Right))
        .paragraph("body")
        .build();

    assert!(model.setup.title_page);
    assert_eq!(model.setup.first_header.len(), 2);
    assert_eq!(model.setup.even_header.len(), 2);
    assert_eq!(model.setup.first_footer.len(), 2);
    assert_eq!(model.setup.even_footer.len(), 2);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let settings_xml = String::from_utf8(parts["word/settings.xml"].clone()).unwrap();
    let part_containing = |needle: &str| {
        parts
            .values()
            .find_map(|bytes| {
                let xml = String::from_utf8(bytes.clone()).ok()?;
                xml.contains(needle).then_some(xml)
            })
            .unwrap_or_else(|| panic!("{needle} part written"))
    };
    let first_header_xml = part_containing("First confidential");
    let even_header_xml = part_containing("Even confidential");
    let first_footer_xml = part_containing("First footer");
    let even_footer_xml = part_containing("Even footer");

    assert!(
        document_xml.contains("<w:titlePg/>")
            && document_xml.contains(r#"<w:headerReference w:type="first""#)
            && document_xml.contains(r#"<w:headerReference w:type="even""#)
            && document_xml.contains(r#"<w:footerReference w:type="first""#)
            && document_xml.contains(r#"<w:footerReference w:type="even""#),
        "styled variant references missing: {document_xml}"
    );
    assert!(
        settings_xml.contains("<w:evenAndOddHeaders/>"),
        "even/odd settings missing: {settings_xml}"
    );
    assert!(
        first_header_xml.contains("<w:b/>")
            && first_header_xml.contains("Manual first header")
            && first_header_xml.contains(r#"<w:u w:val="single"/>"#),
        "styled first header missing: {first_header_xml}"
    );
    assert!(
        even_header_xml.contains("<w:i/>")
            && even_header_xml.contains("Manual even header")
            && even_header_xml.contains(r#"<w:u w:val="single"/>"#),
        "styled even header missing: {even_header_xml}"
    );
    assert!(
        first_footer_xml.contains("<w:b/>")
            && first_footer_xml.contains("Manual first footer")
            && first_footer_xml.contains(r#"<w:u w:val="single"/>"#),
        "styled first footer missing: {first_footer_xml}"
    );
    assert!(
        even_footer_xml.contains("<w:i/>")
            && even_footer_xml.contains("Manual even footer")
            && even_footer_xml.contains(r#"<w:u w:val="single"/>"#),
        "styled even footer missing: {even_footer_xml}"
    );
}

#[test]
fn write_docx_emits_header_footer_images_with_local_relationships() {
    let png = tiny_png();
    let image_url = "https://example.com/header-image?a=1&b=2";
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![
                Block::Image(
                    ImageBuilder::new(png.clone(), "image/png")
                        .alt("Header <logo>")
                        .size_px(200, 100)
                        .rotate_degrees(90)
                        .into(),
                ),
                Block::Paragraph(Paragraph {
                    runs: vec![rwml::Run {
                        image: Some(
                            ImageBuilder::new(png.clone(), "image/png")
                                .alt("Inline <badge>")
                                .size_px(40, 20)
                                .into(),
                        ),
                        field: FieldRole::Hyperlink {
                            url: image_url.to_string(),
                        },
                        ..rwml::Run::default()
                    }],
                    ..Paragraph::default()
                }),
                Block::Table(
                    TableBuilder::new()
                        .row([CellBuilder::new().rich_table(
                            TableBuilder::new().row([CellBuilder::new().push_block(Block::Image(
                                ImageBuilder::new(png.clone(), "image/png")
                                    .alt("Nested cell image")
                                    .size_px(30, 15)
                                    .into(),
                            ))]),
                        )])
                        .into(),
                ),
            ],
            footer: vec![Block::Image(
                ImageBuilder::new(png.clone(), "image/png")
                    .alt("Footer <mark>")
                    .size_px(80, 40)
                    .floating_offset_emu(91_440, 182_880)
                    .into(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    assert_eq!(bytes, rwml::write_docx(&model));
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    let header_rels = String::from_utf8(parts["word/_rels/header1.xml.rels"].clone()).unwrap();
    let footer_rels = String::from_utf8(parts["word/_rels/footer1.xml.rels"].clone()).unwrap();
    let document_rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    assert!(
        header_xml.contains(
            r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing""#
        ) && header_xml
            .contains(r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#)
            && header_xml.contains(
                r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#
            ),
        "header drawing namespaces missing: {header_xml}"
    );
    assert_eq!(header_xml.matches("<w:drawing>").count(), 3, "{header_xml}");
    assert!(
        header_xml.contains(r#"<wp:extent cx="1905000" cy="952500"/>"#)
            && header_xml
                .contains(r#"<wp:docPr id="1" name="Image1" descr="Header &lt;logo&gt;"/>"#,)
            && header_xml.contains(r#"<a:xfrm rot="5400000">"#)
            && header_xml.contains(r#"<w:hyperlink r:id="rId2"><w:r><w:drawing>"#)
            && header_xml.contains(r#"<a:blip r:embed="rId3"/>"#)
            && header_xml.contains("Nested cell image"),
        "header image drawings missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing""#)
            && footer_xml.contains("<wp:anchor ")
            && footer_xml.contains(
                r#"<wp:positionH relativeFrom="page"><wp:posOffset>91440</wp:posOffset></wp:positionH>"#,
            )
            && footer_xml.contains(
                r#"<wp:docPr id="4" name="Image4" descr="Footer &lt;mark&gt;"/>"#,
            )
            && footer_xml.contains(r#"<a:blip r:embed="rId1"/>"#),
        "footer floating image missing: {footer_xml}"
    );
    assert!(
        header_rels.contains(r#"Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png""#)
            && header_rels.contains(r#"Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/header-image?a=1&amp;b=2" TargetMode="External""#)
            && header_rels.contains(r#"Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png""#)
            && header_rels.contains(r#"Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image3.png""#),
        "header-local image relationships missing: {header_rels}"
    );
    assert!(
        footer_rels.contains(r#"Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image4.png""#),
        "footer-local image relationship missing: {footer_rels}"
    );
    assert!(
        !document_rels.contains("relationships/image"),
        "{document_rels}"
    );
    assert!(!document_rels.contains(image_url), "{document_rels}");
    assert!(content_types.contains(r#"ContentType="image/png""#));
    for index in 1..=4 {
        assert_eq!(parts[&format!("word/media/image{index}.png")], png);
    }

    let reopened = Document::open(&bytes).expect("header/footer image .docx reopens");
    let setup = &reopened.model().setup;
    let [Block::Paragraph(header_block), Block::Paragraph(header_inline), Block::Table(outer)] =
        setup.header.as_slice()
    else {
        panic!("unexpected reopened header blocks: {:?}", setup.header);
    };
    let header_image = header_block.runs[0].image.as_ref().expect("header image");
    assert_eq!(header_image.bytes.as_deref(), Some(png.as_slice()));
    assert_eq!(header_image.alt.as_deref(), Some("Header <logo>"));
    assert_eq!(header_image.rotation_degrees, Some(90));
    let inline_image = header_inline.runs[0].image.as_ref().expect("inline image");
    assert_eq!(inline_image.bytes.as_deref(), Some(png.as_slice()));
    assert!(matches!(
        &header_inline.runs[0].field,
        FieldRole::Hyperlink { url } if url == image_url
    ));
    let Block::Table(nested) = &outer.rows[0].cells[0].blocks[0] else {
        panic!("expected nested running table");
    };
    let Block::Paragraph(nested_image) = &nested.rows[0].cells[0].blocks[0] else {
        panic!("expected nested image paragraph");
    };
    assert_eq!(
        nested_image.runs[0]
            .image
            .as_ref()
            .and_then(|image| image.alt.as_deref()),
        Some("Nested cell image")
    );
    let [Block::Paragraph(footer)] = setup.footer.as_slice() else {
        panic!("unexpected reopened footer blocks: {:?}", setup.footer);
    };
    let footer_image = footer.runs[0].image.as_ref().expect("footer image");
    assert_eq!(footer_image.bytes.as_deref(), Some(png.as_slice()));
    assert_eq!(footer_image.floating_offset_emu, Some((91_440, 182_880)));
}

#[test]
fn write_docx_keeps_missing_header_footer_images_as_visible_placeholders() {
    let missing = |alt: &str| rwml::Image {
        alt: Some(alt.to_string()),
        width_px: Some(40),
        height_px: Some(20),
        ..rwml::Image::default()
    };
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Image(missing("Header <missing>"))],
            footer: vec![Block::Table(
                TableBuilder::new()
                    .row([CellBuilder::new().paragraph_runs([rwml::Run {
                        image: Some(missing("Footer <missing>")),
                        ..rwml::Run::default()
                    }])])
                    .into(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    assert!(header_xml.contains("[rwml image placeholder: Header &lt;missing&gt;]"));
    assert!(footer_xml.contains("[rwml image placeholder: Footer &lt;missing&gt;]"));
    assert!(!header_xml.contains("xmlns:wp=") && !footer_xml.contains("xmlns:wp="));
    assert!(!parts.keys().any(|path| path.starts_with("word/media/")));
    assert!(!parts.contains_key("word/_rels/header1.xml.rels"));
    assert!(!parts.contains_key("word/_rels/footer1.xml.rels"));

    let reopened = Document::open(&bytes).expect("missing running image .docx reopens");
    let text = reopened.text();
    assert!(
        text.contains("[rwml image placeholder: Header <missing>]")
            && text.contains("[rwml image placeholder: Footer <missing>]")
    );
}

#[cfg(feature = "render")]
#[test]
fn fresh_docx_running_images_survive_reopen_and_render() {
    let model = |bytes: Option<Vec<u8>>| {
        let image = || rwml::Image {
            bytes: bytes.clone(),
            mime: Some("image/png".to_string()),
            alt: Some("Running raster".to_string()),
            width_px: Some(120),
            height_px: Some(60),
            ..rwml::Image::default()
        };
        DocModel {
            blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
            setup: DocSetup {
                header: vec![Block::Image(image())],
                footer: vec![Block::Table(
                    TableBuilder::new()
                        .row([CellBuilder::new().paragraph_runs([rwml::Run {
                            image: Some(image()),
                            ..rwml::Run::default()
                        }])])
                        .into(),
                )],
                ..DocSetup::default()
            },
            ..DocModel::default()
        }
    };
    let present = Document::open(&rwml::write_docx(&model(Some(tiny_png()))))
        .expect("present running images reopen");
    let missing =
        Document::open(&rwml::write_docx(&model(None))).expect("missing running images reopen");

    let present_pdf = present.to_pdf_with_report();
    let missing_pdf = missing.to_pdf_with_report();
    assert_eq!(present_pdf.report.pages, 1);
    assert_eq!(missing_pdf.report.pages, 1);
    assert_ne!(
        present_pdf.pdf, missing_pdf.pdf,
        "present running rasters must not render as missing-byte placeholders"
    );
    assert_eq!(present_pdf.pdf, present.to_pdf_with_report().pdf);
}

#[test]
fn write_docx_keeps_header_footer_table_text_visible() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Table(
                TableBuilder::new()
                    .row([CellBuilder::text("Header metric"), CellBuilder::text("42")])
                    .into(),
            )],
            footer: vec![Block::Table(
                TableBuilder::new()
                    .row([CellBuilder::text("Footer note"), CellBuilder::text("ok")])
                    .into(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        header_xml.contains("Header metric") && header_xml.contains("42"),
        "header table text missing: {header_xml}"
    );
    assert!(
        footer_xml.contains("Footer note") && footer_xml.contains("ok"),
        "footer table text missing: {footer_xml}"
    );

    let reopened = Document::open(&bytes).expect("header/footer table fallback .docx reopens");
    let text = reopened.text();
    assert!(
        text.contains("Header metric") && text.contains("Footer note"),
        "header/footer table text not readable after reopen: {text:?}"
    );
}

#[test]
fn write_docx_emits_rich_header_footer_tables_with_local_relationships() {
    let border = Color::rgb(0x1F, 0x4E, 0x78);
    let shade = Color::rgb(0xDE, 0xEB, 0xF7);
    let header_url = "https://example.com/header-table?a=1&b=2";
    let header_table = TableBuilder::new()
        .header_rows(1)
        .col_widths_pct([0.4, 0.6])
        .width_pct(0.8)
        .fixed_layout()
        .bidi_visual()
        .indent_twips(240)
        .align(Align::Center)
        .border_color(border)
        .border_size_eighths(12)
        .border_style(TableBorderStyle::Double)
        .row([CellBuilder::text("Header metrics").header().col_span(2)])
        .row([
            CellBuilder::new()
                .rich_paragraph(
                    ParagraphBuilder::new()
                        .align(Align::Right)
                        .runs([RunBuilder::new("Linked value")
                            .hyperlink(header_url)
                            .bold()
                            .build()]),
                )
                .row_span(2)
                .shading(shade)
                .margins_twips(80, 120, 160, 200)
                .valign(VCell::Bottom)
                .width_pct(0.4),
            CellBuilder::new()
                .rich_table(TableBuilder::new().row([CellBuilder::text("Nested table")])),
        ])
        .row([CellBuilder::text("Tail cell")]);
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Table(header_table.into())],
            footer: vec![Block::Table(
                TableBuilder::new()
                    .row([
                        CellBuilder::new(),
                        CellBuilder::new().push_block(Block::Image(
                            ImageBuilder::new(tiny_png(), "image/png")
                                .alt("Footer cell image")
                                .into(),
                        )),
                        CellBuilder::new().push_block(Block::Chart(
                            ChartBuilder::bar()
                                .categories(["Q1"])
                                .series("Revenue", [7.0])
                                .alt("Footer cell chart")
                                .into(),
                        )),
                    ])
                    .into(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    assert_eq!(bytes, rwml::write_docx(&model));
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    let header_rels = String::from_utf8(parts["word/_rels/header1.xml.rels"].clone()).unwrap();
    let footer_rels = String::from_utf8(parts["word/_rels/footer1.xml.rels"].clone()).unwrap();
    let document_rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    assert_eq!(header_xml.matches("<w:tbl>").count(), 2, "{header_xml}");
    for expected in [
        "<w:tblHeader/>",
        r#"<w:tblW w:w="4000" w:type="pct"/>"#,
        r#"<w:tblInd w:w="240" w:type="dxa"/>"#,
        r#"<w:jc w:val="center"/>"#,
        r#"<w:tblLayout w:type="fixed"/>"#,
        r#"<w:gridSpan w:val="2"/>"#,
        r#"<w:vMerge w:val="restart"/>"#,
        "<w:vMerge/>",
        r#"<w:shd w:val="clear" w:color="auto" w:fill="DEEBF7"/>"#,
        r#"<w:top w:w="80" w:type="dxa"/>"#,
        r#"<w:vAlign w:val="bottom"/>"#,
        r#"<w:jc w:val="right"/>"#,
        r#"<w:hyperlink r:id="rId1">"#,
        "<w:b/>",
        "Nested table",
        "Tail cell",
    ] {
        assert!(
            header_xml.contains(expected),
            "missing {expected}: {header_xml}"
        );
    }
    assert!(
        header_xml.contains(r#"<w:top w:val="double" w:sz="12" w:space="0" w:color="1F4E78"/>"#),
        "rich border missing: {header_xml}"
    );
    assert!(
        header_xml.contains("</w:tbl><w:p/></w:tc>"),
        "nested table terminal paragraph missing: {header_xml}"
    );
    assert!(
        footer_xml.contains("<w:tbl>")
            && footer_xml.contains("<w:tcPr></w:tcPr><w:p/>")
            && footer_xml.contains("<w:drawing>")
            && footer_xml.contains("Footer cell image")
            && footer_xml.contains(r#"<c:chart r:id="rId2"/>"#)
            && footer_xml.contains("Footer cell chart"),
        "empty footer table cell missing: {footer_xml}"
    );
    assert!(
        parts.contains_key("word/media/image1.png")
            && parts.contains_key("word/charts/chart1.xml")
            && parts.contains_key("word/charts/_rels/chart1.xml.rels")
            && parts.contains_key("word/embeddings/Microsoft_Excel_Worksheet1.xlsx"),
        "running-table raster/chart package parts are wrong: {:?}",
        parts.keys().collect::<Vec<_>>()
    );
    assert!(
        header_rels.contains("relationships/hyperlink")
            && header_rels.contains(r#"Target="https://example.com/header-table?a=1&amp;b=2""#)
            && header_rels.contains(r#"TargetMode="External""#),
        "header-local hyperlink relationship missing: {header_rels}"
    );
    assert!(
        footer_rels.contains("relationships/image")
            && footer_rels.contains(r#"Target="media/image1.png""#)
            && footer_rels.contains("relationships/chart")
            && footer_rels.contains(r#"Target="charts/chart1.xml""#),
        "footer-local media relationships missing: {footer_rels}"
    );
    assert!(!document_rels.contains(header_url), "{document_rels}");
    assert!(
        !document_rels.contains("relationships/image"),
        "{document_rels}"
    );
    assert!(
        !document_rels.contains("relationships/chart"),
        "{document_rels}"
    );
    assert!(
        document_rels.contains(r#"Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml""#)
            && document_rels.contains(r#"Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml""#),
        "document relationship ids changed: {document_rels}"
    );

    let reopened = Document::open(&bytes).expect("rich header/footer table .docx reopens");
    let Block::Table(header) = &reopened.model().setup.header[0] else {
        panic!("expected reopened header table");
    };
    assert_eq!(header.header_rows, 1);
    assert!(header.fixed_layout);
    assert!(header.bidi_visual);
    assert_eq!(header.rows[0].cells[0].col_span, 2);
    assert_eq!(header.rows[1].cells[0].row_span, 2);
    assert_eq!(header.rows[1].cells[0].shading, Some(shade));
    assert_eq!(header.rows[1].cells[0].valign, VCell::Bottom);
    assert!(matches!(
        &header.rows[1].cells[1].blocks[0],
        Block::Table(nested) if nested.rows[0].cells[0].text() == "Nested table"
    ));
    let Block::Table(footer) = &reopened.model().setup.footer[0] else {
        panic!("expected reopened footer table");
    };
    let Block::Paragraph(image_para) = &footer.rows[0].cells[1].blocks[0] else {
        panic!("expected reopened footer image paragraph");
    };
    assert_eq!(
        image_para.runs[0]
            .image
            .as_ref()
            .and_then(|image| image.alt.as_deref()),
        Some("Footer cell image")
    );
    let Block::Chart(chart) = &footer.rows[0].cells[2].blocks[0] else {
        panic!("expected reopened footer chart");
    };
    assert_eq!(chart.alt.as_deref(), Some("Footer cell chart"));
    assert_eq!(chart.kind, ChartKind::Bar);
}

#[test]
fn write_docx_emits_tables_in_all_header_footer_variants() {
    let table =
        |text: &str| Block::Table(TableBuilder::new().row([CellBuilder::text(text)]).into());
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![table("Default header table")],
            first_header: vec![table("First header table")],
            even_header: vec![table("Even header table")],
            footer: vec![table("Default footer table")],
            first_footer: vec![table("First footer table")],
            even_footer: vec![table("Even footer table")],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    for (path, text) in [
        ("word/header1.xml", "Default header table"),
        ("word/header2.xml", "First header table"),
        ("word/header3.xml", "Even header table"),
        ("word/footer1.xml", "Default footer table"),
        ("word/footer2.xml", "First footer table"),
        ("word/footer3.xml", "Even footer table"),
    ] {
        let xml = String::from_utf8(parts[path].clone()).unwrap();
        assert!(
            xml.contains("<w:tbl>") && xml.contains(text),
            "running table missing from {path}: {xml}"
        );
    }

    let reopened = Document::open(&bytes).expect("running table variants .docx reopens");
    let setup = &reopened.model().setup;
    for (blocks, text) in [
        (&setup.header, "Default header table"),
        (&setup.first_header, "First header table"),
        (&setup.even_header, "Even header table"),
        (&setup.footer, "Default footer table"),
        (&setup.first_footer, "First footer table"),
        (&setup.even_footer, "Even footer table"),
    ] {
        assert!(matches!(
            blocks.as_slice(),
            [Block::Table(table)] if table.rows[0].cells[0].text() == text
        ));
    }
}

#[test]
fn write_docx_emits_images_in_all_header_footer_variants() {
    let png = tiny_png();
    let image = |alt: &str| {
        Block::Image(
            ImageBuilder::new(png.clone(), "image/png")
                .alt(alt)
                .size_px(24, 12)
                .into(),
        )
    };
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![image("Default header image")],
            first_header: vec![image("First header image")],
            even_header: vec![image("Even header image")],
            footer: vec![image("Default footer image")],
            first_footer: vec![image("First footer image")],
            even_footer: vec![image("Even footer image")],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    assert_eq!(bytes, rwml::write_docx(&model));
    let parts = unzip_parts(&bytes);
    for (part_path, rels_path, alt) in [
        (
            "word/header1.xml",
            "word/_rels/header1.xml.rels",
            "Default header image",
        ),
        (
            "word/header2.xml",
            "word/_rels/header2.xml.rels",
            "First header image",
        ),
        (
            "word/header3.xml",
            "word/_rels/header3.xml.rels",
            "Even header image",
        ),
        (
            "word/footer1.xml",
            "word/_rels/footer1.xml.rels",
            "Default footer image",
        ),
        (
            "word/footer2.xml",
            "word/_rels/footer2.xml.rels",
            "First footer image",
        ),
        (
            "word/footer3.xml",
            "word/_rels/footer3.xml.rels",
            "Even footer image",
        ),
    ] {
        let xml = String::from_utf8(parts[part_path].clone()).unwrap();
        let rels = String::from_utf8(parts[rels_path].clone()).unwrap();
        assert!(
            xml.contains("<w:drawing>") && xml.contains(alt),
            "running image missing from {part_path}: {xml}"
        );
        assert!(
            rels.contains("relationships/image") && rels.contains(r#"Target="media/image"#),
            "part-local image relationship missing from {rels_path}: {rels}"
        );
    }
    assert_eq!(
        parts
            .keys()
            .filter(|path| path.starts_with("word/media/image"))
            .count(),
        6
    );

    let reopened = Document::open(&bytes).expect("running image variants .docx reopens");
    let setup = &reopened.model().setup;
    for (blocks, alt) in [
        (&setup.header, "Default header image"),
        (&setup.first_header, "First header image"),
        (&setup.even_header, "Even header image"),
        (&setup.footer, "Default footer image"),
        (&setup.first_footer, "First footer image"),
        (&setup.even_footer, "Even footer image"),
    ] {
        assert!(matches!(
            blocks.as_slice(),
            [Block::Paragraph(paragraph)]
                if paragraph.runs[0]
                    .image
                    .as_ref()
                    .and_then(|image| image.alt.as_deref()) == Some(alt)
        ));
    }
}

#[test]
fn write_docx_emits_header_footer_charts_with_local_relationships() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Chart(
                ChartBuilder::bar()
                    .title("Header chart")
                    .categories(["Q1"])
                    .series("Revenue", [42.0])
                    .alt("Header <chart>")
                    .into(),
            )],
            footer: vec![Block::Chart(
                ChartBuilder::bar()
                    .title("Footer chart")
                    .categories(["Q1"])
                    .series("Revenue", [7.0])
                    .into(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    let header_rels = String::from_utf8(parts["word/_rels/header1.xml.rels"].clone()).unwrap();
    let footer_rels = String::from_utf8(parts["word/_rels/footer1.xml.rels"].clone()).unwrap();
    let document_rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    assert!(
        header_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && header_xml.contains(r#"descr="Header &lt;chart&gt;""#),
        "header chart drawing missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "footer chart drawing missing: {footer_xml}"
    );
    assert!(
        parts.contains_key("word/charts/chart1.xml")
            && parts.contains_key("word/charts/chart2.xml")
            && parts.contains_key("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
            && parts.contains_key("word/embeddings/Microsoft_Excel_Worksheet2.xlsx"),
        "header/footer chart package parts missing: {:?}",
        parts.keys().collect::<Vec<_>>()
    );
    assert!(
        header_rels.contains("relationships/chart")
            && header_rels.contains(r#"Target="charts/chart1.xml""#),
        "header-local chart relationship missing: {header_rels}"
    );
    assert!(
        footer_rels.contains("relationships/chart")
            && footer_rels.contains(r#"Target="charts/chart2.xml""#),
        "footer-local chart relationship missing: {footer_rels}"
    );
    assert!(
        !document_rels.contains("relationships/chart"),
        "{document_rels}"
    );

    let reopened = Document::open(&bytes).expect("header/footer chart .docx reopens");
    let reopened_model = reopened.model();
    let [Block::Chart(header)] = reopened_model.setup.header.as_slice() else {
        panic!("expected reopened header chart");
    };
    let [Block::Chart(footer)] = reopened_model.setup.footer.as_slice() else {
        panic!("expected reopened footer chart");
    };
    assert_eq!(header.alt.as_deref(), Some("Header <chart>"));
    assert_eq!(footer.title.as_deref(), Some("Footer chart"));
    assert_eq!(reopened.report().features.unsupported_charts, 0);
}

#[test]
fn write_docx_keeps_header_footer_page_break_blocks() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::PageBreak],
            footer: vec![Block::PageBreak],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        header_xml.contains(r#"<w:br w:type="page"/>"#),
        "header page break missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:br w:type="page"/>"#),
        "footer page break missing: {footer_xml}"
    );

    Document::open(&bytes).expect("header/footer page-break .docx reopens");
}

#[test]
fn write_docx_keeps_header_footer_simple_field_runs() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("report.docx").field("FILENAME \\p").build()],
                ..Paragraph::default()
            })],
            footer: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("1").field("PAGE").build()],
                ..Paragraph::default()
            })],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        header_xml.contains(r#"<w:fldSimple w:instr=" FILENAME \p ">"#),
        "header field XML missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:fldSimple w:instr=" PAGE ">"#),
        "footer field XML missing: {footer_xml}"
    );

    let reopened = Document::open(&bytes).expect("header/footer field .docx reopens");
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Filename);
    assert_eq!(fields[0].instruction, "FILENAME \\p");
    assert_eq!(fields[0].result, "report.docx");
    assert_eq!(fields[1].kind, FieldKind::Page);
    assert_eq!(fields[1].instruction, "PAGE");
    assert_eq!(fields[1].result, "1");
    let header_text = reopened.header_text();
    assert!(
        header_text.contains("report.docx") && header_text.contains('1'),
        "header/footer field result text not readable after reopen: {header_text:?}"
    );
}

#[test]
fn write_docx_keeps_header_footer_content_control_runs() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Client")
                    .content_control(
                        ContentControlBuilder::new()
                            .alias(" Client name ")
                            .tag(" client-name "),
                    )
                    .build()],
                ..Paragraph::default()
            })],
            footer: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Footer")
                    .content_control(ContentControlBuilder::new().tag(" footer-note "))
                    .build()],
                ..Paragraph::default()
            })],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        header_xml.contains("<w:sdt>")
            && header_xml.contains(r#"<w:alias w:val="Client name"/>"#)
            && header_xml.contains(r#"<w:tag w:val="client-name"/>"#)
            && header_xml.contains("Client"),
        "header content-control XML missing: {header_xml}"
    );
    assert!(
        footer_xml.contains("<w:sdt>")
            && footer_xml.contains(r#"<w:tag w:val="footer-note"/>"#)
            && footer_xml.contains("Footer"),
        "footer content-control XML missing: {footer_xml}"
    );

    let reopened = Document::open(&bytes).expect("header/footer content-control .docx reopens");
    let reopened_model = reopened.model();
    let Block::Paragraph(header) = &reopened_model.setup.header[0] else {
        panic!("expected reopened header paragraph");
    };
    let header_control = header.runs[0]
        .content_control
        .as_ref()
        .expect("reopened header run carries content-control metadata");
    assert_eq!(header_control.alias.as_deref(), Some("Client name"));
    assert_eq!(header_control.tag.as_deref(), Some("client-name"));

    let Block::Paragraph(footer) = &reopened_model.setup.footer[0] else {
        panic!("expected reopened footer paragraph");
    };
    assert_eq!(
        footer.runs[0]
            .content_control
            .as_ref()
            .and_then(|control| control.tag.as_deref()),
        Some("footer-note")
    );
}

#[test]
fn write_docx_keeps_header_footer_bookmark_and_internal_anchor_runs() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    RunBuilder::new("Header anchor")
                        .bookmark("HeaderMark")
                        .build(),
                    RunBuilder::new(" header jump")
                        .hyperlink("#HeaderMark")
                        .build(),
                ],
                ..Paragraph::default()
            })],
            footer: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    RunBuilder::new("Footer anchor")
                        .bookmark("FooterMark")
                        .build(),
                    RunBuilder::new(" footer jump")
                        .hyperlink("#FooterMark")
                        .build(),
                ],
                ..Paragraph::default()
            })],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    let header_start = header_xml
        .find(r#"<w:bookmarkStart w:id="0" w:name="HeaderMark"/>"#)
        .unwrap_or(usize::MAX);
    let header_text = header_xml
        .find(r#"<w:t xml:space="preserve">Header anchor</w:t>"#)
        .unwrap_or(usize::MAX);
    let header_end = header_xml
        .find(r#"<w:bookmarkEnd w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    assert!(
        header_start < header_text && header_text < header_end,
        "header bookmark XML missing or out of order: {header_xml}"
    );
    assert!(
        header_xml.contains(r#"<w:hyperlink w:anchor="HeaderMark">"#),
        "header internal anchor missing: {header_xml}"
    );

    let footer_start = footer_xml
        .find(r#"<w:bookmarkStart w:id="1" w:name="FooterMark"/>"#)
        .unwrap_or(usize::MAX);
    let footer_text = footer_xml
        .find(r#"<w:t xml:space="preserve">Footer anchor</w:t>"#)
        .unwrap_or(usize::MAX);
    let footer_end = footer_xml
        .find(r#"<w:bookmarkEnd w:id="1"/>"#)
        .unwrap_or(usize::MAX);
    assert!(
        footer_start < footer_text && footer_text < footer_end,
        "footer bookmark XML missing or out of order: {footer_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:hyperlink w:anchor="FooterMark">"#),
        "footer internal anchor missing: {footer_xml}"
    );
    assert!(!parts.contains_key("word/_rels/header1.xml.rels"));
    assert!(!parts.contains_key("word/_rels/footer1.xml.rels"));

    let reopened = Document::open(&bytes).expect("header/footer bookmark .docx reopens");
    let reopened_model = reopened.model();
    let Block::Paragraph(header) = &reopened_model.setup.header[0] else {
        panic!("expected reopened header paragraph")
    };
    assert!(matches!(
        &header.runs[1].field,
        FieldRole::Hyperlink { url } if url == "#HeaderMark"
    ));
    let Block::Paragraph(footer) = &reopened_model.setup.footer[0] else {
        panic!("expected reopened footer paragraph")
    };
    assert!(matches!(
        &footer.runs[1].field,
        FieldRole::Hyperlink { url } if url == "#FooterMark"
    ));
    let header_text = reopened.header_text();
    assert!(
        header_text.contains("Header anchor header jump")
            && header_text.contains("Footer anchor footer jump"),
        "header/footer bookmark text not readable after reopen: {header_text:?}"
    );
}

#[test]
fn write_docx_keeps_header_footer_paragraph_layout_props() {
    let shade = Color::rgb(0xEE, 0xF2, 0xF7);
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(
                ParagraphBuilder::text("Header layout")
                    .align(Align::Justify)
                    .spacing_before_pt(12.0)
                    .spacing_after_pt(6.0)
                    .line_pct(1.5)
                    .indent_left_pt(24.0)
                    .indent_right_pt(12.0)
                    .first_line_pt(18.0)
                    .page_break_before()
                    .shading(shade)
                    .build(),
            )],
            footer: vec![Block::Paragraph(
                ParagraphBuilder::text("Footer layout")
                    .align(Align::Right)
                    .shading(shade)
                    .build(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        header_xml.contains("<w:pageBreakBefore/>")
            && header_xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="EEF2F7"/>"#)
            && header_xml.contains(
                r#"<w:spacing w:before="240" w:after="120" w:line="360" w:lineRule="auto"/>"#
            )
            && header_xml.contains(r#"<w:ind w:left="480" w:right="240" w:firstLine="360"/>"#)
            && header_xml.contains(r#"<w:jc w:val="both"/>"#),
        "header paragraph layout XML missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="EEF2F7"/>"#)
            && footer_xml.contains(r#"<w:jc w:val="right"/>"#),
        "footer paragraph layout XML missing: {footer_xml}"
    );

    let reopened = Document::open(&bytes).expect("header/footer paragraph layout .docx reopens");
    let reopened_model = reopened.model();
    let Block::Paragraph(header) = &reopened_model.setup.header[0] else {
        panic!("expected reopened header paragraph");
    };
    assert_eq!(header.props.align, Align::Justify);
    assert!(header.props.page_break_before);
    assert_eq!(header.props.shading, Some(shade));
    assert_eq!(header.props.spacing.before_pt, Some(12.0));
    assert_eq!(header.props.indent.left_pt, Some(24.0));

    let Block::Paragraph(footer) = &reopened_model.setup.footer[0] else {
        panic!("expected reopened footer paragraph");
    };
    assert_eq!(footer.props.align, Align::Right);
    assert_eq!(footer.props.shading, Some(shade));
}

#[test]
fn write_docx_keeps_header_footer_revision_runs() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    RunBuilder::new("Header added")
                        .revision(
                            RevisionBuilder::insertion()
                                .author(" Alice ")
                                .date(" 2026-06-24T01:00:00Z "),
                        )
                        .bold()
                        .build(),
                    RunBuilder::new("Header removed")
                        .revision(RevisionBuilder::deletion())
                        .italic()
                        .build(),
                ],
                ..Paragraph::default()
            })],
            footer: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    RunBuilder::new("Footer added")
                        .revision(RevisionBuilder::insertion())
                        .build(),
                    RunBuilder::new("Footer removed")
                        .revision(RevisionBuilder::deletion())
                        .build(),
                ],
                ..Paragraph::default()
            })],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();

    assert!(
        header_xml.contains(r#"<w:ins w:id="0" w:author="Alice" w:date="2026-06-24T01:00:00Z">"#)
            && header_xml.contains("<w:b/>")
            && header_xml.contains(r#"<w:t xml:space="preserve">Header added</w:t>"#)
            && header_xml.contains(r#"<w:del w:id="1">"#)
            && header_xml.contains("<w:i/>")
            && header_xml.contains(r#"<w:delText xml:space="preserve">Header removed</w:delText>"#),
        "header revision XML missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:ins w:id="2">"#)
            && footer_xml.contains(r#"<w:t xml:space="preserve">Footer added</w:t>"#)
            && footer_xml.contains(r#"<w:del w:id="3">"#)
            && footer_xml.contains(r#"<w:delText xml:space="preserve">Footer removed</w:delText>"#),
        "footer revision XML missing: {footer_xml}"
    );

    let reopened = Document::open(&bytes).expect("header/footer revision .docx reopens");
    let revisions = reopened.revisions();
    assert_eq!(revisions.len(), 4);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].author.as_deref(), Some("Alice"));
    assert_eq!(revisions[0].date.as_deref(), Some("2026-06-24T01:00:00Z"));
    assert_eq!(revisions[0].text, "Header added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Header removed");
    assert_eq!(revisions[2].kind, RevisionKind::Insertion);
    assert_eq!(revisions[2].text, "Footer added");
    assert_eq!(revisions[3].kind, RevisionKind::Deletion);
    assert_eq!(revisions[3].text, "Footer removed");
    let header_text = reopened.header_text();
    assert!(
        header_text.contains("Header added")
            && header_text.contains("Footer added")
            && !header_text.contains("Header removed")
            && !header_text.contains("Footer removed"),
        "header/footer revisions should reopen as accepted current text: {header_text:?}"
    );
}

#[test]
fn write_docx_keeps_header_footer_hyperlink_runs() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Header link")
                    .hyperlink("https://example.com/header?x=1&y=2")
                    .bold()
                    .build()],
                ..Paragraph::default()
            })],
            footer: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Footer link")
                    .hyperlink("https://example.com/footer")
                    .build()],
                ..Paragraph::default()
            })],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    let header_rels = String::from_utf8(parts["word/_rels/header1.xml.rels"].clone()).unwrap();
    let footer_rels = String::from_utf8(parts["word/_rels/footer1.xml.rels"].clone()).unwrap();

    assert!(
        header_xml.contains(r#"<w:hyperlink r:id="rId1">"#)
            && header_xml.contains("<w:b/>")
            && header_xml.contains("Header link"),
        "header hyperlink XML missing: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:hyperlink r:id="rId1">"#) && footer_xml.contains("Footer link"),
        "footer hyperlink XML missing: {footer_xml}"
    );
    assert!(
        header_rels.contains("relationships/hyperlink")
            && header_rels.contains(r#"Target="https://example.com/header?x=1&amp;y=2""#)
            && header_rels.contains(r#"TargetMode="External""#),
        "header hyperlink relationship missing: {header_rels}"
    );
    assert!(
        footer_rels.contains("relationships/hyperlink")
            && footer_rels.contains(r#"Target="https://example.com/footer""#)
            && footer_rels.contains(r#"TargetMode="External""#),
        "footer hyperlink relationship missing: {footer_rels}"
    );

    let reopened = Document::open(&bytes).expect("header/footer hyperlink .docx reopens");
    let reopened_model = reopened.model();
    let Block::Paragraph(header) = &reopened_model.setup.header[0] else {
        panic!("expected reopened header paragraph");
    };
    assert!(matches!(
        &header.runs[0].field,
        FieldRole::Hyperlink { url } if url == "https://example.com/header?x=1&y=2"
    ));
    let Block::Paragraph(footer) = &reopened_model.setup.footer[0] else {
        panic!("expected reopened footer paragraph");
    };
    assert!(matches!(
        &footer.runs[0].field,
        FieldRole::Hyperlink { url } if url == "https://example.com/footer"
    ));
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_keeps_running_surface_hyperlink_annotations() {
    let header_url = "https://example.com/running-header";
    let footer_url = "https://example.com/running-footer";
    let table_url = "https://example.com/running-table";
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Header link").hyperlink(header_url).build()],
                ..Paragraph::default()
            })],
            footer: vec![
                Block::Paragraph(Paragraph {
                    runs: vec![RunBuilder::new("Footer link").hyperlink(footer_url).build()],
                    ..Paragraph::default()
                }),
                Block::Table(
                    TableBuilder::new()
                        .row([
                            CellBuilder::new().paragraph_runs([RunBuilder::new("Table link")
                                .hyperlink(table_url)
                                .build()]),
                        ])
                        .build(),
                ),
            ],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };
    let contains_target = |pdf: &[u8], target: &str| {
        pdf.windows(target.len())
            .any(|window| window == target.as_bytes())
    };

    let direct = rwml::render_pdf_with_report(&model);
    assert_eq!(direct.report.pages, 1);
    for target in [header_url, footer_url, table_url] {
        assert!(
            contains_target(&direct.pdf, target),
            "direct-model running target missing from PDF: {target}"
        );
    }
    assert_eq!(direct.pdf, rwml::render_pdf_with_report(&model).pdf);

    let visible_url = "https://example.com/visible-running-row";
    let clipped_url = "https://example.com/clipped-running-row";
    let clipped_model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Table(
                TableBuilder::new()
                    .row([CellBuilder::new()
                        .paragraph_runs([RunBuilder::new("Visible link")
                            .hyperlink(visible_url)
                            .build()])
                        .paragraph("Spacer one")
                        .paragraph("Spacer two")
                        .paragraph("Spacer three")
                        .paragraph_runs([RunBuilder::new("Clipped link")
                            .hyperlink(clipped_url)
                            .build()])])
                    .build(),
            )],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };
    let clipped = rwml::render_pdf_with_report(&clipped_model);
    assert_eq!(clipped.report.pages, 1);
    assert!(contains_target(&clipped.pdf, visible_url));
    assert!(
        !contains_target(&clipped.pdf, clipped_url),
        "fully clipped running-row target must not create an annotation"
    );

    let bytes = rwml::write_docx(&model);
    let reopened = Document::open(&bytes).expect("running-link DOCX reopens");
    let opened = reopened.to_pdf_with_report();
    assert_eq!(opened.report.pages, 1);
    for target in [header_url, footer_url, table_url] {
        assert!(
            contains_target(&opened.pdf, target),
            "reopened running target missing from PDF: {target}"
        );
    }
    assert_eq!(opened.pdf, reopened.to_pdf_with_report().pdf);
}

#[test]
fn write_docx_keeps_header_footer_comment_runs() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(plain_paragraph("Body"))],
        setup: DocSetup {
            header: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Header clause")
                    .comment(
                        CommentBuilder::new("Header note")
                            .author("Reviewer")
                            .date("2026-06-24T00:00:00Z"),
                    )
                    .build()],
                ..Paragraph::default()
            })],
            footer: vec![Block::Paragraph(Paragraph {
                runs: vec![RunBuilder::new("Footer clause")
                    .comment(CommentBuilder::new("Footer note").author("Approver"))
                    .build()],
                ..Paragraph::default()
            })],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let header_xml = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
    let footer_xml = String::from_utf8(parts["word/footer1.xml"].clone()).unwrap();
    let comments_xml = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();

    let header_start = header_xml
        .find(r#"<w:commentRangeStart w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    let header_text = header_xml
        .find(r#"<w:t xml:space="preserve">Header clause</w:t>"#)
        .unwrap_or(usize::MAX);
    let header_end = header_xml
        .find(r#"<w:commentRangeEnd w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    let header_ref = header_xml
        .find(r#"<w:commentReference w:id="0"/>"#)
        .unwrap_or(usize::MAX);
    assert!(
        header_start < header_text && header_text < header_end && header_end < header_ref,
        "header comment markers missing or out of order: {header_xml}"
    );
    assert!(
        footer_xml.contains(r#"<w:commentRangeStart w:id="1"/>"#)
            && footer_xml.contains(r#"<w:commentReference w:id="1"/>"#),
        "footer comment markers missing: {footer_xml}"
    );
    assert!(
        comments_xml
            .contains(r#"<w:comment w:id="0" w:author="Reviewer" w:date="2026-06-24T00:00:00Z">"#)
            && comments_xml.contains(r#"<w:t>Header note</w:t>"#)
            && comments_xml.contains(r#"<w:comment w:id="1" w:author="Approver">"#)
            && comments_xml.contains(r#"<w:t>Footer note</w:t>"#),
        "comments part missing header/footer comments: {comments_xml}"
    );
    assert!(
        rels.contains("relationships/comments") && rels.contains(r#"Target="comments.xml""#),
        "comments relationship missing: {rels}"
    );

    let reopened = Document::open(&bytes).expect("header/footer comment .docx reopens");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].text, "Header note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Header clause")
    );
    assert_eq!(comments[1].text, "Footer note");
    assert_eq!(
        comments[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Footer clause")
    );
    let header_text = reopened.header_text();
    assert!(
        header_text.contains("Header clause") && header_text.contains("Footer clause"),
        "header/footer commented text not readable after reopen: {header_text:?}"
    );
}

#[test]
fn table_builder_adds_rich_table_cells() {
    let navy = Color::rgb(0x1F, 0x38, 0x64);
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .header_rows(1)
                .col_widths_pct([0.25, 0.50, 0.25])
                .row([
                    CellBuilder::text("Metric")
                        .header()
                        .shading(navy)
                        .valign(VCell::Center)
                        .width_pct(0.25),
                    CellBuilder::text("Quarter")
                        .header()
                        .shading(navy)
                        .valign(VCell::Center)
                        .width_pct(0.50)
                        .col_span(2),
                ])
                .row([
                    CellBuilder::text("Revenue")
                        .row_span(2)
                        .valign(VCell::Bottom),
                    CellBuilder::new()
                        .paragraph_runs([RunBuilder::new("Q1").bold().build()])
                        .width_pct(0.50),
                    CellBuilder::text("42"),
                ])
                .row([CellBuilder::text("Q2"), CellBuilder::text("51")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.header_rows, 1);
    assert_eq!(table.col_widths_pct, vec![0.25, 0.50, 0.25]);
    assert!(table.rows[0].cells[0].is_header);
    assert_eq!(table.rows[0].cells[0].shading, Some(navy));
    assert_eq!(table.rows[0].cells[1].col_span, 2);
    assert_eq!(table.rows[1].cells[0].row_span, 2);
    assert_eq!(table.rows[1].cells[0].valign, VCell::Bottom);
    assert!(matches!(
        &table.rows[1].cells[1].blocks[0],
        Block::Paragraph(p) if p.runs[0].props.bold && p.text() == "Q1"
    ));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains("<w:tblHeader/>")
            && document_xml.contains(
                r#"<w:tblGrid><w:gridCol w:w="2250"/><w:gridCol w:w="4500"/><w:gridCol w:w="2250"/></w:tblGrid>"#
            )
            && document_xml.contains(r#"<w:gridSpan w:val="2"/>"#)
            && document_xml.contains(r#"<w:vMerge w:val="restart"/>"#)
            && document_xml.contains(r#"<w:vAlign w:val="center"/>"#)
            && document_xml.contains(r#"<w:vAlign w:val="bottom"/>"#)
            && document_xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="1F3864"/>"#)
            && document_xml.contains(r#"<w:tcW w:w="2500" w:type="pct"/>"#)
            && document_xml.contains("<w:b/>"),
        "rich table XML missing: {document_xml}"
    );
    assert_eq!(bytes, rwml::write_docx(&model));

    let reopened = Document::open(&bytes).expect("rich builder table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened rich table");
    };
    assert_eq!(reopened_table.header_rows, 1);
    assert_eq!(reopened_table.col_widths_pct, vec![0.25, 0.50, 0.25]);
    assert_eq!(reopened_table.rows[0].cells[0].shading, Some(navy));
    assert_eq!(reopened_table.rows[0].cells[1].col_span, 2);
    assert_eq!(reopened_table.rows[1].cells[0].row_span, 2);
    assert_eq!(reopened_table.rows[1].cells[0].valign, VCell::Bottom);
    assert_eq!(reopened_table.rows[1].cells[1].text(), "Q1");
}

#[test]
fn rtl_properties_round_trip_through_docx() {
    let model = DocBuilder::new()
        .rich_paragraph(
            ParagraphBuilder::new()
                .bidi()
                .runs([RunBuilder::new("rtl text").rtl().build()]),
        )
        .rich_table(
            TableBuilder::new()
                .bidi_visual()
                .row([CellBuilder::text("visual table")]),
        )
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains("<w:bidi/>")
            && document_xml.contains("<w:rtl/>")
            && document_xml.contains("<w:bidiVisual/>")
            && document_xml.contains(r#"<w:jc w:val="left"/>"#),
        "RTL properties missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("RTL property .docx reopens");
    let reopened_model = reopened.model();
    let Block::Paragraph(paragraph) = &reopened_model.blocks[0] else {
        panic!("expected reopened paragraph");
    };
    assert!(paragraph.props.bidi);
    assert_eq!(paragraph.props.align, Align::Left);
    assert!(paragraph.runs[0].props.rtl);

    let Block::Table(table) = &reopened_model.blocks[1] else {
        panic!("expected reopened table");
    };
    assert!(table.bidi_visual);
}

#[test]
fn table_builder_adds_fixed_layout() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .fixed_layout()
                .col_widths_pct([0.4, 0.6])
                .row([CellBuilder::text("A"), CellBuilder::text("B")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert!(table.fixed_layout);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:tblLayout w:type="fixed"/>"#),
        "fixed table layout XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("fixed-layout table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert!(reopened_table.fixed_layout);
}

#[test]
fn invalid_authored_table_grid_widths_use_equal_writer_fallback() {
    for widths in [
        vec![0.25],
        vec![0.0, 1.0],
        vec![f32::INFINITY, 1.0],
        vec![f32::NAN, 1.0],
    ] {
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![
                        CellBuilder::text("A").build(),
                        CellBuilder::text("B").build(),
                    ],
                }],
                col_widths_pct: widths,
                ..Table::default()
            })],
            ..DocModel::default()
        };

        let bytes = rwml::write_docx(&model);
        let parts = unzip_parts(&bytes);
        let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
        assert!(
            document_xml.contains(
                r#"<w:tblGrid><w:gridCol w:w="4500"/><w:gridCol w:w="4500"/></w:tblGrid>"#
            ),
            "invalid widths must retain equal fallback: {document_xml}"
        );
        assert_eq!(bytes, rwml::write_docx(&model));
    }
}

#[test]
fn extreme_valid_table_grid_widths_serialize_as_positive_bounded_twips() {
    let model = DocModel {
        blocks: vec![Block::Table(Table {
            rows: vec![Row {
                cells: vec![
                    CellBuilder::text("A").build(),
                    CellBuilder::text("B").build(),
                ],
            }],
            col_widths_pct: vec![f32::MIN_POSITIVE, f32::MAX],
            ..Table::default()
        })],
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml
            .contains(r#"<w:tblGrid><w:gridCol w:w="1"/><w:gridCol w:w="8999"/></w:tblGrid>"#),
        "valid extreme proportions must remain positive and bounded: {document_xml}"
    );
    let reopened = Document::open(&bytes).expect("extreme grid .docx reopens");
    let Block::Table(table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(table.col_widths_pct.len(), 2);
    assert!(table.col_widths_pct[0] > 0.0);
    assert!(table.col_widths_pct[1] > table.col_widths_pct[0]);
    assert_eq!(
        bytes,
        rwml::write_docx(&reopened.model()),
        "normalized grid proportions must remain stable after reopen"
    );
}

#[test]
fn table_builder_adds_table_indent() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .indent_twips(720)
                .row([CellBuilder::text("Indented")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.indent_twips, Some(720));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:tblInd w:w="720" w:type="dxa"/>"#),
        "table indent XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("indented table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.indent_twips, Some(720));
}

#[test]
fn table_builder_adds_table_alignment() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .align(Align::Center)
                .row([CellBuilder::text("Centered")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.align, Some(Align::Center));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:jc w:val="center"/>"#),
        "table alignment XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("aligned table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.align, Some(Align::Center));
}

#[test]
fn table_builder_adds_table_width() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .width_pct(0.8)
                .row([CellBuilder::text("Wide")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.width_pct, Some(0.8));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:tblW w:w="4000" w:type="pct"/>"#),
        "table width XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("width table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.width_pct, Some(0.8));
}

#[test]
fn table_builder_adds_table_border_color() {
    let border = Color::rgb(0x22, 0x66, 0xAA);
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_color(border)
                .row([CellBuilder::text("Bordered")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.border_color, Some(border));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:top w:val="single" w:sz="4" w:space="0" w:color="2266AA"/>"#),
        "table border color XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("border-color table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.border_color, Some(border));
}

#[test]
fn table_builder_adds_table_border_size() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_size_eighths(12)
                .row([CellBuilder::text("Thick")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.border_size_eighths, Some(12));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:top w:val="single" w:sz="12" w:space="0" w:color="auto"/>"#)
            && document_xml
                .contains(r#"<w:insideV w:val="single" w:sz="12" w:space="0" w:color="auto"/>"#),
        "table border size XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("border-size table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.border_size_eighths, Some(12));
}

#[cfg(feature = "render")]
#[test]
fn authored_uniform_table_border_paint_survives_reopen_and_changes_pdf() {
    let border = Color::rgb(0xB3, 0x1B, 0x34);
    let styled_model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_color(border)
                .border_size_eighths(24)
                .row([CellBuilder::text("Border paint")]),
        )
        .build();
    let reopened =
        Document::open(&rwml::write_docx(&styled_model)).expect("styled table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.border_color, Some(border));
    assert_eq!(reopened_table.border_size_eighths, Some(24));

    let default_model = DocBuilder::new()
        .rich_table(TableBuilder::new().row([CellBuilder::text("Border paint")]))
        .build();
    let default_reopened =
        Document::open(&rwml::write_docx(&default_model)).expect("default table .docx reopens");
    let default_pdf = default_reopened
        .try_to_pdf()
        .expect("reopened default table renders");
    let styled_pdf = reopened
        .try_to_pdf()
        .expect("reopened styled table renders");

    assert!(styled_pdf.starts_with(b"%PDF"));
    assert!(
        styled_pdf != default_pdf,
        "reopened uniform border paint must affect PDF bytes"
    );
    assert_eq!(styled_pdf, reopened.try_to_pdf().expect("repeat render"));
}

#[cfg(feature = "render")]
#[test]
fn authored_six_way_table_border_paint_survives_reopen_and_changes_pdf() {
    let baseline_model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .row([CellBuilder::text("A"), CellBuilder::text("B")])
                .row([CellBuilder::text("C"), CellBuilder::text("D")]),
        )
        .build();
    let baseline =
        Document::open(&rwml::write_docx(&baseline_model)).expect("baseline table .docx reopens");
    let baseline_pdf = baseline.try_to_pdf().expect("baseline table renders");

    for (index, side) in [
        TableBorderSide::Top,
        TableBorderSide::Left,
        TableBorderSide::Bottom,
        TableBorderSide::Right,
        TableBorderSide::InsideHorizontal,
        TableBorderSide::InsideVertical,
    ]
    .into_iter()
    .enumerate()
    {
        let color = Color::rgb(0x31 + index as u8, 0x72, 0xB4);
        let size = 16 + index as u16;
        let model = DocBuilder::new()
            .rich_table(
                TableBuilder::new()
                    .border_side_color(side, color)
                    .border_side_size_eighths(side, size)
                    .row([CellBuilder::text("A"), CellBuilder::text("B")])
                    .row([CellBuilder::text("C"), CellBuilder::text("D")]),
            )
            .build();
        let reopened =
            Document::open(&rwml::write_docx(&model)).expect("six-way table .docx reopens");
        let Block::Table(table) = &reopened.model().blocks[0] else {
            panic!("expected reopened table");
        };
        assert_eq!(table.border_colors.get(side), Some(color), "side={side:?}");
        assert_eq!(table.border_sizes.get(side), Some(size), "side={side:?}");

        let pdf = reopened.try_to_pdf().expect("six-way table renders");
        assert_ne!(pdf, baseline_pdf, "side={side:?}");
        assert_eq!(
            pdf,
            reopened.try_to_pdf().expect("repeat six-way render"),
            "side={side:?}"
        );
    }
}

#[test]
fn table_builder_adds_table_border_style() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_style(TableBorderStyle::Dotted)
                .row([CellBuilder::text("Dotted")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.border_style, Some(TableBorderStyle::Dotted));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:top w:val="dotted" w:sz="4" w:space="0" w:color="auto"/>"#)
            && document_xml
                .contains(r#"<w:insideV w:val="dotted" w:sz="4" w:space="0" w:color="auto"/>"#),
        "table border style XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("border-style table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.border_style, Some(TableBorderStyle::Dotted));
}

#[test]
fn table_builder_adds_side_specific_border_colors() {
    let border = Color::rgb(0x22, 0x66, 0xAA);
    let top = Color::rgb(0xAA, 0x33, 0x11);
    let inside = Color::rgb(0x11, 0x88, 0x44);
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_color(border)
                .border_side_color(TableBorderSide::Top, top)
                .border_side_color(TableBorderSide::InsideHorizontal, inside)
                .row([CellBuilder::text("Bordered")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.border_color, Some(border));
    assert_eq!(table.border_colors.top, Some(top));
    assert_eq!(table.border_colors.inside_h, Some(inside));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:top w:val="single" w:sz="4" w:space="0" w:color="AA3311"/>"#)
            && document_xml
                .contains(r#"<w:left w:val="single" w:sz="4" w:space="0" w:color="2266AA"/>"#)
            && document_xml
                .contains(r#"<w:insideH w:val="single" w:sz="4" w:space="0" w:color="118844"/>"#),
        "side-specific table border XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("side-border table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.border_colors.top, Some(top));
    assert_eq!(reopened_table.border_colors.left, Some(border));
    assert_eq!(reopened_table.border_colors.inside_h, Some(inside));
}

#[test]
fn table_builder_adds_side_specific_border_styles() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_style(TableBorderStyle::Single)
                .border_side_style(TableBorderSide::Top, TableBorderStyle::Double)
                .border_side_style(TableBorderSide::InsideVertical, TableBorderStyle::Dotted)
                .row([CellBuilder::text("Styled")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.border_style, Some(TableBorderStyle::Single));
    assert_eq!(table.border_styles.top, Some(TableBorderStyle::Double));
    assert_eq!(table.border_styles.inside_v, Some(TableBorderStyle::Dotted));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/>"#)
            && document_xml
                .contains(r#"<w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#)
            && document_xml
                .contains(r#"<w:insideV w:val="dotted" w:sz="4" w:space="0" w:color="auto"/>"#),
        "side-specific table border style XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("side-border-style table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(
        reopened_table.border_styles.top,
        Some(TableBorderStyle::Double)
    );
    assert_eq!(
        reopened_table.border_styles.left,
        Some(TableBorderStyle::Single)
    );
    assert_eq!(
        reopened_table.border_styles.inside_v,
        Some(TableBorderStyle::Dotted)
    );
}

#[test]
fn table_builder_adds_side_specific_border_sizes() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new()
                .border_size_eighths(4)
                .border_side_size_eighths(TableBorderSide::Top, 12)
                .border_side_size_eighths(TableBorderSide::InsideHorizontal, 8)
                .row([CellBuilder::text("Sized")]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.border_size_eighths, Some(4));
    assert_eq!(table.border_sizes.top, Some(12));
    assert_eq!(table.border_sizes.inside_h, Some(8));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<w:top w:val="single" w:sz="12" w:space="0" w:color="auto"/>"#)
            && document_xml
                .contains(r#"<w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#)
            && document_xml
                .contains(r#"<w:insideH w:val="single" w:sz="8" w:space="0" w:color="auto"/>"#),
        "side-specific table border size XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("side-border-size table .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.border_sizes.top, Some(12));
    assert_eq!(reopened_table.border_sizes.left, Some(4));
    assert_eq!(reopened_table.border_sizes.inside_h, Some(8));
}

#[test]
fn cell_builder_adds_cell_margins() {
    let margins = CellMargins {
        top: 120,
        right: 240,
        bottom: 360,
        left: 480,
    };
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new().row([CellBuilder::text("Padded")
                .shading(Color::rgb(0x11, 0x22, 0x33))
                .margins_twips(margins.top, margins.right, margins.bottom, margins.left)
                .valign(VCell::Bottom)]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected builder to add a table block");
    };
    assert_eq!(table.rows[0].cells[0].margins, Some(margins));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(
            r#"<w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="112233"/><w:tcMar><w:top w:w="120" w:type="dxa"/><w:left w:w="480" w:type="dxa"/><w:bottom w:w="360" w:type="dxa"/><w:right w:w="240" w:type="dxa"/></w:tcMar><w:vAlign w:val="bottom"/></w:tcPr>"#
        ),
        "cell property XML missing or out of schema order: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("cell margins .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened table");
    };
    assert_eq!(reopened_table.rows[0].cells[0].margins, Some(margins));
}

#[test]
fn cell_builder_adds_typed_nested_blocks() {
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new().row([CellBuilder::new()
                .rich_paragraph(
                    ParagraphBuilder::new()
                        .runs([RunBuilder::new("Outer note").bold().build()])
                        .align(Align::Center),
                )
                .rich_table(TableBuilder::new().row([CellBuilder::text("Inner value")]))]),
        )
        .build();

    let Block::Table(table) = &model.blocks[0] else {
        panic!("expected outer table");
    };
    let outer_cell = &table.rows[0].cells[0];
    assert_eq!(outer_cell.blocks.len(), 2);
    assert!(matches!(
        &outer_cell.blocks[0],
        Block::Paragraph(paragraph)
            if paragraph.props.align == Align::Center
                && paragraph.runs[0].props.bold
                && paragraph.text() == "Outer note"
    ));
    assert!(matches!(
        &outer_cell.blocks[1],
        Block::Table(nested)
            if nested.rows.len() == 1
                && nested.rows[0].cells.len() == 1
                && nested.rows[0].cells[0].text() == "Inner value"
    ));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.matches("<w:tbl>").count() >= 2
            && document_xml.contains("Outer note")
            && document_xml.contains("Inner value"),
        "nested table XML missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("nested cell blocks .docx reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("expected reopened outer table");
    };
    let reopened_cell = &reopened_table.rows[0].cells[0];
    assert!(matches!(
        &reopened_cell.blocks[0],
        Block::Paragraph(paragraph)
            if paragraph.props.align == Align::Center && paragraph.text() == "Outer note"
    ));
    assert!(matches!(
        &reopened_cell.blocks[1],
        Block::Table(nested)
            if nested.rows[0].cells[0].text() == "Inner value"
    ));
}

#[test]
fn authored_table_cell_lists_round_trip_in_source_order() {
    let numbered = |text: &str| Paragraph {
        props: ParaProps {
            list: Some(rwml::ListInfo {
                level: 0,
                ordered: true,
                label: String::new(),
                marker_font: None,
            }),
            ..ParaProps::default()
        },
        runs: vec![RunBuilder::new(text).build()],
    };
    let model = DocBuilder::new()
        .rich_table(
            TableBuilder::new().row([CellBuilder::new()
                .rich_paragraph(numbered("Direct cell"))
                .rich_table(
                    TableBuilder::new()
                        .row([CellBuilder::new().rich_paragraph(numbered("Nested cell"))]),
                )]),
        )
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert_eq!(document_xml.matches("<w:numPr>").count(), 2);
    assert!(parts.contains_key("word/numbering.xml"));
    assert!(
        rels.contains("relationships/numbering") && rels.contains(r#"Target="numbering.xml""#),
        "numbering relationship missing: {rels}"
    );

    let reopened = Document::open(&bytes).expect("table-cell lists .docx reopens");
    let reopened_model = reopened.model();
    let [Block::Table(table)] = reopened_model.blocks.as_slice() else {
        panic!("expected one table block");
    };
    let Block::Paragraph(direct) = &table.rows[0].cells[0].blocks[0] else {
        panic!("expected direct cell list paragraph");
    };
    let Block::Table(nested) = &table.rows[0].cells[0].blocks[1] else {
        panic!("expected nested table");
    };
    let Block::Paragraph(nested_item) = &nested.rows[0].cells[0].blocks[0] else {
        panic!("expected nested cell list paragraph");
    };

    let paragraphs = [direct, nested_item];
    let observed: Vec<_> = paragraphs
        .into_iter()
        .map(|paragraph| {
            let list = paragraph
                .props
                .list
                .as_ref()
                .expect("reopened paragraph remains a list item");
            (
                paragraph.text(),
                list.level,
                list.ordered,
                list.label.clone(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        [
            ("Direct cell".to_string(), 0, true, "1.".to_string()),
            ("Nested cell".to_string(), 0, true, "2.".to_string()),
        ]
    );
}

#[test]
fn image_builder_adds_alt_text_and_size() {
    let png = tiny_png();
    let model = DocBuilder::new()
        .rich_image(
            ImageBuilder::new(png.clone(), "image/png")
                .alt(" Chart <trend> ")
                .size_px(200, 100),
        )
        .build();

    let Block::Image(image) = &model.blocks[0] else {
        panic!("expected image block");
    };
    assert_eq!(image.bytes.as_deref(), Some(png.as_slice()));
    assert_eq!(image.mime.as_deref(), Some("image/png"));
    assert_eq!(image.alt.as_deref(), Some("Chart <trend>"));
    assert_eq!(image.width_px, Some(200));
    assert_eq!(image.height_px, Some(100));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<wp:extent cx="1905000" cy="952500"/>"#)
            && document_xml
                .contains(r#"<wp:docPr id="1" name="Image1" descr="Chart &lt;trend&gt;"/>"#),
        "image metadata missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("image builder .docx reopens");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].bytes.as_deref(), Some(png.as_slice()));
    assert_eq!(images[0].alt.as_deref(), Some("Chart <trend>"));
    assert_eq!(reopened.to_markdown(), "![Chart \\<trend\\>]()");
    assert!(
        reopened.to_html().contains(r#"alt="Chart &lt;trend&gt;""#),
        "HTML export must preserve escaped image alt text"
    );

    let converted = Document::open(&reopened.to_docx()).expect("fresh DOCX conversion reopens");
    assert_eq!(converted.images()[0].alt.as_deref(), Some("Chart <trend>"));
}

#[test]
fn write_docx_emits_visible_placeholders_for_missing_image_bytes() {
    let model = DocModel {
        blocks: vec![
            Block::Image(rwml::Image {
                alt: Some("Missing <logo>".to_string()),
                width_px: Some(80),
                height_px: Some(40),
                ..rwml::Image::default()
            }),
            Block::Paragraph(Paragraph {
                runs: vec![rwml::Run {
                    image: Some(rwml::Image {
                        alt: Some("Inline <icon>".to_string()),
                        width_px: Some(16),
                        height_px: Some(16),
                        ..rwml::Image::default()
                    }),
                    ..rwml::Run::default()
                }],
                ..Paragraph::default()
            }),
        ],
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    assert!(
        document_xml.contains("[rwml image placeholder: Missing &lt;logo&gt;]")
            && document_xml.contains("[rwml image placeholder: Inline &lt;icon&gt;]"),
        "missing-image placeholder text not serialized: {document_xml}"
    );
    assert!(
        !parts.keys().any(|part| part.starts_with("word/media/")),
        "missing image bytes should not emit media parts: {:?}",
        parts.keys().collect::<Vec<_>>()
    );

    let reopened = Document::open(&bytes).expect("missing-image placeholder .docx reopens");
    let text = reopened.text();
    assert!(
        text.contains("[rwml image placeholder: Missing <logo>]")
            && text.contains("[rwml image placeholder: Inline <icon>]"),
        "placeholder text not readable after reopen: {text:?}"
    );
}

#[test]
fn image_builder_preserves_common_raster_media_types() {
    for (mime, ext, bytes) in [
        ("image/jpeg", "jpg", tiny_jpeg()),
        ("image/gif", "gif", tiny_gif()),
        ("image/bmp", "bmp", tiny_bmp()),
    ] {
        let model = DocBuilder::new()
            .rich_image(ImageBuilder::new(bytes.clone(), mime))
            .build();

        let docx = rwml::write_docx(&model);
        let parts = unzip_parts(&docx);
        let media = format!("word/media/image1.{ext}");
        let target = format!(r#"Target="media/image1.{ext}""#);
        let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
        let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

        assert!(
            rels.contains(&target),
            "expected media target {target}: {rels}"
        );
        assert!(
            content_types.contains(&format!(r#"ContentType="{mime}""#)),
            "expected content type {mime}: {content_types}"
        );
        assert_eq!(parts[&media], bytes);

        let reopened = Document::open(&docx).expect("authored image .docx reopens");
        let images = reopened.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime.as_deref(), Some(mime));
        assert_eq!(images[0].bytes.as_deref(), Some(bytes.as_slice()));
        assert_eq!(images[0].width_px, Some(2));
        assert_eq!(images[0].height_px, Some(3));
    }
}

#[test]
fn image_builder_preserves_webp_media_type() {
    let webp = tiny_webp();
    let model = DocBuilder::new()
        .rich_image(ImageBuilder::new(webp.clone(), "image/webp"))
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    assert!(
        rels.contains(r#"Target="media/image1.webp""#),
        "expected webp media target: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="image/webp""#),
        "expected webp content type: {content_types}"
    );
    assert_eq!(parts["word/media/image1.webp"], webp);

    let reopened = Document::open(&bytes).expect("webp image .docx reopens");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/webp"));
    assert_eq!(images[0].bytes.as_deref(), Some(webp.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn image_builder_preserves_tiff_media_type() {
    let tiff = tiny_tiff();
    let model = DocBuilder::new()
        .rich_image(ImageBuilder::new(tiff.clone(), "image/tiff"))
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    assert!(
        rels.contains(r#"Target="media/image1.tif""#),
        "expected tiff media target: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="image/tiff""#),
        "expected tiff content type: {content_types}"
    );
    assert_eq!(parts["word/media/image1.tif"], tiff);

    let reopened = Document::open(&bytes).expect("tiff image .docx reopens");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime.as_deref(), Some("image/tiff"));
    assert_eq!(images[0].bytes.as_deref(), Some(tiff.as_slice()));
    assert_eq!(images[0].width_px, Some(2));
    assert_eq!(images[0].height_px, Some(3));
}

#[test]
fn image_builder_adds_rotation() {
    let png = tiny_png();
    let model = DocBuilder::new()
        .rich_image(
            ImageBuilder::new(png.clone(), "image/png")
                .alt(" ")
                .size_px(200, 100)
                .rotate_degrees(90),
        )
        .build();

    let Block::Image(image) = &model.blocks[0] else {
        panic!("expected image block");
    };
    assert_eq!(image.rotation_degrees, Some(90));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<a:xfrm rot="5400000"><a:off x="0" y="0"/>"#)
            && !document_xml.contains(r#" descr=""#),
        "image rotation missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("rotated image builder .docx reopens");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].bytes.as_deref(), Some(png.as_slice()));
    assert_eq!(images[0].rotation_degrees, Some(90));
}

#[cfg(feature = "render")]
#[test]
fn authored_image_rotation_survives_reopen_and_changes_pdf() {
    let image = ImageBuilder::new(tiny_png(), "image/png")
        .size_px(200, 100)
        .build();
    let model_with_rotation = |rotation_degrees| DocModel {
        blocks: vec![
            Block::Image(rwml::Image {
                rotation_degrees,
                ..image.clone()
            }),
            Block::Paragraph(Paragraph {
                runs: vec![rwml::Run {
                    image: Some(rwml::Image {
                        rotation_degrees,
                        ..image.clone()
                    }),
                    ..rwml::Run::default()
                }],
                ..Paragraph::default()
            }),
        ],
        ..DocModel::default()
    };

    let default_reopened = Document::open(&rwml::write_docx(&model_with_rotation(None)))
        .expect("unrotated image document reopens");
    let rotated_reopened = Document::open(&rwml::write_docx(&model_with_rotation(Some(90))))
        .expect("rotated image document reopens");
    assert_eq!(
        rotated_reopened
            .images()
            .iter()
            .map(|image| image.rotation_degrees)
            .collect::<Vec<_>>(),
        vec![Some(90), Some(90)]
    );

    let default_pdf = default_reopened
        .try_to_pdf()
        .expect("unrotated image document renders");
    let rotated_pdf = rotated_reopened
        .try_to_pdf()
        .expect("rotated image document renders");
    assert!(rotated_pdf.starts_with(b"%PDF"));
    assert_ne!(
        rotated_pdf, default_pdf,
        "reopened image rotation must affect PDF bytes"
    );
    assert_eq!(
        rotated_pdf,
        rotated_reopened
            .try_to_pdf()
            .expect("rotated image document repeat render")
    );

    let direct_default = rwml::try_render_pdf(&model_with_rotation(None))
        .expect("direct unrotated image model renders");
    let direct_full_turn = rwml::try_render_pdf(&model_with_rotation(Some(360)))
        .expect("direct full-turn image model renders");
    assert_eq!(
        direct_full_turn, direct_default,
        "a normalized full turn must preserve unrotated PDF output"
    );
    let direct_quarter_turn = rwml::try_render_pdf(&model_with_rotation(Some(90)))
        .expect("direct quarter-turn image model renders");
    let direct_negative_equivalent = rwml::try_render_pdf(&model_with_rotation(Some(-270)))
        .expect("direct negative equivalent image model renders");
    assert_eq!(
        direct_negative_equivalent, direct_quarter_turn,
        "equivalent direct-model angles must render identically"
    );
    let direct_arbitrary = rwml::try_render_pdf(&model_with_rotation(Some(45)))
        .expect("direct arbitrary-angle image model renders");
    assert_eq!(
        direct_arbitrary,
        rwml::try_render_pdf(&model_with_rotation(Some(45)))
            .expect("direct arbitrary-angle image model repeat render")
    );

    let run_only_model = |rotation_degrees| DocModel {
        blocks: vec![Block::Paragraph(Paragraph {
            runs: vec![rwml::Run {
                image: Some(rwml::Image {
                    rotation_degrees,
                    ..image.clone()
                }),
                ..rwml::Run::default()
            }],
            ..Paragraph::default()
        })],
        ..DocModel::default()
    };
    let run_only_default =
        rwml::try_render_pdf(&run_only_model(None)).expect("run-only unrotated model renders");
    let run_only_rotated =
        rwml::try_render_pdf(&run_only_model(Some(90))).expect("run-only rotated model renders");
    assert_ne!(
        run_only_rotated, run_only_default,
        "run-attached image rotation must independently affect PDF bytes"
    );
}

#[test]
fn image_builder_adds_floating_anchor_offset() {
    let png = tiny_png();
    let model = DocBuilder::new()
        .rich_image(
            ImageBuilder::new(png.clone(), "image/png")
                .size_px(200, 100)
                .alt("Float <image>")
                .floating_offset_emu(91440, 182880),
        )
        .build();

    let Block::Image(image) = &model.blocks[0] else {
        panic!("expected image block");
    };
    assert_eq!(image.floating_offset_emu, Some((91440, 182880)));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    assert!(
        document_xml.contains(r#"<wp:anchor "#)
            && !document_xml.contains("<wp:inline")
            && document_xml.contains(
                r#"<wp:positionH relativeFrom="page"><wp:posOffset>91440</wp:posOffset></wp:positionH>"#
            )
            && document_xml.contains(
                r#"<wp:positionV relativeFrom="page"><wp:posOffset>182880</wp:posOffset></wp:positionV>"#
            )
            && document_xml.contains(r#"<wp:wrapSquare wrapText="bothSides"/>"#)
            && document_xml
                .contains(r#"<wp:docPr id="1" name="Image1" descr="Float &lt;image&gt;"/>"#),
        "floating image anchor missing: {document_xml}"
    );

    let reopened = Document::open(&bytes).expect("floating image builder .docx reopens");
    let images = reopened.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].bytes.as_deref(), Some(png.as_slice()));
    assert_eq!(images[0].floating_offset_emu, Some((91440, 182880)));
    let shapes = reopened.floating_shapes();
    assert_eq!(shapes.len(), 1);
    assert_eq!(
        shapes[0]
            .horizontal_position
            .as_ref()
            .and_then(|pos| pos.offset_emu),
        Some(91440)
    );
    assert_eq!(
        shapes[0]
            .vertical_position
            .as_ref()
            .and_then(|pos| pos.offset_emu),
        Some(182880)
    );
}

#[test]
fn doc_builder_adds_bar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar()
                .title(" Quarterly revenue ")
                .categories(["Q1", "Q2"])
                .series("Revenue", [42.0, 51.5])
                .size_px(480, 320)
                .alt(" Revenue chart "),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.title.as_deref(), Some("Quarterly revenue"));
    assert_eq!(chart.categories, ["Q1".to_string(), "Q2".to_string()]);
    assert_eq!(chart.series.len(), 1);
    assert_eq!(chart.series[0].name, "Revenue");
    assert_eq!(chart.series[0].values, [42.0, 51.5]);
    assert_eq!(chart.alt.as_deref(), Some("Revenue chart"));

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:extent cx="4572000" cy="3048000"/>"#)
            && document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Revenue chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "chart rel missing: {rels}"
    );
    assert!(
        content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml""#
        ),
        "chart content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains("<c:barChart>")
            && chart_xml.contains("<a:t>Quarterly revenue</a:t>")
            && chart_xml.contains("<c:v>Revenue</c:v>")
            && chart_xml.contains("<c:v>Q1</c:v>")
            && chart_xml.contains("<c:v>42</c:v>")
            && chart_xml.contains("<c:v>51.5</c:v>"),
        "chart payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("chart builder .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn chart_builder_ignores_blank_chart_title() {
    let mut model = DocBuilder::new()
        .chart(
            ChartBuilder::bar()
                .title(" \t ")
                .categories(["Q1"])
                .series("Revenue", [42.0]),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.title, None);

    let Block::Chart(chart) = &mut model.blocks[0] else {
        panic!("expected chart block");
    };
    chart.title = Some(" \n ".to_string());

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    assert!(
        !chart_xml.contains("<c:title>"),
        "blank chart title should not emit title XML: {chart_xml}"
    );
}

#[test]
fn doc_builder_embeds_workbook_backing_chart_data() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar()
                .title("Quarterly revenue")
                .categories(["Q1", "Q2"])
                .series("Revenue", [42.0, 51.5]),
        )
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(
        parts
            .get("word/charts/_rels/chart1.xml.rels")
            .expect("chart relationship part")
            .clone(),
    )
    .unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let workbook_content_types =
        String::from_utf8(workbook_parts["[Content_Types].xml"].clone()).unwrap();
    let workbook_xml = String::from_utf8(workbook_parts["xl/workbook.xml"].clone()).unwrap();
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let shared_strings = String::from_utf8(workbook_parts["xl/sharedStrings.xml"].clone()).unwrap();

    assert!(
        content_types.contains(r#"PartName="/word/embeddings/Microsoft_Excel_Worksheet1.xlsx""#)
            && content_types.contains(
                r#"ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet""#
            ),
        "embedded workbook content type missing: {content_types}"
    );
    assert!(
        chart_xml
            .contains(r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#),
        "chart external workbook reference missing: {chart_xml}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        workbook_content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml""#
        ) && workbook_content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml""#
        ) && workbook_content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml""#
        ),
        "workbook content types missing: {workbook_content_types}"
    );
    assert!(
        workbook_xml.contains(r#"<sheet name="Chart Data" sheetId="1" r:id="rId1"/>"#),
        "workbook sheet missing: {workbook_xml}"
    );
    assert!(
        shared_strings.contains("<t>Category</t>")
            && shared_strings.contains("<t>Revenue</t>")
            && shared_strings.contains("<t>Q1</t>")
            && shared_strings.contains("<t>Q2</t>"),
        "chart labels missing from shared strings: {shared_strings}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>42</v></c>"#)
            && sheet_xml.contains(r#"<c r="B3"><v>51.5</v></c>"#),
        "chart values missing from worksheet: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("workbook-backed chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_column_and_line_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::column()
                .title("Quarterly pipeline")
                .categories(["Q1", "Q2"])
                .series("Pipeline", [10.0, 14.5])
                .size_px(420, 280)
                .alt("Pipeline column chart"),
        )
        .chart(
            ChartBuilder::line()
                .title("Retention trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Retention", [0.91, 0.93, 0.95])
                .size_px(420, 280)
                .alt("Retention line chart"),
        )
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let column_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let line_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Pipeline column chart"/>"#)
            && document_xml
                .contains(r#"<wp:docPr id="2" name="Chart2" descr="Retention line chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "chart relationships missing: {rels}"
    );
    assert!(
        column_xml.contains("<c:barChart>")
            && column_xml.contains(r#"<c:barDir val="col"/>"#)
            && column_xml.contains("<a:t>Quarterly pipeline</a:t>")
            && column_xml.contains("<c:v>Pipeline</c:v>")
            && column_xml.contains("<c:v>14.5</c:v>"),
        "column chart payload missing: {column_xml}"
    );
    assert!(
        line_xml.contains("<c:lineChart>")
            && line_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && line_xml.contains("<a:t>Retention trend</a:t>")
            && line_xml.contains("<c:v>Retention</c:v>")
            && line_xml.contains("<c:v>0.95</c:v>"),
        "line chart payload missing: {line_xml}"
    );

    let reopened = Document::open(&bytes).expect("multi-chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_markerless_line_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::line_no_markers()
                .title("Retention trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Retention", [0.91, 0.93, 0.95])
                .size_px(420, 280)
                .alt("Retention markerless line chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::LineNoMarkers);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded markerless line chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Retention markerless line chart"/>"#
        ) && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "markerless line chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "markerless line chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "markerless line chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:lineChart>")
            && chart_xml.contains(r#"<c:marker><c:symbol val="none"/></c:marker>"#)
            && chart_xml.contains("<a:t>Retention trend</a:t>")
            && chart_xml.contains("<c:v>Retention</c:v>")
            && chart_xml.contains("<c:v>Mar</c:v>")
            && chart_xml.contains("<c:v>0.95</c:v>"),
        "markerless line chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>0.91</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>0.95</v></c>"#),
        "markerless line chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("markerless line chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_smooth_line_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::smooth_line()
                .title("Retention curve")
                .categories(["Jan", "Feb", "Mar"])
                .series("Retention", [0.91, 0.93, 0.95])
                .size_px(420, 280)
                .alt("Retention smooth line chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::SmoothLine);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded smooth line chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Retention smooth line chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "smooth line chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "smooth line chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "smooth line chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:lineChart>")
            && chart_xml.contains(r#"<c:smooth val="1"/>"#)
            && chart_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && chart_xml.contains("<a:t>Retention curve</a:t>")
            && chart_xml.contains("<c:v>Retention</c:v>")
            && chart_xml.contains("<c:v>Mar</c:v>")
            && chart_xml.contains("<c:v>0.95</c:v>"),
        "smooth line chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>0.91</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>0.95</v></c>"#),
        "smooth line chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("smooth line chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_stacked_line_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_line()
                .title("Retention stack")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(420, 280)
                .alt("Retention stacked line chart"),
        )
        .chart(
            ChartBuilder::percent_stacked_line()
                .title("Retention mix")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(420, 280)
                .alt("Retention percent stacked line chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::StackedLine);
    assert_eq!(second.kind, ChartKind::PercentStackedLine);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let stacked_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let percent_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stacked line chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded percent stacked line chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Retention stacked line chart"/>"#)
            && document_xml.contains(
                r#"<wp:docPr id="2" name="Chart2" descr="Retention percent stacked line chart"/>"#
            )
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "stacked line chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "stacked line chart relationships missing: {rels}"
    );
    assert!(
        stacked_xml.contains("<c:lineChart>")
            && stacked_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && stacked_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && stacked_xml.contains("<a:t>Retention stack</a:t>")
            && stacked_xml.contains("<c:v>Paid</c:v>")
            && stacked_xml.contains("<c:v>Mar</c:v>")
            && stacked_xml.contains("<c:v>21</c:v>"),
        "stacked line chart payload missing: {stacked_xml}"
    );
    assert!(
        percent_xml.contains("<c:lineChart>")
            && percent_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && percent_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && percent_xml.contains("<a:t>Retention mix</a:t>")
            && percent_xml.contains("<c:v>Free</c:v>")
            && percent_xml.contains("<c:v>Mar</c:v>")
            && percent_xml.contains("<c:v>21</c:v>"),
        "percent stacked line chart payload missing: {percent_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C4"><v>21</v></c>"#)
            && sheet2_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet2_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "stacked line chart workbook values missing: {sheet1_xml} {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("stacked line charts .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_stacked_bar_and_column_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_bar()
                .title("Regional backlog")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0])
                .size_px(460, 300)
                .alt("Regional backlog stacked bar chart"),
        )
        .chart(
            ChartBuilder::stacked_column()
                .title("Quarterly pipeline")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0])
                .series("Committed", [6.0, 8.5, 11.0])
                .size_px(460, 300)
                .alt("Quarterly pipeline stacked column chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::StackedBar);
    assert_eq!(second.kind, ChartKind::StackedColumn);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let bar_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let column_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stacked bar chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded stacked column chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Regional backlog stacked bar chart"/>"#
        ) && document_xml.contains(
            r#"<wp:docPr id="2" name="Chart2" descr="Quarterly pipeline stacked column chart"/>"#
        ) && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "stacked chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "stacked chart relationships missing: {rels}"
    );
    assert!(
        bar_xml.contains("<c:barChart>")
            && bar_xml.contains(r#"<c:barDir val="bar"/>"#)
            && bar_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && bar_xml.contains(r#"<c:overlap val="100"/>"#)
            && bar_xml.contains("<a:t>Regional backlog</a:t>")
            && bar_xml.contains("<c:v>Closed</c:v>")
            && bar_xml.contains("<c:v>23.5</c:v>"),
        "stacked bar chart payload missing: {bar_xml}"
    );
    assert!(
        column_xml.contains("<c:barChart>")
            && column_xml.contains(r#"<c:barDir val="col"/>"#)
            && column_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && column_xml.contains(r#"<c:overlap val="100"/>"#)
            && column_xml.contains("<a:t>Quarterly pipeline</a:t>")
            && column_xml.contains("<c:v>Committed</c:v>")
            && column_xml.contains("<c:v>Q3</c:v>"),
        "stacked column chart payload missing: {column_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>18</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C3"><v>12</v></c>"#),
        "stacked bar chart workbook values missing: {sheet1_xml}"
    );
    assert!(
        sheet2_xml.contains(r#"<c r="B4"><v>18</v></c>"#)
            && sheet2_xml.contains(r#"<c r="C4"><v>11</v></c>"#),
        "stacked column chart workbook values missing: {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("stacked bar/column chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_percent_stacked_bar_and_column_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::percent_stacked_bar()
                .title("Regional mix")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0])
                .size_px(460, 300)
                .alt("Regional mix percent stacked bar chart"),
        )
        .chart(
            ChartBuilder::percent_stacked_column()
                .title("Quarterly mix")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0])
                .series("Committed", [6.0, 8.5, 11.0])
                .size_px(460, 300)
                .alt("Quarterly mix percent stacked column chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::PercentStackedBar);
    assert_eq!(second.kind, ChartKind::PercentStackedColumn);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let bar_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let column_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded percent stacked column chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        bar_xml.contains("<c:barChart>")
            && bar_xml.contains(r#"<c:barDir val="bar"/>"#)
            && bar_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && bar_xml.contains(r#"<c:overlap val="100"/>"#)
            && bar_xml.contains("<a:t>Regional mix</a:t>")
            && bar_xml.contains("<c:v>Closed</c:v>")
            && bar_xml.contains("<c:v>23.5</c:v>"),
        "percent stacked bar chart payload missing: {bar_xml}"
    );
    assert!(
        column_xml.contains("<c:barChart>")
            && column_xml.contains(r#"<c:barDir val="col"/>"#)
            && column_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && column_xml.contains(r#"<c:overlap val="100"/>"#)
            && column_xml.contains("<a:t>Quarterly mix</a:t>")
            && column_xml.contains("<c:v>Committed</c:v>")
            && column_xml.contains("<c:v>Q3</c:v>"),
        "percent stacked column chart payload missing: {column_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B4"><v>18</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>11</v></c>"#),
        "percent stacked column chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("percent stacked chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_3d_bar_and_column_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar_3d()
                .title("Regional backlog")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0])
                .size_px(460, 300)
                .alt("Regional backlog 3-D bar chart"),
        )
        .chart(
            ChartBuilder::column_3d()
                .title("Quarterly pipeline")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0])
                .size_px(460, 300)
                .alt("Quarterly pipeline 3-D column chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::Bar3D);
    assert_eq!(second.kind, ChartKind::Column3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let bar_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let column_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let bar_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let column_rels =
        String::from_utf8(parts["word/charts/_rels/chart2.xml.rels"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 3-D bar chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded 3-D column chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Regional backlog 3-D bar chart"/>"#)
            && document_xml.contains(
                r#"<wp:docPr id="2" name="Chart2" descr="Quarterly pipeline 3-D column chart"/>"#
            )
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "3-D bar/column chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "3-D bar/column chart relationships missing: {rels}"
    );
    assert!(
        bar_rels.contains("relationships/package")
            && bar_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "3-D bar chart workbook relationship missing: {bar_rels}"
    );
    assert!(
        column_rels.contains("relationships/package")
            && column_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet2.xlsx""#),
        "3-D column chart workbook relationship missing: {column_rels}"
    );
    assert!(
        bar_xml.contains("<c:bar3DChart>")
            && bar_xml.contains(r#"<c:barDir val="bar"/>"#)
            && bar_xml.contains(r#"<c:grouping val="clustered"/>"#)
            && bar_xml.contains(r#"<c:shape val="box"/>"#)
            && bar_xml.contains(r#"<c:gapDepth val="150"/>"#)
            && bar_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && bar_xml.contains("<a:t>Regional backlog</a:t>")
            && bar_xml.contains("<c:v>Open</c:v>")
            && bar_xml.contains("<c:v>Closed</c:v>")
            && bar_xml.contains("<c:v>South</c:v>")
            && bar_xml.contains("<c:v>23.5</c:v>"),
        "3-D bar chart payload missing: {bar_xml}"
    );
    assert!(
        column_xml.contains("<c:bar3DChart>")
            && column_xml.contains(r#"<c:barDir val="col"/>"#)
            && column_xml.contains(r#"<c:grouping val="clustered"/>"#)
            && column_xml.contains(r#"<c:shape val="box"/>"#)
            && column_xml.contains(r#"<c:gapDepth val="150"/>"#)
            && column_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && column_xml.contains("<a:t>Quarterly pipeline</a:t>")
            && column_xml.contains("<c:v>Pipeline</c:v>")
            && column_xml.contains("<c:v>Q3</c:v>")
            && column_xml.contains("<c:v>18</c:v>"),
        "3-D column chart payload missing: {column_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>18</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C3"><v>12</v></c>"#),
        "3-D bar chart workbook values missing: {sheet1_xml}"
    );
    assert!(
        sheet2_xml.contains(r#"<c r="B2"><v>10</v></c>"#)
            && sheet2_xml.contains(r#"<c r="B4"><v>18</v></c>"#),
        "3-D column chart workbook values missing: {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("3-D bar/column chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_stacked_3d_column_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_column_3d()
                .shape(ChartShape::Pyramid)
                .title("Quarterly stack")
                .categories(["Q1", "Q2", "Q3"])
                .series("Committed", [10.0, 14.5, 18.0])
                .series("Upside", [4.0, 6.5, 7.0])
                .size_px(460, 300)
                .alt("Quarterly stacked 3-D column chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::StackedColumn3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone())
        .expect("chart rels utf8");
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stacked 3-D column chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Quarterly stacked 3-D column chart"/>"#
        ),
        "stacked 3-D column chart drawing missing: {document_xml}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "stacked 3-D column chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:bar3DChart>")
            && chart_xml.contains(r#"<c:barDir val="col"/>"#)
            && chart_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && chart_xml.contains(r#"<c:overlap val="100"/>"#)
            && chart_xml.contains(r#"<c:shape val="pyramid"/>"#)
            && chart_xml.contains("<a:t>Quarterly stack</a:t>")
            && chart_xml.contains("<c:v>Committed</c:v>")
            && chart_xml.contains("<c:v>Upside</c:v>")
            && chart_xml.contains("<c:v>Q3</c:v>")
            && chart_xml.contains("<c:v>18</c:v>")
            && chart_xml.contains("<c:v>7</c:v>"),
        "stacked 3-D column chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>10</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>7</v></c>"#),
        "stacked 3-D column chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("stacked 3-D column chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_percent_stacked_3d_column_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::percent_stacked_column_3d()
                .shape(ChartShape::Cone)
                .title("Quarterly share")
                .categories(["Q1", "Q2", "Q3"])
                .series("Committed", [10.0, 14.5, 18.0])
                .series("Upside", [4.0, 6.5, 7.0])
                .size_px(460, 300)
                .alt("Quarterly 100% stacked 3-D column chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::PercentStackedColumn3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 100% stacked 3-D column chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Quarterly 100% stacked 3-D column chart"/>"#
        ),
        "100% stacked 3-D column chart drawing missing: {document_xml}"
    );
    assert!(
        chart_xml.contains("<c:bar3DChart>")
            && chart_xml.contains(r#"<c:barDir val="col"/>"#)
            && chart_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && chart_xml.contains(r#"<c:overlap val="100"/>"#)
            && chart_xml.contains(r#"<c:shape val="cone"/>"#)
            && chart_xml.contains("<a:t>Quarterly share</a:t>")
            && chart_xml.contains("<c:v>Committed</c:v>")
            && chart_xml.contains("<c:v>Upside</c:v>")
            && chart_xml.contains("<c:v>Q3</c:v>")
            && chart_xml.contains("<c:v>18</c:v>")
            && chart_xml.contains("<c:v>7</c:v>"),
        "100% stacked 3-D column chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>10</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>7</v></c>"#),
        "100% stacked 3-D column chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("100% stacked 3-D column chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_stacked_3d_bar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_bar_3d()
                .shape(ChartShape::Cylinder)
                .title("Regional stack")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0])
                .size_px(460, 300)
                .alt("Regional stacked 3-D bar chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::StackedBar3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stacked 3-D bar chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Regional stacked 3-D bar chart"/>"#),
        "stacked 3-D bar chart drawing missing: {document_xml}"
    );
    assert!(
        chart_xml.contains("<c:bar3DChart>")
            && chart_xml.contains(r#"<c:barDir val="bar"/>"#)
            && chart_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && chart_xml.contains(r#"<c:overlap val="100"/>"#)
            && chart_xml.contains(r#"<c:shape val="cylinder"/>"#)
            && chart_xml.contains("<a:t>Regional stack</a:t>")
            && chart_xml.contains("<c:v>Open</c:v>")
            && chart_xml.contains("<c:v>Closed</c:v>")
            && chart_xml.contains("<c:v>South</c:v>")
            && chart_xml.contains("<c:v>23.5</c:v>")
            && chart_xml.contains("<c:v>12</c:v>"),
        "stacked 3-D bar chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>18</v></c>"#)
            && sheet_xml.contains(r#"<c r="C3"><v>12</v></c>"#),
        "stacked 3-D bar chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("stacked 3-D bar chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_percent_stacked_3d_bar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::percent_stacked_bar_3d()
                .shape(ChartShape::Cylinder)
                .title("Regional share")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0])
                .size_px(460, 300)
                .alt("Regional 100% stacked 3-D bar chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::PercentStackedBar3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 100% stacked 3-D bar chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Regional 100% stacked 3-D bar chart"/>"#
        ),
        "100% stacked 3-D bar chart drawing missing: {document_xml}"
    );
    assert!(
        chart_xml.contains("<c:bar3DChart>")
            && chart_xml.contains(r#"<c:barDir val="bar"/>"#)
            && chart_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && chart_xml.contains(r#"<c:overlap val="100"/>"#)
            && chart_xml.contains(r#"<c:shape val="cylinder"/>"#)
            && chart_xml.contains("<a:t>Regional share</a:t>")
            && chart_xml.contains("<c:v>Open</c:v>")
            && chart_xml.contains("<c:v>Closed</c:v>")
            && chart_xml.contains("<c:v>South</c:v>")
            && chart_xml.contains("<c:v>23.5</c:v>")
            && chart_xml.contains("<c:v>12</c:v>"),
        "100% stacked 3-D bar chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>18</v></c>"#)
            && sheet_xml.contains(r#"<c r="C3"><v>12</v></c>"#),
        "100% stacked 3-D bar chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("100% stacked 3-D bar chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_3d_bar_and_column_shape_styling() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar_3d()
                .shape(ChartShape::Cylinder)
                .title("Regional backlog cylinders")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0])
                .size_px(460, 300)
                .alt("Regional backlog cylinder 3-D bar chart"),
        )
        .chart(
            ChartBuilder::column_3d()
                .shape(ChartShape::Pyramid)
                .title("Quarterly pipeline pyramids")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0])
                .size_px(460, 300)
                .alt("Quarterly pipeline pyramid 3-D column chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::Bar3D);
    assert_eq!(first.shape, ChartShape::Cylinder);
    assert_eq!(second.kind, ChartKind::Column3D);
    assert_eq!(second.shape, ChartShape::Pyramid);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let bar_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let column_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let bar_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let column_rels =
        String::from_utf8(parts["word/charts/_rels/chart2.xml.rels"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Regional backlog cylinder 3-D bar chart"/>"#
        ) && document_xml.contains(
            r#"<wp:docPr id="2" name="Chart2" descr="Quarterly pipeline pyramid 3-D column chart"/>"#
        ),
        "3-D shaped bar/column chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "3-D shaped bar/column chart relationships missing: {rels}"
    );
    assert!(
        bar_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#)
            && column_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet2.xlsx""#),
        "3-D shaped bar/column workbook relationships missing: {bar_rels} {column_rels}"
    );
    assert!(
        bar_xml.contains("<c:bar3DChart>")
            && bar_xml.contains(r#"<c:barDir val="bar"/>"#)
            && bar_xml.contains(r#"<c:shape val="cylinder"/>"#)
            && bar_xml.contains("<a:t>Regional backlog cylinders</a:t>")
            && bar_xml.contains("<c:v>Closed</c:v>")
            && bar_xml.contains("<c:v>23.5</c:v>"),
        "3-D cylinder bar chart payload missing: {bar_xml}"
    );
    assert!(
        column_xml.contains("<c:bar3DChart>")
            && column_xml.contains(r#"<c:barDir val="col"/>"#)
            && column_xml.contains(r#"<c:shape val="pyramid"/>"#)
            && column_xml.contains("<a:t>Quarterly pipeline pyramids</a:t>")
            && column_xml.contains("<c:v>Q3</c:v>")
            && column_xml.contains("<c:v>18</c:v>"),
        "3-D pyramid column chart payload missing: {column_xml}"
    );

    let reopened = Document::open(&bytes).expect("3-D shaped bar/column chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_area_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::area()
                .title("Adoption trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(420, 280)
                .alt("Adoption area chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Area);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded area chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Adoption area chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "area chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "area chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "area chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:areaChart>")
            && chart_xml.contains(r#"<c:grouping val="standard"/>"#)
            && chart_xml.contains(r#"<c:varyColors val="0"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Adoption trend</a:t>")
            && chart_xml.contains("<c:v>Free</c:v>")
            && chart_xml.contains("<c:v>Paid</c:v>")
            && chart_xml.contains("<c:v>Mar</c:v>")
            && chart_xml.contains("<c:v>21</c:v>"),
        "area chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet_xml.contains(r#"<c r="C3"><v>13.5</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "area chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("area chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_stacked_area_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_area()
                .title("Adoption stack")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(420, 280)
                .alt("Adoption stacked area chart"),
        )
        .chart(
            ChartBuilder::percent_stacked_area()
                .title("Adoption mix")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(420, 280)
                .alt("Adoption percent stacked area chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::StackedArea);
    assert_eq!(second.kind, ChartKind::PercentStackedArea);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let stacked_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let percent_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stacked area chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded percent stacked area chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Adoption stacked area chart"/>"#)
            && document_xml.contains(
                r#"<wp:docPr id="2" name="Chart2" descr="Adoption percent stacked area chart"/>"#
            )
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "stacked area chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "stacked area chart relationships missing: {rels}"
    );
    assert!(
        stacked_xml.contains("<c:areaChart>")
            && stacked_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && stacked_xml.contains("<a:t>Adoption stack</a:t>")
            && stacked_xml.contains("<c:v>Paid</c:v>")
            && stacked_xml.contains("<c:v>Mar</c:v>")
            && stacked_xml.contains("<c:v>21</c:v>"),
        "stacked area chart payload missing: {stacked_xml}"
    );
    assert!(
        percent_xml.contains("<c:areaChart>")
            && percent_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && percent_xml.contains("<a:t>Adoption mix</a:t>")
            && percent_xml.contains("<c:v>Free</c:v>")
            && percent_xml.contains("<c:v>Mar</c:v>")
            && percent_xml.contains("<c:v>21</c:v>"),
        "percent stacked area chart payload missing: {percent_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C4"><v>21</v></c>"#)
            && sheet2_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet2_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "stacked area chart workbook values missing: {sheet1_xml} {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("stacked area charts .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_3d_line_and_area_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::line_3d()
                .title("Retention trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Current", [0.91, 0.93, 0.95])
                .series("Target", [0.94, 0.95, 0.97])
                .size_px(460, 300)
                .alt("Retention trend 3-D line chart"),
        )
        .chart(
            ChartBuilder::area_3d()
                .title("Adoption trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(460, 300)
                .alt("Adoption trend 3-D area chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::Line3D);
    assert_eq!(second.kind, ChartKind::Area3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let line_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let area_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let line_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let area_rels = String::from_utf8(parts["word/charts/_rels/chart2.xml.rels"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 3-D line chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded 3-D area chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Retention trend 3-D line chart"/>"#)
            && document_xml.contains(
                r#"<wp:docPr id="2" name="Chart2" descr="Adoption trend 3-D area chart"/>"#
            )
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "3-D line/area chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "3-D line/area chart relationships missing: {rels}"
    );
    assert!(
        line_rels.contains("relationships/package")
            && line_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "3-D line chart workbook relationship missing: {line_rels}"
    );
    assert!(
        area_rels.contains("relationships/package")
            && area_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet2.xlsx""#),
        "3-D area chart workbook relationship missing: {area_rels}"
    );
    assert!(
        line_xml.contains("<c:line3DChart>")
            && line_xml.contains(r#"<c:grouping val="standard"/>"#)
            && line_xml.contains(r#"<c:varyColors val="0"/>"#)
            && line_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && line_xml.contains(r#"<c:gapDepth val="150"/>"#)
            && line_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && line_xml.contains("<a:t>Retention trend</a:t>")
            && line_xml.contains("<c:v>Current</c:v>")
            && line_xml.contains("<c:v>Target</c:v>")
            && line_xml.contains("<c:v>Mar</c:v>")
            && line_xml.contains("<c:v>0.97</c:v>"),
        "3-D line chart payload missing: {line_xml}"
    );
    assert!(
        area_xml.contains("<c:area3DChart>")
            && area_xml.contains(r#"<c:grouping val="standard"/>"#)
            && area_xml.contains(r#"<c:varyColors val="0"/>"#)
            && area_xml.contains(r#"<c:gapDepth val="150"/>"#)
            && area_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && area_xml.contains("<a:t>Adoption trend</a:t>")
            && area_xml.contains("<c:v>Free</c:v>")
            && area_xml.contains("<c:v>Paid</c:v>")
            && area_xml.contains("<c:v>Mar</c:v>")
            && area_xml.contains("<c:v>21</c:v>"),
        "3-D area chart payload missing: {area_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>0.91</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C4"><v>0.97</v></c>"#),
        "3-D line chart workbook values missing: {sheet1_xml}"
    );
    assert!(
        sheet2_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet2_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "3-D area chart workbook values missing: {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("3-D line/area chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_stacked_3d_area_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_area_3d()
                .title("Adoption 3-D stack")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(460, 300)
                .alt("Adoption stacked 3-D area chart"),
        )
        .chart(
            ChartBuilder::percent_stacked_area_3d()
                .title("Adoption 3-D mix")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0])
                .size_px(460, 300)
                .alt("Adoption 100% stacked 3-D area chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::StackedArea3D);
    assert_eq!(second.kind, ChartKind::PercentStackedArea3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let stacked_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let percent_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let stacked_rels =
        String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let percent_rels =
        String::from_utf8(parts["word/charts/_rels/chart2.xml.rels"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stacked 3-D area chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded 100% stacked 3-D area chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Adoption stacked 3-D area chart"/>"#
        ) && document_xml.contains(
            r#"<wp:docPr id="2" name="Chart2" descr="Adoption 100% stacked 3-D area chart"/>"#
        ),
        "stacked 3-D area chart drawings missing: {document_xml}"
    );
    assert!(
        stacked_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#)
            && percent_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet2.xlsx""#),
        "stacked 3-D area chart workbook relationships missing: {stacked_rels} {percent_rels}"
    );
    assert!(
        stacked_xml.contains("<c:area3DChart>")
            && stacked_xml.contains(r#"<c:grouping val="stacked"/>"#)
            && stacked_xml.contains(r#"<c:gapDepth val="150"/>"#)
            && stacked_xml.contains("<a:t>Adoption 3-D stack</a:t>")
            && stacked_xml.contains("<c:v>Paid</c:v>")
            && stacked_xml.contains("<c:v>Mar</c:v>")
            && stacked_xml.contains("<c:v>21</c:v>"),
        "stacked 3-D area chart payload missing: {stacked_xml}"
    );
    assert!(
        percent_xml.contains("<c:area3DChart>")
            && percent_xml.contains(r#"<c:grouping val="percentStacked"/>"#)
            && percent_xml.contains(r#"<c:gapDepth val="150"/>"#)
            && percent_xml.contains("<a:t>Adoption 3-D mix</a:t>")
            && percent_xml.contains("<c:v>Free</c:v>")
            && percent_xml.contains("<c:v>Mar</c:v>")
            && percent_xml.contains("<c:v>21</c:v>"),
        "100% stacked 3-D area chart payload missing: {percent_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C4"><v>21</v></c>"#)
            && sheet2_xml.contains(r#"<c r="B2"><v>20</v></c>"#)
            && sheet2_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "stacked 3-D area chart workbook values missing: {sheet1_xml} {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("stacked 3-D area charts .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_radar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::radar()
                .title("Capability profile")
                .categories(["Speed", "Quality", "Cost", "Reach"])
                .series("Current", [4.0, 3.0, 2.0, 5.0])
                .series("Target", [5.0, 4.0, 4.0, 5.0])
                .size_px(420, 320)
                .alt("Capability radar chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Radar);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded radar chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Capability radar chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "radar chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "radar chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "radar chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:radarChart>")
            && chart_xml.contains(r#"<c:radarStyle val="standard"/>"#)
            && chart_xml.contains(r#"<c:varyColors val="0"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Capability profile</a:t>")
            && chart_xml.contains("<c:v>Current</c:v>")
            && chart_xml.contains("<c:v>Target</c:v>")
            && chart_xml.contains("<c:v>Speed</c:v>")
            && chart_xml.contains("<c:v>5</c:v>"),
        "radar chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>4</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>4</v></c>"#)
            && sheet_xml.contains(r#"<c r="C5"><v>5</v></c>"#),
        "radar chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("radar chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_radar_with_markers_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::radar_with_markers()
                .title("Capability markers")
                .categories(["Speed", "Quality", "Cost", "Reach"])
                .series("Current", [4.0, 3.0, 2.0, 5.0])
                .series("Target", [5.0, 4.0, 4.0, 5.0])
                .size_px(420, 320)
                .alt("Capability radar markers chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::RadarWithMarkers);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded radar markers chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Capability radar markers chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "radar markers chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "radar markers chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "radar markers chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:radarChart>")
            && chart_xml.contains(r#"<c:radarStyle val="marker"/>"#)
            && chart_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && chart_xml.contains("<a:t>Capability markers</a:t>")
            && chart_xml.contains("<c:v>Current</c:v>")
            && chart_xml.contains("<c:v>Target</c:v>")
            && chart_xml.contains("<c:v>Speed</c:v>")
            && chart_xml.contains("<c:v>5</c:v>"),
        "radar markers chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>4</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>4</v></c>"#)
            && sheet_xml.contains(r#"<c r="C5"><v>5</v></c>"#),
        "radar markers chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("radar markers chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_filled_radar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::filled_radar()
                .title("Capability coverage")
                .categories(["Speed", "Quality", "Cost", "Reach"])
                .series("Current", [4.0, 3.0, 2.0, 5.0])
                .series("Target", [5.0, 4.0, 4.0, 5.0])
                .size_px(420, 320)
                .alt("Capability filled radar chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::FilledRadar);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded filled radar chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Capability filled radar chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "filled radar chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "filled radar chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "filled radar chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:radarChart>")
            && chart_xml.contains(r#"<c:radarStyle val="filled"/>"#)
            && chart_xml.contains(r#"<c:varyColors val="0"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Capability coverage</a:t>")
            && chart_xml.contains("<c:v>Current</c:v>")
            && chart_xml.contains("<c:v>Target</c:v>")
            && chart_xml.contains("<c:v>Speed</c:v>")
            && chart_xml.contains("<c:v>5</c:v>"),
        "filled radar chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>4</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>4</v></c>"#)
            && sheet_xml.contains(r#"<c r="C5"><v>5</v></c>"#),
        "filled radar chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("filled radar chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_scatter_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter()
                .title("Latency distribution")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0])
                .size_px(420, 280)
                .alt("Latency scatter chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Scatter);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded scatter chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Latency scatter chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "scatter chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "scatter chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "scatter chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:scatterChart>")
            && chart_xml.contains(r#"<c:scatterStyle val="lineMarker"/>"#)
            && chart_xml.contains(r#"<c:varyColors val="0"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Latency distribution</a:t>")
            && chart_xml.contains("<c:v>Before</c:v>")
            && chart_xml.contains("<c:v>After</c:v>")
            && chart_xml.contains("<c:xVal>")
            && chart_xml.contains("<c:yVal>")
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>1</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>3</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>120</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>360</c:v></c:pt>"#),
        "scatter chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>120</v></c>"#)
            && sheet_xml.contains(r#"<c r="C3"><v>180</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>360</v></c>"#),
        "scatter chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("scatter chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_marker_only_scatter_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter_markers()
                .title("Latency points")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0])
                .size_px(420, 280)
                .alt("Latency marker-only scatter chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::ScatterMarkers);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded marker-only scatter chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Latency marker-only scatter chart"/>"#
        ) && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "marker-only scatter chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "marker-only scatter chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "marker-only scatter chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:scatterChart>")
            && chart_xml.contains(r#"<c:scatterStyle val="marker"/>"#)
            && chart_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && chart_xml.contains("<a:t>Latency points</a:t>")
            && chart_xml.contains("<c:v>Before</c:v>")
            && chart_xml.contains("<c:v>After</c:v>")
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>1</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>3</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>120</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>360</c:v></c:pt>"#),
        "marker-only scatter chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>120</v></c>"#)
            && sheet_xml.contains(r#"<c r="C3"><v>180</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>360</v></c>"#),
        "marker-only scatter chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("marker-only scatter chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_line_only_scatter_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter_lines()
                .title("Latency line")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0])
                .size_px(420, 280)
                .alt("Latency line-only scatter chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::ScatterLines);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded line-only scatter chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Latency line-only scatter chart"/>"#
        ) && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "line-only scatter chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "line-only scatter chart relationship missing: {rels}"
    );
    assert!(
        chart_xml.contains("<c:scatterChart>")
            && chart_xml.contains(r#"<c:scatterStyle val="line"/>"#)
            && chart_xml.contains(r#"<c:marker><c:symbol val="none"/></c:marker>"#)
            && chart_xml.contains("<a:t>Latency line</a:t>")
            && chart_xml.contains("<c:v>Before</c:v>")
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>360</c:v></c:pt>"#),
        "line-only scatter chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>120</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>360</v></c>"#),
        "line-only scatter chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("line-only scatter chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_smooth_scatter_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter_smooth()
                .title("Latency smooth markers")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0])
                .size_px(420, 280)
                .alt("Latency smooth scatter chart"),
        )
        .chart(
            ChartBuilder::scatter_smooth_no_markers()
                .title("Latency smooth")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0])
                .size_px(420, 280)
                .alt("Latency smooth markerless scatter chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::ScatterSmooth);
    assert_eq!(second.kind, ChartKind::ScatterSmoothNoMarkers);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let smooth_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let markerless_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded smooth scatter chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded smooth markerless scatter chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Latency smooth scatter chart"/>"#)
            && document_xml.contains(
                r#"<wp:docPr id="2" name="Chart2" descr="Latency smooth markerless scatter chart"/>"#
            )
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId2"/>"#),
        "smooth scatter drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "smooth scatter relationships missing: {rels}"
    );
    assert!(
        smooth_xml.contains("<c:scatterChart>")
            && smooth_xml.contains(r#"<c:scatterStyle val="smoothMarker"/>"#)
            && smooth_xml.contains(r#"<c:marker><c:symbol val="circle"/></c:marker>"#)
            && smooth_xml.contains("<a:t>Latency smooth markers</a:t>")
            && smooth_xml.contains("<c:v>Before</c:v>")
            && smooth_xml.contains(r#"<c:pt idx="2"><c:v>360</c:v></c:pt>"#),
        "smooth scatter payload missing: {smooth_xml}"
    );
    assert!(
        markerless_xml.contains("<c:scatterChart>")
            && markerless_xml.contains(r#"<c:scatterStyle val="smooth"/>"#)
            && markerless_xml.contains(r#"<c:marker><c:symbol val="none"/></c:marker>"#)
            && markerless_xml.contains("<a:t>Latency smooth</a:t>")
            && markerless_xml.contains("<c:v>After</c:v>")
            && markerless_xml.contains(r#"<c:pt idx="2"><c:v>360</c:v></c:pt>"#),
        "smooth markerless scatter payload missing: {markerless_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B2"><v>120</v></c>"#)
            && sheet1_xml.contains(r#"<c r="C4"><v>360</v></c>"#)
            && sheet2_xml.contains(r#"<c r="B2"><v>120</v></c>"#)
            && sheet2_xml.contains(r#"<c r="C4"><v>360</v></c>"#),
        "smooth scatter workbook values missing: {sheet1_xml} {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("smooth scatter charts .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_bubble_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bubble()
                .title("Opportunity size")
                .categories(["Segment A", "Segment B", "Segment C"])
                .bubble_series("Pipeline", [12.0, 24.0, 36.0], [5.0, 12.5, 21.0])
                .size_px(420, 280)
                .alt("Opportunity bubble chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Bubble);
    assert_eq!(chart.series[0].values, [12.0, 24.0, 36.0]);
    assert_eq!(chart.series[0].bubble_sizes, [5.0, 12.5, 21.0]);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded bubble chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let shared_strings = String::from_utf8(workbook_parts["xl/sharedStrings.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Opportunity bubble chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "bubble chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "bubble chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "bubble chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:bubbleChart>")
            && chart_xml.contains(r#"<c:varyColors val="0"/>"#)
            && chart_xml.contains(r#"<c:bubbleScale val="100"/>"#)
            && chart_xml.contains(r#"<c:showNegBubbles val="0"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Opportunity size</a:t>")
            && chart_xml.contains("<c:v>Pipeline</c:v>")
            && chart_xml.contains("<c:xVal>")
            && chart_xml.contains("<c:yVal>")
            && chart_xml.contains("<c:bubbleSize>")
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>1</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>3</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>12</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>36</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>5</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>21</c:v></c:pt>"#),
        "bubble chart payload missing: {chart_xml}"
    );
    assert!(
        shared_strings.contains("<t>Pipeline size</t>")
            && sheet_xml.contains(r#"<c r="B2"><v>12</v></c>"#)
            && sheet_xml.contains(r#"<c r="C2"><v>5</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "bubble chart workbook values missing: {sheet_xml} / {shared_strings}"
    );

    let reopened = Document::open(&bytes).expect("bubble chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_3d_bubble_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bubble_3d()
                .title("Opportunity depth")
                .categories(["Segment A", "Segment B", "Segment C"])
                .bubble_series("Pipeline", [12.0, 24.0, 36.0], [5.0, 12.5, 21.0])
                .size_px(420, 280)
                .alt("Opportunity 3-D bubble chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Bubble3D);
    assert_eq!(chart.series[0].values, [12.0, 24.0, 36.0]);
    assert_eq!(chart.series[0].bubble_sizes, [5.0, 12.5, 21.0]);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 3-D bubble chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let shared_strings = String::from_utf8(workbook_parts["xl/sharedStrings.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Opportunity 3-D bubble chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "3-D bubble chart drawing missing: {document_xml}"
    );
    assert!(
        chart_xml.contains("<c:bubbleChart>")
            && chart_xml.contains(r#"<c:bubble3D val="1"/>"#)
            && chart_xml.contains(r#"<c:bubbleScale val="100"/>"#)
            && chart_xml.contains(r#"<c:showNegBubbles val="0"/>"#)
            && chart_xml.contains("<a:t>Opportunity depth</a:t>")
            && chart_xml.contains("<c:v>Pipeline</c:v>")
            && chart_xml.contains("<c:bubbleSize>")
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>12</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>36</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="0"><c:v>5</c:v></c:pt>"#)
            && chart_xml.contains(r#"<c:pt idx="2"><c:v>21</c:v></c:pt>"#),
        "3-D bubble chart payload missing: {chart_xml}"
    );
    assert!(
        shared_strings.contains("<t>Pipeline size</t>")
            && sheet_xml.contains(r#"<c r="B2"><v>12</v></c>"#)
            && sheet_xml.contains(r#"<c r="C2"><v>5</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>21</v></c>"#),
        "3-D bubble chart workbook values missing: {sheet_xml} / {shared_strings}"
    );

    let reopened = Document::open(&bytes).expect("3-D bubble chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::pie()
                .title("Revenue mix")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0])
                .size_px(420, 280)
                .alt("Revenue mix pie chart"),
        )
        .build();

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Revenue mix pie chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "pie chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "pie chart relationship missing: {rels}"
    );
    assert!(
        chart_xml.contains("<c:pieChart>")
            && chart_xml.contains(r#"<c:varyColors val="1"/>"#)
            && chart_xml.contains(r#"<c:firstSliceAng val="0"/>"#)
            && chart_xml.contains("<a:t>Revenue mix</a:t>")
            && chart_xml.contains("<c:v>Share</c:v>")
            && chart_xml.contains("<c:v>Cloud</c:v>")
            && chart_xml.contains("<c:v>65</c:v>"),
        "pie chart payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("pie chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_exploded_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::exploded_pie()
                .title("Revenue breakout")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0])
                .size_px(420, 280)
                .alt("Revenue exploded pie chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::ExplodedPie);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded exploded pie chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Revenue exploded pie chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "exploded pie chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "exploded pie chart relationship missing: {rels}"
    );
    assert!(
        chart_xml.contains("<c:pieChart>")
            && chart_xml.contains(r#"<c:explosion val="25"/>"#)
            && chart_xml.contains("<a:t>Revenue breakout</a:t>")
            && chart_xml.contains("<c:v>Share</c:v>")
            && chart_xml.contains("<c:v>Cloud</c:v>")
            && chart_xml.contains("<c:v>65</c:v>"),
        "exploded pie chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>65</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>10</v></c>"#),
        "exploded pie chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("exploded pie chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_3d_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::pie_3d()
                .title("Revenue mix")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0])
                .size_px(420, 280)
                .alt("Revenue mix 3-D pie chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Pie3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 3-D pie chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Revenue mix 3-D pie chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "3-D pie chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "3-D pie chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "3-D pie chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:pie3DChart>")
            && chart_xml.contains(r#"<c:varyColors val="1"/>"#)
            && chart_xml.contains(r#"<c:firstSliceAng val="0"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Revenue mix</a:t>")
            && chart_xml.contains("<c:v>Share</c:v>")
            && chart_xml.contains("<c:v>Cloud</c:v>")
            && chart_xml.contains("<c:v>65</c:v>"),
        "3-D pie chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>65</v></c>"#)
            && sheet_xml.contains(r#"<c r="B3"><v>25</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>10</v></c>"#),
        "3-D pie chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("3-D pie chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_exploded_3d_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::exploded_pie_3d()
                .title("Revenue 3-D breakout")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0])
                .size_px(420, 280)
                .alt("Revenue exploded 3-D pie chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::ExplodedPie3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded exploded 3-D pie chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Revenue exploded 3-D pie chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "exploded 3-D pie chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "exploded 3-D pie chart relationship missing: {rels}"
    );
    assert!(
        chart_xml.contains("<c:pie3DChart>")
            && chart_xml.contains(r#"<c:explosion val="25"/>"#)
            && chart_xml.contains("<a:t>Revenue 3-D breakout</a:t>")
            && chart_xml.contains("<c:v>Share</c:v>")
            && chart_xml.contains("<c:v>Cloud</c:v>")
            && chart_xml.contains("<c:v>65</c:v>"),
        "exploded 3-D pie chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>65</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>10</v></c>"#),
        "exploded 3-D pie chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("exploded 3-D pie chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_doughnut_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::doughnut()
                .title("Revenue mix")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0])
                .size_px(420, 280)
                .alt("Revenue mix doughnut chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Doughnut);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded doughnut chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Revenue mix doughnut chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "doughnut chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "doughnut chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "doughnut chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:doughnutChart>")
            && chart_xml.contains(r#"<c:varyColors val="1"/>"#)
            && chart_xml.contains(r#"<c:firstSliceAng val="0"/>"#)
            && chart_xml.contains(r#"<c:holeSize val="50"/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Revenue mix</a:t>")
            && chart_xml.contains("<c:v>Share</c:v>")
            && chart_xml.contains("<c:v>Cloud</c:v>")
            && chart_xml.contains("<c:v>65</c:v>"),
        "doughnut chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>65</v></c>"#)
            && sheet_xml.contains(r#"<c r="B3"><v>25</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>10</v></c>"#),
        "doughnut chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("doughnut chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_exploded_doughnut_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::exploded_doughnut()
                .title("Revenue ring breakout")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0])
                .size_px(420, 280)
                .alt("Revenue exploded doughnut chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::ExplodedDoughnut);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded exploded doughnut chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Revenue exploded doughnut chart"/>"#
        ) && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "exploded doughnut chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "exploded doughnut chart relationship missing: {rels}"
    );
    assert!(
        chart_xml.contains("<c:doughnutChart>")
            && chart_xml.contains(r#"<c:explosion val="25"/>"#)
            && chart_xml.contains(r#"<c:holeSize val="50"/>"#)
            && chart_xml.contains("<a:t>Revenue ring breakout</a:t>")
            && chart_xml.contains("<c:v>Share</c:v>")
            && chart_xml.contains("<c:v>Cloud</c:v>")
            && chart_xml.contains("<c:v>65</c:v>"),
        "exploded doughnut chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>65</v></c>"#)
            && sheet_xml.contains(r#"<c r="B4"><v>10</v></c>"#),
        "exploded doughnut chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("exploded doughnut chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_surface_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::surface()
                .title("Risk surface")
                .categories(["Low", "Medium", "High"])
                .series("Exposure", [1.0, 3.5, 6.0])
                .series("Control", [0.5, 2.0, 4.5])
                .size_px(460, 300)
                .alt("Risk surface chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Surface);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded surface chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Risk surface chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "surface chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "surface chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "surface chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:surfaceChart>")
            && chart_xml.contains(r#"<c:wireframe val="0"/>"#)
            && chart_xml.contains(r#"<c:bandFmts/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Risk surface</a:t>")
            && chart_xml.contains("<c:v>Exposure</c:v>")
            && chart_xml.contains("<c:v>High</c:v>")
            && chart_xml.contains("<c:v>6</c:v>"),
        "surface chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>1</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>4.5</v></c>"#),
        "surface chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("surface chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_3d_surface_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::surface_3d()
                .title("Terrain surface")
                .categories(["North", "Center", "South"])
                .series("Elevation", [8.0, 4.5, 6.0])
                .series("Slope", [2.0, 3.5, 5.0])
                .size_px(460, 300)
                .alt("Terrain 3-D surface chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Surface3D);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded 3-D surface chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Terrain 3-D surface chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "3-D surface chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "3-D surface chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "3-D surface chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:surface3DChart>")
            && chart_xml.contains(r#"<c:wireframe val="0"/>"#)
            && chart_xml.contains(r#"<c:bandFmts/>"#)
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Terrain surface</a:t>")
            && chart_xml.contains("<c:v>Elevation</c:v>")
            && chart_xml.contains("<c:v>South</c:v>")
            && chart_xml.contains("<c:v>6</c:v>"),
        "3-D surface chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>8</v></c>"#)
            && sheet_xml.contains(r#"<c r="C4"><v>5</v></c>"#),
        "3-D surface chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("3-D surface chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_wireframe_surface_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::surface()
                .wireframe()
                .title("Risk wireframe")
                .categories(["Low", "Medium", "High"])
                .series("Exposure", [1.0, 3.5, 6.0])
                .series("Control", [0.5, 2.0, 4.5])
                .size_px(460, 300)
                .alt("Risk wireframe surface chart"),
        )
        .chart(
            ChartBuilder::surface_3d()
                .wireframe()
                .title("Terrain wireframe")
                .categories(["North", "Center", "South"])
                .series("Elevation", [8.0, 4.5, 6.0])
                .series("Slope", [2.0, 3.5, 5.0])
                .size_px(460, 300)
                .alt("Terrain wireframe 3-D surface chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::Surface);
    assert!(first.wireframe);
    assert_eq!(second.kind, ChartKind::Surface3D);
    assert!(second.wireframe);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let surface_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let surface_3d_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let surface_rels =
        String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let surface_3d_rels =
        String::from_utf8(parts["word/charts/_rels/chart2.xml.rels"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Risk wireframe surface chart"/>"#)
            && document_xml.contains(
                r#"<wp:docPr id="2" name="Chart2" descr="Terrain wireframe 3-D surface chart"/>"#
            ),
        "wireframe surface chart drawings missing: {document_xml}"
    );
    assert!(
        rels.contains(r#"Target="charts/chart1.xml""#)
            && rels.contains(r#"Target="charts/chart2.xml""#),
        "wireframe surface chart relationships missing: {rels}"
    );
    assert!(
        surface_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#)
            && surface_3d_rels
                .contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet2.xlsx""#),
        "wireframe surface chart workbook relationships missing: {surface_rels} {surface_3d_rels}"
    );
    assert!(
        surface_xml.contains("<c:surfaceChart>")
            && surface_xml.contains(r#"<c:wireframe val="1"/>"#)
            && surface_xml.contains("<a:t>Risk wireframe</a:t>")
            && surface_xml.contains("<c:v>Control</c:v>")
            && surface_xml.contains("<c:v>4.5</c:v>"),
        "wireframe surface chart payload missing: {surface_xml}"
    );
    assert!(
        surface_3d_xml.contains("<c:surface3DChart>")
            && surface_3d_xml.contains(r#"<c:wireframe val="1"/>"#)
            && surface_3d_xml.contains("<a:t>Terrain wireframe</a:t>")
            && surface_3d_xml.contains("<c:v>Slope</c:v>")
            && surface_3d_xml.contains("<c:v>5</c:v>"),
        "wireframe 3-D surface chart payload missing: {surface_3d_xml}"
    );

    let reopened = Document::open(&bytes).expect("wireframe surface charts .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_stock_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stock()
                .title("Share price")
                .categories(["Mon", "Tue", "Wed"])
                .series("Open", [10.0, 11.0, 12.5])
                .series("High", [12.0, 13.5, 14.0])
                .series("Low", [9.5, 10.5, 11.0])
                .series("Close", [11.0, 12.8, 13.2])
                .size_px(460, 300)
                .alt("Share price stock chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Stock);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded stock chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Share price stock chart"/>"#)
            && document_xml.contains(r#"<c:chart r:id="rId1"/>"#),
        "stock chart drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("relationships/chart") && rels.contains(r#"Target="charts/chart1.xml""#),
        "stock chart relationship missing: {rels}"
    );
    assert!(
        chart_rels.contains("relationships/package")
            && chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "stock chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:stockChart>")
            && chart_xml.contains("<c:hiLowLines/>")
            && chart_xml.contains("<c:upDownBars>")
            && chart_xml.contains(
                r#"<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>"#
            )
            && chart_xml.contains("<a:t>Share price</a:t>")
            && chart_xml.contains("<c:v>Open</c:v>")
            && chart_xml.contains("<c:v>Close</c:v>")
            && chart_xml.contains("<c:v>13.2</c:v>"),
        "stock chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>10</v></c>"#)
            && sheet_xml.contains(r#"<c r="E4"><v>13.2</v></c>"#),
        "stock chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("stock chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_high_low_close_stock_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stock_high_low_close()
                .title("Share range")
                .categories(["Mon", "Tue", "Wed"])
                .series("High", [12.0, 13.5, 14.0])
                .series("Low", [9.5, 10.5, 11.0])
                .series("Close", [11.0, 12.8, 13.2])
                .size_px(460, 300)
                .alt("Share range high-low-close stock chart"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::StockHighLowClose);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart_rels = String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let workbook_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded high-low-close stock chart workbook");
    let workbook_parts = unzip_parts(workbook_bytes);
    let sheet_xml = String::from_utf8(workbook_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(
            r#"<wp:docPr id="1" name="Chart1" descr="Share range high-low-close stock chart"/>"#
        ),
        "high-low-close stock chart drawing missing: {document_xml}"
    );
    assert!(
        chart_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#),
        "high-low-close stock chart workbook relationship missing: {chart_rels}"
    );
    assert!(
        chart_xml.contains("<c:stockChart>")
            && chart_xml.contains("<c:hiLowLines/>")
            && !chart_xml.contains("<c:upDownBars>")
            && chart_xml.contains("<a:t>Share range</a:t>")
            && chart_xml.contains("<c:v>High</c:v>")
            && chart_xml.contains("<c:v>Low</c:v>")
            && chart_xml.contains("<c:v>Close</c:v>")
            && chart_xml.contains("<c:v>13.2</c:v>"),
        "high-low-close stock chart payload missing: {chart_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B2"><v>12</v></c>"#)
            && sheet_xml.contains(r#"<c r="D4"><v>13.2</v></c>"#),
        "high-low-close stock chart workbook values missing: {sheet_xml}"
    );

    let reopened = Document::open(&bytes).expect("high-low-close stock chart .docx reopens");
    assert_eq!(reopened.report().features.charts, 1);
}

#[test]
fn doc_builder_adds_pie_of_pie_and_bar_of_pie_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::pie_of_pie()
                .title("Pipeline mix")
                .categories(["Core", "Expansion", "Services", "Other"])
                .series("Share", [55.0, 24.0, 14.0, 7.0])
                .size_px(420, 280)
                .alt("Pipeline pie-of-pie chart"),
        )
        .chart(
            ChartBuilder::bar_of_pie()
                .title("Support split")
                .categories(["Priority", "Standard", "Deferred", "Closed"])
                .series("Tickets", [12.0, 32.0, 7.0, 18.0])
                .size_px(420, 280)
                .alt("Support bar-of-pie chart"),
        )
        .build();

    let Block::Chart(first) = &model.blocks[0] else {
        panic!("expected first chart block");
    };
    let Block::Chart(second) = &model.blocks[1] else {
        panic!("expected second chart block");
    };
    assert_eq!(first.kind, ChartKind::PieOfPie);
    assert_eq!(second.kind, ChartKind::BarOfPie);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let chart1_xml = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let chart2_xml = String::from_utf8(parts["word/charts/chart2.xml"].clone()).unwrap();
    let chart1_rels =
        String::from_utf8(parts["word/charts/_rels/chart1.xml.rels"].clone()).unwrap();
    let chart2_rels =
        String::from_utf8(parts["word/charts/_rels/chart2.xml.rels"].clone()).unwrap();
    let workbook1_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx")
        .expect("embedded pie-of-pie chart workbook");
    let workbook2_bytes = parts
        .get("word/embeddings/Microsoft_Excel_Worksheet2.xlsx")
        .expect("embedded bar-of-pie chart workbook");
    let workbook1_parts = unzip_parts(workbook1_bytes);
    let workbook2_parts = unzip_parts(workbook2_bytes);
    let sheet1_xml =
        String::from_utf8(workbook1_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
    let sheet2_xml =
        String::from_utf8(workbook2_parts["xl/worksheets/sheet1.xml"].clone()).unwrap();

    assert!(
        document_xml
            .contains(r#"<wp:docPr id="1" name="Chart1" descr="Pipeline pie-of-pie chart"/>"#)
            && document_xml
                .contains(r#"<wp:docPr id="2" name="Chart2" descr="Support bar-of-pie chart"/>"#),
        "of-pie chart drawings missing: {document_xml}"
    );
    assert!(
        chart1_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet1.xlsx""#)
            && chart2_rels.contains(r#"Target="../embeddings/Microsoft_Excel_Worksheet2.xlsx""#),
        "of-pie chart workbook relationships missing: {chart1_rels} / {chart2_rels}"
    );
    assert!(
        chart1_xml.contains("<c:ofPieChart>")
            && chart1_xml.contains(r#"<c:ofPieType val="pie"/>"#)
            && chart1_xml.contains(r#"<c:secondPieSize val="75"/>"#)
            && chart1_xml.contains("<c:serLines/>")
            && chart1_xml.contains("<a:t>Pipeline mix</a:t>")
            && chart1_xml.contains("<c:v>Share</c:v>")
            && chart1_xml.contains("<c:v>Other</c:v>")
            && chart1_xml.contains("<c:v>7</c:v>"),
        "pie-of-pie chart payload missing: {chart1_xml}"
    );
    assert!(
        chart2_xml.contains("<c:ofPieChart>")
            && chart2_xml.contains(r#"<c:ofPieType val="bar"/>"#)
            && chart2_xml.contains(r#"<c:secondPieSize val="75"/>"#)
            && chart2_xml.contains("<c:serLines/>")
            && chart2_xml.contains("<a:t>Support split</a:t>")
            && chart2_xml.contains("<c:v>Tickets</c:v>")
            && chart2_xml.contains("<c:v>Closed</c:v>")
            && chart2_xml.contains("<c:v>18</c:v>"),
        "bar-of-pie chart payload missing: {chart2_xml}"
    );
    assert!(
        sheet1_xml.contains(r#"<c r="B5"><v>7</v></c>"#)
            && sheet2_xml.contains(r#"<c r="B5"><v>18</v></c>"#),
        "of-pie workbook values missing: {sheet1_xml} / {sheet2_xml}"
    );

    let reopened = Document::open(&bytes).expect("of-pie charts .docx reopens");
    assert_eq!(reopened.report().features.charts, 2);
}

#[test]
fn doc_builder_adds_waterfall_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::waterfall()
                .title("Cash bridge")
                .categories(["Start", "Sales", "Costs", "End"])
                .series("Cash", [120.0, 45.0, -30.0, 135.0])
                .size_px(420, 280)
                .alt("Cash bridge waterfall"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Waterfall);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chartEx1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Cash bridge waterfall"/>"#)
            && document_xml.contains(r#"<cx:chart r:id="rId1"/>"#),
        "waterfall chartEx drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
            && rels.contains(r#"Target="charts/chartEx1.xml""#),
        "waterfall chartEx relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.chartex+xml""#),
        "chartEx content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains(r#"<cx:chartSpace"#)
            && chart_xml.contains(r#"layoutId="waterfall""#)
            && chart_xml.contains("<cx:chartData>")
            && chart_xml.contains(r#"<cx:strDim type="cat">"#)
            && chart_xml.contains(r#"<cx:numDim type="val">"#)
            && chart_xml.contains("<cx:v>Cash</cx:v>")
            && chart_xml.contains("<cx:v>Start</cx:v>")
            && chart_xml.contains("<cx:v>-30</cx:v>"),
        "waterfall chartEx payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("waterfall chartEx .docx reopens");
    let report = reopened.report();
    assert_eq!(report.features.charts, 1);
    assert!(
        !report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored chartEx should not produce a preserved-chart warning: {:?}",
        report.warnings
    );
}

#[test]
fn doc_builder_adds_treemap_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::treemap()
                .title("Portfolio allocation")
                .categories(["Cloud", "Services", "Support"])
                .series("Spend", [55.0, 30.0, 15.0])
                .size_px(420, 280)
                .alt("Portfolio treemap"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Treemap);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let document_xml = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chartEx1.xml"].clone()).unwrap();

    assert!(
        document_xml.contains(r#"<wp:docPr id="1" name="Chart1" descr="Portfolio treemap"/>"#)
            && document_xml.contains(r#"<cx:chart r:id="rId1"/>"#),
        "treemap chartEx drawing missing: {document_xml}"
    );
    assert!(
        rels.contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
            && rels.contains(r#"Target="charts/chartEx1.xml""#),
        "treemap chartEx relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.chartex+xml""#),
        "chartEx content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains(r#"<cx:chartSpace"#)
            && chart_xml.contains(r#"layoutId="treemap""#)
            && chart_xml.contains("<cx:chartData>")
            && chart_xml.contains(r#"<cx:strDim type="cat">"#)
            && chart_xml.contains(r#"<cx:numDim type="val">"#)
            && chart_xml.contains("<cx:v>Spend</cx:v>")
            && chart_xml.contains("<cx:v>Cloud</cx:v>")
            && chart_xml.contains("<cx:v>55</cx:v>"),
        "treemap chartEx payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("treemap chartEx .docx reopens");
    let report = reopened.report();
    assert_eq!(report.features.charts, 1);
    assert!(
        !report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored treemap chartEx should not produce a preserved-chart warning: {:?}",
        report.warnings
    );
}

#[test]
fn doc_builder_adds_sunburst_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::sunburst()
                .title("Portfolio rings")
                .categories(["Cloud", "Services", "Support"])
                .series("Spend", [55.0, 30.0, 15.0])
                .size_px(420, 280)
                .alt("Portfolio sunburst"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Sunburst);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chartEx1.xml"].clone()).unwrap();

    assert!(
        rels.contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
            && rels.contains(r#"Target="charts/chartEx1.xml""#),
        "sunburst chartEx relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.chartex+xml""#),
        "chartEx content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains(r#"<cx:chartSpace"#)
            && chart_xml.contains(r#"layoutId="sunburst""#)
            && chart_xml.contains("<cx:chartData>")
            && chart_xml.contains(r#"<cx:strDim type="cat">"#)
            && chart_xml.contains(r#"<cx:numDim type="val">"#)
            && chart_xml.contains("<cx:v>Spend</cx:v>")
            && chart_xml.contains("<cx:v>Support</cx:v>")
            && chart_xml.contains("<cx:v>15</cx:v>"),
        "sunburst chartEx payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("sunburst chartEx .docx reopens");
    let report = reopened.report();
    assert_eq!(report.features.charts, 1);
    assert!(
        !report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored sunburst chartEx should not produce a preserved-chart warning: {:?}",
        report.warnings
    );
}

#[test]
fn doc_builder_adds_histogram_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::histogram()
                .title("Latency histogram")
                .categories(["0-50", "50-100", "100-200"])
                .series("Requests", [12.0, 32.0, 9.0])
                .size_px(420, 280)
                .alt("Latency histogram"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Histogram);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chartEx1.xml"].clone()).unwrap();

    assert!(
        rels.contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
            && rels.contains(r#"Target="charts/chartEx1.xml""#),
        "histogram chartEx relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.chartex+xml""#),
        "chartEx content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains(r#"<cx:chartSpace"#)
            && chart_xml.contains(r#"layoutId="histogram""#)
            && chart_xml.contains("<cx:chartData>")
            && chart_xml.contains(r#"<cx:strDim type="cat">"#)
            && chart_xml.contains(r#"<cx:numDim type="val">"#)
            && chart_xml.contains("<cx:v>Requests</cx:v>")
            && chart_xml.contains("<cx:v>50-100</cx:v>")
            && chart_xml.contains("<cx:v>32</cx:v>"),
        "histogram chartEx payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("histogram chartEx .docx reopens");
    let report = reopened.report();
    assert_eq!(report.features.charts, 1);
    assert!(
        !report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored histogram chartEx should not produce a preserved-chart warning: {:?}",
        report.warnings
    );
}

#[test]
fn doc_builder_adds_box_whisker_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::box_whisker()
                .title("Response spread")
                .categories(["P25", "Median", "P75", "Max"])
                .series("Latency", [80.0, 120.0, 180.0, 260.0])
                .size_px(420, 280)
                .alt("Latency box whisker"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::BoxWhisker);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chartEx1.xml"].clone()).unwrap();

    assert!(
        rels.contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
            && rels.contains(r#"Target="charts/chartEx1.xml""#),
        "boxWhisker chartEx relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.chartex+xml""#),
        "chartEx content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains(r#"<cx:chartSpace"#)
            && chart_xml.contains(r#"layoutId="boxWhisker""#)
            && chart_xml.contains("<cx:chartData>")
            && chart_xml.contains(r#"<cx:strDim type="cat">"#)
            && chart_xml.contains(r#"<cx:numDim type="val">"#)
            && chart_xml.contains("<cx:v>Latency</cx:v>")
            && chart_xml.contains("<cx:v>Median</cx:v>")
            && chart_xml.contains("<cx:v>260</cx:v>"),
        "boxWhisker chartEx payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("boxWhisker chartEx .docx reopens");
    let report = reopened.report();
    assert_eq!(report.features.charts, 1);
    assert!(
        !report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored boxWhisker chartEx should not produce a preserved-chart warning: {:?}",
        report.warnings
    );
}

#[test]
fn doc_builder_adds_funnel_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::funnel()
                .title("Pipeline funnel")
                .categories(["Leads", "Qualified", "Closed"])
                .series("Deals", [120.0, 64.0, 24.0])
                .size_px(420, 280)
                .alt("Pipeline funnel"),
        )
        .build();

    let Block::Chart(chart) = &model.blocks[0] else {
        panic!("expected chart block");
    };
    assert_eq!(chart.kind, ChartKind::Funnel);

    let bytes = rwml::write_docx(&model);
    let parts = unzip_parts(&bytes);
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let chart_xml = String::from_utf8(parts["word/charts/chartEx1.xml"].clone()).unwrap();

    assert!(
        rels.contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
            && rels.contains(r#"Target="charts/chartEx1.xml""#),
        "funnel chartEx relationship missing: {rels}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.chartex+xml""#),
        "chartEx content type missing: {content_types}"
    );
    assert!(
        chart_xml.contains(r#"<cx:chartSpace"#)
            && chart_xml.contains(r#"layoutId="funnel""#)
            && chart_xml.contains("<cx:chartData>")
            && chart_xml.contains(r#"<cx:strDim type="cat">"#)
            && chart_xml.contains(r#"<cx:numDim type="val">"#)
            && chart_xml.contains("<cx:v>Deals</cx:v>")
            && chart_xml.contains("<cx:v>Qualified</cx:v>")
            && chart_xml.contains("<cx:v>64</cx:v>"),
        "funnel chartEx payload missing: {chart_xml}"
    );

    let reopened = Document::open(&bytes).expect("funnel chartEx .docx reopens");
    let report = reopened.report();
    assert_eq!(report.features.charts, 1);
    assert!(
        !report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored funnel chartEx should not produce a preserved-chart warning: {:?}",
        report.warnings
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_bar_chart_without_unsupported_chart_warning() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar()
                .title("Quarterly revenue")
                .categories(["Q1", "Q2", "Q3"])
                .series("Revenue", [42.0, 51.5, 63.0])
                .size_px(480, 320)
                .alt("Revenue chart"),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored chart should be rendered by the model renderer, not reported as preserved-only"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_charts_in_running_surface_bands() {
    let chart = ChartBuilder::bar()
        .title("Quarterly revenue")
        .categories(["Q1", "Q2", "Q3"])
        .series("Revenue", [42.0, 51.5, 63.0])
        .size_px(160, 80)
        .alt("Running revenue chart")
        .build();
    let page = PageSetup {
        width_pt: 240.0,
        height_pt: 200.0,
        margin_pt: 60.0,
        ..PageSetup::default()
    };
    let model = |header, footer, page| DocModel {
        blocks: vec![Block::Paragraph(
            ParagraphBuilder::new()
                .runs([RunBuilder::new("Body").build()])
                .build(),
        )],
        setup: DocSetup {
            page,
            header,
            footer,
            ..DocSetup::default()
        },
        ..DocModel::default()
    };
    let baseline = model(Vec::new(), Vec::new(), page);
    let header = model(vec![Block::Chart(chart.clone())], Vec::new(), page);
    let footer = model(Vec::new(), vec![Block::Chart(chart.clone())], page);
    let both = model(
        vec![Block::Chart(chart.clone())],
        vec![Block::Chart(chart.clone())],
        page,
    );
    let tiny_page = PageSetup {
        margin_top_pt: Some(30.0),
        ..page
    };
    let tiny_baseline = model(Vec::new(), Vec::new(), tiny_page);
    let tiny_header = model(vec![Block::Chart(chart)], Vec::new(), tiny_page);

    let baseline_render = rwml::render_pdf_with_report(&baseline);
    let header_render = rwml::render_pdf_with_report(&header);
    let footer_render = rwml::render_pdf_with_report(&footer);
    let both_render = rwml::render_pdf_with_report(&both);
    let tiny_baseline_render = rwml::render_pdf_with_report(&tiny_baseline);
    let tiny_header_render = rwml::render_pdf_with_report(&tiny_header);
    for rendered in [
        &baseline_render,
        &header_render,
        &footer_render,
        &both_render,
        &tiny_baseline_render,
        &tiny_header_render,
    ] {
        assert_eq!(rendered.report.pages, 1);
        assert_eq!(rendered.report.unsupported.charts, 0);
    }
    assert_ne!(header_render.pdf, baseline_render.pdf);
    assert_ne!(footer_render.pdf, baseline_render.pdf);
    assert_ne!(header_render.pdf, footer_render.pdf);
    assert_ne!(both_render.pdf, header_render.pdf);
    assert_ne!(both_render.pdf, footer_render.pdf);
    assert_ne!(tiny_header_render.pdf, tiny_baseline_render.pdf);
    assert_eq!(header_render.pdf, rwml::render_pdf_with_report(&header).pdf);
    assert_eq!(footer_render.pdf, rwml::render_pdf_with_report(&footer).pdf);
    assert_eq!(both_render.pdf, rwml::render_pdf_with_report(&both).pdf);
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_waterfall_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::waterfall()
                .title("Cash bridge")
                .categories(["Start", "Sales", "Costs", "End"])
                .series("Cash", [120.0, 45.0, -30.0, 135.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored waterfall chartEx should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "waterfall chartEx drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_treemap_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::treemap()
                .title("Portfolio allocation")
                .categories(["Cloud", "Services", "Support"])
                .series("Spend", [55.0, 30.0, 15.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored treemap chartEx should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "treemap chartEx drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_sunburst_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::sunburst()
                .title("Portfolio rings")
                .categories(["Cloud", "Services", "Support"])
                .series("Spend", [55.0, 30.0, 15.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored sunburst chartEx should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "sunburst chartEx drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_histogram_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::histogram()
                .title("Latency histogram")
                .categories(["0-50", "50-100", "100-200"])
                .series("Requests", [12.0, 32.0, 9.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored histogram chartEx should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "histogram chartEx drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_box_whisker_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::box_whisker()
                .title("Response spread")
                .categories(["P25", "Median", "P75", "Max"])
                .series("Latency", [80.0, 120.0, 180.0, 260.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored boxWhisker chartEx should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "boxWhisker chartEx drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_funnel_chart_ex() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::funnel()
                .title("Pipeline funnel")
                .categories(["Leads", "Qualified", "Closed"])
                .series("Deals", [120.0, 64.0, 24.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored funnel chartEx should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "funnel chartEx drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_column_and_line_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::column()
                .title("Quarterly pipeline")
                .categories(["Q1", "Q2"])
                .series("Pipeline", [10.0, 14.5]),
        )
        .chart(
            ChartBuilder::line()
                .title("Retention trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Retention", [0.91, 0.93, 0.95]),
        )
        .build();

    let bar = rwml::render_pdf_with_report(
        &DocBuilder::new()
            .chart(
                ChartBuilder::bar()
                    .categories(["Q1", "Q2"])
                    .series("Pipeline", [10.0, 14.5]),
            )
            .build(),
    );
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored column/line charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > bar.pdf.len() + 100,
        "two rendered chart families should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_markerless_line_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::line_no_markers()
                .title("Retention trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Retention", [0.91, 0.93, 0.95]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored markerless line chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "markerless line chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_smooth_line_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::smooth_line()
                .title("Retention curve")
                .categories(["Jan", "Feb", "Mar"])
                .series("Retention", [0.91, 0.93, 0.95]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored smooth line chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "smooth line chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stacked_line_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_line()
                .title("Retention stack")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .chart(
            ChartBuilder::percent_stacked_line()
                .title("Retention mix")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stacked line charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stacked line chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_area_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::area()
                .title("Adoption trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored area chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "area chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stacked_area_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_area()
                .title("Adoption stack")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .chart(
            ChartBuilder::percent_stacked_area()
                .title("Adoption mix")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stacked area charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stacked area chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_radar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::radar()
                .title("Capability profile")
                .categories(["Speed", "Quality", "Cost", "Reach"])
                .series("Current", [4.0, 3.0, 2.0, 5.0])
                .series("Target", [5.0, 4.0, 4.0, 5.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored radar chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "radar chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_radar_with_markers_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::radar_with_markers()
                .title("Capability markers")
                .categories(["Speed", "Quality", "Cost", "Reach"])
                .series("Current", [4.0, 3.0, 2.0, 5.0])
                .series("Target", [5.0, 4.0, 4.0, 5.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored radar markers chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "radar markers chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_filled_radar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::filled_radar()
                .title("Capability coverage")
                .categories(["Speed", "Quality", "Cost", "Reach"])
                .series("Current", [4.0, 3.0, 2.0, 5.0])
                .series("Target", [5.0, 4.0, 4.0, 5.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored filled radar chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "filled radar chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_scatter_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter()
                .title("Latency distribution")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored scatter chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "scatter chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_marker_only_scatter_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter_markers()
                .title("Latency points")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored marker-only scatter chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "marker-only scatter chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_line_only_scatter_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter_lines()
                .title("Latency line")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored line-only scatter chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "line-only scatter chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_smooth_scatter_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::scatter_smooth()
                .title("Latency smooth markers")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0]),
        )
        .chart(
            ChartBuilder::scatter_smooth_no_markers()
                .title("Latency smooth")
                .categories(["P50", "P90", "P99"])
                .series("Before", [120.0, 240.0, 510.0])
                .series("After", [90.0, 180.0, 360.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored smooth scatter charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "smooth scatter chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_bubble_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bubble()
                .title("Opportunity size")
                .categories(["Segment A", "Segment B", "Segment C"])
                .bubble_series("Pipeline", [12.0, 24.0, 36.0], [5.0, 12.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored bubble chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "bubble chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_3d_bubble_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bubble_3d()
                .title("Opportunity depth")
                .categories(["Segment A", "Segment B", "Segment C"])
                .bubble_series("Pipeline", [12.0, 24.0, 36.0], [5.0, 12.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 3-D bubble chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "3-D bubble chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::pie()
                .title("Revenue mix")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored pie chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "pie chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_exploded_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::exploded_pie()
                .title("Revenue breakout")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored exploded pie chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "exploded pie chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_3d_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::pie_3d()
                .title("Revenue mix")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 3-D pie chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "3-D pie chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stacked_bar_and_column_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_bar()
                .title("Regional backlog")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0]),
        )
        .chart(
            ChartBuilder::stacked_column()
                .title("Quarterly pipeline")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0])
                .series("Committed", [6.0, 8.5, 11.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stacked bar/column charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stacked bar/column chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_percent_stacked_bar_and_column_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::percent_stacked_bar()
                .title("Regional mix")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0]),
        )
        .chart(
            ChartBuilder::percent_stacked_column()
                .title("Quarterly mix")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0])
                .series("Committed", [6.0, 8.5, 11.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored percent stacked charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "percent stacked chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_3d_bar_and_column_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar_3d()
                .title("Regional backlog")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0]),
        )
        .chart(
            ChartBuilder::column_3d()
                .title("Quarterly pipeline")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 3-D bar/column charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "3-D bar/column chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stacked_3d_column_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_column_3d()
                .title("Quarterly stack")
                .categories(["Q1", "Q2", "Q3"])
                .series("Committed", [10.0, 14.5, 18.0])
                .series("Upside", [4.0, 6.5, 7.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stacked 3-D column chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stacked 3-D column chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_percent_stacked_3d_column_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::percent_stacked_column_3d()
                .title("Quarterly share")
                .categories(["Q1", "Q2", "Q3"])
                .series("Committed", [10.0, 14.5, 18.0])
                .series("Upside", [4.0, 6.5, 7.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 100% stacked 3-D column chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "100% stacked 3-D column chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stacked_3d_bar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_bar_3d()
                .title("Regional stack")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stacked 3-D bar chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stacked 3-D bar chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_percent_stacked_3d_bar_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::percent_stacked_bar_3d()
                .title("Regional share")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 100% stacked 3-D bar chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "100% stacked 3-D bar chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_3d_bar_and_column_shape_styling() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::bar_3d()
                .shape(ChartShape::Cylinder)
                .title("Regional backlog cylinders")
                .categories(["North", "South"])
                .series("Open", [18.0, 23.5])
                .series("Closed", [9.0, 12.0]),
        )
        .chart(
            ChartBuilder::column_3d()
                .shape(ChartShape::Pyramid)
                .title("Quarterly pipeline pyramids")
                .categories(["Q1", "Q2", "Q3"])
                .series("Pipeline", [10.0, 14.5, 18.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored shaped 3-D bar/column charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "shaped 3-D bar/column chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_3d_line_and_area_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::line_3d()
                .title("Retention trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Current", [0.91, 0.93, 0.95])
                .series("Target", [0.94, 0.95, 0.97]),
        )
        .chart(
            ChartBuilder::area_3d()
                .title("Adoption trend")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 3-D line/area charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "3-D line/area chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stacked_3d_area_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stacked_area_3d()
                .title("Adoption 3-D stack")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .chart(
            ChartBuilder::percent_stacked_area_3d()
                .title("Adoption 3-D mix")
                .categories(["Jan", "Feb", "Mar"])
                .series("Free", [20.0, 28.0, 33.0])
                .series("Paid", [8.0, 13.5, 21.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stacked 3-D area charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stacked 3-D area chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_exploded_3d_pie_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::exploded_pie_3d()
                .title("Revenue 3-D breakout")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored exploded 3-D pie chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "exploded 3-D pie chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_doughnut_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::doughnut()
                .title("Revenue mix")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored doughnut chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "doughnut chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_exploded_doughnut_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::exploded_doughnut()
                .title("Revenue ring breakout")
                .categories(["Cloud", "Services", "Support"])
                .series("Share", [65.0, 25.0, 10.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored exploded doughnut chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "exploded doughnut chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_surface_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::surface()
                .title("Risk surface")
                .categories(["Low", "Medium", "High"])
                .series("Exposure", [1.0, 3.5, 6.0])
                .series("Control", [0.5, 2.0, 4.5]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored surface chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "surface chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_3d_surface_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::surface_3d()
                .title("Terrain surface")
                .categories(["North", "Center", "South"])
                .series("Elevation", [8.0, 4.5, 6.0])
                .series("Slope", [2.0, 3.5, 5.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored 3-D surface chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "3-D surface chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_wireframe_surface_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::surface()
                .wireframe()
                .title("Risk wireframe")
                .categories(["Low", "Medium", "High"])
                .series("Exposure", [1.0, 3.5, 6.0])
                .series("Control", [0.5, 2.0, 4.5]),
        )
        .chart(
            ChartBuilder::surface_3d()
                .wireframe()
                .title("Terrain wireframe")
                .categories(["North", "Center", "South"])
                .series("Elevation", [8.0, 4.5, 6.0])
                .series("Slope", [2.0, 3.5, 5.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored wireframe surface charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "wireframe surface chart drawings should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_stock_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stock()
                .title("Share price")
                .categories(["Mon", "Tue", "Wed"])
                .series("Open", [10.0, 11.0, 12.5])
                .series("High", [12.0, 13.5, 14.0])
                .series("Low", [9.5, 10.5, 11.0])
                .series("Close", [11.0, 12.8, 13.2]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored stock chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "stock chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_high_low_close_stock_chart() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::stock_high_low_close()
                .title("Share range")
                .categories(["Mon", "Tue", "Wed"])
                .series("High", [12.0, 13.5, 14.0])
                .series("Low", [9.5, 10.5, 11.0])
                .series("Close", [11.0, 12.8, 13.2]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored high-low-close stock chart should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "high-low-close stock chart drawing should add visible PDF content"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_draws_authored_of_pie_charts() {
    let model = DocBuilder::new()
        .chart(
            ChartBuilder::pie_of_pie()
                .title("Pipeline mix")
                .categories(["Core", "Expansion", "Services", "Other"])
                .series("Share", [55.0, 24.0, 14.0, 7.0]),
        )
        .chart(
            ChartBuilder::bar_of_pie()
                .title("Support split")
                .categories(["Priority", "Standard", "Deferred", "Closed"])
                .series("Tickets", [12.0, 32.0, 7.0, 18.0]),
        )
        .build();

    let empty = rwml::render_pdf_with_report(&DocModel::default());
    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 1);
    assert_eq!(rendered.report.unsupported.charts, 0);
    assert!(
        !rendered.report.warnings.iter().any(|warning| matches!(
            warning,
            rwml::RenderWarning::ChartsPreservedButNotModeled { .. }
        )),
        "authored of-pie charts should render without preserved-only warnings"
    );
    assert!(
        rendered.pdf.len() > empty.pdf.len() + 100,
        "of-pie chart drawing should add visible PDF content"
    );
}

#[test]
fn write_docx_round_trips_through_reader() {
    let bytes = rwml::write_docx(&report());
    let blob = String::from_utf8_lossy(&bytes);
    assert!(blob.contains("word/document.xml"), "not an OPC package");
    assert!(blob.contains("word/styles.xml"), "named styles missing");
    let doc = Document::open(&bytes).expect("authored .docx must re-open");
    let text = doc.text();
    assert!(text.contains("분기 운영 리포트"), "title lost: {text:?}");
    assert!(
        text.contains("작업") && text.contains("플랫폼팀"),
        "table text lost"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_entry_points_produce_pdf() {
    let model = report();
    let a = rwml::render_pdf(&model);
    assert!(a.starts_with(b"%PDF") && a.len() > 800);
    // The with-fonts entry point with no extra fonts behaves identically.
    let b = rwml::render_pdf_with_fonts(&model, &[]);
    assert!(b.starts_with(b"%PDF"));
    let try_a = rwml::try_render_pdf(&model).expect("fallible render succeeds");
    assert!(try_a.starts_with(b"%PDF") && try_a.len() > 800);
    let try_b =
        rwml::try_render_pdf_with_fonts(&model, &[]).expect("fallible font render succeeds");
    assert!(try_b.starts_with(b"%PDF"));

    let doc = Document::open(&rwml::write_docx(&model)).expect("authored docx reopens");
    assert!(doc
        .try_to_pdf()
        .expect("document render succeeds")
        .starts_with(b"%PDF"));
    assert!(doc.to_pdf_with_fonts(&[]).starts_with(b"%PDF"));
    assert!(doc
        .try_to_pdf_with_fonts(&[])
        .expect("document font render succeeds")
        .starts_with(b"%PDF"));
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_honors_paragraph_page_break_before() {
    let model = DocBuilder::new()
        .paragraph("First page")
        .rich_paragraph(ParagraphBuilder::text("Second page").page_break_before())
        .build();

    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 2);
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_treats_page_and_filename_fields_as_supported() {
    let model = DocBuilder::new()
        .field("PAGE", "1")
        .field("FILENAME \\p", "report.docx")
        .field("TOC \\o \"1-3\"", "Contents")
        .build();

    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.unsupported.fields, 1);
    assert_eq!(rendered.report.unsupported.field_kinds.len(), 1);
    assert_eq!(
        rendered.report.unsupported.field_kinds[0].kind,
        FieldKind::Toc
    );
    assert!(rendered.report.warnings.iter().any(|warning| matches!(
        warning,
        rwml::RenderWarning::UnsupportedFieldEvaluation { count: 1, field_kinds }
            if field_kinds.len() == 1 && field_kinds[0].kind == FieldKind::Toc
    )));
    let json = rendered.report.to_json();
    assert!(json.contains(r#""pages":1"#), "{json}");
    assert!(json.contains(r#""unsupported":{"#), "{json}");
    assert!(!json.contains(r#""kind":"FILENAME""#), "{json}");
    assert!(
        json.contains(r#""kind":"UnsupportedFieldEvaluation""#),
        "{json}"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_flags_malformed_filename_fields() {
    let model = DocBuilder::new()
        .field("FILENAME \\x", "cached filename")
        .build();

    let rendered = rwml::render_pdf_with_report(&model);

    assert_eq!(rendered.report.unsupported.fields, 1);
    assert_eq!(
        rendered.report.unsupported.field_kinds,
        vec![rwml::FieldKindCount {
            kind: FieldKind::Filename,
            count: 1,
        }]
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_counts_authored_hyperlink_runs() {
    let model = DocBuilder::new()
        .paragraph_runs([RunBuilder::new("rwml")
            .hyperlink("https://example.com/rwml")
            .build()])
        .build();

    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.unsupported.hyperlinks, 1);
    assert_eq!(rendered.report.unsupported.fields, 0);
    assert!(!rendered.report.warnings.iter().any(|warning| {
        matches!(
            warning,
            rwml::RenderWarning::UnsupportedFieldEvaluation { .. }
        )
    }));
    let json = rendered.report.to_json();
    assert!(json.contains(r#""hyperlinks":1"#), "{json}");
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_warns_about_undecodable_raster_images() {
    let model = DocBuilder::new()
        .rich_image(ImageBuilder::new(tiny_bmp(), "image/bmp"))
        .build();

    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert!(rendered.report.warnings.iter().any(|warning| matches!(
        warning,
        rwml::RenderWarning::UndecodableRasterImages { count: 1 }
    )));
    let json = rendered.report.to_json();
    assert!(
        json.contains(r#""kind":"UndecodableRasterImages","count":1"#),
        "{json}"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_warns_about_missing_image_bytes() {
    let model = DocModel {
        blocks: vec![Block::Image(rwml::Image {
            alt: Some("missing source".to_string()),
            width_px: Some(64),
            height_px: Some(48),
            ..rwml::Image::default()
        })],
        ..DocModel::default()
    };

    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert!(rendered
        .report
        .warnings
        .iter()
        .any(|warning| matches!(warning, rwml::RenderWarning::MissingImageBytes { count: 1 })));
    let json = rendered.report.to_json();
    assert!(
        json.contains(r#""kind":"MissingImageBytes","count":1"#),
        "{json}"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_treats_page_ref_as_named_unsupported_field() {
    let model = DocBuilder::new().field("PAGEREF Figure1 \\h", "3").build();

    let rendered = rwml::render_pdf_with_report(&model);

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.unsupported.fields, 1);
    assert_eq!(
        rendered.report.unsupported.field_kinds,
        vec![rwml::FieldKindCount {
            kind: FieldKind::PageRef,
            count: 1,
        }]
    );
    assert!(rendered.report.warnings.iter().any(|warning| matches!(
        warning,
        rwml::RenderWarning::UnsupportedFieldEvaluation { count: 1, field_kinds }
            if field_kinds.len() == 1 && field_kinds[0].kind == FieldKind::PageRef
    )));
    let json = rendered.report.to_json();
    assert!(
        json.contains(r#""field_kinds":[{"kind":"PAGEREF","count":1}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""unsupported_field_kinds":[{"kind":"PAGEREF","count":1}]"#),
        "{json}"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pdf_report_exposes_pages_and_warnings() {
    let model = DocBuilder::new()
        .paragraph("First page")
        .page_break()
        .paragraph("Second page")
        .field("TOC \\o \"1-3\"", "Contents")
        .build();

    let rendered = rwml::render_pdf_with_report(&model);
    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.report.pages, 2);
    assert_eq!(rendered.report.unsupported.fields, 1);
    assert!(rendered.report.warnings.iter().any(|warning| matches!(
        warning,
        rwml::RenderWarning::UnsupportedFieldEvaluation { count: 1, .. }
    )));
    let try_rendered = rwml::try_render_pdf_with_report(&model).expect("report render succeeds");
    assert!(try_rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(try_rendered.report.pages, 2);
    let try_rendered_fonts =
        rwml::try_render_pdf_with_fonts_and_report(&model, &[]).expect("font report succeeds");
    assert!(try_rendered_fonts.pdf.starts_with(b"%PDF"));
    assert_eq!(try_rendered_fonts.report.pages, 2);

    let doc = Document::open(&rwml::write_docx(&model)).expect("authored field docx reopens");
    let rendered_doc = doc.to_pdf_with_report();
    assert!(rendered_doc.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered_doc.report.pages, 2);
    assert_eq!(rendered_doc.report.unsupported.fields, 1);
    let try_rendered_doc = doc
        .try_to_pdf_with_report()
        .expect("document report render succeeds");
    assert!(try_rendered_doc.pdf.starts_with(b"%PDF"));
    assert_eq!(try_rendered_doc.report.pages, 2);
    assert_eq!(try_rendered_doc.report.unsupported.fields, 1);

    let rendered_doc_fonts = doc.to_pdf_with_fonts_and_report(&[]);
    assert!(rendered_doc_fonts.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered_doc_fonts.report.pages, 2);
    assert_eq!(rendered_doc_fonts.report.unsupported.fields, 1);
    let try_rendered_doc_fonts = doc
        .try_to_pdf_with_fonts_and_report(&[])
        .expect("document font report render succeeds");
    assert!(try_rendered_doc_fonts.pdf.starts_with(b"%PDF"));
    assert_eq!(try_rendered_doc_fonts.report.pages, 2);
    assert_eq!(try_rendered_doc_fonts.report.unsupported.fields, 1);
}

/// A paragraph's own outline level survives a write/reopen round trip even
/// when it is not a heading.
#[test]
fn non_heading_outline_level_round_trips() {
    let model = DocModel {
        blocks: vec![Block::Paragraph(rwml::Paragraph {
            props: rwml::ParaProps {
                outline_level: Some(3),
                ..rwml::ParaProps::default()
            },
            runs: vec![rwml::Run {
                text: "outlined body text".to_string(),
                ..rwml::Run::default()
            }],
        })],
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let document_xml = {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let mut xml = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        xml
    };
    assert!(
        document_xml.contains(r#"<w:outlineLvl w:val="3"/>"#),
        "outline level missing from written XML: {document_xml}"
    );

    let reopened = rwml::Document::open(&bytes).expect("reopen");
    let Block::Paragraph(p) = &reopened.model().blocks[0] else {
        panic!("paragraph");
    };
    assert_eq!(p.props.outline_level, Some(3));
    assert_eq!(p.props.heading_level, None);
}
