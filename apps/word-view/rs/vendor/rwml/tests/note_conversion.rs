#![cfg(feature = "docx")]

use std::io::{Read, Write};

use rwml::{
    Block, Chart, ChartKind, ChartSeries, DocModel, Document, FieldEvaluationReason,
    FieldEvaluationReasonCount, FieldKind, FieldRole,
};

fn docx_fixture(parts: &[(&str, &str)]) -> Vec<u8> {
    docx_fixture_with_media(parts, &[])
}

fn docx_fixture_with_media(parts: &[(&str, &str)], media: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        for (name, body) in media {
            zip.start_file(*name, options).unwrap();
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

fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36,
        0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x60,
        0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

fn source_inline_drawing(rel_id: &str, alt: &str, rotation: i64) -> String {
    format!(
        r#"<w:r><w:drawing><wp:inline><wp:extent cx="19050" cy="28575"/><wp:docPr id="1" name="Source image" descr="{alt}"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="{rel_id}"/></pic:blipFill><pic:spPr><a:xfrm rot="{rotation}"/></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

fn source_chart_drawing(rel_id: &str, tag: &str, uri: &str, alt: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:inline><wp:extent cx="3810000" cy="2286000"/><wp:docPr id="1" name="Source chart" descr="{alt}"/><a:graphic><a:graphicData uri="{uri}"><{tag} r:id="{rel_id}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

fn native_chart_xml() -> (String, String) {
    let model = DocModel {
        blocks: vec![
            Block::Chart(Chart {
                kind: ChartKind::Bar,
                title: Some("Core source".to_string()),
                categories: vec!["North".to_string(), "South".to_string()],
                series: vec![ChartSeries {
                    name: "Core values".to_string(),
                    values: vec![12.0, 18.0],
                    bubble_sizes: Vec::new(),
                }],
                width_px: Some(400),
                height_px: Some(240),
                ..Chart::default()
            }),
            Block::Chart(Chart {
                kind: ChartKind::Waterfall,
                title: Some("Extended source".to_string()),
                categories: vec!["Start".to_string(), "Delta".to_string()],
                series: vec![ChartSeries {
                    name: "Extended values".to_string(),
                    values: vec![30.0, -7.0],
                    bubble_sizes: Vec::new(),
                }],
                width_px: Some(400),
                height_px: Some(240),
                ..Chart::default()
            }),
        ],
        ..DocModel::default()
    };
    let parts = unzip_parts(&rwml::write_docx(&model));
    let mut core = String::from_utf8(parts["word/charts/chart1.xml"].clone()).unwrap();
    let external_start = core.find("<c:externalData").unwrap();
    let external_end = external_start
        + core[external_start..].find("</c:externalData>").unwrap()
        + "</c:externalData>".len();
    core.replace_range(external_start..external_end, "");
    let extended = String::from_utf8(parts["word/charts/chartEx2.xml"].clone()).unwrap();
    (core, extended)
}

fn exact_note_paragraphs_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="9"/></w:r><w:r><w:t> BODY C</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:id="7"><w:sdt><w:sdtContent><w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/><w:shd w:val="clear" w:fill="DDEEFF"/><w:tabs><w:tab w:val="center" w:pos="720" w:leader="hyphen"/></w:tabs><w:spacing w:line="260" w:lineRule="exact"/><w:ind w:left="240" w:firstLine="120"/></w:pPr><w:r><w:rPr><w:b/><w:highlight w:val="yellow"/></w:rPr><w:t>FOOT A</w:t></w:r><w:r><w:br w:type="column"/><w:t>FOOT B</w:t><w:tab/><w:t>FOOT TAB</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:pPr><w:keepLines/><w:spacing w:line="320" w:lineRule="atLeast"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:i/><w:vertAlign w:val="superscript"/></w:rPr><w:t>FOOT C</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:endnote><w:endnote w:id="9"><w:customXml><w:p><w:pPr><w:widowControl w:val="off"/><w:shd w:val="clear" w:fill="FFEEDD"/><w:tabs><w:tab w:val="right" w:pos="1200" w:leader="dot"/></w:tabs><w:spacing w:line="280" w:lineRule="atLeast"/></w:pPr><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>END A</w:t></w:r><w:r><w:br w:type="column"/><w:t>END B</w:t></w:r></w:p></w:customXml><w:p><w:pPr><w:keepNext/><w:ind w:left="300" w:hanging="100"/><w:jc w:val="right"/></w:pPr><w:r><w:rPr><w:smallCaps/></w:rPr><w:t>END C</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn mixed_supported_and_table_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="7"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="9"/></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="7"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>TABLE NOTE</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="9"><w:p><w:pPr><w:keepNext/><w:spacing w:line="240" w:lineRule="exact"/></w:pPr><w:r><w:t>END ONE</w:t></w:r><w:r><w:br w:type="column"/><w:t>END BREAK</w:t></w:r></w:p><w:p><w:r><w:t>END TWO</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn note_table_docx(document_xml: &str, footnotes_xml: &str, endnotes_xml: &str) -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        ("word/document.xml", document_xml),
        ("word/footnotes.xml", footnotes_xml),
        ("word/endnotes.xml", endnotes_xml),
    ])
}

fn rich_note_tables_docx() -> Vec<u8> {
    note_table_docx(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="11"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="12"/></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="11">
                <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>FOOT PREFIX A</w:t><w:br w:type="page"/><w:t>FOOT PREFIX B</w:t></w:r></w:p>
                <w:tbl>
                    <w:tblPr><w:tblW w:w="4000" w:type="pct"/><w:tblInd w:w="720" w:type="dxa"/><w:jc w:val="center"/><w:tblBorders><w:top w:val="double" w:sz="12" w:color="112233"/></w:tblBorders><w:tblLayout w:type="fixed"/></w:tblPr>
                    <w:tblGrid><w:gridCol w:w="1200"/><w:gridCol w:w="2400"/></w:tblGrid>
                    <w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>
                        <w:tc><w:tcPr><w:vMerge w:val="restart"/><w:shd w:fill="DDEEFF"/><w:vAlign w:val="center"/><w:tcW w:w="2500" w:type="pct"/><w:tcMar><w:top w:w="120" w:type="dxa"/><w:right w:w="240" w:type="dxa"/></w:tcMar></w:tcPr><w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/><w:spacing w:line="260" w:lineRule="exact"/><w:tabs><w:tab w:val="center" w:pos="720" w:leader="hyphen"/></w:tabs></w:pPr><w:r><w:rPr><w:b/><w:highlight w:val="yellow"/></w:rPr><w:t>FOOT TABLE A</w:t><w:tab/><w:t>FOOT TAB</w:t><w:br w:type="column"/><w:t>FOOT TABLE B</w:t><w:br w:type="page"/><w:t>FOOT TABLE C</w:t></w:r></w:p></w:tc>
                        <w:tc><w:tcPr/><w:p><w:pPr><w:spacing w:line="320" w:lineRule="atLeast"/></w:pPr><w:r><w:rPr><w:i/><w:vertAlign w:val="superscript"/></w:rPr><w:t>FOOT SIDE</w:t></w:r></w:p></w:tc>
                    </w:tr>
                    <w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:r><w:t>DROPPED MERGE TEXT</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>FOOT LAST</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:p><w:r><w:t>FOOT SUFFIX</w:t></w:r></w:p>
            </w:footnote>
        </w:footnotes>"#,
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:id="12">
                <w:p><w:r><w:t>END PREFIX</w:t></w:r></w:p>
                <w:tbl>
                    <w:tblPr><w:tblW w:w="3500" w:type="pct"/><w:bidiVisual/><w:jc w:val="right"/></w:tblPr>
                    <w:tblGrid><w:gridCol w:w="1800"/><w:gridCol w:w="1800"/></w:tblGrid>
                    <w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr><w:gridSpan w:val="2"/><w:shd w:fill="FFEEDD"/><w:vAlign w:val="bottom"/></w:tcPr><w:p><w:pPr><w:keepNext/><w:widowControl w:val="off"/><w:spacing w:line="280" w:lineRule="atLeast"/><w:tabs><w:tab w:val="right" w:pos="1200" w:leader="dot"/></w:tabs></w:pPr><w:r><w:rPr><w:u w:val="single"/><w:smallCaps/></w:rPr><w:t>END TABLE A</w:t><w:br w:type="column"/><w:t>END TABLE B</w:t><w:tab/><w:t>END TAB</w:t><w:br w:type="page"/><w:t>END PAGE</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="3000" w:type="pct"/><w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr/><w:p><w:pPr><w:keepLines/><w:spacing w:line="360" w:lineRule="exact"/><w:tabs><w:tab w:val="left" w:pos="600" w:leader="hyphen"/></w:tabs></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>END NESTED A</w:t><w:br w:type="column"/><w:t>END NESTED B</w:t><w:tab/><w:t>END NESTED TAB</w:t><w:br w:type="page"/><w:t>END NESTED PAGE</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr>
                </w:tbl>
                <w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>END SUFFIX</w:t></w:r></w:p>
            </w:endnote>
        </w:endnotes>"#,
    )
}

fn nested_and_relationship_table_note_docx() -> Vec<u8> {
    note_table_docx(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="21"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="22"/></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="21"><w:tbl><w:tblPr><w:tblW w:w="4000" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr/><w:p><w:pPr><w:keepNext/><w:spacing w:line="220" w:lineRule="exact"/></w:pPr><w:r><w:t>OUTER TABLE A</w:t><w:br w:type="page"/><w:t>OUTER TABLE B</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="3000" w:type="pct"/><w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr/><w:p><w:pPr><w:keepLines/><w:widowControl w:val="off"/><w:spacing w:line="300" w:lineRule="atLeast"/><w:tabs><w:tab w:val="center" w:pos="900" w:leader="dot"/></w:tabs></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>NESTED TABLE A</w:t><w:br w:type="column"/><w:t>NESTED TABLE B</w:t><w:tab/><w:t>NESTED TAB</w:t><w:br w:type="page"/><w:t>NESTED PAGE</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:footnote></w:footnotes>"#,
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="22"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:fldSimple w:instr=" CUSTOM unsupported-table "><w:r><w:t>UNSUPPORTED FIELD TABLE</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:endnote></w:endnotes>"#,
    )
}

fn relationship_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="31"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="41"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="32"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="42"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:endnoteReference w:id="43"/></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:footnote w:id="31"><w:p><w:hyperlink r:id="rFootOne"><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>FOOT LINK ONE</w:t></w:r></w:hyperlink></w:p></w:footnote><w:footnote w:id="32"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:hyperlink r:id="rFootTwo"><w:r><w:rPr><w:b/></w:rPr><w:t>FOOT NESTED LINK</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:footnote></w:footnotes>"#,
        ),
        (
            "word/_rels/footnotes.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rFootOne" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/foot-one?x=1&amp;y=2" TargetMode="External"/><Relationship Id="rFootTwo" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/foot-two" TargetMode="External"/></Relationships>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:endnote w:id="41"><w:p><w:hyperlink r:id="rEndOne"><w:r><w:rPr><w:i/></w:rPr><w:t>END LINK ONE</w:t></w:r></w:hyperlink></w:p></w:endnote><w:endnote w:id="42"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:hyperlink r:id="rEndTwo"><w:r><w:rPr><w:smallCaps/></w:rPr><w:t>END NESTED LINK</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="43"><w:p><w:hyperlink r:id="rRejected"><w:r><w:t>REJECTED LINK</w:t></w:r></w:hyperlink><w:fldSimple w:instr=" CUSTOM rejected-link "><w:r><w:t> UNSUPPORTED FIELD</w:t></w:r></w:fldSimple></w:p></w:endnote></w:endnotes>"#,
        ),
        (
            "word/_rels/endnotes.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rEndOne" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/end-one" TargetMode="External"/><Relationship Id="rEndTwo" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/end-two" TargetMode="External"/><Relationship Id="rRejected" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/rejected" TargetMode="External"/></Relationships>"#,
        ),
    ])
}

fn internal_anchor_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="5" w:name="BodyTarget"/><w:r><w:t>BODY TARGET</w:t></w:r><w:bookmarkEnd w:id="5"/><w:r><w:t> BODY A</w:t></w:r><w:r><w:footnoteReference w:id="51"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="61"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="52"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="62"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="51"><w:p><w:hyperlink w:anchor=" BodyTarget "><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>FOOT BODY LINK</w:t></w:r></w:hyperlink><w:r><w:t> | </w:t></w:r><w:bookmarkStart w:id="21" w:name="FootTarget"/><w:r><w:rPr><w:b/></w:rPr><w:t>FOOT TARGET</w:t></w:r><w:bookmarkEnd w:id="21"/><w:r><w:t> | </w:t></w:r><w:hyperlink w:anchor="FootTarget"><w:r><w:t>FOOT LOCAL LINK</w:t></w:r></w:hyperlink></w:p></w:footnote><w:footnote w:id="52"><w:p><w:hyperlink w:anchor=" Bad Target "><w:r><w:t>REJECTED ANCHOR</w:t></w:r></w:hyperlink><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="61"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:bookmarkStart w:id="31" w:name="EndTarget"/><w:r><w:rPr><w:i/></w:rPr><w:t>END TARGET</w:t></w:r><w:bookmarkEnd w:id="31"/><w:r><w:t> | </w:t></w:r><w:hyperlink w:anchor="BodyTarget"><w:r><w:t>END BODY LINK</w:t></w:r></w:hyperlink><w:r><w:t> | </w:t></w:r><w:hyperlink w:anchor="EndTarget"><w:r><w:t>END LOCAL LINK</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="62"><w:p><w:bookmarkStart w:id="41" w:name="Bad Target"/><w:r><w:t>REJECTED BOOKMARK</w:t></w:r><w:bookmarkEnd w:id="41"/><w:hyperlink w:anchor="BodyTarget"><w:r><w:t> VALID LINK</w:t></w:r></w:hyperlink></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn content_control_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="71"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="81"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="72"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="82"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="71"><w:p><w:sdt><w:sdtPr><w:alias w:val=" Foot alias "/><w:tag w:val=" foot-tag "/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:b/></w:rPr><w:t>FOOT CONTROL</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t> TAIL</w:t></w:r></w:p></w:footnote><w:footnote w:id="72"><w:p><w:sdt><w:sdtPr><w:dataBinding w:xpath=" /root/half "/></w:sdtPr><w:sdtContent><w:r><w:t>REJECTED HALF BINDING</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="81"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:sdt><w:sdtPr><w:alias w:val=" End binding "/><w:dataBinding w:xpath=" /root/client " w:storeItemID=" {11111111-2222-3333-4444-555555555555} "/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:i/></w:rPr><w:t>END BOUND CONTROL</w:t></w:r></w:sdtContent></w:sdt></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="82"><w:p><w:sdt><w:sdtPr><w:tag w:val=" rejected-tag "/><w:dataBinding w:storeItemID=" {AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE} "/></w:sdtPr><w:sdtContent><w:r><w:t>REJECTED TAGGED HALF</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn cached_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="91"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="101"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="92"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="102"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="91"><w:p><w:fldSimple w:instr=" PRIVATE legacy-data "><w:r><w:rPr><w:b/></w:rPr><w:t>PRIVATE CACHE</w:t></w:r></w:fldSimple><w:r><w:t> | </w:t></w:r><w:fldSimple w:instr=" INCLUDETEXT &quot;appendix.docx&quot; "><w:r><w:t>INCLUDE CACHE</w:t></w:r></w:fldSimple></w:p></w:footnote><w:footnote w:id="92"><w:p><w:fldSimple w:instr=" ADDIN &quot;bad "><w:r><w:t>REJECTED MALFORMED FIELD</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="101"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:fldSimple w:instr=" ADDRESSBLOCK "><w:r><w:t>ADDRESS CACHE</w:t></w:r></w:fldSimple><w:r><w:t> | </w:t></w:r><w:fldSimple w:instr=" BARCODE &quot;9781234567890&quot; "><w:r><w:rPr><w:i/></w:rPr><w:t>BARCODE CACHE</w:t></w:r></w:fldSimple><w:r><w:t> | </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PRIVATE complex-data </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>COMPLEX CACHE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="102"><w:p><w:fldSimple w:instr=" REF MissingTarget "><w:r><w:t>REJECTED CONTEXT FIELD</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn merge_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="111"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="121"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="112"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="122"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="111"><w:p><w:fldSimple w:instr=" MERGEFIELD &quot;Client Name&quot; \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>CLIENT CACHE</w:t></w:r></w:fldSimple><w:r><w:t> FOOT TAIL</w:t></w:r></w:p></w:footnote><w:footnote w:id="112"><w:p><w:fldSimple w:instr=" MERGEFIELD \* MERGEFORMAT "><w:r><w:t>REJECTED MALFORMED MERGE</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="121"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> MERGEFIELD ProjectName \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>PROJECT CACHE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END TAIL</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="122"><w:p><w:fldSimple w:instr=" CUSTOM payload "><w:r><w:t>REJECTED UNKNOWN FIELD</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn filename_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="131"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="141"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="132"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="142"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="131"><w:p><w:fldSimple w:instr=" FILENAME \p \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>/work/REPORT.DOCX</w:t></w:r></w:fldSimple><w:r><w:t> FOOT TAIL</w:t></w:r></w:p></w:footnote><w:footnote w:id="132"><w:p><w:fldSimple w:instr=" FILENAME \x "><w:r><w:t>REJECTED MALFORMED FILENAME</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="141"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> FILENAME \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>report.docx</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END TAIL</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="142"><w:p><w:fldSimple w:instr=" CUSTOM filename-payload "><w:r><w:t>REJECTED UNKNOWN FILENAME SIBLING</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn reference_index_marker_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="151"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="161"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="152"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="162"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="151"><w:p><w:r><w:t xml:space="preserve">FOOT BEFORE </w:t></w:r><w:fldSimple w:instr=" XE &quot;Mercury&quot; \t &quot;See planets&quot; \* FirstCap "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE XE CACHE</w:t></w:r></w:fldSimple><w:r><w:t>FOOT AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="152"><w:p><w:fldSimple w:instr=" TA \l &quot;Broken Case&quot; \c "><w:r><w:t>REJECTED MALFORMED TA</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="161"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TA \l &quot;Case v. Example&quot; \c 1 \* CHARFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE TA CACHE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> BETWEEN </w:t></w:r><w:fldSimple w:instr=" RD &quot;appendix.docx&quot; \* MERGEFORMAT "><w:r><w:rPr><w:smallCaps/></w:rPr><w:t>STALE RD CACHE</w:t></w:r></w:fldSimple><w:r><w:t> END AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="162"><w:p><w:fldSimple w:instr=" INDEX \e &quot; - &quot; "><w:r><w:t>REJECTED GENERATED INDEX</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn toc_entry_marker_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="171"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="181"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="172"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="182"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="171"><w:p><w:r><w:t xml:space="preserve">FOOT TC BEFORE </w:t></w:r><w:fldSimple w:instr=" TC &quot;manual foot entry&quot; \f m \l 2 \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT TC CACHE</w:t></w:r></w:fldSimple><w:r><w:t>FOOT TC AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="172"><w:p><w:fldSimple w:instr=" TC \f m \l 2 "><w:r><w:t>REJECTED MALFORMED TC</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="181"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END TC BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TC Manual end entry \f e \l 3 \n </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END TC CACHE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>END TC AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="182"><w:p><w:r><w:t xml:space="preserve">GENERATED TOC BEFORE </w:t></w:r><w:fldSimple w:instr=" TOC \f e "><w:r><w:t>STALE GENERATED TOC</w:t></w:r></w:fldSimple><w:r><w:t> GENERATED TOC AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn symbol_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="191"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="201"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="192"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="202"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="191"><w:p><w:r><w:t xml:space="preserve">FOOT SYMBOL BEFORE </w:t></w:r><w:fldSimple w:instr=" SYMBOL 183 \f Symbol \s 12 "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT SYMBOL</w:t></w:r></w:fldSimple><w:r><w:t> FOOT SYMBOL AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="192"><w:p><w:fldSimple w:instr=" SYMBOL 65 \f &quot;Wingdings "><w:r><w:t>REJECTED MALFORMED SYMBOL</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="201"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END SYMBOL BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> SYMBOL 0x03BB \u \f &quot;Times New Roman&quot; \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END SYMBOL</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END SYMBOL AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="202"><w:p><w:r><w:t xml:space="preserve">ACTION BEFORE </w:t></w:r><w:fldSimple w:instr=" MACROBUTTON RunReport &quot;Fresh Action&quot; "><w:r><w:t>STALE ACTION</w:t></w:r></w:fldSimple><w:r><w:t> ACTION AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn quote_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="211"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="221"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="212"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="222"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="213"/></w:r><w:r><w:t> BODY F</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="211"><w:p><w:r><w:t xml:space="preserve">FOOT QUOTE BEFORE </w:t></w:r><w:fldSimple w:instr=" QUOTE &quot;fresh foot words&quot; \* Caps "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT QUOTE</w:t></w:r></w:fldSimple><w:r><w:t> FOOT QUOTE AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="212"><w:p><w:fldSimple w:instr=" QUOTE &quot;broken foot "><w:r><w:t>REJECTED MALFORMED QUOTE</w:t></w:r></w:fldSimple><w:r><w:t> FALLBACK</w:t></w:r></w:p></w:footnote><w:footnote w:id="213"><w:p><w:r><w:t xml:space="preserve">SPLIT QUOTE BEFORE </w:t></w:r><w:fldSimple w:instr=" QUOTE &quot;split quote&quot; \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT QUOTE AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="221"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END QUOTE BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> QUOTE &quot;fresh end words&quot; \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END QUOTE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END QUOTE AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="222"><w:p><w:r><w:t xml:space="preserve">FILLIN BEFORE </w:t></w:r><w:fldSimple w:instr=" FILLIN &quot;Client?&quot; "><w:r><w:t>CACHED FILLIN</w:t></w:r></w:fldSimple><w:r><w:t> FILLIN AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn if_compare_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="231"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="241"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="232"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="242"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="233"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="243"/></w:r><w:r><w:t> BODY G</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="231"><w:p><w:r><w:t xml:space="preserve">FOOT IF BEFORE </w:t></w:r><w:fldSimple w:instr=" IF 2 &gt;= 1 &quot;fresh foot if&quot; &quot;bad foot if&quot; \* Caps "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT IF</w:t></w:r></w:fldSimple><w:r><w:t> FOOT IF AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="232"><w:p><w:fldSimple w:instr=" SET GateState &quot;Ready&quot; "><w:r><w:t>STALE SET</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve">STATE IF BEFORE </w:t></w:r><w:fldSimple w:instr=" IF GateState = &quot;Ready&quot; &quot;source yes&quot; &quot;source no&quot; "><w:r><w:t>STALE STATE IF</w:t></w:r></w:fldSimple><w:r><w:t> STATE IF AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="233"><w:p><w:r><w:t xml:space="preserve">SPLIT IF BEFORE </w:t></w:r><w:fldSimple w:instr=" IF 1=1 &quot;split if&quot; &quot;bad split&quot; \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT IF A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT IF B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT IF AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="241"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END COMPARE BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> COMPARE &quot;Alpha-42&quot; = &quot;Alpha-*&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END COMPARE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END COMPARE AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="242"><w:p><w:r><w:t xml:space="preserve">NONFINITE COMPARE BEFORE </w:t></w:r><w:fldSimple w:instr=" COMPARE 1e309 &gt; 0 "><w:r><w:t>CACHED NONFINITE COMPARE</w:t></w:r></w:fldSimple><w:r><w:t> NONFINITE COMPARE AFTER</w:t></w:r></w:p></w:endnote><w:endnote w:id="243"><w:p><w:r><w:t xml:space="preserve">FILLIN BEFORE </w:t></w:r><w:fldSimple w:instr=" FILLIN &quot;Client?&quot; "><w:r><w:t>CACHED FILLIN</w:t></w:r></w:fldSimple><w:r><w:t> FILLIN AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn formula_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="251"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="261"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="252"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="262"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="253"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="263"/></w:r><w:r><w:t> BODY G</w:t></w:r><w:r><w:footnoteReference w:id="254"/></w:r><w:r><w:t> BODY H</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="251"><w:p><w:r><w:t xml:space="preserve">FOOT FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = 10 / 4 \# &quot;0.00&quot; "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT FORMULA</w:t></w:r></w:fldSimple><w:r><w:t> FOOT FORMULA AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="252"><w:p><w:fldSimple w:instr=" SET Amount 7 "><w:r><w:t>STALE AMOUNT SET</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve">STATE FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = Amount + 1 "><w:r><w:t>STALE STATE FORMULA</w:t></w:r></w:fldSimple><w:r><w:t> STATE FORMULA AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="253"><w:p><w:r><w:t xml:space="preserve">SPLIT FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = 2 + 3 \* OrdText "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT FORMULA A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT FORMULA B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT FORMULA AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="254"><w:p><w:fldSimple w:instr=" SET Known 5 "><w:r><w:t>STALE KNOWN SET</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve">DEFINED FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = DEFINED(Known) "><w:r><w:t>STALE DEFINED FORMULA</w:t></w:r></w:fldSimple><w:r><w:t> DEFINED FORMULA AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="261"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END FORMULA BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> = ROUND(AVERAGE(2; 4; 7); 1) \# &quot;0.0&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END FORMULA</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END FORMULA AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="262"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1600"/><w:gridCol w:w="1600"/><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">POSITIONAL FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>STALE POSITIONAL FORMULA</w:t></w:r></w:fldSimple><w:r><w:t> POSITIONAL FORMULA AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="263"><w:p><w:r><w:t xml:space="preserve">NONFINITE FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = 1e309 + 1 "><w:r><w:t>CACHED NONFINITE FORMULA</w:t></w:r></w:fldSimple><w:r><w:t> NONFINITE FORMULA AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn table_formula_field_note_docx() -> Vec<u8> {
    note_table_docx(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="351"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="361"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="352"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="362"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="351">
                <w:p><w:r><w:t>FOOT TABLE FORMULA</w:t></w:r></w:p>
                <w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1200"/><w:gridCol w:w="1200"/><w:gridCol w:w="1800"/></w:tblGrid><w:tr>
                    <w:tc><w:tcPr/><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr/><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr/><w:p><w:fldSimple w:instr=" = SUM(LEFT) \# &quot;0.00&quot; "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT TABLE FORMULA</w:t></w:r></w:fldSimple></w:p></w:tc>
                </w:tr></w:tbl>
                <w:p><w:r><w:t xml:space="preserve">FOOT SCALAR FORMULA BEFORE </w:t></w:r><w:fldSimple w:instr=" = 8 / 2 "><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>STALE FOOT SCALAR FORMULA</w:t></w:r></w:fldSimple><w:r><w:t> FOOT SCALAR FORMULA AFTER</w:t></w:r></w:p>
            </w:footnote>
            <w:footnote w:id="352">
                <w:p><w:r><w:t>MULTI TABLE FORMULA</w:t></w:r></w:p>
                <w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1200"/><w:gridCol w:w="1800"/></w:tblGrid>
                    <w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:fldSimple w:instr=" = A1 + 1 "><w:r><w:t>STALE MULTI FORMULA ONE</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:fldSimple w:instr=" = A2 + 1 "><w:r><w:t>STALE MULTI FORMULA TWO</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr>
                </w:tbl>
            </w:footnote>
        </w:footnotes>"#,
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:id="361">
                <w:p><w:r><w:t>END NESTED TABLE FORMULA</w:t></w:r></w:p>
                <w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid><w:tr><w:tc><w:tcPr/>
                    <w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1200"/><w:gridCol w:w="1800"/></w:tblGrid>
                        <w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc></w:tr>
                        <w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> = SUM(A1:B1) \# &quot;0.0&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END TABLE FORMULA</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:tc></w:tr>
                    </w:tbl>
                </w:tc></w:tr></w:tbl>
            </w:endnote>
            <w:endnote w:id="362">
                <w:p><w:r><w:t>SPAN TABLE FORMULA</w:t></w:r></w:p>
                <w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1200"/><w:gridCol w:w="1200"/><w:gridCol w:w="1800"/></w:tblGrid>
                    <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>9</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:tcPr/><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>STALE SPAN FORMULA</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr>
                </w:tbl>
            </w:endnote>
        </w:endnotes>"#,
    )
}

fn explicit_reset_sequence_field_note_docx() -> Vec<u8> {
    note_table_docx(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="371"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="381"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="372"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="382"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="373"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="383"/></w:r><w:r><w:t> BODY G</w:t></w:r><w:r><w:footnoteReference w:id="374"/></w:r><w:r><w:t> BODY H</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="371"><w:p><w:r><w:t xml:space="preserve">FOOT RESET BEFORE </w:t></w:r><w:fldSimple w:instr=" SEQ Invoice \r 21 \* DollarText \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT RESET</w:t></w:r></w:fldSimple><w:r><w:t> FOOT RESET AFTER</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="372"><w:p><w:r><w:t xml:space="preserve">ORDINARY SEQ BEFORE </w:t></w:r><w:fldSimple w:instr=" SEQ Invoice "><w:r><w:t>STALE ORDINARY SEQ</w:t></w:r></w:fldSimple><w:r><w:t> ORDINARY SEQ AFTER</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="373"><w:p><w:r><w:t>HIDDEN RESET BEFORE[</w:t></w:r><w:fldSimple w:instr=" SEQ Hidden \r 4 \h "><w:r><w:t>STALE HIDDEN RESET</w:t></w:r></w:fldSimple><w:r><w:t>]HIDDEN RESET AFTER</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="374"><w:p><w:r><w:t xml:space="preserve">SPLIT RESET BEFORE </w:t></w:r><w:fldSimple w:instr=" SEQ Split \r 9 "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT RESET A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT RESET B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT RESET AFTER</w:t></w:r></w:p></w:footnote>
        </w:footnotes>"#,
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:id="381"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END RESET BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> SEQ Appendix \r31 \* Hex </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END RESET</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END RESET AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote>
            <w:endnote w:id="382"><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t xml:space="preserve">HEADING RESET BEFORE </w:t></w:r><w:fldSimple w:instr=" SEQ Figure \s 1 "><w:r><w:t>STALE HEADING RESET</w:t></w:r></w:fldSimple><w:r><w:t> HEADING RESET AFTER</w:t></w:r></w:p></w:endnote>
            <w:endnote w:id="383"><w:p><w:r><w:t xml:space="preserve">MALFORMED RESET BEFORE </w:t></w:r><w:fldSimple w:instr=" SEQ Broken \r -1 "><w:r><w:t>CACHED MALFORMED RESET</w:t></w:r></w:fldSimple><w:r><w:t> MALFORMED RESET AFTER</w:t></w:r></w:p></w:endnote>
        </w:endnotes>"#,
    )
}

fn explicit_start_listnum_field_note_docx() -> Vec<u8> {
    note_table_docx(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="391"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="401"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="392"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="402"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="393"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="403"/></w:r><w:r><w:t> BODY G</w:t></w:r><w:r><w:footnoteReference w:id="394"/></w:r><w:r><w:t> BODY H</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="391"><w:p><w:r><w:t xml:space="preserve">FOOT LISTNUM BEFORE </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault \s 21 \* DollarText \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT LISTNUM</w:t></w:r></w:fldSimple><w:r><w:t> FOOT LISTNUM AFTER</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="392"><w:p><w:r><w:t xml:space="preserve">ORDINARY LISTNUM BEFORE </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault "><w:r><w:t>STALE ORDINARY LISTNUM</w:t></w:r></w:fldSimple><w:r><w:t> ORDINARY LISTNUM AFTER</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="393"><w:p><w:r><w:t xml:space="preserve">LEVEL TWO BEFORE </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault \l 2 "><w:r><w:t>CACHED LEVEL TWO</w:t></w:r></w:fldSimple><w:r><w:t> LEVEL TWO AFTER</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="394"><w:p><w:r><w:t xml:space="preserve">SPLIT LISTNUM BEFORE </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault \s 9 "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT LISTNUM A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT LISTNUM B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT LISTNUM AFTER</w:t></w:r></w:p></w:footnote>
        </w:footnotes>"#,
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:id="401"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END LISTNUM BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> LISTNUM LegalDefault \l1 \s31 \* Hex </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END LISTNUM</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END LISTNUM AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote>
            <w:endnote w:id="402"><w:p><w:r><w:t xml:space="preserve">AUTONUM BEFORE </w:t></w:r><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>STALE AUTONUM</w:t></w:r></w:fldSimple><w:r><w:t> AUTONUM AFTER</w:t></w:r></w:p></w:endnote>
            <w:endnote w:id="403"><w:p><w:r><w:t xml:space="preserve">NEGATIVE START BEFORE </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault \s -1 "><w:r><w:t>CACHED NEGATIVE START</w:t></w:r></w:fldSimple><w:r><w:t> NEGATIVE START AFTER</w:t></w:r></w:p></w:endnote>
        </w:endnotes>"#,
    )
}

fn same_note_style_ref_field_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rIdNumbering" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/></w:style><w:style w:type="paragraph" w:styleId="Callout"><w:name w:val="Callout"/></w:style><w:style w:type="paragraph" w:styleId="NumberedBody"><w:name w:val="Numbered Body"/></w:style></w:styles>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="42"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="42"><w:abstractNumId w:val="42"/></w:num></w:numbering>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="411"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="421"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="412"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="422"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="413"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="423"/></w:r><w:r><w:t> BODY G</w:t></w:r><w:r><w:footnoteReference w:id="414"/></w:r><w:r><w:t> BODY H</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:footnote w:id="411"><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Foot Heading</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">FOOT STYLE BEFORE </w:t></w:r><w:fldSimple w:instr=" STYLEREF &quot;Heading 1&quot; \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT STYLE</w:t></w:r></w:fldSimple><w:r><w:t> FOOT STYLE AFTER</w:t></w:r></w:p></w:footnote>
                <w:footnote w:id="412"><w:p><w:r><w:t xml:space="preserve">CROSS STYLE BEFORE </w:t></w:r><w:fldSimple w:instr=" STYLEREF Heading1 "><w:r><w:t>STALE CROSS STYLE</w:t></w:r></w:fldSimple><w:r><w:t> CROSS STYLE AFTER</w:t></w:r></w:p></w:footnote>
                <w:footnote w:id="413"><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t xml:space="preserve">Multi </w:t></w:r><w:r><w:t>Source</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">MULTI SOURCE BEFORE </w:t></w:r><w:fldSimple w:instr=" STYLEREF Heading1 "><w:r><w:t>STALE MULTI SOURCE</w:t></w:r></w:fldSimple><w:r><w:t> MULTI SOURCE AFTER</w:t></w:r></w:p></w:footnote>
                <w:footnote w:id="414"><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Split Source</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">SPLIT STYLE BEFORE </w:t></w:r><w:fldSimple w:instr=" STYLEREF Heading1 "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT STYLE A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT STYLE B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT STYLE AFTER</w:t></w:r></w:p></w:footnote>
            </w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:endnote w:id="421"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END STYLE BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> STYLEREF \p Callout \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END STYLE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END STYLE AFTER</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Callout"/></w:pPr><w:r><w:t>End Target</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote>
                <w:endnote w:id="422"><w:p><w:pPr><w:pStyle w:val="NumberedBody"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="42"/></w:numPr></w:pPr><w:r><w:t>Numbered Source</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">NUMBERED STYLE BEFORE </w:t></w:r><w:fldSimple w:instr=" STYLEREF NumberedBody \n "><w:r><w:t>STALE NUMBERED STYLE</w:t></w:r></w:fldSimple><w:r><w:t> NUMBERED STYLE AFTER</w:t></w:r></w:p></w:endnote>
                <w:endnote w:id="423"><w:p><w:r><w:t xml:space="preserve">MALFORMED STYLE BEFORE </w:t></w:r><w:fldSimple w:instr=" STYLEREF &quot;Broken Style "><w:r><w:t>CACHED MALFORMED STYLE</w:t></w:r></w:fldSimple><w:r><w:t> MALFORMED STYLE AFTER</w:t></w:r></w:p></w:endnote>
            </w:endnotes>"#,
        ),
    ])
}

fn fill_in_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="271"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="281"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="272"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="282"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="273"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="283"/></w:r><w:r><w:t> BODY G</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="271"><w:p><w:r><w:t xml:space="preserve">FOOT FILLIN BEFORE </w:t></w:r><w:fldSimple w:instr=" FILLIN &quot;Client?&quot; \d &quot;fresh foot words&quot; \o \* Caps "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT FILLIN</w:t></w:r></w:fldSimple><w:r><w:t> FOOT FILLIN AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="272"><w:p><w:r><w:t xml:space="preserve">NO DEFAULT BEFORE </w:t></w:r><w:fldSimple w:instr=" FILLIN &quot;No default?&quot; "><w:r><w:t>CACHED NO DEFAULT FILLIN</w:t></w:r></w:fldSimple><w:r><w:t> NO DEFAULT AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="273"><w:p><w:r><w:t xml:space="preserve">SPLIT FILLIN BEFORE </w:t></w:r><w:fldSimple w:instr=" FILLIN &quot;Split?&quot; \d &quot;split answer&quot; \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT FILLIN A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT FILLIN B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT FILLIN AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="281"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END FILLIN BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> FILLIN Project display prompt \d Client 42 \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END FILLIN</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END FILLIN AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="282"><w:p><w:r><w:t xml:space="preserve">MALFORMED FILLIN BEFORE </w:t></w:r><w:fldSimple w:instr=" FILLIN &quot;broken prompt "><w:r><w:t>CACHED MALFORMED FILLIN</w:t></w:r></w:fldSimple><w:r><w:t> MALFORMED FILLIN AFTER</w:t></w:r></w:p></w:endnote><w:endnote w:id="283"><w:p><w:r><w:t>ASK BEFORE[</w:t></w:r><w:fldSimple w:instr=" ASK ClientCode &quot;Client code?&quot; \d &quot;ac-42&quot; \o "><w:r><w:t>STALE ASK</w:t></w:r></w:fldSimple><w:r><w:t>]ASK AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn display_field_note_docx() -> Vec<u8> {
    note_table_docx(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="291"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="301"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="292"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="302"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="293"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="303"/></w:r><w:r><w:t> BODY G</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="291"><w:p><w:r><w:t xml:space="preserve">FOOT EQ BEFORE </w:t></w:r><w:fldSimple w:instr=" EQ \f( &quot;Alpha, One&quot; , &quot;Beta Two&quot; ) \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT EQ</w:t></w:r></w:fldSimple><w:r><w:t> FOOT EQ AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="292"><w:p><w:r><w:t xml:space="preserve">MALFORMED EQ BEFORE </w:t></w:r><w:fldSimple w:instr=" EQ \f(1, "><w:r><w:t>CACHED MALFORMED EQ</w:t></w:r></w:fldSimple><w:r><w:t> MALFORMED EQ AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="293"><w:p><w:r><w:t xml:space="preserve">SPLIT EQ BEFORE </w:t></w:r><w:fldSimple w:instr=" EQ \f(1,2) "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT EQ A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT EQ B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT EQ AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="301"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END ADVANCE BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> ADVANCE \r&quot;2&quot; \d4 \* MERGEFORMAT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END ADVANCE</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END ADVANCE AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="302"><w:p><w:r><w:t xml:space="preserve">BAD ADVANCE BEFORE </w:t></w:r><w:fldSimple w:instr=" ADVANCE \z 2 "><w:r><w:t>CACHED BAD ADVANCE</w:t></w:r></w:fldSimple><w:r><w:t> BAD ADVANCE AFTER</w:t></w:r></w:p></w:endnote><w:endnote w:id="303"><w:p><w:r><w:t xml:space="preserve">ACTION BEFORE </w:t></w:r><w:fldSimple w:instr=" MACROBUTTON RunReport &quot;Fresh Action&quot; "><w:r><w:t>STALE ACTION</w:t></w:r></w:fldSimple><w:r><w:t> ACTION AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
    )
}

fn document_info_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rIdCustom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/><Relationship Id="rIdApp" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="311"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="321"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="312"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="322"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:footnoteReference w:id="313"/></w:r><w:r><w:t> BODY F</w:t></w:r><w:r><w:endnoteReference w:id="323"/></w:r><w:r><w:t> BODY G</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="311"><w:p><w:r><w:t xml:space="preserve">FOOT CORE BEFORE </w:t></w:r><w:fldSimple w:instr=" DOCPROPERTY Subject \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT CORE</w:t></w:r></w:fldSimple><w:r><w:t> FOOT CORE AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="312"><w:p><w:r><w:t xml:space="preserve">EXTENDED BEFORE </w:t></w:r><w:fldSimple w:instr=" NUMPAGES \* ROMAN "><w:r><w:t>STALE EXTENDED</w:t></w:r></w:fldSimple><w:r><w:t> EXTENDED AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="313"><w:p><w:r><w:t xml:space="preserve">MALFORMED PROPERTY BEFORE </w:t></w:r><w:fldSimple w:instr=" DOCPROPERTY &quot;Broken Name "><w:r><w:t>CACHED MALFORMED PROPERTY</w:t></w:r></w:fldSimple><w:r><w:t> MALFORMED PROPERTY AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="321"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END CUSTOM BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> DOCPROPERTY &quot;Client Name&quot; \* Caps </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END CUSTOM</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END CUSTOM AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="322"><w:p><w:r><w:t xml:space="preserve">VARIABLE BEFORE </w:t></w:r><w:fldSimple w:instr=" DOCVARIABLE ClientCode \* Upper "><w:r><w:t>STALE VARIABLE</w:t></w:r></w:fldSimple><w:r><w:t> VARIABLE AFTER</w:t></w:r></w:p></w:endnote><w:endnote w:id="323"><w:p><w:r><w:t xml:space="preserve">SPLIT CORE BEFORE </w:t></w:r><w:fldSimple w:instr=" TITLE \* Upper "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT CORE A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT CORE B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT CORE AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Quarter Plan</dc:title><dc:subject>Pipeline</dc:subject></cp:coreProperties>"#,
        ),
        (
            "docProps/custom.xml",
            r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Client Name"><vt:lpwstr>acme launch</vt:lpwstr></property></Properties>"#,
        ),
        (
            "docProps/app.xml",
            r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Pages>12</Pages></Properties>"#,
        ),
        (
            "word/settings.xml",
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docVars><w:docVar w:name="ClientCode" w:val="alpha-42"/></w:docVars></w:settings>"#,
        ),
    ])
}

fn revision_number_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="411"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="421"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="412"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="422"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="411"><w:p><w:r><w:t xml:space="preserve">FOOT REVISION BEFORE </w:t></w:r><w:fldSimple w:instr=" REVNUM \* Caps "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT REVISION</w:t></w:r></w:fldSimple><w:r><w:t> FOOT REVISION AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="412"><w:p><w:r><w:t xml:space="preserve">MALFORMED REVISION BEFORE </w:t></w:r><w:fldSimple w:instr=" REVNUM \x "><w:r><w:t>CACHED MALFORMED REVISION</w:t></w:r></w:fldSimple><w:r><w:t> MALFORMED REVISION AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="421"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t xml:space="preserve">END REVISION BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> REVNUM \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END REVISION</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END REVISION AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="422"><w:p><w:r><w:t xml:space="preserve">SPLIT REVISION BEFORE </w:t></w:r><w:fldSimple w:instr=" REVNUM "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE SPLIT REVISION A</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE SPLIT REVISION B</w:t></w:r></w:fldSimple><w:r><w:t> SPLIT REVISION AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"><cp:revision>draft revision</cp:revision></cp:coreProperties>"#,
        ),
    ])
}

fn local_ref_field_note_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="431"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="441"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="432"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="442"/></w:r><w:r><w:t> BODY E</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="431"><w:p><w:bookmarkStart w:id="51" w:name="FootLocal"/><w:r><w:rPr><w:i/></w:rPr><w:t>alpha launch</w:t></w:r><w:bookmarkEnd w:id="51"/><w:r><w:t xml:space="preserve"> | FOOT REF BEFORE </w:t></w:r><w:fldSimple w:instr=" REF FootLocal \h \* Caps "><w:r><w:rPr><w:b/></w:rPr><w:t>STALE FOOT REF</w:t></w:r></w:fldSimple><w:r><w:t> FOOT REF AFTER</w:t></w:r></w:p></w:footnote><w:footnote w:id="432"><w:p><w:r><w:t xml:space="preserve">CROSS REF BEFORE </w:t></w:r><w:fldSimple w:instr=" REF FootLocal \* Upper "><w:r><w:t>STALE CROSS REF</w:t></w:r></w:fldSimple><w:r><w:t> CROSS REF AFTER</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r##"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="441"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:bookmarkStart w:id="61" w:name="EndAmount"/><w:r><w:t>1250.5</w:t></w:r><w:bookmarkEnd w:id="61"/><w:r><w:t xml:space="preserve"> | END REF BEFORE </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> REF EndAmount \# &quot;$#,##0.00&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>STALE END REF</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> END REF AFTER</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="442"><w:p><w:bookmarkStart w:id="71" w:name="MultiTarget"/><w:r><w:t>multi</w:t></w:r><w:r><w:t xml:space="preserve"> target</w:t></w:r><w:bookmarkEnd w:id="71"/><w:r><w:t xml:space="preserve"> | MULTI REF BEFORE </w:t></w:r><w:fldSimple w:instr=" REF MultiTarget \* Upper "><w:r><w:t>STALE MULTI REF</w:t></w:r></w:fldSimple><w:r><w:t> MULTI REF AFTER</w:t></w:r></w:p></w:endnote></w:endnotes>"##,
        ),
    ])
}

fn raster_note_docx() -> Vec<u8> {
    let png = tiny_png();
    let body_image = source_inline_drawing("rBodyImage", "Body image", 0);
    let foot_one_image = source_inline_drawing("rFootImageOne", "Foot &lt;one&gt;", 0);
    let foot_two_image = source_inline_drawing("rFootImageTwo", "Foot nested", 0);
    let end_one_image = source_inline_drawing("rEndImageOne", "End rotate", 5_400_000);
    let end_two_image = source_inline_drawing("rEndImageTwo", "End nested", 0);
    let rejected_image = source_inline_drawing("rRejectedImage", "Rejected image", 0);
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p>{body_image}</w:p><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="51"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="61"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="52"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="62"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:endnoteReference w:id="63"/></w:r></w:p><w:sectPr/></w:body></w:document>"#
    );
    let footnotes_xml = format!(
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:footnote w:id="51"><w:p><w:r><w:t>FOOT IMAGE ONE</w:t></w:r><w:hyperlink r:id="rFootLink">{foot_one_image}</w:hyperlink></w:p></w:footnote><w:footnote w:id="52"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>FOOT NESTED IMAGE</w:t></w:r>{foot_two_image}</w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:footnote></w:footnotes>"#
    );
    let endnotes_xml = format!(
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:endnote w:id="61"><w:p><w:r><w:t>END IMAGE ONE</w:t></w:r>{end_one_image}</w:p></w:endnote><w:endnote w:id="62"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>END NESTED IMAGE</w:t></w:r><w:hyperlink r:id="rEndLink">{end_two_image}</w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="63"><w:p><w:r><w:t>REJECTED IMAGE</w:t></w:r>{rejected_image}<w:fldSimple w:instr=" CUSTOM rejected-image "><w:r><w:t> UNSUPPORTED FIELD</w:t></w:r></w:fldSimple></w:p></w:endnote></w:endnotes>"#
    );

    docx_fixture_with_media(
        &[
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/><Relationship Id="rBodyImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/body.png"/></Relationships>"#,
            ),
            ("word/document.xml", document_xml.as_str()),
            ("word/footnotes.xml", footnotes_xml.as_str()),
            (
                "word/_rels/footnotes.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rFootLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/foot-image" TargetMode="External"/><Relationship Id="rFootImageOne" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/foot-one.png"/><Relationship Id="rFootImageTwo" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/foot-two.png"/></Relationships>"#,
            ),
            ("word/endnotes.xml", endnotes_xml.as_str()),
            (
                "word/_rels/endnotes.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rEndImageOne" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/end-one.png"/><Relationship Id="rEndLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/end-image" TargetMode="External"/><Relationship Id="rEndImageTwo" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/end-two.png"/><Relationship Id="rRejectedImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/rejected.png"/></Relationships>"#,
            ),
        ],
        &[
            ("word/media/body.png", png.as_slice()),
            ("word/media/foot-one.png", png.as_slice()),
            ("word/media/foot-two.png", png.as_slice()),
            ("word/media/end-one.png", png.as_slice()),
            ("word/media/end-two.png", png.as_slice()),
            ("word/media/rejected.png", png.as_slice()),
        ],
    )
}

fn chart_note_docx() -> Vec<u8> {
    let (core_chart, extended_chart) = native_chart_xml();
    let core_uri = "http://schemas.openxmlformats.org/drawingml/2006/chart";
    let extended_uri = "http://schemas.microsoft.com/office/drawing/2014/chartex";
    let body_chart = source_chart_drawing("rBodyChart", "c:chart", core_uri, "Body chart");
    let foot_one_chart =
        source_chart_drawing("rFootChartOne", "c:chart", core_uri, "Foot &lt;chart&gt;");
    let foot_two_chart =
        source_chart_drawing("rFootChartTwo", "c:chart", core_uri, "Foot nested chart");
    let end_one_chart = source_chart_drawing(
        "rEndChartOne",
        "cx:chart",
        extended_uri,
        "End extended chart",
    );
    let end_two_chart =
        source_chart_drawing("rEndChartTwo", "cx:chart", extended_uri, "End nested chart");
    let rejected_chart =
        source_chart_drawing("rRejectedChart", "c:chart", core_uri, "Rejected chart");
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><w:body><w:p>{body_chart}</w:p><w:p><w:r><w:t>BODY A</w:t></w:r><w:r><w:footnoteReference w:id="71"/></w:r><w:r><w:t> BODY B</w:t></w:r><w:r><w:endnoteReference w:id="81"/></w:r><w:r><w:t> BODY C</w:t></w:r><w:r><w:footnoteReference w:id="72"/></w:r><w:r><w:t> BODY D</w:t></w:r><w:r><w:endnoteReference w:id="82"/></w:r><w:r><w:t> BODY E</w:t></w:r><w:r><w:endnoteReference w:id="83"/></w:r></w:p><w:sectPr/></w:body></w:document>"#
    );
    let footnotes_xml = format!(
        r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><w:footnote w:id="71"><w:p><w:r><w:t>FOOT CHART ONE</w:t></w:r></w:p><w:p>{foot_one_chart}</w:p></w:footnote><w:footnote w:id="72"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>FOOT NESTED CHART</w:t></w:r></w:p><w:p>{foot_two_chart}</w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:footnote></w:footnotes>"#
    );
    let endnotes_xml = format!(
        r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><w:endnote w:id="81"><w:p><w:r><w:t>END CHART ONE</w:t></w:r></w:p><w:p>{end_one_chart}</w:p></w:endnote><w:endnote w:id="82"><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>END NESTED CHART</w:t></w:r></w:p><w:p>{end_two_chart}</w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:endnote><w:endnote w:id="83"><w:p><w:r><w:t>REJECTED CHART</w:t></w:r><w:fldSimple w:instr=" CUSTOM rejected-chart "><w:r><w:t> UNSUPPORTED FIELD</w:t></w:r></w:fldSimple></w:p><w:p>{rejected_chart}</w:p></w:endnote></w:endnotes>"#
    );

    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/><Override PartName="/word/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/><Override PartName="/word/charts/chart2.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/><Override PartName="/word/charts/chart3.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/><Override PartName="/word/charts/chartEx4.xml" ContentType="application/vnd.ms-office.chartex+xml"/><Override PartName="/word/charts/chartEx5.xml" ContentType="application/vnd.ms-office.chartex+xml"/><Override PartName="/word/charts/chart6.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rIdEnd" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/><Relationship Id="rBodyChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart1.xml"/></Relationships>"#,
        ),
        ("word/document.xml", document_xml.as_str()),
        ("word/footnotes.xml", footnotes_xml.as_str()),
        (
            "word/_rels/footnotes.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rFootChartOne" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart2.xml"/><Relationship Id="rFootChartTwo" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart3.xml"/></Relationships>"#,
        ),
        ("word/endnotes.xml", endnotes_xml.as_str()),
        (
            "word/_rels/endnotes.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rEndChartOne" Type="http://schemas.microsoft.com/office/2014/relationships/chartEx" Target="charts/chartEx4.xml"/><Relationship Id="rEndChartTwo" Type="http://schemas.microsoft.com/office/2014/relationships/chartEx" Target="charts/chartEx5.xml"/><Relationship Id="rRejectedChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart6.xml"/></Relationships>"#,
        ),
        ("word/charts/chart1.xml", core_chart.as_str()),
        ("word/charts/chart2.xml", core_chart.as_str()),
        ("word/charts/chart3.xml", core_chart.as_str()),
        ("word/charts/chartEx4.xml", extended_chart.as_str()),
        ("word/charts/chartEx5.xml", extended_chart.as_str()),
        ("word/charts/chart6.xml", core_chart.as_str()),
    ])
}

fn note_with_marker<'a>(xml: &'a str, item: &str, marker: &str) -> &'a str {
    let marker_offset = xml
        .find(marker)
        .unwrap_or_else(|| panic!("missing note marker {marker:?}: {xml}"));
    let start_marker = format!("<w:{item} w:id=");
    let start = xml[..marker_offset]
        .rfind(&start_marker)
        .unwrap_or_else(|| panic!("missing {item} start for {marker:?}: {xml}"));
    let end_marker = format!("</w:{item}>");
    let end = marker_offset
        + xml[marker_offset..]
            .find(&end_marker)
            .unwrap_or_else(|| panic!("missing {item} end for {marker:?}: {xml}"))
        + end_marker.len();
    &xml[start..end]
}

fn clear_simple_field_ownership(block: &mut Block) {
    match block {
        Block::Paragraph(paragraph) => {
            for run in &mut paragraph.runs {
                if matches!(run.field, FieldRole::Simple { .. }) {
                    run.field = FieldRole::Other;
                }
            }
        }
        Block::Table(table) => {
            for block in table
                .rows
                .iter_mut()
                .flat_map(|row| &mut row.cells)
                .flat_map(|cell| &mut cell.blocks)
            {
                clear_simple_field_ownership(block);
            }
        }
        _ => {}
    }
}

#[test]
fn opened_docx_exact_note_paragraph_payload_roundtrips_through_fresh_conversion() {
    let document = Document::open(&exact_note_paragraphs_docx()).expect("note fixture opens");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let footnote = note_with_marker(footnotes, "footnote", "FOOT A");
    let endnote = note_with_marker(endnotes, "endnote", "END A");

    assert_eq!(footnote.matches("<w:p>").count(), 2, "{footnote}");
    assert_eq!(endnote.matches("<w:p>").count(), 2, "{endnote}");
    assert_eq!(
        footnote.matches(r#"<w:br w:type="column"/>"#).count(),
        1,
        "{footnote}"
    );
    assert_eq!(
        endnote.matches(r#"<w:br w:type="column"/>"#).count(),
        1,
        "{endnote}"
    );
    assert!(footnote.contains("<w:keepNext/>"), "{footnote}");
    assert_eq!(footnote.matches("<w:keepLines/>").count(), 2, "{footnote}");
    assert!(footnote.contains(r#"<w:widowControl w:val="0"/>"#));
    assert!(footnote.contains(r#"w:line="260" w:lineRule="exact""#));
    assert!(footnote.contains(r#"w:line="320" w:lineRule="atLeast""#));
    assert!(footnote.contains(r#"w:val="center" w:pos="720" w:leader="hyphen""#));
    assert!(footnote.contains(r#"w:fill="DDEEFF""#));
    assert!(footnote.contains("<w:b/>"));
    assert!(footnote.contains(r#"<w:highlight w:val="yellow"/>"#));
    assert!(footnote.contains("<w:i/>"));
    assert!(footnote.contains(r#"<w:vertAlign w:val="superscript"/>"#));

    assert!(endnote.contains(r#"<w:widowControl w:val="0"/>"#));
    assert!(endnote.contains(r#"w:line="280" w:lineRule="atLeast""#));
    assert!(endnote.contains(r#"w:val="right" w:pos="1200" w:leader="dot""#));
    assert!(endnote.contains(r#"w:fill="FFEEDD""#));
    assert!(endnote.contains("<w:keepNext/>"));
    assert!(endnote.contains(r#"<w:u w:val="single"/>"#));
    assert!(endnote.contains("<w:smallCaps/>"));

    let reopened = Document::open(&converted).expect("converted notes reopen");
    assert_eq!(reopened.model(), normalized_model);
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
}

#[test]
fn opened_docx_simple_table_note_roundtrips_without_disabling_supported_sibling_payload() {
    let document =
        Document::open(&mixed_supported_and_table_note_docx()).expect("mixed note fixture opens");
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx());
    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let footnote = note_with_marker(footnotes, "footnote", "TABLE NOTE");
    let endnote = note_with_marker(endnotes, "endnote", "END ONE");

    assert_eq!(footnote.matches("<w:tbl>").count(), 1, "{footnote}");
    assert_eq!(footnote.matches("<w:p>").count(), 1, "{footnote}");
    assert_eq!(endnote.matches("<w:p>").count(), 2, "{endnote}");
    assert!(endnote.contains("<w:keepNext/>"), "{endnote}");
    assert!(endnote.contains(r#"w:line="240" w:lineRule="exact""#));
    assert_eq!(
        endnote.matches(r#"<w:br w:type="column"/>"#).count(),
        1,
        "{endnote}"
    );
}

#[test]
fn opened_docx_rich_mixed_note_tables_roundtrip_through_fresh_conversion() {
    let document = Document::open(&rich_note_tables_docx()).expect("rich note table fixture opens");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let footnote = note_with_marker(footnotes, "footnote", "FOOT TABLE A");
    let endnote = note_with_marker(endnotes, "endnote", "END TABLE A");

    assert_eq!(
        footnote.matches(r#"<w:br w:type="page"/>"#).count(),
        2,
        "{footnote}"
    );
    let foot_table = footnote.find("<w:tbl>").unwrap();
    assert!(footnote.find("FOOT PREFIX").unwrap() < foot_table);
    assert!(foot_table < footnote.find("FOOT SUFFIX").unwrap());
    assert_eq!(footnote.matches("<w:tbl>").count(), 1, "{footnote}");
    assert!(footnote.contains(r#"<w:tblW w:w="4000" w:type="pct"/>"#));
    assert!(footnote.contains(r#"<w:tblInd w:w="720" w:type="dxa"/>"#));
    assert!(footnote.contains(r#"<w:jc w:val="center"/>"#));
    assert!(footnote.contains(r#"<w:top w:val="double" w:sz="12" w:space="0" w:color="112233"/>"#));
    assert!(footnote.contains(r#"<w:tblLayout w:type="fixed"/>"#));
    assert!(footnote.contains(r#"<w:vMerge w:val="restart"/>"#));
    assert!(footnote.contains("<w:vMerge/>"));
    assert!(!footnote.contains("DROPPED MERGE TEXT"));
    assert!(footnote.contains("<w:cantSplit/>"));
    assert!(footnote.contains("<w:tblHeader/>"));
    assert!(footnote.contains("<w:keepNext/>"));
    assert_eq!(footnote.matches("<w:keepLines/>").count(), 3, "{footnote}");
    assert!(footnote.contains(r#"<w:widowControl w:val="0"/>"#));
    assert!(footnote.contains(r#"w:line="260" w:lineRule="exact""#));
    assert!(footnote.contains(r#"w:line="320" w:lineRule="atLeast""#));
    assert!(footnote.contains(r#"w:val="center" w:pos="720" w:leader="hyphen""#));
    assert_eq!(footnote.matches(r#"<w:br w:type="column"/>"#).count(), 1);
    assert!(footnote.contains(r#"w:fill="DDEEFF""#));
    assert!(footnote.contains("<w:b/>"));
    assert!(footnote.contains(r#"<w:highlight w:val="yellow"/>"#));
    assert!(footnote.contains("<w:i/>"));
    assert!(footnote.contains(r#"<w:vertAlign w:val="superscript"/>"#));

    assert_eq!(
        endnote.matches(r#"<w:br w:type="page"/>"#).count(),
        2,
        "{endnote}"
    );
    let end_table = endnote.find("<w:tbl>").unwrap();
    assert!(endnote.find("END PREFIX").unwrap() < end_table);
    assert!(end_table < endnote.find("END SUFFIX").unwrap());
    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert!(endnote.contains("<w:bidiVisual/>"));
    assert!(endnote.contains(r#"<w:jc w:val="right"/>"#));
    assert!(endnote.contains(r#"<w:gridSpan w:val="2"/>"#));
    assert_eq!(endnote.matches("<w:cantSplit/>").count(), 2, "{endnote}");
    assert!(endnote.contains(r#"w:line="280" w:lineRule="atLeast""#));
    assert!(endnote.contains(r#"w:val="right" w:pos="1200" w:leader="dot""#));
    assert_eq!(endnote.matches(r#"<w:br w:type="column"/>"#).count(), 2);
    assert!(endnote.contains(r#"w:fill="FFEEDD""#));
    assert!(endnote.contains(r#"<w:vAlign w:val="bottom"/>"#));
    assert!(endnote.contains(r#"<w:u w:val="single"/>"#));
    assert!(endnote.contains("<w:smallCaps/>"));
    assert!(endnote.contains("END NESTED A"));
    assert!(endnote.contains(r#"w:line="360" w:lineRule="exact""#));
    assert!(endnote.contains(r#"w:val="left" w:pos="600" w:leader="hyphen""#));

    let reopened = Document::open(&converted).expect("converted note tables reopen");
    assert_eq!(reopened.model(), normalized_model);
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
}

#[test]
fn nested_note_table_roundtrips_without_disabling_supported_sibling() {
    let document = Document::open(&nested_and_relationship_table_note_docx())
        .expect("mixed supported and unsupported note tables open");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx());
    assert_eq!(document.model(), source_model);
    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let footnote = note_with_marker(footnotes, "footnote", "NESTED TABLE A");
    let endnote = note_with_marker(endnotes, "endnote", "UNSUPPORTED FIELD TABLE");

    assert_eq!(footnote.matches("<w:tbl>").count(), 2, "{footnote}");
    assert_eq!(
        footnote.matches(r#"<w:br w:type="page"/>"#).count(),
        2,
        "{footnote}"
    );
    assert_eq!(footnote.matches("<w:cantSplit/>").count(), 2, "{footnote}");
    assert!(footnote.contains("<w:keepNext/>"), "{footnote}");
    assert!(footnote.contains("<w:keepLines/>"), "{footnote}");
    assert!(footnote.contains(r#"<w:widowControl w:val="0"/>"#));
    assert!(footnote.contains(r#"w:line="220" w:lineRule="exact""#));
    assert!(footnote.contains(r#"w:line="300" w:lineRule="atLeast""#));
    assert!(footnote.contains(r#"w:val="center" w:pos="900" w:leader="dot""#));
    assert_eq!(footnote.matches(r#"<w:br w:type="column"/>"#).count(), 1);
    assert!(footnote.contains("<w:b/>"));

    assert!(!endnote.contains("<w:tbl>"), "{endnote}");
    assert_eq!(endnote.matches("<w:p>").count(), 1, "{endnote}");
    assert!(!endnote.contains("<w:fldSimple"), "{endnote}");

    let reopened = Document::open(&converted).expect("converted nested note table reopens");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
}

#[test]
fn opened_docx_note_external_hyperlinks_keep_part_local_relationships() {
    let document = Document::open(&relationship_note_docx()).expect("relationship notes open");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let document_rels = std::str::from_utf8(&parts["word/_rels/document.xml.rels"]).unwrap();
    assert!(parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(parts.contains_key("word/_rels/endnotes.xml.rels"));
    let footnote_rels = std::str::from_utf8(&parts["word/_rels/footnotes.xml.rels"]).unwrap();
    let endnote_rels = std::str::from_utf8(&parts["word/_rels/endnotes.xml.rels"]).unwrap();

    let foot_one = note_with_marker(footnotes, "footnote", "FOOT LINK ONE");
    let foot_two = note_with_marker(footnotes, "footnote", "FOOT NESTED LINK");
    let end_one = note_with_marker(endnotes, "endnote", "END LINK ONE");
    let end_two = note_with_marker(endnotes, "endnote", "END NESTED LINK");
    let rejected = note_with_marker(endnotes, "endnote", "REJECTED LINK");
    assert!(footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(endnotes.contains("xmlns:r="), "{endnotes}");
    assert!(foot_one.contains(r#"<w:hyperlink r:id="rId1">"#));
    assert!(foot_two.contains(r#"<w:hyperlink r:id="rId2">"#));
    assert_eq!(foot_two.matches("<w:tbl>").count(), 2, "{foot_two}");
    assert!(end_one.contains(r#"<w:hyperlink r:id="rId1">"#));
    assert!(end_two.contains(r#"<w:hyperlink r:id="rId2">"#));
    assert_eq!(end_two.matches("<w:tbl>").count(), 2, "{end_two}");
    assert!(!rejected.contains("<w:hyperlink"), "{rejected}");
    assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
    assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");

    assert_eq!(footnote_rels.matches("TargetMode=\"External\"").count(), 2);
    assert!(footnote_rels.contains(r#"Id="rId1""#));
    assert!(footnote_rels.contains(r#"Id="rId2""#));
    assert!(footnote_rels.contains("https://example.com/foot-one?x=1&amp;y=2"));
    assert!(footnote_rels.contains("https://example.com/foot-two"));
    assert_eq!(endnote_rels.matches("TargetMode=\"External\"").count(), 2);
    assert!(endnote_rels.contains(r#"Id="rId1""#));
    assert!(endnote_rels.contains(r#"Id="rId2""#));
    assert!(endnote_rels.contains("https://example.com/end-one"));
    assert!(endnote_rels.contains("https://example.com/end-two"));
    assert!(!endnote_rels.contains("https://example.com/rejected"));
    for target in ["foot-one", "foot-two", "end-one", "end-two", "rejected"] {
        assert!(!document_rels.contains(target), "{document_rels}");
    }

    let reopened = Document::open(&converted).expect("converted relationship notes reopen");
    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    assert_eq!(
        &reopened_model.blocks[..reopened_model.blocks.len() - 1],
        &normalized_model.blocks[..normalized_model.blocks.len() - 1]
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_internal_anchors_keep_bookmarks_without_relationships() {
    let document = Document::open(&internal_anchor_note_docx()).expect("anchor notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone anchor normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let document_xml = std::str::from_utf8(&parts["word/document.xml"]).unwrap();
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let document_rels = std::str::from_utf8(&parts["word/_rels/document.xml.rels"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!document_rels.contains("relationships/hyperlink"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT BODY LINK");
    let endnote = note_with_marker(endnotes, "endnote", "END TARGET");
    let rejected_anchor = note_with_marker(footnotes, "footnote", "REJECTED ANCHOR");
    let rejected_bookmark = note_with_marker(endnotes, "endnote", "REJECTED BOOKMARK");
    assert!(footnote.contains(r#"<w:hyperlink w:anchor="BodyTarget">"#));
    assert!(footnote.contains(r#"<w:hyperlink w:anchor="FootTarget">"#));
    assert!(footnote.contains(r#"<w:bookmarkStart w:id="1" w:name="FootTarget"/>"#));
    assert!(footnote.contains(r#"<w:bookmarkEnd w:id="1"/>"#));
    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert!(endnote.contains(r#"<w:hyperlink w:anchor="BodyTarget">"#));
    assert!(endnote.contains(r#"<w:hyperlink w:anchor="EndTarget">"#));
    assert!(endnote.contains(r#"<w:bookmarkStart w:id="2" w:name="EndTarget"/>"#));
    assert!(endnote.contains(r#"<w:bookmarkEnd w:id="2"/>"#));

    let all_stories = format!("{document_xml}{footnotes}{endnotes}");
    for (id, name) in [("0", "BodyTarget"), ("1", "FootTarget"), ("2", "EndTarget")] {
        assert_eq!(
            all_stories
                .matches(&format!(
                    r#"<w:bookmarkStart w:id="{id}" w:name="{name}"/>"#
                ))
                .count(),
            1,
            "{all_stories}"
        );
        assert_eq!(
            all_stories
                .matches(&format!(r#"<w:bookmarkEnd w:id="{id}"/>"#))
                .count(),
            1,
            "{all_stories}"
        );
    }
    assert!(
        !rejected_anchor.contains("<w:hyperlink"),
        "{rejected_anchor}"
    );
    assert!(
        !rejected_anchor.contains("<w:bookmark"),
        "{rejected_anchor}"
    );
    assert_eq!(rejected_anchor.matches("<w:p>").count(), 1);
    assert!(
        !rejected_bookmark.contains("<w:hyperlink"),
        "{rejected_bookmark}"
    );
    assert!(
        !rejected_bookmark.contains("<w:bookmark"),
        "{rejected_bookmark}"
    );
    assert_eq!(rejected_bookmark.matches("<w:p>").count(), 1);

    let reopened = Document::open(&converted).expect("converted anchor notes reopen");
    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_anchor) = &reopened_model.blocks[2] else {
        panic!("rejected anchor fallback paragraph")
    };
    assert_eq!(rejected_anchor.text(), "REJECTED ANCHOR FALLBACK");
    let Block::Paragraph(rejected_bookmark) = &reopened_model.blocks[4] else {
        panic!("rejected bookmark fallback paragraph")
    };
    assert_eq!(rejected_bookmark.text(), "REJECTED BOOKMARK VALID LINK");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_content_controls_keep_complete_modeled_metadata() {
    let document =
        Document::open(&content_control_note_docx()).expect("content-control notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.content_controls, 4);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone content-control normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT CONTROL");
    let endnote = note_with_marker(endnotes, "endnote", "END BOUND CONTROL");
    let rejected_binding = note_with_marker(footnotes, "footnote", "REJECTED HALF BINDING");
    let rejected_tagged = note_with_marker(endnotes, "endnote", "REJECTED TAGGED HALF");

    let foot_sdt = footnote.find("<w:sdt>").unwrap_or(usize::MAX);
    let foot_alias = footnote
        .find(r#"<w:alias w:val="Foot alias"/>"#)
        .unwrap_or(usize::MAX);
    let foot_tag = footnote
        .find(r#"<w:tag w:val="foot-tag"/>"#)
        .unwrap_or(usize::MAX);
    let foot_content = footnote.find("<w:sdtContent>").unwrap_or(usize::MAX);
    let foot_bold = footnote.find("<w:b/>").unwrap_or(usize::MAX);
    let foot_text = footnote.find("FOOT CONTROL").unwrap_or(usize::MAX);
    let foot_end = footnote.find("</w:sdt>").unwrap_or(usize::MAX);
    assert!(
        foot_sdt < foot_alias
            && foot_alias < foot_tag
            && foot_tag < foot_content
            && foot_content < foot_bold
            && foot_bold < foot_text
            && foot_text < foot_end,
        "footnote content-control XML missing or out of order: {footnote}"
    );

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    let end_alias = endnote
        .find(r#"<w:alias w:val="End binding"/>"#)
        .unwrap_or(usize::MAX);
    let end_binding = endnote
        .find(r#"<w:dataBinding w:xpath="/root/client" w:storeItemID="{11111111-2222-3333-4444-555555555555}"/>"#)
        .unwrap_or(usize::MAX);
    let end_content = endnote.find("<w:sdtContent>").unwrap_or(usize::MAX);
    let end_italic = endnote.find("<w:i/>").unwrap_or(usize::MAX);
    let end_text = endnote.find("END BOUND CONTROL").unwrap_or(usize::MAX);
    assert!(
        end_alias < end_binding
            && end_binding < end_content
            && end_content < end_italic
            && end_italic < end_text,
        "endnote data-bound control XML missing or out of order: {endnote}"
    );

    for rejected in [rejected_binding, rejected_tagged] {
        assert!(!rejected.contains("<w:sdt"), "{rejected}");
        assert!(!rejected.contains("<w:dataBinding"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("/root/half"), "{footnotes}");
    assert!(!endnotes.contains("rejected-tag"), "{endnotes}");
    assert!(!endnotes.contains("AAAAAAAA-BBBB"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted content-control notes reopen");
    assert_eq!(reopened.report().features.content_controls, 2);
    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_binding) = &reopened_model.blocks[2] else {
        panic!("rejected half-binding fallback paragraph")
    };
    assert_eq!(rejected_binding.text(), "REJECTED HALF BINDING FALLBACK");
    assert!(rejected_binding
        .runs
        .iter()
        .all(|run| run.content_control.is_none()));
    let Block::Paragraph(rejected_tagged) = &reopened_model.blocks[4] else {
        panic!("rejected tagged half-binding fallback paragraph")
    };
    assert_eq!(rejected_tagged.text(), "REJECTED TAGGED HALF FALLBACK");
    assert!(rejected_tagged
        .runs
        .iter()
        .all(|run| run.content_control.is_none()));
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_cache_only_fields_keep_normalized_results() {
    let document = Document::open(&cached_field_note_docx()).expect("cached-field notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 7);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone cached-field normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "PRIVATE CACHE");
    let endnote = note_with_marker(endnotes, "endnote", "ADDRESS CACHE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED FIELD");
    let rejected_context = note_with_marker(endnotes, "endnote", "REJECTED CONTEXT FIELD");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 2, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" PRIVATE legacy-data ">"#)
            && footnote
                .contains(r#"<w:fldSimple w:instr=" INCLUDETEXT &quot;appendix.docx&quot; ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("PRIVATE CACHE")
            && footnote.contains("INCLUDE CACHE"),
        "top-level cached fields missing: {footnote}"
    );
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 3, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" ADDRESSBLOCK ">"#)
            && endnote.contains(r#"<w:fldSimple w:instr=" BARCODE &quot;9781234567890&quot; ">"#)
            && endnote.contains(r#"<w:fldSimple w:instr=" PRIVATE complex-data ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("BARCODE CACHE")
            && endnote.contains("COMPLEX CACHE"),
        "nested cached fields missing: {endnote}"
    );
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_context] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("ADDIN"), "{footnotes}");
    assert!(!endnotes.contains("REF MissingTarget"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted cached-field notes reopen");
    assert_eq!(reopened.report().features.fields, 5);
    assert_eq!(
        reopened.report().features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::NoComputedResult,
            count: 5,
        }]
    );
    let fields = reopened.fields();
    assert_eq!(fields.len(), 5);
    assert_eq!(
        fields[0].kind,
        FieldKind::Compatibility("PRIVATE".to_string())
    );
    assert_eq!(fields[0].instruction, "PRIVATE legacy-data");
    assert_eq!(fields[0].result, "PRIVATE CACHE");
    assert_eq!(
        fields[1].kind,
        FieldKind::InsertedContent("INCLUDETEXT".to_string())
    );
    assert_eq!(fields[1].instruction, r#"INCLUDETEXT "appendix.docx""#);
    assert_eq!(fields[1].result, "INCLUDE CACHE");
    assert_eq!(
        fields[2].kind,
        FieldKind::MailMerge("ADDRESSBLOCK".to_string())
    );
    assert_eq!(fields[2].result, "ADDRESS CACHE");
    assert_eq!(fields[3].kind, FieldKind::Barcode("BARCODE".to_string()));
    assert_eq!(fields[3].instruction, r#"BARCODE "9781234567890""#);
    assert_eq!(fields[3].result, "BARCODE CACHE");
    assert_eq!(
        fields[4].kind,
        FieldKind::Compatibility("PRIVATE".to_string())
    );
    assert_eq!(fields[4].instruction, "PRIVATE complex-data");
    assert_eq!(fields[4].result, "COMPLEX CACHE");
    assert!(fields.iter().all(|field| field.computed_result.is_none()));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-field fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "REJECTED MALFORMED FIELD FALLBACK"
    );
    let Block::Paragraph(rejected_context) = &reopened_model.blocks[4] else {
        panic!("rejected context-field fallback paragraph")
    };
    assert_eq!(rejected_context.text(), "REJECTED CONTEXT FIELD FALLBACK");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_merge_fields_keep_validated_cached_results() {
    let document = Document::open(&merge_field_note_docx()).expect("merge-field notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 4);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone merge-field normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "CLIENT CACHE");
    let endnote = note_with_marker(endnotes, "endnote", "PROJECT CACHE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED MERGE");
    let rejected_unknown = note_with_marker(endnotes, "endnote", "REJECTED UNKNOWN FIELD");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote
            .contains(r#"<w:fldSimple w:instr=" MERGEFIELD &quot;Client Name&quot; \* Upper ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("CLIENT CACHE")
            && footnote.contains("FOOT TAIL"),
        "top-level merge field missing: {footnote}"
    );
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" MERGEFIELD ProjectName \* MERGEFORMAT ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("PROJECT CACHE")
            && endnote.contains("END TAIL"),
        "nested merge field missing: {endnote}"
    );
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_unknown] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(
        !footnotes.contains("MERGEFIELD \\* MERGEFORMAT"),
        "{footnotes}"
    );
    assert!(!endnotes.contains("CUSTOM payload"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted merge-field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert!(fields
        .iter()
        .all(|field| field.kind == FieldKind::MergeField));
    assert_eq!(
        fields[0].instruction,
        r#"MERGEFIELD "Client Name" \* Upper"#
    );
    assert_eq!(fields[0].result, "CLIENT CACHE");
    assert_eq!(
        fields[1].instruction,
        r#"MERGEFIELD ProjectName \* MERGEFORMAT"#
    );
    assert_eq!(fields[1].result, "PROJECT CACHE");
    assert!(fields.iter().all(|field| field.computed_result.is_none()));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-merge fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "REJECTED MALFORMED MERGE FALLBACK"
    );
    let Block::Paragraph(rejected_unknown) = &reopened_model.blocks[4] else {
        panic!("rejected unknown-field fallback paragraph")
    };
    assert_eq!(rejected_unknown.text(), "REJECTED UNKNOWN FIELD FALLBACK");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_filename_fields_keep_validated_cached_results() {
    let document = Document::open(&filename_field_note_docx()).expect("filename notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 4);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone filename normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "/work/REPORT.DOCX");
    let endnote = note_with_marker(endnotes, "endnote", "report.docx");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED FILENAME");
    let rejected_unknown =
        note_with_marker(endnotes, "endnote", "REJECTED UNKNOWN FILENAME SIBLING");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" FILENAME \p \* Upper ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("/work/REPORT.DOCX")
            && footnote.contains("FOOT TAIL"),
        "top-level filename field missing: {footnote}"
    );
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" FILENAME \* MERGEFORMAT ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("report.docx")
            && endnote.contains("END TAIL"),
        "nested filename field missing: {endnote}"
    );
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_unknown] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("FILENAME \\x"), "{footnotes}");
    assert!(!endnotes.contains("CUSTOM filename-payload"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted filename notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|field| field.kind == FieldKind::Filename));
    assert_eq!(fields[0].instruction, r#"FILENAME \p \* Upper"#);
    assert_eq!(fields[0].result, "/work/REPORT.DOCX");
    assert_eq!(fields[1].instruction, r#"FILENAME \* MERGEFORMAT"#);
    assert_eq!(fields[1].result, "report.docx");
    assert!(fields.iter().all(|field| field.computed_result.is_none()));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-filename fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "REJECTED MALFORMED FILENAME FALLBACK"
    );
    let Block::Paragraph(rejected_unknown) = &reopened_model.blocks[4] else {
        panic!("rejected unknown-field fallback paragraph")
    };
    assert_eq!(
        rejected_unknown.text(),
        "REJECTED UNKNOWN FILENAME SIBLING FALLBACK"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_reference_index_markers_keep_hidden_instructions() {
    let document = Document::open(&reference_index_marker_note_docx())
        .expect("reference-index marker notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 5);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone marker normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED TA");
    let rejected_generated = note_with_marker(endnotes, "endnote", "REJECTED GENERATED INDEX");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(
            r#"<w:fldSimple w:instr=" XE &quot;Mercury&quot; \t &quot;See planets&quot; \* FirstCap ">"#
        ) && footnote.contains("<w:b/>")
            && footnote.contains("FOOT BEFORE ")
            && footnote.contains("FOOT AFTER"),
        "top-level XE marker missing: {footnote}"
    );
    assert!(!footnote.contains("STALE XE CACHE"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 2, "{endnote}");
    assert!(
        endnote.contains(
            r#"<w:fldSimple w:instr=" TA \l &quot;Case v. Example&quot; \c 1 \* CHARFORMAT ">"#
        ) && endnote
            .contains(r#"<w:fldSimple w:instr=" RD &quot;appendix.docx&quot; \* MERGEFORMAT ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("<w:smallCaps/>")
            && endnote.contains("END BEFORE ")
            && endnote.contains(" BETWEEN ")
            && endnote.contains(" END AFTER"),
        "nested TA/RD markers missing: {endnote}"
    );
    let ta_offset = endnote.find(" TA \\l").expect("TA marker order");
    let rd_offset = endnote.find(" RD &quot;").expect("RD marker order");
    assert!(ta_offset < rd_offset, "marker order changed: {endnote}");
    assert!(!endnote.contains("STALE TA CACHE"), "{endnote}");
    assert!(!endnote.contains("STALE RD CACHE"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_generated] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"TA \l &quot;Broken Case&quot;"#));
    assert!(!endnotes.contains(r#"INDEX \e"#));

    let reopened = Document::open(&converted).expect("converted marker notes reopen");
    assert_eq!(reopened.report().features.fields, 3);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].kind, FieldKind::ReferenceIndex("XE".to_string()));
    assert_eq!(
        fields[0].instruction,
        r#"XE "Mercury" \t "See planets" \* FirstCap"#
    );
    assert_eq!(fields[1].kind, FieldKind::ReferenceIndex("TA".to_string()));
    assert_eq!(
        fields[1].instruction,
        r#"TA \l "Case v. Example" \c 1 \* CHARFORMAT"#
    );
    assert_eq!(fields[2].kind, FieldKind::ReferenceIndex("RD".to_string()));
    assert_eq!(
        fields[2].instruction,
        r#"RD "appendix.docx" \* MERGEFORMAT"#
    );
    assert!(fields.iter().all(|field| field.result.is_empty()));
    assert!(fields
        .iter()
        .all(|field| field.computed_result.as_deref() == Some("")));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-marker fallback paragraph")
    };
    assert_eq!(rejected_malformed.text(), "REJECTED MALFORMED TA FALLBACK");
    let Block::Paragraph(rejected_generated) = &reopened_model.blocks[4] else {
        panic!("rejected generated-index fallback paragraph")
    };
    assert_eq!(
        rejected_generated.text(),
        "REJECTED GENERATED INDEX FALLBACK"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_toc_entry_markers_keep_hidden_instructions() {
    let document = Document::open(&toc_entry_marker_note_docx()).expect("TC marker notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 4);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone TC normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT TC BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END TC BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED TC");
    let rejected_generated = note_with_marker(endnotes, "endnote", "GENERATED TOC BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(
            r#"<w:fldSimple w:instr=" TC &quot;manual foot entry&quot; \f m \l 2 \* Upper ">"#
        ) && footnote.contains("<w:b/>")
            && footnote.contains("FOOT TC BEFORE ")
            && footnote.contains("FOOT TC AFTER"),
        "top-level TC marker missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT TC CACHE"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" TC Manual end entry \f e \l 3 \n ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("END TC BEFORE ")
            && endnote.contains("END TC AFTER"),
        "nested TC marker missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END TC CACHE"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_generated] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"TC \f m \l 2"#));
    assert!(!endnotes.contains(r#"TOC \f e"#));
    assert!(!endnotes.contains("STALE GENERATED TOC"));

    let reopened = Document::open(&converted).expect("converted TC marker notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::TocEntry);
    assert_eq!(
        fields[0].instruction,
        r#"TC "manual foot entry" \f m \l 2 \* Upper"#
    );
    assert_eq!(fields[1].kind, FieldKind::TocEntry);
    assert_eq!(fields[1].instruction, r#"TC Manual end entry \f e \l 3 \n"#);
    assert!(fields.iter().all(|field| field.result.is_empty()));
    assert!(fields
        .iter()
        .all(|field| field.computed_result.as_deref() == Some("")));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-TC fallback paragraph")
    };
    assert_eq!(rejected_malformed.text(), "REJECTED MALFORMED TC FALLBACK");
    let Block::Paragraph(rejected_generated) = &reopened_model.blocks[4] else {
        panic!("rejected generated-TOC fallback paragraph")
    };
    let rejected_generated_text = rejected_generated.text();
    assert!(
        rejected_generated_text.starts_with("GENERATED TOC BEFORE ")
            && rejected_generated_text.ends_with(" GENERATED TOC AFTER")
            && rejected_generated_text.contains("Manual end entry"),
        "generated TOC fallback text changed: {rejected_generated_text:?}"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_symbol_fields_keep_computed_characters_and_instructions() {
    let document = Document::open(&symbol_field_note_docx()).expect("SYMBOL field notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 4);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone SYMBOL normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT SYMBOL BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END SYMBOL BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED SYMBOL");
    let rejected_action = note_with_marker(endnotes, "endnote", "ACTION BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" SYMBOL 183 \f Symbol \s 12 ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains('\u{2022}')
            && footnote.contains("FOOT SYMBOL BEFORE ")
            && footnote.contains(" FOOT SYMBOL AFTER"),
        "top-level SYMBOL field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT SYMBOL"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(
            r#"<w:fldSimple w:instr=" SYMBOL 0x03BB \u \f &quot;Times New Roman&quot; \* Upper ">"#
        ) && endnote.contains("<w:i/>")
            && endnote.contains('\u{039b}')
            && endnote.contains("END SYMBOL BEFORE ")
            && endnote.contains(" END SYMBOL AFTER"),
        "nested SYMBOL field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END SYMBOL"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_action] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"SYMBOL 65 \f"#));
    assert!(!endnotes.contains("MACROBUTTON"));
    assert!(!endnotes.contains("STALE ACTION"));
    assert!(endnotes.contains("Fresh Action"));

    let reopened = Document::open(&converted).expect("converted SYMBOL field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Display("SYMBOL".to_string()));
    assert_eq!(fields[0].instruction, r#"SYMBOL 183 \f Symbol \s 12"#);
    assert_eq!(fields[0].result, "\u{2022}");
    assert_eq!(fields[0].computed_result.as_deref(), Some("\u{2022}"));
    assert_eq!(fields[1].kind, FieldKind::Display("SYMBOL".to_string()));
    assert_eq!(
        fields[1].instruction,
        r#"SYMBOL 0x03BB \u \f "Times New Roman" \* Upper"#
    );
    assert_eq!(fields[1].result, "\u{039b}");
    assert_eq!(fields[1].computed_result.as_deref(), Some("\u{039b}"));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-SYMBOL fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "REJECTED MALFORMED SYMBOL FALLBACK"
    );
    let Block::Paragraph(rejected_action) = &reopened_model.blocks[4] else {
        panic!("rejected action fallback paragraph")
    };
    assert_eq!(
        rejected_action.text(),
        "ACTION BEFORE Fresh Action ACTION AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_quote_fields_keep_computed_text_and_instructions() {
    let document = Document::open(&quote_field_note_docx()).expect("QUOTE field notes open");
    assert_eq!(document.notes().len(), 5, "source note records missing");
    assert_eq!(document.report().features.fields, 5);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone QUOTE normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT QUOTE BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END QUOTE BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "REJECTED MALFORMED");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT QUOTE BEFORE");
    let rejected_fillin = note_with_marker(endnotes, "endnote", "FILLIN BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote
            .contains(r#"<w:fldSimple w:instr=" QUOTE &quot;fresh foot words&quot; \* Caps ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("Fresh Foot Words")
            && footnote.contains("FOOT QUOTE BEFORE ")
            && footnote.contains(" FOOT QUOTE AFTER"),
        "top-level QUOTE field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT QUOTE"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" QUOTE &quot;fresh end words&quot; \* Upper ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("FRESH END WORDS")
            && endnote.contains("END QUOTE BEFORE ")
            && endnote.contains(" END QUOTE AFTER"),
        "nested QUOTE field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END QUOTE"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_split, rejected_fillin] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"QUOTE &quot;broken foot"#));
    assert!(!footnotes.contains(r#"QUOTE &quot;split quote&quot;"#));
    assert!(!endnotes.contains(r#"FILLIN &quot;Client?&quot;"#));
    assert!(!footnotes.contains("STALE SPLIT A"));
    assert!(!footnotes.contains("STALE SPLIT B"));
    assert!(endnotes.contains("CACHED FILLIN"));

    let reopened = Document::open(&converted).expect("converted QUOTE field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("QUOTE".to_string()));
    assert_eq!(fields[0].instruction, r#"QUOTE "fresh foot words" \* Caps"#);
    assert_eq!(fields[0].result, "Fresh Foot Words");
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("Fresh Foot Words")
    );
    assert_eq!(fields[1].kind, FieldKind::Dynamic("QUOTE".to_string()));
    assert_eq!(fields[1].instruction, r#"QUOTE "fresh end words" \* Upper"#);
    assert_eq!(fields[1].result, "FRESH END WORDS");
    assert_eq!(
        fields[1].computed_result.as_deref(),
        Some("FRESH END WORDS")
    );

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 4] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("rejected malformed-QUOTE fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "REJECTED MALFORMED QUOTE FALLBACK"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[3] else {
        panic!("rejected split-result QUOTE fallback paragraph")
    };
    assert_eq!(
        rejected_split.text(),
        "SPLIT QUOTE BEFORE SPLIT QUOTE SPLIT QUOTE AFTER"
    );
    let Block::Paragraph(rejected_fillin) = &reopened_model.blocks[5] else {
        panic!("rejected FILLIN fallback paragraph")
    };
    assert_eq!(
        rejected_fillin.text(),
        "FILLIN BEFORE CACHED FILLIN FILLIN AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_context_free_if_compare_fields_keep_results_and_instructions() {
    let document =
        Document::open(&if_compare_field_note_docx()).expect("IF/COMPARE field notes open");
    assert_eq!(document.notes().len(), 6, "source note records missing");
    assert_eq!(document.report().features.fields, 7);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone IF/COMPARE normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT IF BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END COMPARE BEFORE");
    let rejected_state = note_with_marker(footnotes, "footnote", "STATE IF BEFORE");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT IF BEFORE");
    let rejected_nonfinite = note_with_marker(endnotes, "endnote", "NONFINITE COMPARE BEFORE");
    let rejected_fillin = note_with_marker(endnotes, "endnote", "FILLIN BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(
            r#"<w:fldSimple w:instr=" IF 2 &gt;= 1 &quot;fresh foot if&quot; &quot;bad foot if&quot; \* Caps ">"#
        ) && footnote.contains("<w:b/>")
            && footnote.contains("Fresh Foot If")
            && footnote.contains("FOOT IF BEFORE ")
            && footnote.contains(" FOOT IF AFTER"),
        "top-level context-free IF field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT IF"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(
            r#"<w:fldSimple w:instr=" COMPARE &quot;Alpha-42&quot; = &quot;Alpha-*&quot; ">"#
        ) && endnote.contains("<w:i/>")
            && endnote.contains(">1</w:t>")
            && endnote.contains("END COMPARE BEFORE ")
            && endnote.contains(" END COMPARE AFTER"),
        "nested context-free COMPARE field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END COMPARE"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_state,
        rejected_split,
        rejected_nonfinite,
        rejected_fillin,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"SET GateState &quot;Ready&quot;"#));
    assert!(!footnotes.contains(r#"IF GateState = &quot;Ready&quot;"#));
    assert!(!footnotes.contains(r#"IF 1=1 &quot;split if&quot;"#));
    assert!(!endnotes.contains("COMPARE 1e309"));
    assert!(!endnotes.contains(r#"FILLIN &quot;Client?&quot;"#));
    assert!(!footnotes.contains("STALE SET"));
    assert!(!footnotes.contains("STALE STATE IF"));
    assert!(!footnotes.contains("STALE SPLIT IF A"));
    assert!(!footnotes.contains("STALE SPLIT IF B"));

    let reopened = Document::open(&converted).expect("converted IF/COMPARE field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("IF".to_string()));
    assert_eq!(
        fields[0].instruction,
        r#"IF 2 >= 1 "fresh foot if" "bad foot if" \* Caps"#
    );
    assert_eq!(fields[0].result, "Fresh Foot If");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Fresh Foot If"));
    assert_eq!(fields[1].kind, FieldKind::Dynamic("COMPARE".to_string()));
    assert_eq!(fields[1].instruction, r#"COMPARE "Alpha-42" = "Alpha-*""#);
    assert_eq!(fields[1].result, "1");
    assert_eq!(fields[1].computed_result.as_deref(), Some("1"));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 4] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_state) = &reopened_model.blocks[2] else {
        panic!("SET-backed IF fallback paragraph")
    };
    assert_eq!(
        rejected_state.text(),
        "STATE IF BEFORE source yes STATE IF AFTER"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[3] else {
        panic!("split-result IF fallback paragraph")
    };
    assert_eq!(
        rejected_split.text(),
        "SPLIT IF BEFORE SPLIT IF SPLIT IF AFTER"
    );
    let Block::Paragraph(rejected_nonfinite) = &reopened_model.blocks[5] else {
        panic!("nonfinite COMPARE fallback paragraph")
    };
    assert_eq!(
        rejected_nonfinite.text(),
        "NONFINITE COMPARE BEFORE CACHED NONFINITE COMPARE NONFINITE COMPARE AFTER"
    );
    let Block::Paragraph(rejected_fillin) = &reopened_model.blocks[6] else {
        panic!("FILLIN fallback paragraph")
    };
    assert_eq!(
        rejected_fillin.text(),
        "FILLIN BEFORE CACHED FILLIN FILLIN AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_context_free_formula_fields_keep_results_and_instructions() {
    let document = Document::open(&formula_field_note_docx()).expect("formula field notes open");
    assert_eq!(document.notes().len(), 7, "source note records missing");
    assert_eq!(document.report().features.fields, 9);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone formula normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT FORMULA BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END FORMULA BEFORE");
    let rejected_state = note_with_marker(footnotes, "footnote", "STATE FORMULA BEFORE");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT FORMULA BEFORE");
    let rejected_defined = note_with_marker(footnotes, "footnote", "DEFINED FORMULA BEFORE");
    let rejected_positional = note_with_marker(endnotes, "endnote", "POSITIONAL FORMULA BEFORE");
    let rejected_nonfinite = note_with_marker(endnotes, "endnote", "NONFINITE FORMULA BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" = 10 / 4 \# &quot;0.00&quot; ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains(">2.50</w:t>")
            && footnote.contains("FOOT FORMULA BEFORE ")
            && footnote.contains(" FOOT FORMULA AFTER"),
        "top-level context-free formula field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT FORMULA"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(
            r#"<w:fldSimple w:instr=" = ROUND(AVERAGE(2; 4; 7); 1) \# &quot;0.0&quot; ">"#
        ) && endnote.contains("<w:i/>")
            && endnote.contains(">4.3</w:t>")
            && endnote.contains("END FORMULA BEFORE ")
            && endnote.contains(" END FORMULA AFTER"),
        "nested context-free formula field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END FORMULA"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_state,
        rejected_split,
        rejected_defined,
        rejected_positional,
        rejected_nonfinite,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
    }
    assert!(!footnotes.contains("SET Amount 7"));
    assert!(!footnotes.contains("= Amount + 1"));
    assert!(!footnotes.contains(r#"= 2 + 3 \* OrdText"#));
    assert!(!footnotes.contains("SET Known 5"));
    assert!(!footnotes.contains("= DEFINED(Known)"));
    assert!(!endnotes.contains("= SUM(LEFT)"));
    assert!(!endnotes.contains("= 1e309 + 1"));
    assert!(!footnotes.contains("STALE AMOUNT SET"));
    assert!(!footnotes.contains("STALE STATE FORMULA"));
    assert!(!footnotes.contains("STALE SPLIT FORMULA A"));
    assert!(!footnotes.contains("STALE SPLIT FORMULA B"));
    assert!(!footnotes.contains("STALE KNOWN SET"));

    let reopened = Document::open(&converted).expect("converted formula field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("=".to_string()));
    assert_eq!(fields[0].instruction, r#"= 10 / 4 \# "0.00""#);
    assert_eq!(fields[0].result, "2.50");
    assert_eq!(fields[0].computed_result.as_deref(), Some("2.50"));
    assert_eq!(fields[1].kind, FieldKind::Dynamic("=".to_string()));
    assert_eq!(
        fields[1].instruction,
        r#"= ROUND(AVERAGE(2; 4; 7); 1) \# "0.0""#
    );
    assert_eq!(fields[1].result, "4.3");
    assert_eq!(fields[1].computed_result.as_deref(), Some("4.3"));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 5] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_state) = &reopened_model.blocks[2] else {
        panic!("SET-backed formula fallback paragraph")
    };
    assert_eq!(
        rejected_state.text(),
        "STATE FORMULA BEFORE 8 STATE FORMULA AFTER"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[3] else {
        panic!("split-result formula fallback paragraph")
    };
    assert_eq!(
        rejected_split.text(),
        "SPLIT FORMULA BEFORE fifth SPLIT FORMULA AFTER"
    );
    let Block::Paragraph(rejected_defined) = &reopened_model.blocks[4] else {
        panic!("DEFINED formula fallback paragraph")
    };
    assert_eq!(
        rejected_defined.text(),
        "DEFINED FORMULA BEFORE 1 DEFINED FORMULA AFTER"
    );
    let Block::Paragraph(rejected_positional) = &reopened_model.blocks[6] else {
        panic!("positional formula flattened fallback paragraph")
    };
    assert_eq!(
        rejected_positional.text(),
        "2\t3\tPOSITIONAL FORMULA BEFORE 5 POSITIONAL FORMULA AFTER"
    );
    let Block::Paragraph(rejected_nonfinite) = &reopened_model.blocks[7] else {
        panic!("nonfinite formula fallback paragraph")
    };
    assert_eq!(
        rejected_nonfinite.text(),
        "NONFINITE FORMULA BEFORE CACHED NONFINITE FORMULA NONFINITE FORMULA AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_span_free_table_formulas_keep_results_and_instructions() {
    let document =
        Document::open(&table_formula_field_note_docx()).expect("table-formula notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 6);
    assert!(document
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let source_fields = document.fields();
    for (instruction, result) in [
        (r#"= SUM(LEFT) \# "0.00""#, "5.00"),
        ("= 8 / 2", "4"),
        ("= A1 + 1", "3"),
        ("= A2 + 1", "5"),
        (r#"= SUM(A1:B1) \# "0.0""#, "6.0"),
        ("= SUM(LEFT)", "5"),
    ] {
        let field = source_fields
            .iter()
            .find(|field| field.instruction == instruction)
            .unwrap_or_else(|| panic!("missing source field {instruction:?}"));
        assert_eq!(
            field.computed_result.as_deref(),
            Some(result),
            "{instruction}"
        );
    }

    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone table-formula normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT TABLE FORMULA");
    let endnote = note_with_marker(endnotes, "endnote", "END NESTED TABLE FORMULA");
    let rejected_multi = note_with_marker(footnotes, "footnote", "MULTI TABLE FORMULA");
    let rejected_span = note_with_marker(endnotes, "endnote", "SPAN TABLE FORMULA");

    assert_eq!(footnote.matches("<w:tbl>").count(), 1, "{footnote}");
    assert_eq!(footnote.matches("<w:fldSimple").count(), 2, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" = SUM(LEFT) \# &quot;0.00&quot; ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains(">5.00</w:t>")
            && footnote.contains(r#"<w:fldSimple w:instr=" = 8 / 2 ">"#)
            && footnote.contains("<w:u w:val=\"single\"/>")
            && footnote.contains(">4</w:t>"),
        "top-level table and following scalar formulas missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" = SUM(A1:B1) \# &quot;0.0&quot; ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains(">6.0</w:t>"),
        "nested table formula missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END TABLE FORMULA"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_multi, rejected_span] {
        assert!(!rejected.contains("<w:tbl>"), "{rejected}");
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(
        rejected_multi.contains("MULTI TABLE FORMULA")
            && rejected_multi.contains(">2</w:t>")
            && rejected_multi.contains(">3</w:t>")
            && rejected_multi.contains(">4</w:t>")
            && rejected_multi.contains(">5</w:t>")
            && rejected_multi.matches("<w:tab/>").count() == 2
            && rejected_multi.matches("<w:br/>").count() == 2,
        "{rejected_multi}"
    );
    assert!(
        rejected_span.contains("SPAN TABLE FORMULA")
            && rejected_span.contains(">9</w:t>")
            && rejected_span.contains(">1</w:t>")
            && rejected_span.contains(">2</w:t>")
            && rejected_span.contains(">3</w:t>")
            && rejected_span.contains(">5</w:t>")
            && rejected_span.matches("<w:tab/>").count() == 3
            && rejected_span.matches("<w:br/>").count() == 2,
        "{rejected_span}"
    );
    assert!(!footnotes.contains("STALE MULTI FORMULA"), "{footnotes}");
    assert!(!endnotes.contains("STALE SPAN FORMULA"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted table-formula notes reopen");
    assert_eq!(reopened.report().features.fields, 3);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let reopened_fields = reopened.fields();
    for (instruction, result) in [
        (r#"= SUM(LEFT) \# "0.00""#, "5.00"),
        ("= 8 / 2", "4"),
        (r#"= SUM(A1:B1) \# "0.0""#, "6.0"),
    ] {
        let field = reopened_fields
            .iter()
            .find(|field| field.instruction == instruction)
            .unwrap_or_else(|| panic!("missing reopened field {instruction:?}"));
        assert_eq!(field.kind, FieldKind::Dynamic("=".to_string()));
        assert_eq!(field.result, result, "{instruction}");
        assert_eq!(
            field.computed_result.as_deref(),
            Some(result),
            "{instruction}"
        );
    }

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), 8);
    assert_eq!(normalized_model.blocks.len(), 10);
    for (reopened_index, normalized_index) in [(0, 0), (1, 1), (3, 3), (5, 6)] {
        assert_eq!(
            reopened_model.blocks[reopened_index],
            normalized_model.blocks[normalized_index]
        );
    }
    for (reopened_index, normalized_index) in [(2, 2), (6, 7)] {
        let mut normalized_ownership = reopened_model.blocks[reopened_index].clone();
        clear_simple_field_ownership(&mut normalized_ownership);
        assert_eq!(
            normalized_ownership,
            normalized_model.blocks[normalized_index]
        );
    }
    let Block::Paragraph(rejected_multi) = &reopened_model.blocks[4] else {
        panic!("multiple-formula fallback paragraph")
    };
    assert_eq!(rejected_multi.text(), "MULTI TABLE FORMULA\n2\t3\n4\t5");
    let Block::Paragraph(rejected_span) = &reopened_model.blocks[7] else {
        panic!("spanned-formula fallback paragraph")
    };
    assert_eq!(rejected_span.text(), "SPAN TABLE FORMULA\n9\t1\n2\t3\t5");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_explicit_reset_sequence_fields_keep_results_and_instructions() {
    let document = Document::open(&explicit_reset_sequence_field_note_docx())
        .expect("explicit-reset SEQ notes open");
    assert_eq!(document.notes().len(), 7, "source note records missing");
    assert_eq!(document.report().features.fields, 7);
    assert_eq!(
        document.report().features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
    let source_fields = document.fields();
    for (instruction, result) in [
        (
            "SEQ Invoice \\r 21 \\* DollarText \\* Upper",
            "TWENTY-ONE AND 00/100",
        ),
        ("SEQ Appendix \\r31 \\* Hex", "1F"),
    ] {
        let field = source_fields
            .iter()
            .find(|field| field.instruction == instruction)
            .unwrap_or_else(|| panic!("missing source field {instruction:?}"));
        assert_eq!(field.kind, FieldKind::Sequence);
        assert_eq!(field.computed_result.as_deref(), Some(result));
    }
    for (instruction, result) in [
        ("SEQ Invoice", Some("22")),
        ("SEQ Hidden \\r 4 \\h", Some("")),
        ("SEQ Split \\r 9", Some("9")),
        ("SEQ Figure \\s 1", Some("1")),
        ("SEQ Broken \\r -1", None),
    ] {
        let field = source_fields
            .iter()
            .find(|field| field.instruction == instruction)
            .unwrap_or_else(|| panic!("missing source field {instruction:?}"));
        assert_eq!(field.kind, FieldKind::Sequence);
        assert_eq!(field.computed_result.as_deref(), result, "{instruction}");
    }

    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone explicit-reset SEQ normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT RESET BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END RESET BEFORE");
    let rejected_ordinary = note_with_marker(footnotes, "footnote", "ORDINARY SEQ BEFORE");
    let rejected_hidden = note_with_marker(footnotes, "footnote", "HIDDEN RESET BEFORE[");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT RESET BEFORE");
    let rejected_heading = note_with_marker(endnotes, "endnote", "HEADING RESET BEFORE");
    let rejected_malformed = note_with_marker(endnotes, "endnote", "MALFORMED RESET BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" SEQ Invoice \r 21 \* DollarText \* Upper ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains(">TWENTY-ONE AND 00/100</w:t>")
            && footnote.contains("FOOT RESET BEFORE ")
            && footnote.contains(" FOOT RESET AFTER"),
        "top-level explicit reset missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT RESET"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" SEQ Appendix \r31 \* Hex ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains(">1F</w:t>")
            && endnote.contains("END RESET BEFORE ")
            && endnote.contains(" END RESET AFTER"),
        "nested explicit reset missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END RESET"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_ordinary,
        rejected_hidden,
        rejected_split,
        rejected_heading,
        rejected_malformed,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("STALE ORDINARY SEQ"), "{footnotes}");
    assert!(!footnotes.contains("STALE HIDDEN RESET"), "{footnotes}");
    assert!(!footnotes.contains("STALE SPLIT RESET"), "{footnotes}");
    assert!(!endnotes.contains("STALE HEADING RESET"), "{endnotes}");
    assert!(endnotes.contains("CACHED MALFORMED RESET"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted explicit-reset SEQ notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    for (field, instruction, result) in [
        (
            &fields[0],
            "SEQ Invoice \\r 21 \\* DollarText \\* Upper",
            "TWENTY-ONE AND 00/100",
        ),
        (&fields[1], "SEQ Appendix \\r31 \\* Hex", "1F"),
    ] {
        assert_eq!(field.kind, FieldKind::Sequence);
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.result, result);
        assert_eq!(field.computed_result.as_deref(), Some(result));
    }

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), 8);
    assert_eq!(normalized_model.blocks.len(), 8);
    assert_eq!(reopened_model.blocks[0], normalized_model.blocks[0]);
    for index in [1, 5] {
        let mut normalized_ownership = reopened_model.blocks[index].clone();
        clear_simple_field_ownership(&mut normalized_ownership);
        assert_eq!(normalized_ownership, normalized_model.blocks[index]);
    }
    for (index, expected) in [
        (2, "ORDINARY SEQ BEFORE 22 ORDINARY SEQ AFTER"),
        (3, "HIDDEN RESET BEFORE[]HIDDEN RESET AFTER"),
        (4, "SPLIT RESET BEFORE 9 SPLIT RESET AFTER"),
        (6, "HEADING RESET BEFORE 1 HEADING RESET AFTER"),
        (
            7,
            "MALFORMED RESET BEFORE CACHED MALFORMED RESET MALFORMED RESET AFTER",
        ),
    ] {
        let Block::Paragraph(rejected) = &reopened_model.blocks[index] else {
            panic!("SEQ fallback block {index} was not a paragraph")
        };
        assert_eq!(rejected.text(), expected, "fallback block {index}");
    }
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_explicit_start_listnum_fields_keep_results_and_instructions() {
    let document = Document::open(&explicit_start_listnum_field_note_docx())
        .expect("explicit-start LISTNUM notes open");
    assert_eq!(document.notes().len(), 7, "source note records missing");
    assert_eq!(document.report().features.fields, 7);
    assert_eq!(
        document.report().features.unsupported_field_reasons,
        vec![
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::NoComputedResult,
                count: 1,
            },
            FieldEvaluationReasonCount {
                reason: FieldEvaluationReason::UnsupportedSwitch,
                count: 1,
            },
        ]
    );
    let source_fields = document.fields();
    for (instruction, result) in [
        (
            "LISTNUM NumberDefault \\s 21 \\* DollarText \\* Upper",
            "TWENTY-ONE AND 00/100",
        ),
        ("LISTNUM LegalDefault \\l1 \\s31 \\* Hex", "1F"),
    ] {
        let field = source_fields
            .iter()
            .find(|field| field.instruction == instruction)
            .unwrap_or_else(|| panic!("missing source field {instruction:?}"));
        assert_eq!(field.kind, FieldKind::Numbering("LISTNUM".to_string()));
        assert_eq!(field.computed_result.as_deref(), Some(result));
    }
    for (instruction, kind, result) in [
        (
            "LISTNUM NumberDefault",
            FieldKind::Numbering("LISTNUM".to_string()),
            Some("22"),
        ),
        (
            "AUTONUM",
            FieldKind::Numbering("AUTONUM".to_string()),
            Some("1"),
        ),
        (
            "LISTNUM NumberDefault \\l 2",
            FieldKind::Numbering("LISTNUM".to_string()),
            None,
        ),
        (
            "LISTNUM NumberDefault \\s -1",
            FieldKind::Numbering("LISTNUM".to_string()),
            None,
        ),
        (
            "LISTNUM NumberDefault \\s 9",
            FieldKind::Numbering("LISTNUM".to_string()),
            Some("9"),
        ),
    ] {
        let field = source_fields
            .iter()
            .find(|field| field.instruction == instruction)
            .unwrap_or_else(|| panic!("missing source field {instruction:?}"));
        assert_eq!(field.kind, kind, "{instruction}");
        assert_eq!(field.computed_result.as_deref(), result, "{instruction}");
    }

    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone explicit-start LISTNUM normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT LISTNUM BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END LISTNUM BEFORE");
    let rejected_ordinary = note_with_marker(footnotes, "footnote", "ORDINARY LISTNUM BEFORE");
    let rejected_level = note_with_marker(footnotes, "footnote", "LEVEL TWO BEFORE");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT LISTNUM BEFORE");
    let rejected_autonum = note_with_marker(endnotes, "endnote", "AUTONUM BEFORE");
    let rejected_negative = note_with_marker(endnotes, "endnote", "NEGATIVE START BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(
            r#"<w:fldSimple w:instr=" LISTNUM NumberDefault \s 21 \* DollarText \* Upper ">"#
        ) && footnote.contains("<w:b/>")
            && footnote.contains(">TWENTY-ONE AND 00/100</w:t>")
            && footnote.contains("FOOT LISTNUM BEFORE ")
            && footnote.contains(" FOOT LISTNUM AFTER"),
        "top-level explicit start missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT LISTNUM"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" LISTNUM LegalDefault \l1 \s31 \* Hex ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains(">1F</w:t>")
            && endnote.contains("END LISTNUM BEFORE ")
            && endnote.contains(" END LISTNUM AFTER"),
        "nested explicit start missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END LISTNUM"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_ordinary,
        rejected_level,
        rejected_split,
        rejected_autonum,
        rejected_negative,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("STALE ORDINARY LISTNUM"), "{footnotes}");
    assert!(footnotes.contains("CACHED LEVEL TWO"), "{footnotes}");
    assert!(!footnotes.contains("STALE SPLIT LISTNUM"), "{footnotes}");
    assert!(!endnotes.contains("STALE AUTONUM"), "{endnotes}");
    assert!(endnotes.contains("CACHED NEGATIVE START"), "{endnotes}");

    let reopened =
        Document::open(&converted).expect("converted explicit-start LISTNUM notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    for (field, instruction, result) in [
        (
            &fields[0],
            "LISTNUM NumberDefault \\s 21 \\* DollarText \\* Upper",
            "TWENTY-ONE AND 00/100",
        ),
        (&fields[1], "LISTNUM LegalDefault \\l1 \\s31 \\* Hex", "1F"),
    ] {
        assert_eq!(field.kind, FieldKind::Numbering("LISTNUM".to_string()));
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.result, result);
        assert_eq!(field.computed_result.as_deref(), Some(result));
    }

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), 8);
    assert_eq!(normalized_model.blocks.len(), 8);
    assert_eq!(reopened_model.blocks[0], normalized_model.blocks[0]);
    for index in [1, 5] {
        let mut normalized_ownership = reopened_model.blocks[index].clone();
        clear_simple_field_ownership(&mut normalized_ownership);
        assert_eq!(normalized_ownership, normalized_model.blocks[index]);
    }
    for (index, expected) in [
        (2, "ORDINARY LISTNUM BEFORE 22 ORDINARY LISTNUM AFTER"),
        (3, "LEVEL TWO BEFORE CACHED LEVEL TWO LEVEL TWO AFTER"),
        (4, "SPLIT LISTNUM BEFORE 9 SPLIT LISTNUM AFTER"),
        (6, "AUTONUM BEFORE 1 AUTONUM AFTER"),
        (
            7,
            "NEGATIVE START BEFORE CACHED NEGATIVE START NEGATIVE START AFTER",
        ),
    ] {
        let Block::Paragraph(rejected) = &reopened_model.blocks[index] else {
            panic!("LISTNUM fallback block {index} was not a paragraph")
        };
        assert_eq!(rejected.text(), expected, "fallback block {index}");
    }
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_same_note_style_ref_fields_keep_results_and_instructions() {
    let document =
        Document::open(&same_note_style_ref_field_docx()).expect("same-note STYLEREF notes open");
    assert_eq!(document.notes().len(), 7, "source note records missing");
    assert_eq!(document.report().features.fields, 7);
    assert_eq!(
        document.report().features.unsupported_field_reasons,
        vec![FieldEvaluationReasonCount {
            reason: FieldEvaluationReason::UnsupportedSwitch,
            count: 1,
        }]
    );
    let source_fields = document.fields();
    for (instruction, result) in [
        (r#"STYLEREF "Heading 1" \* Upper"#, Some("FOOT HEADING")),
        ("STYLEREF Heading1", Some("Foot Heading")),
        ("STYLEREF Heading1", Some("Multi Source")),
        ("STYLEREF Heading1", Some("Split Source")),
        (r#"STYLEREF \p Callout \* Upper"#, Some("BELOW")),
        (r#"STYLEREF NumberedBody \n"#, Some("1")),
        (r#"STYLEREF "Broken Style "#, None),
    ] {
        let field = source_fields
            .iter()
            .find(|field| {
                field.instruction == instruction && field.computed_result.as_deref() == result
            })
            .unwrap_or_else(|| panic!("missing source field {instruction:?} with {result:?}"));
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("STYLEREF".to_string())
        );
    }

    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone same-note STYLEREF normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT STYLE BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END STYLE BEFORE");
    let rejected_cross = note_with_marker(footnotes, "footnote", "CROSS STYLE BEFORE");
    let rejected_multi = note_with_marker(footnotes, "footnote", "MULTI SOURCE BEFORE");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT STYLE BEFORE");
    let rejected_number = note_with_marker(endnotes, "endnote", "NUMBERED STYLE BEFORE");
    let rejected_malformed = note_with_marker(endnotes, "endnote", "MALFORMED STYLE BEFORE");

    assert_eq!(footnote.matches("<w:p>").count(), 2, "{footnote}");
    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:pStyle w:val="Heading1"/>"#)
            && footnote
                .contains(r#"<w:fldSimple w:instr=" STYLEREF &quot;Heading 1&quot; \* Upper ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains(">FOOT HEADING</w:t>")
            && footnote.contains("FOOT STYLE BEFORE ")
            && footnote.contains(" FOOT STYLE AFTER"),
        "same-note top-level style ref missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT STYLE"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" STYLEREF \p Callout \* Upper ">"#)
            && endnote.contains(r#"<w:pStyle w:val="Callout"/>"#)
            && endnote.contains("<w:i/>")
            && endnote.contains(">BELOW</w:t>")
            && endnote.contains("END STYLE BEFORE ")
            && endnote.contains(" END STYLE AFTER")
            && endnote.contains(">End Target</w:t>"),
        "same-note nested relative style ref missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END STYLE"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_cross,
        rejected_multi,
        rejected_split,
        rejected_number,
        rejected_malformed,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("STALE CROSS STYLE"), "{footnotes}");
    assert!(!footnotes.contains("STALE MULTI SOURCE"), "{footnotes}");
    assert!(!footnotes.contains("STALE SPLIT STYLE"), "{footnotes}");
    assert!(!endnotes.contains("STALE NUMBERED STYLE"), "{endnotes}");
    assert!(endnotes.contains("CACHED MALFORMED STYLE"), "{endnotes}");

    let reopened = Document::open(&converted).expect("converted same-note STYLEREF notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    for (field, instruction, result) in [
        (
            &fields[0],
            r#"STYLEREF "Heading 1" \* Upper"#,
            "FOOT HEADING",
        ),
        (&fields[1], r#"STYLEREF \p Callout \* Upper"#, "BELOW"),
    ] {
        assert_eq!(
            field.kind,
            FieldKind::DocumentStructure("STYLEREF".to_string())
        );
        assert_eq!(field.instruction, instruction);
        assert_eq!(field.result, result);
        assert_eq!(field.computed_result.as_deref(), Some(result));
    }

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), 9);
    assert_eq!(normalized_model.blocks.len(), 12);
    assert_eq!(reopened_model.blocks[0], normalized_model.blocks[0]);
    assert_eq!(reopened_model.blocks[1], normalized_model.blocks[1]);
    let mut normalized_foot_field = reopened_model.blocks[2].clone();
    clear_simple_field_ownership(&mut normalized_foot_field);
    assert_eq!(normalized_foot_field, normalized_model.blocks[2]);
    let mut normalized_end_field = reopened_model.blocks[6].clone();
    clear_simple_field_ownership(&mut normalized_end_field);
    assert_eq!(normalized_end_field, normalized_model.blocks[8]);
    for (index, expected) in [
        (3, "CROSS STYLE BEFORE Foot Heading CROSS STYLE AFTER"),
        (
            4,
            "Multi Source\nMULTI SOURCE BEFORE Multi Source MULTI SOURCE AFTER",
        ),
        (
            5,
            "Split Source\nSPLIT STYLE BEFORE Split Source SPLIT STYLE AFTER",
        ),
        (
            7,
            "Numbered Source\nNUMBERED STYLE BEFORE 1 NUMBERED STYLE AFTER",
        ),
        (
            8,
            "MALFORMED STYLE BEFORE CACHED MALFORMED STYLE MALFORMED STYLE AFTER",
        ),
    ] {
        let Block::Paragraph(rejected) = &reopened_model.blocks[index] else {
            panic!("STYLEREF fallback block {index} was not a paragraph")
        };
        assert_eq!(rejected.text(), expected, "fallback block {index}");
    }
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_explicit_default_fill_in_fields_keep_results_and_instructions() {
    let document = Document::open(&fill_in_field_note_docx()).expect("FILLIN field notes open");
    assert_eq!(document.notes().len(), 6, "source note records missing");
    assert_eq!(document.report().features.fields, 6);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone FILLIN normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT FILLIN BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END FILLIN BEFORE");
    let rejected_no_default = note_with_marker(footnotes, "footnote", "NO DEFAULT BEFORE");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT FILLIN BEFORE");
    let rejected_malformed = note_with_marker(endnotes, "endnote", "MALFORMED FILLIN BEFORE");
    let rejected_ask = note_with_marker(endnotes, "endnote", "ASK BEFORE[");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(
            r#"<w:fldSimple w:instr=" FILLIN &quot;Client?&quot; \d &quot;fresh foot words&quot; \o \* Caps ">"#
        ) && footnote.contains("<w:b/>")
            && footnote.contains("Fresh Foot Words")
            && footnote.contains("FOOT FILLIN BEFORE ")
            && footnote.contains(" FOOT FILLIN AFTER"),
        "top-level explicit-default FILLIN field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT FILLIN"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(
            r#"<w:fldSimple w:instr=" FILLIN Project display prompt \d Client 42 \* Upper ">"#
        ) && endnote.contains("<w:i/>")
            && endnote.contains("CLIENT 42")
            && endnote.contains("END FILLIN BEFORE ")
            && endnote.contains(" END FILLIN AFTER"),
        "nested explicit-default FILLIN field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END FILLIN"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_no_default,
        rejected_split,
        rejected_malformed,
        rejected_ask,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"FILLIN &quot;No default?&quot;"#));
    assert!(!footnotes.contains(r#"FILLIN &quot;Split?&quot;"#));
    assert!(!endnotes.contains(r#"FILLIN &quot;broken prompt"#));
    assert!(!endnotes.contains("ASK ClientCode"));
    assert!(!footnotes.contains("STALE SPLIT FILLIN A"));
    assert!(!footnotes.contains("STALE SPLIT FILLIN B"));
    assert!(!endnotes.contains("STALE ASK"));
    assert!(footnotes.contains("CACHED NO DEFAULT FILLIN"));
    assert!(endnotes.contains("CACHED MALFORMED FILLIN"));

    let reopened = Document::open(&converted).expect("converted FILLIN field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(
        fields[0].instruction,
        r#"FILLIN "Client?" \d "fresh foot words" \o \* Caps"#
    );
    assert_eq!(fields[0].result, "Fresh Foot Words");
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("Fresh Foot Words")
    );
    assert_eq!(fields[1].kind, FieldKind::Dynamic("FILLIN".to_string()));
    assert_eq!(
        fields[1].instruction,
        r#"FILLIN Project display prompt \d Client 42 \* Upper"#
    );
    assert_eq!(fields[1].result, "CLIENT 42");
    assert_eq!(fields[1].computed_result.as_deref(), Some("CLIENT 42"));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 4] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_no_default) = &reopened_model.blocks[2] else {
        panic!("default-less FILLIN fallback paragraph")
    };
    assert_eq!(
        rejected_no_default.text(),
        "NO DEFAULT BEFORE CACHED NO DEFAULT FILLIN NO DEFAULT AFTER"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[3] else {
        panic!("split-result FILLIN fallback paragraph")
    };
    assert_eq!(
        rejected_split.text(),
        "SPLIT FILLIN BEFORE SPLIT ANSWER SPLIT FILLIN AFTER"
    );
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[5] else {
        panic!("malformed FILLIN fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "MALFORMED FILLIN BEFORE CACHED MALFORMED FILLIN MALFORMED FILLIN AFTER"
    );
    let Block::Paragraph(rejected_ask) = &reopened_model.blocks[6] else {
        panic!("ASK fallback paragraph")
    };
    assert_eq!(rejected_ask.text(), "ASK BEFORE[]ASK AFTER");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_context_free_display_fields_keep_results_and_instructions() {
    let document = Document::open(&display_field_note_docx()).expect("display field notes open");
    assert_eq!(document.notes().len(), 6, "source note records missing");
    assert_eq!(document.report().features.fields, 6);
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone display normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT EQ BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END ADVANCE BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "MALFORMED EQ BEFORE");
    let rejected_split = note_with_marker(footnotes, "footnote", "SPLIT EQ BEFORE");
    let rejected_advance = note_with_marker(endnotes, "endnote", "BAD ADVANCE BEFORE");
    let rejected_action = note_with_marker(endnotes, "endnote", "ACTION BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(
            r#"<w:fldSimple w:instr=" EQ \f( &quot;Alpha, One&quot; , &quot;Beta Two&quot; ) \* Upper ">"#
        ) && footnote.contains("<w:b/>")
            && footnote.contains("ALPHA, ONE/BETA TWO")
            && footnote.contains("FOOT EQ BEFORE ")
            && footnote.contains(" FOOT EQ AFTER"),
        "top-level computed EQ field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT EQ"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" ADVANCE \r&quot;2&quot; \d4 \* MERGEFORMAT ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("END ADVANCE BEFORE ")
            && endnote.contains(" END ADVANCE AFTER"),
        "nested computed ADVANCE field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END ADVANCE"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_malformed,
        rejected_split,
        rejected_advance,
        rejected_action,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"EQ \f(1,"#));
    assert!(!footnotes.contains("STALE SPLIT EQ A"));
    assert!(!footnotes.contains("STALE SPLIT EQ B"));
    assert!(!endnotes.contains(r#"ADVANCE \z 2"#));
    assert!(!endnotes.contains("MACROBUTTON"));
    assert!(!endnotes.contains("STALE ACTION"));
    assert!(footnotes.contains("CACHED MALFORMED EQ"));
    assert!(endnotes.contains("CACHED BAD ADVANCE"));
    assert!(endnotes.contains("Fresh Action"));

    let reopened = Document::open(&converted).expect("converted display field notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Display("EQ".to_string()));
    assert_eq!(
        fields[0].instruction,
        r#"EQ \f( "Alpha, One" , "Beta Two" ) \* Upper"#
    );
    assert_eq!(fields[0].result, "ALPHA, ONE/BETA TWO");
    assert_eq!(
        fields[0].computed_result.as_deref(),
        Some("ALPHA, ONE/BETA TWO")
    );
    assert_eq!(fields[1].kind, FieldKind::Display("ADVANCE".to_string()));
    assert_eq!(fields[1].instruction, r#"ADVANCE \r"2" \d4 \* MERGEFORMAT"#);
    assert_eq!(fields[1].result, "");
    assert_eq!(fields[1].computed_result.as_deref(), Some(""));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 4] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("malformed EQ fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "MALFORMED EQ BEFORE CACHED MALFORMED EQ MALFORMED EQ AFTER"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[3] else {
        panic!("split-result EQ fallback paragraph")
    };
    assert_eq!(rejected_split.text(), "SPLIT EQ BEFORE 1/2 SPLIT EQ AFTER");
    let Block::Paragraph(rejected_advance) = &reopened_model.blocks[5] else {
        panic!("unsupported ADVANCE fallback paragraph")
    };
    assert_eq!(
        rejected_advance.text(),
        "BAD ADVANCE BEFORE CACHED BAD ADVANCE BAD ADVANCE AFTER"
    );
    let Block::Paragraph(rejected_action) = &reopened_model.blocks[6] else {
        panic!("action fallback paragraph")
    };
    assert_eq!(
        rejected_action.text(),
        "ACTION BEFORE Fresh Action ACTION AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_core_and_custom_property_fields_keep_results_and_instructions() {
    let document =
        Document::open(&document_info_field_note_docx()).expect("document-info field notes open");
    assert_eq!(document.notes().len(), 6, "source note records missing");
    assert_eq!(document.report().features.fields, 6);
    assert_eq!(
        document.core_properties().title.as_deref(),
        Some("Quarter Plan")
    );
    assert_eq!(
        document.core_properties().subject.as_deref(),
        Some("Pipeline")
    );
    let source_model = document.model();
    assert_eq!(
        source_model
            .custom_properties
            .get("Client Name")
            .map(String::as_str),
        Some("acme launch")
    );
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone document-info normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let core = std::str::from_utf8(&parts["docProps/core.xml"]).unwrap();
    let custom = std::str::from_utf8(&parts["docProps/custom.xml"]).unwrap();
    assert!(core.contains("<dc:title>Quarter Plan</dc:title>"), "{core}");
    assert!(core.contains("<dc:subject>Pipeline</dc:subject>"), "{core}");
    assert!(custom.contains(r#"name="Client Name""#), "{custom}");
    assert!(
        custom.contains("<vt:lpwstr>acme launch</vt:lpwstr>"),
        "{custom}"
    );
    assert!(!parts.contains_key("docProps/app.xml"));
    assert!(!parts.contains_key("word/settings.xml"));
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT CORE BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END CUSTOM BEFORE");
    let rejected_extended = note_with_marker(footnotes, "footnote", "EXTENDED BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "MALFORMED PROPERTY BEFORE");
    let rejected_variable = note_with_marker(endnotes, "endnote", "VARIABLE BEFORE");
    let rejected_split = note_with_marker(endnotes, "endnote", "SPLIT CORE BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" DOCPROPERTY Subject \* Upper ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("PIPELINE")
            && footnote.contains("FOOT CORE BEFORE ")
            && footnote.contains(" FOOT CORE AFTER"),
        "top-level core-property field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT CORE"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote
            .contains(r#"<w:fldSimple w:instr=" DOCPROPERTY &quot;Client Name&quot; \* Caps ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("Acme Launch")
            && endnote.contains("END CUSTOM BEFORE ")
            && endnote.contains(" END CUSTOM AFTER"),
        "nested custom-property field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END CUSTOM"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [
        rejected_extended,
        rejected_malformed,
        rejected_variable,
        rejected_split,
    ] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("NUMPAGES"));
    assert!(!footnotes.contains(r#"DOCPROPERTY &quot;Broken Name"#));
    assert!(!endnotes.contains("DOCVARIABLE"));
    assert!(!endnotes.contains(r#"TITLE \* Upper"#));
    assert!(!footnotes.contains("STALE EXTENDED"));
    assert!(!endnotes.contains("STALE VARIABLE"));
    assert!(!endnotes.contains("STALE SPLIT CORE A"));
    assert!(!endnotes.contains("STALE SPLIT CORE B"));
    assert!(footnotes.contains("XII"));
    assert!(footnotes.contains("CACHED MALFORMED PROPERTY"));
    assert!(endnotes.contains("ALPHA-42"));
    assert!(endnotes.contains("QUARTER PLAN"));

    let reopened = Document::open(&converted).expect("converted document-info notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].kind,
        FieldKind::DocumentInfo("DOCPROPERTY".to_string())
    );
    assert_eq!(fields[0].instruction, r#"DOCPROPERTY Subject \* Upper"#);
    assert_eq!(fields[0].result, "PIPELINE");
    assert_eq!(fields[0].computed_result.as_deref(), Some("PIPELINE"));
    assert_eq!(
        fields[1].kind,
        FieldKind::DocumentInfo("DOCPROPERTY".to_string())
    );
    assert_eq!(
        fields[1].instruction,
        r#"DOCPROPERTY "Client Name" \* Caps"#
    );
    assert_eq!(fields[1].result, "Acme Launch");
    assert_eq!(fields[1].computed_result.as_deref(), Some("Acme Launch"));
    assert_eq!(
        reopened.core_properties().title.as_deref(),
        Some("Quarter Plan")
    );
    assert_eq!(
        reopened.core_properties().subject.as_deref(),
        Some("Pipeline")
    );
    assert_eq!(
        reopened
            .model()
            .custom_properties
            .get("Client Name")
            .map(String::as_str),
        Some("acme launch")
    );

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 4] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_extended) = &reopened_model.blocks[2] else {
        panic!("extended-property fallback paragraph")
    };
    assert_eq!(
        rejected_extended.text(),
        "EXTENDED BEFORE XII EXTENDED AFTER"
    );
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[3] else {
        panic!("malformed-property fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "MALFORMED PROPERTY BEFORE CACHED MALFORMED PROPERTY MALFORMED PROPERTY AFTER"
    );
    let Block::Paragraph(rejected_variable) = &reopened_model.blocks[5] else {
        panic!("document-variable fallback paragraph")
    };
    assert_eq!(
        rejected_variable.text(),
        "VARIABLE BEFORE ALPHA-42 VARIABLE AFTER"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[6] else {
        panic!("split core-property fallback paragraph")
    };
    assert_eq!(
        rejected_split.text(),
        "SPLIT CORE BEFORE QUARTER PLAN SPLIT CORE AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(standalone.contains_key("docProps/core.xml"));
    assert!(standalone.contains_key("docProps/custom.xml"));
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_revision_number_fields_keep_results_and_instructions() {
    let document =
        Document::open(&revision_number_field_note_docx()).expect("revision field notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 4);
    assert_eq!(
        document.core_properties().revision.as_deref(),
        Some("draft revision")
    );
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone revision normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let core = std::str::from_utf8(&parts["docProps/core.xml"]).unwrap();
    assert!(
        core.contains("<cp:revision>draft revision</cp:revision>"),
        "{core}"
    );
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT REVISION BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END REVISION BEFORE");
    let rejected_malformed = note_with_marker(footnotes, "footnote", "MALFORMED REVISION BEFORE");
    let rejected_split = note_with_marker(endnotes, "endnote", "SPLIT REVISION BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:fldSimple w:instr=" REVNUM \* Caps ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("Draft Revision")
            && footnote.contains("FOOT REVISION BEFORE ")
            && footnote.contains(" FOOT REVISION AFTER"),
        "top-level revision field missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT REVISION"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:fldSimple w:instr=" REVNUM \* Upper ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("DRAFT REVISION")
            && endnote.contains("END REVISION BEFORE ")
            && endnote.contains(" END REVISION AFTER"),
        "nested revision field missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END REVISION"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_malformed, rejected_split] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains(r#"REVNUM \x"#));
    assert!(!endnotes.contains(r#"<w:fldSimple w:instr=" REVNUM ""#));
    assert!(!footnotes.contains("STALE FOOT REVISION"));
    assert!(!endnotes.contains("STALE END REVISION"));
    assert!(!endnotes.contains("STALE SPLIT REVISION A"));
    assert!(!endnotes.contains("STALE SPLIT REVISION B"));
    assert!(footnotes.contains("CACHED MALFORMED REVISION"));
    assert!(endnotes.contains("draft revision"));

    let reopened = Document::open(&converted).expect("converted revision notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].kind,
        FieldKind::DocumentStructure("REVNUM".to_string())
    );
    assert_eq!(fields[0].instruction, r#"REVNUM \* Caps"#);
    assert_eq!(fields[0].result, "Draft Revision");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Draft Revision"));
    assert_eq!(
        fields[1].kind,
        FieldKind::DocumentStructure("REVNUM".to_string())
    );
    assert_eq!(fields[1].instruction, r#"REVNUM \* Upper"#);
    assert_eq!(fields[1].result, "DRAFT REVISION");
    assert_eq!(fields[1].computed_result.as_deref(), Some("DRAFT REVISION"));
    assert_eq!(
        reopened.core_properties().revision.as_deref(),
        Some("draft revision")
    );

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        assert_eq!(reopened_model.blocks[index], normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_malformed) = &reopened_model.blocks[2] else {
        panic!("malformed revision fallback paragraph")
    };
    assert_eq!(
        rejected_malformed.text(),
        "MALFORMED REVISION BEFORE CACHED MALFORMED REVISION MALFORMED REVISION AFTER"
    );
    let Block::Paragraph(rejected_split) = &reopened_model.blocks[4] else {
        panic!("split revision fallback paragraph")
    };
    assert_eq!(
        rejected_split.text(),
        "SPLIT REVISION BEFORE draft revision SPLIT REVISION AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(standalone.contains_key("docProps/core.xml"));
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_local_ref_fields_keep_results_instructions_and_targets() {
    let document =
        Document::open(&local_ref_field_note_docx()).expect("local-ref field notes open");
    assert_eq!(document.notes().len(), 4, "source note records missing");
    assert_eq!(document.report().features.fields, 4);
    let source_fields = document.fields();
    assert_eq!(source_fields.len(), 4);
    assert_eq!(
        source_fields[0].computed_result.as_deref(),
        Some("Alpha Launch")
    );
    assert_eq!(
        source_fields[1].computed_result.as_deref(),
        Some("ALPHA LAUNCH")
    );
    assert_eq!(
        source_fields[2].computed_result.as_deref(),
        Some("$1,250.50")
    );
    assert_eq!(
        source_fields[3].computed_result.as_deref(),
        Some("MULTI TARGET")
    );

    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone local-ref normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    assert!(!parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!parts.contains_key("word/_rels/endnotes.xml.rels"));
    assert!(!footnotes.contains("xmlns:r="), "{footnotes}");
    assert!(!endnotes.contains("xmlns:r="), "{endnotes}");

    let footnote = note_with_marker(footnotes, "footnote", "FOOT REF BEFORE");
    let endnote = note_with_marker(endnotes, "endnote", "END REF BEFORE");
    let rejected_cross = note_with_marker(footnotes, "footnote", "CROSS REF BEFORE");
    let rejected_multi = note_with_marker(endnotes, "endnote", "MULTI REF BEFORE");

    assert_eq!(footnote.matches("<w:fldSimple").count(), 1, "{footnote}");
    assert!(
        footnote.contains(r#"<w:bookmarkStart w:id="0" w:name="FootLocal"/>"#)
            && footnote.contains(r#"<w:bookmarkEnd w:id="0"/>"#)
            && footnote.contains(r#"<w:fldSimple w:instr=" REF FootLocal \h \* Caps ">"#)
            && footnote.contains("<w:b/>")
            && footnote.contains("Alpha Launch")
            && footnote.contains("FOOT REF BEFORE ")
            && footnote.contains(" FOOT REF AFTER"),
        "top-level local REF missing: {footnote}"
    );
    assert!(!footnote.contains("STALE FOOT REF"), "{footnote}");
    assert!(!footnote.contains("w:dirty="), "{footnote}");

    assert_eq!(endnote.matches("<w:tbl>").count(), 2, "{endnote}");
    assert_eq!(endnote.matches("<w:fldSimple").count(), 1, "{endnote}");
    assert!(
        endnote.contains(r#"<w:bookmarkStart w:id="1" w:name="EndAmount"/>"#)
            && endnote.contains(r#"<w:bookmarkEnd w:id="1"/>"#)
            && endnote
                .contains(r#"<w:fldSimple w:instr=" REF EndAmount \# &quot;$#,##0.00&quot; ">"#)
            && endnote.contains("<w:i/>")
            && endnote.contains("$1,250.50")
            && endnote.contains("END REF BEFORE ")
            && endnote.contains(" END REF AFTER"),
        "nested local REF missing: {endnote}"
    );
    assert!(!endnote.contains("STALE END REF"), "{endnote}");
    assert!(!endnote.contains("<w:fldChar"), "{endnote}");
    assert!(!endnote.contains("w:dirty="), "{endnote}");

    for rejected in [rejected_cross, rejected_multi] {
        assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
        assert!(!rejected.contains("<w:fldChar"), "{rejected}");
        assert!(!rejected.contains("<w:bookmark"), "{rejected}");
        assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");
    }
    assert!(!footnotes.contains("STALE CROSS REF"));
    assert!(!endnotes.contains("STALE MULTI REF"));
    assert!(!footnotes.contains(r#"w:name="MultiTarget""#));
    assert!(!endnotes.contains(r#"w:name="MultiTarget""#));

    let reopened = Document::open(&converted).expect("converted local-ref notes reopen");
    assert_eq!(reopened.report().features.fields, 2);
    assert!(reopened
        .report()
        .features
        .unsupported_field_reasons
        .is_empty());
    let fields = reopened.fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].kind, FieldKind::Ref);
    assert_eq!(fields[0].instruction, r#"REF FootLocal \h \* Caps"#);
    assert_eq!(fields[0].result, "Alpha Launch");
    assert_eq!(fields[0].computed_result.as_deref(), Some("Alpha Launch"));
    assert_eq!(fields[1].kind, FieldKind::Ref);
    assert_eq!(fields[1].instruction, r##"REF EndAmount \# "$#,##0.00""##);
    assert_eq!(fields[1].result, "$1,250.50");
    assert_eq!(fields[1].computed_result.as_deref(), Some("$1,250.50"));

    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    for index in [0, 1, 3] {
        let mut normalized_ownership = reopened_model.blocks[index].clone();
        clear_simple_field_ownership(&mut normalized_ownership);
        assert_eq!(normalized_ownership, normalized_model.blocks[index]);
    }
    let Block::Paragraph(rejected_cross) = &reopened_model.blocks[2] else {
        panic!("cross-note REF fallback paragraph")
    };
    assert_eq!(
        rejected_cross.text(),
        "CROSS REF BEFORE ALPHA LAUNCH CROSS REF AFTER"
    );
    let Block::Paragraph(rejected_multi) = &reopened_model.blocks[4] else {
        panic!("multi-run REF fallback paragraph")
    };
    assert_eq!(
        rejected_multi.text(),
        "multi target | MULTI REF BEFORE MULTI TARGET MULTI REF AFTER"
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_inline_rasters_keep_media_and_relationship_ownership() {
    let png = tiny_png();
    let document = Document::open(&raster_note_docx()).expect("raster notes open");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone raster normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let document_rels = std::str::from_utf8(&parts["word/_rels/document.xml.rels"]).unwrap();
    assert!(parts.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(parts.contains_key("word/_rels/endnotes.xml.rels"));
    let footnote_rels = std::str::from_utf8(&parts["word/_rels/footnotes.xml.rels"]).unwrap();
    let endnote_rels = std::str::from_utf8(&parts["word/_rels/endnotes.xml.rels"]).unwrap();
    let content_types = std::str::from_utf8(&parts["[Content_Types].xml"]).unwrap();

    let foot_one = note_with_marker(footnotes, "footnote", "FOOT IMAGE ONE");
    let foot_two = note_with_marker(footnotes, "footnote", "FOOT NESTED IMAGE");
    let end_one = note_with_marker(endnotes, "endnote", "END IMAGE ONE");
    let end_two = note_with_marker(endnotes, "endnote", "END NESTED IMAGE");
    let rejected = note_with_marker(endnotes, "endnote", "REJECTED IMAGE");
    for namespace in ["xmlns:r=", "xmlns:wp=", "xmlns:a=", "xmlns:pic="] {
        assert!(footnotes.contains(namespace), "{footnotes}");
        assert!(endnotes.contains(namespace), "{endnotes}");
    }
    assert!(foot_one.contains(r#"<w:hyperlink r:id="rId1">"#));
    assert!(foot_one.contains(r#"r:embed="rId2""#));
    assert!(foot_one.contains(r#"descr="Foot &lt;one&gt;""#));
    assert!(foot_one.contains(r#"<wp:extent cx="19050" cy="28575"/>"#));
    assert!(foot_two.contains(r#"r:embed="rId3""#));
    assert_eq!(foot_two.matches("<w:tbl>").count(), 2, "{foot_two}");
    assert!(end_one.contains(r#"r:embed="rId1""#));
    assert!(end_one.contains(r#"<a:xfrm rot="5400000">"#));
    assert!(end_two.contains(r#"<w:hyperlink r:id="rId2">"#));
    assert!(end_two.contains(r#"r:embed="rId3""#));
    assert_eq!(end_two.matches("<w:tbl>").count(), 2, "{end_two}");
    assert!(!rejected.contains("<w:drawing>"), "{rejected}");
    assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
    assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");

    assert!(document_rels.contains(r#"Target="media/image1.png""#));
    for target in ["image2.png", "image3.png", "image4.png", "image5.png"] {
        assert!(!document_rels.contains(target), "{document_rels}");
    }
    assert!(footnote_rels.contains(r#"Id="rId1""#));
    assert!(footnote_rels.contains(r#"Target="https://example.com/foot-image""#));
    assert!(footnote_rels.contains(r#"Id="rId2""#));
    assert!(footnote_rels.contains(r#"Target="media/image2.png""#));
    assert!(footnote_rels.contains(r#"Id="rId3""#));
    assert!(footnote_rels.contains(r#"Target="media/image4.png""#));
    assert!(endnote_rels.contains(r#"Id="rId1""#));
    assert!(endnote_rels.contains(r#"Target="media/image3.png""#));
    assert!(endnote_rels.contains(r#"Id="rId2""#));
    assert!(endnote_rels.contains(r#"Target="https://example.com/end-image""#));
    assert!(endnote_rels.contains(r#"Id="rId3""#));
    assert!(endnote_rels.contains(r#"Target="media/image5.png""#));
    assert!(!endnote_rels.contains("rejected"));
    assert!(content_types.contains(r#"Extension="png" ContentType="image/png""#));

    let media = parts
        .iter()
        .filter(|(name, _)| name.starts_with("word/media/"))
        .collect::<Vec<_>>();
    assert_eq!(media.len(), 5, "{:?}", parts.keys().collect::<Vec<_>>());
    for index in 1..=5 {
        assert_eq!(parts[&format!("word/media/image{index}.png")], png);
    }
    assert!(!parts.contains_key("word/media/image6.png"));

    let reopened = Document::open(&converted).expect("converted raster notes reopen");
    let reopened_model = reopened.model();
    assert_eq!(reopened_model.blocks.len(), normalized_model.blocks.len());
    assert_eq!(
        &reopened_model.blocks[..reopened_model.blocks.len() - 1],
        &normalized_model.blocks[..normalized_model.blocks.len() - 1]
    );
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}

#[test]
fn opened_docx_note_modeled_charts_keep_package_and_relationship_ownership() {
    let document = Document::open(&chart_note_docx()).expect("modeled chart notes open");
    let source_model = document.model();
    let standalone_bytes = rwml::write_docx(&source_model);
    let normalized_model = Document::open(&standalone_bytes)
        .expect("standalone chart normalization reopens")
        .model();
    let converted = document.to_docx();
    assert_eq!(converted, document.to_docx(), "conversion is deterministic");
    assert_eq!(document.model(), source_model);

    let parts = unzip_parts(&converted);
    let footnotes = std::str::from_utf8(&parts["word/footnotes.xml"]).unwrap();
    let endnotes = std::str::from_utf8(&parts["word/endnotes.xml"]).unwrap();
    let document_rels = std::str::from_utf8(&parts["word/_rels/document.xml.rels"]).unwrap();
    let footnote_rels = std::str::from_utf8(&parts["word/_rels/footnotes.xml.rels"]).unwrap();
    let endnote_rels = std::str::from_utf8(&parts["word/_rels/endnotes.xml.rels"]).unwrap();
    let content_types = std::str::from_utf8(&parts["[Content_Types].xml"]).unwrap();

    let foot_one = note_with_marker(footnotes, "footnote", "FOOT CHART ONE");
    let foot_two = note_with_marker(footnotes, "footnote", "FOOT NESTED CHART");
    let end_one = note_with_marker(endnotes, "endnote", "END CHART ONE");
    let end_two = note_with_marker(endnotes, "endnote", "END NESTED CHART");
    let rejected = note_with_marker(endnotes, "endnote", "REJECTED CHART");
    for namespace in ["xmlns:r=", "xmlns:wp=", "xmlns:a=", "xmlns:c="] {
        assert!(footnotes.contains(namespace), "{footnotes}");
    }
    assert!(!footnotes.contains("xmlns:cx="), "{footnotes}");
    for namespace in ["xmlns:r=", "xmlns:wp=", "xmlns:a=", "xmlns:cx="] {
        assert!(endnotes.contains(namespace), "{endnotes}");
    }
    assert!(!endnotes.contains("xmlns:c="), "{endnotes}");
    assert!(foot_one.contains(r#"<c:chart r:id="rId1"/>"#));
    assert!(foot_one.contains(r#"descr="Foot &lt;chart&gt;""#));
    assert!(foot_one.contains(r#"<wp:extent cx="3810000" cy="2286000"/>"#));
    assert!(foot_two.contains(r#"<c:chart r:id="rId2"/>"#));
    assert_eq!(foot_two.matches("<w:tbl>").count(), 2, "{foot_two}");
    assert!(end_one.contains(r#"<cx:chart r:id="rId1"/>"#));
    assert!(end_two.contains(r#"<cx:chart r:id="rId2"/>"#));
    assert_eq!(end_two.matches("<w:tbl>").count(), 2, "{end_two}");
    assert!(!rejected.contains("<w:drawing>"), "{rejected}");
    assert!(!rejected.contains("<w:fldSimple"), "{rejected}");
    assert_eq!(rejected.matches("<w:p>").count(), 1, "{rejected}");

    assert!(document_rels.contains(r#"Target="charts/chart1.xml""#));
    for target in ["chart2.xml", "chartEx3.xml", "chart4.xml", "chartEx5.xml"] {
        assert!(!document_rels.contains(target), "{document_rels}");
    }
    assert!(footnote_rels.contains(r#"Id="rId1""#));
    assert!(footnote_rels.contains(r#"Target="charts/chart2.xml""#));
    assert!(footnote_rels.contains(r#"Id="rId2""#));
    assert!(footnote_rels.contains(r#"Target="charts/chart4.xml""#));
    assert!(endnote_rels.contains(r#"Id="rId1""#));
    assert!(endnote_rels.contains(r#"Target="charts/chartEx3.xml""#));
    assert!(endnote_rels.contains(r#"Id="rId2""#));
    assert!(endnote_rels.contains(r#"Target="charts/chartEx5.xml""#));
    assert!(!endnote_rels.contains("chart6"));

    let chart_paths = parts
        .keys()
        .filter(|name| name.starts_with("word/charts/chart") && name.ends_with(".xml"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        chart_paths,
        [
            "word/charts/chart1.xml",
            "word/charts/chart2.xml",
            "word/charts/chart4.xml",
            "word/charts/chartEx3.xml",
            "word/charts/chartEx5.xml",
        ]
    );
    assert!(!parts.contains_key("word/charts/chart6.xml"));
    for index in [1, 2, 4] {
        let chart = std::str::from_utf8(&parts[&format!("word/charts/chart{index}.xml")]).unwrap();
        assert!(chart.contains("<a:t>Core source</a:t>"), "{chart}");
        assert!(chart.contains("<c:v>Core values</c:v>"), "{chart}");
        assert!(chart.contains("<c:v>18</c:v>"), "{chart}");
        assert!(parts.contains_key(&format!("word/charts/_rels/chart{index}.xml.rels")));
        assert!(parts.contains_key(&format!(
            "word/embeddings/Microsoft_Excel_Worksheet{index}.xlsx"
        )));
    }
    for index in [3, 5] {
        let chart =
            std::str::from_utf8(&parts[&format!("word/charts/chartEx{index}.xml")]).unwrap();
        assert!(chart.contains(r#"layoutId="waterfall""#), "{chart}");
        assert!(chart.contains("<a:t>Extended source</a:t>"), "{chart}");
        assert!(chart.contains("<cx:v>Extended values</cx:v>"), "{chart}");
        assert!(chart.contains("<cx:v>-7</cx:v>"), "{chart}");
        assert!(!parts.contains_key(&format!("word/charts/_rels/chartEx{index}.xml.rels")));
        assert!(!parts.contains_key(&format!(
            "word/embeddings/Microsoft_Excel_Worksheet{index}.xlsx"
        )));
    }
    assert_eq!(
        content_types
            .matches("application/vnd.openxmlformats-officedocument.drawingml.chart+xml")
            .count(),
        3
    );
    assert_eq!(
        content_types
            .matches("application/vnd.ms-office.chartex+xml")
            .count(),
        2
    );

    let reopened = Document::open(&converted).expect("converted chart notes reopen");
    let reopened_model = reopened.model();
    assert_eq!(
        reopened_model.blocks.len() + 1,
        normalized_model.blocks.len()
    );
    let supported_end = reopened_model.blocks.len() - 1;
    assert_eq!(
        &reopened_model.blocks[..supported_end],
        &normalized_model.blocks[..supported_end]
    );
    let Block::Paragraph(rejected_fallback) = &reopened_model.blocks[supported_end] else {
        panic!("rejected chart note fallback paragraph")
    };
    assert_eq!(rejected_fallback.text(), "REJECTED CHART UNSUPPORTED FIELD");
    assert_eq!(reopened.to_docx(), converted);

    let standalone = unzip_parts(&standalone_bytes);
    assert!(!standalone.contains_key("word/footnotes.xml"));
    assert!(!standalone.contains_key("word/endnotes.xml"));
    assert!(!standalone.contains_key("word/_rels/footnotes.xml.rels"));
    assert!(!standalone.contains_key("word/_rels/endnotes.xml.rels"));
}
