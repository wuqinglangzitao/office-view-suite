#![cfg(feature = "docx")]

use std::io::Write;

use rwml::{Block, CellMargins, Color, Document};

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

fn content_types(include_styles: bool) -> String {
    let styles = if include_styles {
        r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>{styles}</Types>"#
    )
}

fn document_rels(include_styles: bool) -> &'static str {
    if include_styles {
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#
    } else {
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#
    }
}

fn style_inheritance_docx() -> Vec<u8> {
    let content_types = content_types(true);
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:docDefaults>
                    <w:rPrDefault><w:rPr><w:b w:val="0"/><w:sz w:val="20"/></w:rPr></w:rPrDefault>
                </w:docDefaults>
                <w:style w:type="paragraph" w:styleId="Normal">
                    <w:name w:val="Normal"/>
                    <w:rPr><w:color w:val="336699"/></w:rPr>
                </w:style>
                <w:style w:type="paragraph" w:styleId="Heading1">
                    <w:name w:val="heading 1"/>
                    <w:basedOn w:val="Normal"/>
                    <w:rPr><w:b/></w:rPr>
                </w:style>
                <w:style w:type="character" w:styleId="Em">
                    <w:name w:val="Em"/>
                    <w:rPr><w:i/></w:rPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p>
                    <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                    <w:r><w:t>plain</w:t></w:r>
                    <w:r><w:rPr><w:rStyle w:val="Em"/></w:rPr><w:t>em</w:t></w:r>
                    <w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>off</w:t></w:r>
                </w:p>
            </w:body></w:document>"#,
        ),
    ])
}

fn no_styles_docx() -> Vec<u8> {
    let content_types = content_types(false);
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>plain</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>direct</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn default_paragraph_style_docx() -> Vec<u8> {
    let content_types = content_types(true);
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:docDefaults>
                    <w:rPrDefault><w:rPr><w:i/></w:rPr></w:rPrDefault>
                    <w:pPrDefault><w:pPr><w:spacing w:before="60"/></w:pPr></w:pPrDefault>
                </w:docDefaults>
                <w:style w:default="true" w:styleId="Normal">
                    <w:pPr>
                        <w:spacing w:before="240" w:after="120" w:line="360"/>
                        <w:ind w:firstLine="200"/>
                        <w:shd w:val="clear" w:fill="112233"/>
                        <w:pageBreakBefore/>
                    </w:pPr>
                    <w:rPr><w:b/></w:rPr>
                </w:style>
                <w:style w:type="paragraph" w:styleId="Explicit">
                    <w:pPr><w:spacing w:before="360"/></w:pPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>implicit</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Explicit"/></w:pPr><w:r><w:t>explicit</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Missing"/></w:pPr><w:r><w:t>missing</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="  "/></w:pPr><w:r><w:t>empty</w:t></w:r></w:p>
            </w:body></w:document>"#,
        ),
    ])
}

fn paragraph_layout_inheritance_docx() -> Vec<u8> {
    let content_types = content_types(true);
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:docDefaults><w:pPrDefault><w:pPr>
                    <w:spacing w:before="120" w:after="240" w:line="300"/>
                    <w:ind w:firstLine="200"/>
                    <w:shd w:val="clear" w:fill="112233"/>
                    <w:pageBreakBefore/>
                </w:pPr></w:pPrDefault></w:docDefaults>
                <w:style w:type="paragraph" w:styleId="Base">
                    <w:pPr>
                        <w:spacing w:before="0" w:after="360" w:line="360" w:lineRule="auto"/>
                        <w:ind w:hanging="240"/>
                        <w:shd w:val="clear" w:fill="445566"/>
                    </w:pPr>
                </w:style>
                <w:style w:type="paragraph" w:styleId="Derived">
                    <w:basedOn w:val="Base"/>
                    <w:pPr>
                        <w:spacing w:before="480"/>
                        <w:ind w:firstLine="320"/>
                        <w:shd w:val="clear" w:fill="778899"/>
                    </w:pPr>
                </w:style>
                <w:style w:type="paragraph" w:styleId="Blocked">
                    <w:basedOn w:val="Base"/>
                    <w:pPr>
                        <w:spacing w:beforeLines="100" w:afterLines="100"
                                   w:line="480" w:lineRule="exact"/>
                        <w:ind w:hangingChars="100"/>
                        <w:shd w:val="pct20" w:fill="ABCDEF"/>
                        <w:pageBreakBefore w:val="0"/>
                    </w:pPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>defaults</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Base"/></w:pPr><w:r><w:t>base</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Derived"/></w:pPr><w:r><w:t>derived</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Derived"/>
                    <w:spacing w:before="0" w:after="0" w:line="240"/>
                    <w:ind w:hanging="0"/>
                    <w:shd w:val="clear" w:fill="000000"/>
                    <w:pageBreakBefore w:val="0"/>
                </w:pPr><w:r><w:t>direct</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Blocked"/></w:pPr><w:r><w:t>style blocked</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Derived"/>
                    <w:spacing w:beforeLines="100" w:afterLines="100"
                               w:line="480" w:lineRule="atLeast"/>
                    <w:ind w:firstLineChars="100"/>
                    <w:shd w:themeFill="accent1" w:fill="DDEEFF"/>
                    <w:pageBreakBefore w:val="off"/>
                </w:pPr><w:r><w:t>direct blocked</w:t></w:r></w:p>
            </w:body></w:document>"#,
        ),
    ])
}

fn paragraph_layout_cascade_docx() -> Vec<u8> {
    let content_types = content_types(true);
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="paragraph" w:styleId="Automatic"><w:pPr>
                    <w:spacing w:before="120" w:beforeAutospacing="1"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="Exact"><w:pPr>
                    <w:spacing w:line="360" w:lineRule="exact"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="CharacterIndent"><w:pPr>
                    <w:ind w:hangingChars="100"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="LineUnits"><w:pPr>
                    <w:spacing w:before="120" w:beforeLines="100"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="FirstLineChars"><w:pPr>
                    <w:ind w:firstLineChars="100"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="HangingChars"><w:pPr>
                    <w:ind w:hangingChars="100"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="Break"><w:pPr>
                    <w:pageBreakBefore/>
                </w:pPr></w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:pStyle w:val="Automatic"/>
                    <w:spacing w:before="480"/>
                </w:pPr><w:r><w:t>automatic retained</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Automatic"/>
                    <w:spacing w:beforeAutospacing="0"/>
                </w:pPr><w:r><w:t>automatic disabled</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Exact"/>
                    <w:spacing w:line="480"/>
                </w:pPr><w:r><w:t>implicit auto</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Exact"/>
                    <w:spacing w:lineRule="auto"/>
                </w:pPr><w:r><w:t>auto override</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="CharacterIndent"/>
                    <w:ind w:firstLine="240"/>
                </w:pPr><w:r><w:t>character unit retained</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="Break"/>
                    <w:pageBreakBefore w:val="TRUE"/>
                </w:pPr><w:r><w:t>invalid toggle</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="LineUnits"/>
                    <w:spacing w:beforeLines="-0"/>
                </w:pPr><w:r><w:t>line units cleared</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="FirstLineChars"/>
                    <w:ind w:firstLineChars="-0" w:firstLine="240"/>
                </w:pPr><w:r><w:t>character units cleared</w:t></w:r></w:p>
                <w:p><w:pPr><w:spacing w:afterLines="0"/></w:pPr>
                    <w:r><w:t>standalone line unit zero</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="HangingChars"/>
                    <w:ind w:hangingChars="+0" w:hanging="240"/>
                </w:pPr><w:r><w:t>hanging character units cleared</w:t></w:r></w:p>
            </w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "render")]
fn paragraph_layout_render_docx(direct_page_break: &str) -> Vec<u8> {
    let content_types = content_types(true);
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:r><w:t>first page</w:t></w:r></w:p>
            <w:p><w:pPr><w:pStyle w:val="Rendered"/>{direct_page_break}</w:pPr>
                <w:r><w:t>styled second paragraph wraps across its inherited line box</w:t></w:r>
            </w:p>
            <w:sectPr><w:pgSz w:w="6000" w:h="6000"/>
                <w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="paragraph" w:styleId="Rendered"><w:pPr>
                    <w:spacing w:before="240" w:after="120" w:line="360"/>
                    <w:ind w:firstLine="240"/>
                    <w:shd w:val="clear" w:fill="DDEEFF"/>
                    <w:pageBreakBefore/>
                </w:pPr></w:style>
            </w:styles>"#,
        ),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn paragraph_layout_render_variant_docx(style_properties: &str) -> Vec<u8> {
    let content_types = content_types(true);
    let styles_xml = format!(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:style w:type="paragraph" w:styleId="Rendered"><w:pPr>
                {style_properties}
            </w:pPr></w:style>
        </w:styles>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        ("word/styles.xml", &styles_xml),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:pStyle w:val="Rendered"/></w:pPr>
                    <w:r><w:t>styled paragraph wraps over several words for stable layout evidence</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>following paragraph</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="6000"/>
                    <w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
                </w:sectPr>
            </w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "render")]
fn table_cell_absolute_line_spacing_docx(paragraph_properties: &str, nested: bool) -> Vec<u8> {
    let content_types = content_types(false);
    let paragraph = format!(
        r#"<w:p><w:pPr>{paragraph_properties}</w:pPr>
            <w:r><w:t>table-cell absolute spacing</w:t></w:r></w:p>"#
    );
    let cell_content = if nested {
        format!(r#"<w:tbl><w:tr><w:tc>{paragraph}</w:tc></w:tr></w:tbl><w:p/>"#)
    } else {
        paragraph
    };
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl><w:tr><w:tc>{cell_content}</w:tc></w:tr></w:tbl>
            <w:p><w:r><w:t>following paragraph</w:t></w:r></w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="6000"/>
                <w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn table_pagination_docx(
    table_properties: &str,
    direct_row_props: &str,
    styles_xml: &str,
) -> Vec<u8> {
    let content_types = content_types(true);
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:r><w:t>seed</w:t></w:r></w:p>
            <w:tbl><w:tblPr>{table_properties}</w:tblPr><w:tr>{direct_row_props}<w:tc>
                <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>one</w:t></w:r></w:p>
                <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>two</w:t></w:r></w:p>
                <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>three</w:t></w:r></w:p>
                <w:p><w:r><w:t>four</w:t></w:r></w:p>
                <w:p><w:r><w:t>five</w:t></w:r></w:p>
            </w:tc></w:tr></w:tbl>
            <w:p><w:r><w:t>after</w:t></w:r></w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2400"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        ("word/styles.xml", styles_xml),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn table_style_pagination_docx(direct_row_props: &str) -> Vec<u8> {
    table_pagination_docx(
        r#"<w:tblStyle w:val="KeepDerived"/>"#,
        direct_row_props,
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:style w:type="table" w:styleId="KeepBase"><w:trPr><w:cantSplit/></w:trPr></w:style>
            <w:style w:type="table" w:styleId="KeepDerived"><w:basedOn w:val="KeepBase"/></w:style>
        </w:styles>"#,
    )
}

#[cfg(feature = "render")]
fn conditional_table_style_pagination_docx(direct_row_props: &str) -> Vec<u8> {
    table_pagination_docx(
        r#"<w:tblStyle w:val="ConditionalKeep"/><w:tblLook w:firstRow="1"/>"#,
        direct_row_props,
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:style w:type="table" w:styleId="ConditionalKeep">
                <w:tblStylePr w:type="firstRow"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
            </w:style>
        </w:styles>"#,
    )
}

#[cfg(feature = "render")]
fn horizontal_band_table_style_pagination_docx(direct_row_props: &str) -> Vec<u8> {
    table_pagination_docx(
        r#"<w:tblStyle w:val="BandedKeep"/>"#,
        direct_row_props,
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:style w:type="table" w:styleId="BandedKeep">
                <w:tblPr><w:tblStyleRowBandSize w:val="1"/></w:tblPr>
                <w:tblStylePr w:type="band1Horz">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
            </w:style>
        </w:styles>"#,
    )
}

#[cfg(feature = "render")]
fn first_column_table_style_pagination_docx(direct_row_props: &str) -> Vec<u8> {
    table_pagination_docx(
        r#"<w:tblStyle w:val="FirstColumnKeep"/><w:tblLook w:firstColumn="1" w:noHBand="1" w:noVBand="1"/>"#,
        direct_row_props,
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:style w:type="table" w:styleId="FirstColumnKeep">
                <w:tblStylePr w:type="firstCol">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
            </w:style>
        </w:styles>"#,
    )
}

#[cfg(feature = "render")]
fn conditional_cell_presentation_render_docx(presentation: &str) -> Vec<u8> {
    let content_types = content_types(true);
    let styles_xml = format!(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:style w:type="table" w:styleId="ConditionalVisual">
                <w:tblStylePr w:type="firstCol">{presentation}</w:tblStylePr>
            </w:style>
        </w:styles>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        ("word/styles.xml", &styles_xml),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr>
                    <w:tblStyle w:val="ConditionalVisual"/><w:tblLook w:firstColumn="1" w:noHBand="1" w:noVBand="1"/>
                </w:tblPr><w:tr><w:tc><w:p><w:r>
                    <w:t>line one</w:t><w:br/><w:t>line two</w:t><w:br/><w:t>line three</w:t>
                </w:r></w:p></w:tc></w:tr></w:tbl>
                <w:sectPr><w:pgSz w:w="4400" w:h="1800"/>
                    <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
                </w:sectPr>
            </w:body></w:document>"#,
        ),
    ])
}

#[cfg(feature = "render")]
fn table_cell_tab_render_docx(cell_tabs: &str, nested: bool) -> Vec<u8> {
    let content_types = content_types(false);
    let cell_body = if nested {
        format!(
            r#"<w:p><w:r><w:t>outer</w:t></w:r></w:p>
            <w:tbl><w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid>
                <w:tr><w:tc><w:p><w:pPr>{cell_tabs}</w:pPr>
                    <w:r><w:t>lead</w:t><w:tab/><w:t>tail</w:t></w:r>
                </w:p></w:tc></w:tr>
            </w:tbl>"#
        )
    } else {
        format!(
            r#"<w:p><w:pPr>{cell_tabs}</w:pPr>
                <w:r><w:t>lead</w:t><w:tab/><w:t>tail</w:t></w:r>
            </w:p>"#
        )
    };
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl><w:tblPr><w:tblW w:w="3600" w:type="dxa"/></w:tblPr>
                <w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid>
                <w:tr><w:tc>{cell_body}</w:tc></w:tr>
            </w:tbl>
            <w:sectPr><w:pgSz w:w="4400" w:h="2600"/>
                <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn default_tab_stop_render_docx(default_tab_stop_twips: Option<u32>) -> Vec<u8> {
    let content_types = content_types(false);
    let settings = default_tab_stop_twips.map_or_else(String::new, |twips| {
        format!(
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="{twips}"/></w:settings>"#
        )
    });
    let parts = [
        ("[Content_Types].xml", content_types.as_str()),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>lead</w:t><w:tab/><w:t>tail</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="2600"/><w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/></w:sectPr>
            </w:body></w:document>"#,
        ),
    ];
    let mut out = docx_fixture(&parts);
    if !settings.is_empty() {
        let mut with_settings = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut with_settings);
            let mut zip = zip::ZipWriter::new(cursor);
            let opt = zip::write::SimpleFileOptions::default();
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&out)).unwrap();
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).unwrap();
                zip.start_file(entry.name(), opt).unwrap();
                std::io::copy(&mut entry, &mut zip).unwrap();
            }
            zip.start_file("word/settings.xml", opt).unwrap();
            zip.write_all(settings.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        out = with_settings;
    }
    out
}

#[cfg(feature = "render")]
fn aligned_tab_render_docx(jc: &str, stop_twips: Option<u32>) -> Vec<u8> {
    let content_types = content_types(false);
    let tabs = stop_twips.map_or_else(String::new, |twips| {
        format!(r#"<w:tabs><w:tab w:val="left" w:pos="{twips}"/></w:tabs>"#)
    });
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:pPr><w:jc w:val="{jc}"/>{tabs}</w:pPr>
                <w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r>
            </w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2600"/>
                <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn leader_tab_render_docx(tabs: &str) -> Vec<u8> {
    let content_types = content_types(false);
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:pPr>{tabs}</w:pPr>
                <w:r><w:t>A</w:t><w:tab/><w:t>B</w:t><w:tab/><w:t>C</w:t></w:r>
            </w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2600"/>
                <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn rtl_leader_tab_render_docx(tabs: &str) -> Vec<u8> {
    let content_types = content_types(false);
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:pPr><w:bidi/><w:jc w:val="right"/>{tabs}</w:pPr>
                <w:r><w:rPr><w:rtl/></w:rPr><w:t>א</w:t><w:tab/><w:t>ב</w:t><w:tab/><w:t>ג</w:t></w:r>
            </w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2600"/>
                <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn rtl_tab_render_docx(tab_val: &str, text: &str, stop_twips: Option<u32>) -> Vec<u8> {
    let content_types = content_types(false);
    let tabs = stop_twips.map_or_else(String::new, |twips| {
        format!(r#"<w:tabs><w:tab w:val="{tab_val}" w:pos="{twips}"/></w:tabs>"#)
    });
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:pPr><w:bidi/><w:jc w:val="right"/>{tabs}</w:pPr>
                <w:r><w:rPr><w:rtl/></w:rPr><w:t>א</w:t><w:tab/><w:t>{text}</w:t></w:r>
            </w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2600"/>
                <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[cfg(feature = "render")]
fn rtl_table_cell_tab_render_docx(tab_val: &str, text: &str, stop_twips: Option<u32>) -> Vec<u8> {
    let content_types = content_types(false);
    let tabs = stop_twips.map_or_else(String::new, |twips| {
        format!(r#"<w:tabs><w:tab w:val="{tab_val}" w:pos="{twips}"/></w:tabs>"#)
    });
    let document_xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl><w:tblPr><w:tblW w:w="3600" w:type="dxa"/></w:tblPr>
                <w:tblGrid><w:gridCol w:w="3600"/></w:tblGrid>
                <w:tr><w:tc><w:p><w:pPr><w:bidi/><w:jc w:val="right"/>{tabs}</w:pPr>
                    <w:r><w:rPr><w:rtl/></w:rPr><w:t>א</w:t><w:tab/><w:t>{text}</w:t></w:r>
                </w:p></w:tc></w:tr>
            </w:tbl>
            <w:sectPr><w:pgSz w:w="4400" w:h="2600"/>
                <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
            </w:sectPr>
        </w:body></w:document>"#
    );
    docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(false)),
        ("word/document.xml", &document_xml),
    ])
}

#[test]
fn docx_run_props_resolve_docdefaults_paragraph_and_character_styles() {
    let doc = Document::open(&style_inheritance_docx()).expect("fixture opens");
    let model = doc.model();
    let [Block::Paragraph(paragraph)] = model.blocks.as_slice() else {
        panic!("expected one paragraph");
    };
    assert_eq!(paragraph.text(), "plainemoff");
    let [plain, em, off] = paragraph.runs.as_slice() else {
        panic!("expected three runs");
    };

    assert!(plain.props.bold);
    assert!(!plain.props.italic);
    assert_eq!(plain.props.color, Some(Color::rgb(0x33, 0x66, 0x99)));
    assert_eq!(plain.props.size_half_pt, Some(20));

    assert!(em.props.bold);
    assert!(em.props.italic);
    assert_eq!(em.props.color, Some(Color::rgb(0x33, 0x66, 0x99)));
    assert_eq!(em.props.size_half_pt, Some(20));

    assert!(!off.props.bold);
    assert!(!off.props.italic);
    assert_eq!(off.props.color, Some(Color::rgb(0x33, 0x66, 0x99)));
    assert_eq!(off.props.size_half_pt, Some(20));
}

#[test]
fn docx_without_styles_part_keeps_run_defaults_unchanged() {
    let doc = Document::open(&no_styles_docx()).expect("fixture opens");
    let model = doc.model();
    let [Block::Paragraph(paragraph)] = model.blocks.as_slice() else {
        panic!("expected one paragraph");
    };
    let [plain, direct] = paragraph.runs.as_slice() else {
        panic!("expected two runs");
    };

    assert_eq!(plain.text, "plain");
    assert_eq!(plain.props, Default::default());
    assert_eq!(direct.text, "direct");
    assert!(direct.props.italic);
    assert!(!direct.props.bold);
    assert_eq!(direct.props.size_half_pt, None);
}

#[test]
fn docx_default_paragraph_style_applies_only_to_unstyled_paragraphs() {
    let doc = Document::open(&default_paragraph_style_docx()).expect("fixture opens");
    let model = doc.model();
    let paragraphs = model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 4);

    let implicit = paragraphs[0];
    assert_eq!(implicit.props.spacing.before_pt, Some(12.0));
    assert_eq!(implicit.props.spacing.after_pt, Some(6.0));
    assert_eq!(implicit.props.spacing.line_pct, Some(1.5));
    assert_eq!(implicit.props.indent.first_line_pt, Some(10.0));
    assert_eq!(implicit.props.shading, Some(Color::rgb(0x11, 0x22, 0x33)));
    assert!(implicit.props.page_break_before);
    assert!(implicit.runs[0].props.bold);
    assert!(implicit.runs[0].props.italic);

    let explicit = paragraphs[1];
    assert_eq!(explicit.props.spacing.before_pt, Some(18.0));
    assert_eq!(explicit.props.spacing.after_pt, None);
    assert_eq!(explicit.props.spacing.line_pct, None);
    assert!(!explicit.props.page_break_before);
    assert!(!explicit.runs[0].props.bold);
    assert!(explicit.runs[0].props.italic);

    let missing = paragraphs[2];
    assert_eq!(missing.props.spacing.before_pt, Some(3.0));
    assert_eq!(missing.props.spacing.after_pt, None);
    assert!(!missing.props.page_break_before);
    assert!(!missing.runs[0].props.bold);
    assert!(missing.runs[0].props.italic);

    let empty = paragraphs[3];
    assert_eq!(empty.props.spacing.before_pt, Some(3.0));
    assert_eq!(empty.props.style_id, None);
    assert!(!empty.props.page_break_before);
    assert!(!empty.runs[0].props.bold);
    assert!(empty.runs[0].props.italic);
}

#[test]
fn docx_paragraph_layout_resolves_defaults_styles_and_direct_overrides() {
    let doc = Document::open(&paragraph_layout_inheritance_docx()).expect("fixture opens");
    let model = doc.model();
    let paragraphs = model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 6);

    let defaults = paragraphs[0];
    assert_eq!(defaults.props.spacing.before_pt, Some(6.0));
    assert_eq!(defaults.props.spacing.after_pt, Some(12.0));
    assert_eq!(defaults.props.spacing.line_pct, Some(1.25));
    assert_eq!(defaults.props.indent.first_line_pt, Some(10.0));
    assert_eq!(defaults.props.indent.hanging_pt, None);
    assert_eq!(defaults.props.shading, Some(Color::rgb(0x11, 0x22, 0x33)));
    assert!(defaults.props.page_break_before);

    let base = paragraphs[1];
    assert_eq!(base.props.spacing.before_pt, Some(0.0));
    assert_eq!(base.props.spacing.after_pt, Some(18.0));
    assert_eq!(base.props.spacing.line_pct, Some(1.5));
    assert_eq!(base.props.indent.first_line_pt, None);
    assert_eq!(base.props.indent.hanging_pt, Some(12.0));
    assert_eq!(base.props.shading, Some(Color::rgb(0x44, 0x55, 0x66)));
    assert!(base.props.page_break_before);

    let derived = paragraphs[2];
    assert_eq!(derived.props.spacing.before_pt, Some(24.0));
    assert_eq!(derived.props.spacing.after_pt, Some(18.0));
    assert_eq!(derived.props.spacing.line_pct, Some(1.5));
    assert_eq!(derived.props.indent.first_line_pt, Some(16.0));
    assert_eq!(derived.props.indent.hanging_pt, None);
    assert_eq!(derived.props.shading, Some(Color::rgb(0x77, 0x88, 0x99)));
    assert!(derived.props.page_break_before);

    let direct = paragraphs[3];
    assert_eq!(direct.props.spacing.before_pt, Some(0.0));
    assert_eq!(direct.props.spacing.after_pt, Some(0.0));
    assert_eq!(direct.props.spacing.line_pct, Some(1.0));
    assert_eq!(direct.props.indent.first_line_pt, None);
    assert_eq!(direct.props.indent.hanging_pt, Some(0.0));
    assert_eq!(direct.props.shading, Some(Color::rgb(0, 0, 0)));
    assert!(!direct.props.page_break_before);

    for blocked in [&paragraphs[4], &paragraphs[5]] {
        assert_eq!(blocked.props.spacing.before_pt, None);
        assert_eq!(blocked.props.spacing.after_pt, None);
        assert_eq!(blocked.props.spacing.line_pct, None);
        assert_eq!(blocked.props.indent.first_line_pt, None);
        assert_eq!(blocked.props.indent.hanging_pt, None);
        assert_eq!(blocked.props.shading, None);
        assert!(!blocked.props.page_break_before);
    }
}

#[test]
fn docx_paragraph_layout_cascades_dependent_direct_attributes() {
    let doc = Document::open(&paragraph_layout_cascade_docx()).expect("fixture opens");
    let model = doc.model();
    let paragraphs = model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 10);

    assert_eq!(paragraphs[0].props.spacing.before_pt, None);
    assert_eq!(paragraphs[1].props.spacing.before_pt, Some(6.0));
    assert_eq!(paragraphs[2].props.spacing.line_pct, Some(2.0));
    assert_eq!(paragraphs[3].props.spacing.line_pct, Some(1.5));
    assert_eq!(paragraphs[4].props.indent.first_line_pt, None);
    assert_eq!(paragraphs[4].props.indent.hanging_pt, None);
    assert!(!paragraphs[5].props.page_break_before);
    assert_eq!(paragraphs[6].props.spacing.before_pt, Some(6.0));
    assert_eq!(paragraphs[7].props.indent.first_line_pt, Some(12.0));
    assert_eq!(paragraphs[7].props.indent.hanging_pt, None);
    assert_eq!(paragraphs[8].props.spacing.after_pt, Some(0.0));
    assert_eq!(paragraphs[9].props.indent.first_line_pt, None);
    assert_eq!(paragraphs[9].props.indent.hanging_pt, Some(12.0));
}

#[test]
fn style_derived_paragraph_layout_survives_fresh_docx_conversion() {
    let source = Document::open(&paragraph_layout_inheritance_docx()).expect("fixture opens");
    let source_model = source.model();
    let converted = rwml::write_docx(&source_model);
    let reopened = Document::open(&converted).expect("converted document reopens");
    let reopened_model = reopened.model();
    let source_paragraphs = source_model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    let reopened_paragraphs = reopened_model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(source_model.blocks.len(), 6);
    assert_eq!(reopened_model.blocks.len(), 6);
    assert_eq!(source_paragraphs.len(), source_model.blocks.len());
    assert_eq!(reopened_paragraphs.len(), reopened_model.blocks.len());
    for (source, reopened) in source_paragraphs.iter().zip(&reopened_paragraphs) {
        assert_eq!(reopened.props.spacing, source.props.spacing);
        assert_eq!(reopened.props.indent, source.props.indent);
        assert_eq!(reopened.props.shading, source.props.shading);
        assert_eq!(
            reopened.props.page_break_before,
            source.props.page_break_before
        );
    }
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_honors_style_layout_and_direct_page_break_off() {
    let inherited =
        Document::open(&paragraph_layout_render_docx("")).expect("styled fixture opens");
    let direct_off = Document::open(&paragraph_layout_render_docx(
        r#"<w:pageBreakBefore w:val="off"/>"#,
    ))
    .expect("direct-off fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let inherited_layout = inherited
        .layout_pages_with_fonts(&fonts)
        .expect("inherited layout succeeds");
    let direct_off_layout = direct_off
        .layout_pages_with_fonts(&fonts)
        .expect("direct-off layout succeeds");
    assert_eq!((inherited_layout.pages, direct_off_layout.pages), (2, 1));

    let first_pdf = inherited.to_pdf_with_fonts(&fonts);
    let second_pdf = inherited.to_pdf_with_fonts(&fonts);
    assert!(first_pdf.starts_with(b"%PDF-"));
    assert_eq!(first_pdf, second_pdf);
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_each_style_layout_value() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let render = |properties: &str| {
        Document::open(&paragraph_layout_render_variant_docx(properties))
            .expect("variant opens")
            .to_pdf_with_fonts(&fonts)
    };
    let baseline = render("");
    assert_eq!(baseline, render(""));

    for (name, properties) in [
        ("before spacing", r#"<w:spacing w:before="240"/>"#),
        ("after spacing", r#"<w:spacing w:after="240"/>"#),
        (
            "proportional line spacing",
            r#"<w:spacing w:line="480" w:lineRule="auto"/>"#,
        ),
        (
            "exact line spacing",
            r#"<w:spacing w:line="800" w:lineRule="exact"/>"#,
        ),
        (
            "minimum line spacing",
            r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#,
        ),
        ("first-line indent", r#"<w:ind w:firstLine="360"/>"#),
        ("flat shading", r#"<w:shd w:val="clear" w:fill="DDEEFF"/>"#),
    ] {
        let rendered = render(properties);
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, baseline, "{name} must affect PDF output");
        assert_eq!(rendered, render(properties), "{name} must be deterministic");
    }
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_direct_and_nested_table_cell_absolute_line_spacing() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    let render = |properties: &str, nested: bool| {
        Document::open(&table_cell_absolute_line_spacing_docx(properties, nested))
            .expect("table-cell line-spacing fixture opens")
            .to_pdf_with_fonts(&fonts)
    };

    for (name, properties, nested) in [
        (
            "direct exact spacing",
            r#"<w:spacing w:line="100" w:lineRule="exact"/>"#,
            false,
        ),
        (
            "direct minimum spacing",
            r#"<w:spacing w:line="800" w:lineRule="atLeast"/>"#,
            false,
        ),
        (
            "nested exact spacing",
            r#"<w:spacing w:line="800" w:lineRule="exact"/>"#,
            true,
        ),
    ] {
        let baseline = render("", nested);
        let rendered = render(properties, nested);
        assert!(rendered.starts_with(b"%PDF-"), "{name}");
        assert_ne!(rendered, baseline, "{name} must affect PDF output");
        assert_eq!(
            rendered,
            render(properties, nested),
            "{name} must be deterministic"
        );
    }
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_honors_inherited_table_style_cant_split_and_direct_off() {
    let inherited = Document::open(&table_style_pagination_docx("")).expect("fixture opens");
    let direct_off = Document::open(&table_style_pagination_docx(
        r#"<w:trPr><w:cantSplit w:val="off"/></w:trPr>"#,
    ))
    .expect("fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let inherited_pages = inherited
        .layout_pages_with_fonts(&fonts)
        .expect("inherited table style lays out")
        .pages;
    let direct_off_pages = direct_off
        .layout_pages_with_fonts(&fonts)
        .expect("direct override lays out")
        .pages;

    assert_eq!(
        (inherited_pages, direct_off_pages),
        (3, 2),
        "base table style keeps the row together while direct off restores splitting"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_honors_first_row_conditional_table_style_cant_split() {
    let conditional =
        Document::open(&conditional_table_style_pagination_docx("")).expect("fixture opens");
    let direct_off = Document::open(&conditional_table_style_pagination_docx(
        r#"<w:trPr><w:cantSplit w:val="off"/></w:trPr>"#,
    ))
    .expect("fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let conditional_pages = conditional
        .layout_pages_with_fonts(&fonts)
        .expect("conditional table style lays out")
        .pages;
    let direct_off_pages = direct_off
        .layout_pages_with_fonts(&fonts)
        .expect("direct override lays out")
        .pages;

    assert_eq!(
        (conditional_pages, direct_off_pages),
        (3, 2),
        "the selected first-row style keeps the row together while direct off restores splitting"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_honors_horizontal_table_style_band_cant_split() {
    let banded =
        Document::open(&horizontal_band_table_style_pagination_docx("")).expect("fixture opens");
    let direct_off = Document::open(&horizontal_band_table_style_pagination_docx(
        r#"<w:trPr><w:cantSplit w:val="off"/></w:trPr>"#,
    ))
    .expect("fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let banded_pages = banded
        .layout_pages_with_fonts(&fonts)
        .expect("banded table style lays out")
        .pages;
    let direct_off_pages = direct_off
        .layout_pages_with_fonts(&fonts)
        .expect("direct override lays out")
        .pages;

    assert_eq!(
        (banded_pages, direct_off_pages),
        (3, 2),
        "the selected horizontal band keeps the row together while direct off restores splitting"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_honors_first_column_table_style_cant_split() {
    let first_column =
        Document::open(&first_column_table_style_pagination_docx("")).expect("fixture opens");
    let direct_off = Document::open(&first_column_table_style_pagination_docx(
        r#"<w:trPr><w:cantSplit w:val="off"/></w:trPr>"#,
    ))
    .expect("fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let first_column_pages = first_column
        .layout_pages_with_fonts(&fonts)
        .expect("first-column table style lays out")
        .pages;
    let direct_off_pages = direct_off
        .layout_pages_with_fonts(&fonts)
        .expect("direct override lays out")
        .pages;

    assert_eq!(
        (first_column_pages, direct_off_pages),
        (3, 2),
        "the selected first-column style keeps the row together while direct off restores splitting"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_consumes_conditional_cell_presentation_deterministically() {
    let baseline = Document::open(&conditional_cell_presentation_render_docx(""))
        .expect("baseline conditional table opens");
    let styled = Document::open(&conditional_cell_presentation_render_docx(
        r#"<w:tcPr>
            <w:tcMar><w:top w:w="400"/><w:start w:w="100"/>
                <w:bottom w:w="400"/><w:end w:w="100"/></w:tcMar>
            <w:shd w:val="clear" w:fill="DDEEFF"/>
            <w:vAlign w:val="bottom"/>
        </w:tcPr>"#,
    ))
    .expect("styled conditional table opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let baseline_layout = baseline
        .layout_pages_with_fonts(&fonts)
        .expect("baseline layout");
    let styled_layout = styled
        .layout_pages_with_fonts(&fonts)
        .expect("styled layout");
    assert_eq!((baseline_layout.pages, styled_layout.pages), (1, 2));
    assert_eq!(
        styled_layout,
        styled
            .layout_pages_with_fonts(&fonts)
            .expect("repeat styled layout")
    );

    let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
    let styled_pdf = styled.to_pdf_with_fonts(&fonts);
    assert!(styled_pdf.starts_with(b"%PDF-"));
    assert_ne!(styled_pdf, baseline_pdf);
    assert_eq!(styled_pdf, styled.to_pdf_with_fonts(&fonts));
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_uses_explicit_tabs_inside_table_cells() {
    let default = Document::open(&table_cell_tab_render_docx("", false))
        .expect("default table-cell tab fixture opens");
    let explicit = Document::open(&table_cell_tab_render_docx(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs>"#,
        false,
    ))
    .expect("explicit table-cell tab fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let default_pdf = default.to_pdf_with_fonts(&fonts);
    let explicit_pdf = explicit.to_pdf_with_fonts(&fonts);
    let model_pdf = rwml::try_render_pdf_with_fonts(&explicit.model(), &fonts)
        .expect("model-only table-cell tab fixture renders");
    assert!(default_pdf.starts_with(b"%PDF-"));
    assert!(explicit_pdf.starts_with(b"%PDF-"));
    assert_ne!(
        default_pdf, explicit_pdf,
        "an explicit tab stop in a table cell must affect deterministic PDF output"
    );
    assert_ne!(
        explicit_pdf, model_pdf,
        "opened-document cell tab sidecar must affect rendering beyond the model"
    );
    assert_eq!(
        explicit_pdf,
        explicit.to_pdf_with_fonts(&fonts),
        "table-cell tab rendering must remain deterministic"
    );

    let nested_default = Document::open(&table_cell_tab_render_docx("", true))
        .expect("default nested table-cell tab fixture opens");
    let nested_explicit = Document::open(&table_cell_tab_render_docx(
        r#"<w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs>"#,
        true,
    ))
    .expect("explicit nested table-cell tab fixture opens");
    let nested_default_pdf = nested_default.to_pdf_with_fonts(&fonts);
    let nested_explicit_pdf = nested_explicit.to_pdf_with_fonts(&fonts);
    assert_ne!(
        nested_default_pdf, nested_explicit_pdf,
        "explicit tab stops must reach recursively nested table cells"
    );
    assert_eq!(
        nested_explicit_pdf,
        nested_explicit.to_pdf_with_fonts(&fonts),
        "nested table-cell tab rendering must remain deterministic"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_uses_settings_default_tab_stop_interval() {
    let default =
        Document::open(&default_tab_stop_render_docx(None)).expect("default-tab fixture opens");
    let configured = Document::open(&default_tab_stop_render_docx(Some(1440)))
        .expect("configured default-tab fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let default_pdf = default.to_pdf_with_fonts(&fonts);
    let configured_pdf = configured.to_pdf_with_fonts(&fonts);
    let model_pdf = rwml::try_render_pdf_with_fonts(&configured.model(), &fonts)
        .expect("model-only configured default-tab fixture renders");
    assert!(default_pdf.starts_with(b"%PDF-"));
    assert!(configured_pdf.starts_with(b"%PDF-"));
    assert_ne!(
        default_pdf, configured_pdf,
        "settings-defined default tab interval must affect deterministic PDF output"
    );
    assert_ne!(
        configured_pdf, model_pdf,
        "opened-document default-tab sidecar must affect rendering beyond the model"
    );
    assert_eq!(
        configured_pdf,
        configured.to_pdf_with_fonts(&fonts),
        "settings-defined default-tab rendering must remain deterministic"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_uses_explicit_tabs_in_non_left_paragraph_alignments() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    for (jc, stop_twips) in [("center", 2000), ("right", 3800), ("both", 2000)] {
        let default = Document::open(&aligned_tab_render_docx(jc, None))
            .expect("default aligned-tab fixture opens");
        let explicit = Document::open(&aligned_tab_render_docx(jc, Some(stop_twips)))
            .expect("explicit aligned-tab fixture opens");

        let default_pdf = default.to_pdf_with_fonts(&fonts);
        let explicit_pdf = explicit.to_pdf_with_fonts(&fonts);
        let model_pdf = rwml::try_render_pdf_with_fonts(&explicit.model(), &fonts)
            .expect("model-only aligned-tab fixture renders");
        assert!(default_pdf.starts_with(b"%PDF-"));
        assert!(explicit_pdf.starts_with(b"%PDF-"));
        assert_ne!(
            default_pdf, explicit_pdf,
            "explicit {jc} tab stop must affect deterministic PDF output"
        );
        assert_ne!(
            explicit_pdf, model_pdf,
            "opened-document {jc} tab sidecar must affect rendering beyond the model"
        );
        assert_eq!(
            explicit_pdf,
            explicit.to_pdf_with_fonts(&fonts),
            "explicit {jc} tab rendering must remain deterministic"
        );
    }
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_draws_tab_leaders_and_bar_tabs() {
    let plain = Document::open(&leader_tab_render_docx("")).expect("plain tab fixture opens");
    let decorated = Document::open(&leader_tab_render_docx(
        r#"<w:tabs>
            <w:tab w:val="right" w:pos="2000" w:leader="dot"/>
            <w:tab w:val="bar" w:pos="3000"/>
        </w:tabs>"#,
    ))
    .expect("leader and bar tab fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let plain_pdf = plain.to_pdf_with_fonts(&fonts);
    let decorated_pdf = decorated.to_pdf_with_fonts(&fonts);
    let model_pdf = rwml::try_render_pdf_with_fonts(&decorated.model(), &fonts)
        .expect("model-only leader fixture renders");
    assert!(plain_pdf.starts_with(b"%PDF-"));
    assert!(decorated_pdf.starts_with(b"%PDF-"));
    assert_ne!(
        plain_pdf, decorated_pdf,
        "leader and bar tabs must affect PDF paint"
    );
    assert_ne!(
        decorated_pdf, model_pdf,
        "opened-document tab decorations must stay sidecar-only"
    );
    assert_eq!(
        decorated_pdf,
        decorated.to_pdf_with_fonts(&fonts),
        "leader and bar tab rendering must remain deterministic"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_draws_rtl_tab_leaders_and_bar_tabs() {
    let plain =
        Document::open(&rtl_leader_tab_render_docx("")).expect("plain RTL tab fixture opens");
    let decorated = Document::open(&rtl_leader_tab_render_docx(
        r#"<w:tabs>
            <w:tab w:val="left" w:pos="2000" w:leader="dot"/>
            <w:tab w:val="bar" w:pos="3000"/>
        </w:tabs>"#,
    ))
    .expect("RTL leader and bar tab fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let plain_pdf = plain.to_pdf_with_fonts(&fonts);
    let decorated_pdf = decorated.to_pdf_with_fonts(&fonts);
    let model_pdf = rwml::try_render_pdf_with_fonts(&decorated.model(), &fonts)
        .expect("model-only RTL leader fixture renders");
    assert!(plain_pdf.starts_with(b"%PDF-"));
    assert!(decorated_pdf.starts_with(b"%PDF-"));
    assert_ne!(
        plain_pdf, decorated_pdf,
        "RTL leader and bar tabs must affect PDF paint"
    );
    assert_ne!(
        decorated_pdf, model_pdf,
        "opened-document RTL tab decorations must stay sidecar-only"
    );
    assert_eq!(
        decorated_pdf,
        decorated.to_pdf_with_fonts(&fonts),
        "RTL leader and bar tab rendering must remain deterministic"
    );
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_uses_explicit_rtl_center_end_and_decimal_tabs() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    for (tab_val, text) in [("center", "ב"), ("right", "אב"), ("decimal", "12.34")] {
        let default = Document::open(&rtl_tab_render_docx(tab_val, text, None))
            .expect("default RTL-tab fixture opens");
        let explicit = Document::open(&rtl_tab_render_docx(tab_val, text, Some(2000)))
            .expect("explicit RTL-tab fixture opens");

        let default_pdf = default.to_pdf_with_fonts(&fonts);
        let explicit_pdf = explicit.to_pdf_with_fonts(&fonts);
        let model_pdf = rwml::try_render_pdf_with_fonts(&explicit.model(), &fonts)
            .expect("model-only RTL-tab fixture renders");
        assert!(default_pdf.starts_with(b"%PDF-"));
        assert!(explicit_pdf.starts_with(b"%PDF-"));
        assert_ne!(
            default_pdf, explicit_pdf,
            "explicit RTL {tab_val} tab stop must affect deterministic PDF output"
        );
        assert_ne!(
            explicit_pdf, model_pdf,
            "opened-document RTL {tab_val} tab sidecar must affect rendering beyond the model"
        );
        assert_eq!(
            explicit_pdf,
            explicit.to_pdf_with_fonts(&fonts),
            "explicit RTL {tab_val} tab rendering must remain deterministic"
        );
    }
}

#[cfg(feature = "render")]
#[test]
fn opened_docx_render_uses_explicit_rtl_tabs_inside_table_cells() {
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
    for (tab_val, text) in [("center", "ב"), ("right", "אב"), ("decimal", "12.34")] {
        let default = Document::open(&rtl_table_cell_tab_render_docx(tab_val, text, None))
            .expect("default RTL table-cell tab fixture opens");
        let explicit = Document::open(&rtl_table_cell_tab_render_docx(tab_val, text, Some(2000)))
            .expect("explicit RTL table-cell tab fixture opens");

        let default_pdf = default.to_pdf_with_fonts(&fonts);
        let explicit_pdf = explicit.to_pdf_with_fonts(&fonts);
        let model_pdf = rwml::try_render_pdf_with_fonts(&explicit.model(), &fonts)
            .expect("model-only RTL table-cell tab fixture renders");
        assert!(default_pdf.starts_with(b"%PDF-"));
        assert!(explicit_pdf.starts_with(b"%PDF-"));
        assert_ne!(
            default_pdf, explicit_pdf,
            "explicit RTL table-cell {tab_val} tab stop must affect PDF output"
        );
        assert_ne!(
            explicit_pdf, model_pdf,
            "opened-document RTL table-cell {tab_val} sidecar must affect rendering"
        );
        assert_eq!(
            explicit_pdf,
            explicit.to_pdf_with_fonts(&fonts),
            "explicit RTL table-cell {tab_val} rendering must remain deterministic"
        );
    }
}

/// A table style's own `tblCellMar` is the table's cell-margin default, ahead
/// of the schema defaults and behind any direct `tblPr`/`tcMar` declaration.
#[test]
fn table_styles_supply_cell_margin_defaults() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Base">
                    <w:tblPr><w:tblCellMar>
                        <w:top w:w="200" w:type="dxa"/><w:start w:w="300" w:type="dxa"/>
                    </w:tblCellMar></w:tblPr>
                </w:style>
                <w:style w:type="table" w:styleId="Derived">
                    <w:basedOn w:val="Base"/>
                    <w:tblPr><w:tblCellMar><w:bottom w:w="400" w:type="dxa"/></w:tblCellMar></w:tblPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Derived"/></w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>inherited</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr>
                        <w:tblStyle w:val="Derived"/>
                        <w:tblCellMar><w:top w:w="500" w:type="dxa"/></w:tblCellMar>
                    </w:tblPr>
                    <w:tr><w:tc>
                        <w:tcPr><w:tcMar><w:bottom w:w="600" w:type="dxa"/></w:tcMar></w:tcPr>
                        <w:p><w:r><w:t>overridden</w:t></w:r></w:p>
                    </w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr>
                        <w:tblStyle w:val="Derived"/>
                        <w:tblCellMar>
                            <w:top w:w="500" w:type="dxa"/><w:end w:w="550" w:type="dxa"/>
                        </w:tblCellMar>
                    </w:tblPr>
                    <w:tr>
                        <w:tblPrEx><w:tblCellMar><w:top w:w="700" w:type="dxa"/></w:tblCellMar></w:tblPrEx>
                        <w:tc><w:tcPr><w:tcMar><w:bottom w:w="800" w:type="dxa"/></w:tcMar></w:tcPr>
                            <w:p><w:r><w:t>row exception</w:t></w:r></w:p>
                        </w:tc>
                    </w:tr>
                    <w:tr>
                        <w:tblPrEx><w:tblCellMar/></w:tblPrEx>
                        <w:tc><w:p><w:r><w:t>empty row exception</w:t></w:r></w:p></w:tc>
                    </w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("styled table margins .docx opens");
    let tables: Vec<_> = doc
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table.clone()),
            _ => None,
        })
        .collect();

    // The base chain supplies top/leading, the derived style adds bottom, and
    // the untouched side keeps the schema default.
    assert_eq!(
        tables[0].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 200,
            right: 115,
            bottom: 400,
            left: 300,
        })
    );
    // Direct table and cell declarations still win over the style.
    assert_eq!(
        tables[1].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 500,
            right: 115,
            bottom: 600,
            left: 300,
        })
    );
    // A present row exception replaces the direct table margin property. Its
    // omitted sides therefore inherit the style/schema layer, while tcMar is
    // still the final override.
    assert_eq!(
        tables[2].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 700,
            right: 115,
            bottom: 800,
            left: 300,
        })
    );
    assert_eq!(
        tables[2].rows[1].cells[0].margins,
        Some(CellMargins {
            top: 200,
            right: 115,
            bottom: 400,
            left: 300,
        })
    );
}

/// Conditional table-style margins apply on top of the style's own `tblPr`.
#[test]
fn table_style_regions_supply_cell_margins() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Grid">
                    <w:tblPr><w:tblCellMar>
                        <w:top w:w="100" w:type="dxa"/><w:start w:w="200" w:type="dxa"/>
                    </w:tblCellMar></w:tblPr>
                    <w:tblStylePr w:type="wholeTable">
                        <w:tblPr><w:tblCellMar>
                            <w:top w:w="700" w:type="dxa"/><w:bottom w:w="800" w:type="dxa"/>
                        </w:tblCellMar></w:tblPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="firstRow">
                        <w:tblPr><w:tblCellMar><w:top w:w="999" w:type="dxa"/></w:tblCellMar></w:tblPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Grid"/></w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>whole</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("whole-table margin .docx opens");
    let Block::Table(table) = &doc.model().blocks[0] else {
        panic!("table");
    };
    // wholeTable overrides the style's own top and adds bottom. The omitted
    // tblLook uses Word's first-row default, so that later region wins top.
    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 999,
            right: 115,
            bottom: 800,
            left: 200,
        })
    );
}

/// A table style's borders reach the model: Word's built-in grid styles define
/// their borders in the style, not on each table.
#[test]
fn table_styles_supply_table_borders() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Grid">
                    <w:tblPr><w:tblBorders>
                        <w:top w:val="single" w:sz="8" w:color="112233"/>
                        <w:left w:val="single" w:sz="8" w:color="112233"/>
                        <w:bottom w:val="single" w:sz="8" w:color="112233"/>
                        <w:right w:val="single" w:sz="8" w:color="112233"/>
                        <w:insideH w:val="single" w:sz="8" w:color="112233"/>
                        <w:insideV w:val="single" w:sz="8" w:color="112233"/>
                    </w:tblBorders></w:tblPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Grid"/></w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>styled</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr>
                        <w:tblStyle w:val="Grid"/>
                        <w:tblBorders><w:top w:val="single" w:sz="24" w:color="445566"/>
                        <w:left w:val="single" w:sz="24" w:color="445566"/>
                        <w:bottom w:val="single" w:sz="24" w:color="445566"/>
                        <w:right w:val="single" w:sz="24" w:color="445566"/>
                        <w:insideH w:val="single" w:sz="24" w:color="445566"/>
                        <w:insideV w:val="single" w:sz="24" w:color="445566"/></w:tblBorders>
                    </w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>direct</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("styled border .docx opens");
    let tables: Vec<_> = doc
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(tables[0].border_color, Some(Color::rgb(0x11, 0x22, 0x33)));
    assert_eq!(tables[0].border_size_eighths, Some(8));
    // A direct declaration still wins over the style.
    assert_eq!(tables[1].border_color, Some(Color::rgb(0x44, 0x55, 0x66)));
    assert_eq!(tables[1].border_size_eighths, Some(24));
}

/// A table style's own table-level geometry reaches the model when the table
/// declares none of its own.
#[test]
fn table_styles_supply_table_geometry() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Wide">
                    <w:tblPr>
                        <w:tblW w:w="2500" w:type="pct"/>
                        <w:tblInd w:w="360" w:type="dxa"/>
                        <w:jc w:val="center"/>
                    </w:tblPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Wide"/></w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>styled</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr>
                        <w:tblStyle w:val="Wide"/>
                        <w:tblW w:w="5000" w:type="pct"/>
                        <w:tblInd w:w="720" w:type="dxa"/>
                        <w:jc w:val="right"/>
                    </w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>direct</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("styled geometry .docx opens");
    let tables: Vec<_> = doc
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(tables[0].width_pct, Some(0.5));
    assert_eq!(tables[0].indent_twips, Some(360));
    assert_eq!(tables[0].align, Some(rwml::Align::Center));
    // Direct declarations still win.
    assert_eq!(tables[1].width_pct, Some(1.0));
    assert_eq!(tables[1].indent_twips, Some(720));
    assert_eq!(tables[1].align, Some(rwml::Align::Right));
}

/// A table style's whole-table `w:tcPr` supplies cell defaults.
#[test]
fn table_styles_supply_cell_defaults() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Shaded">
                    <w:tblStylePr w:type="wholeTable">
                        <w:tcPr>
                            <w:shd w:val="clear" w:color="auto" w:fill="AABBCC"/>
                            <w:vAlign w:val="center"/>
                        </w:tcPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Shaded"/></w:tblPr>
                    <w:tr>
                        <w:tc><w:p><w:r><w:t>inherits</w:t></w:r></w:p></w:tc>
                        <w:tc>
                            <w:tcPr>
                                <w:shd w:val="clear" w:color="auto" w:fill="DDEEFF"/>
                                <w:vAlign w:val="bottom"/>
                            </w:tcPr>
                            <w:p><w:r><w:t>overrides</w:t></w:r></w:p>
                        </w:tc>
                    </w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("styled cell defaults .docx opens");
    let Block::Table(table) = &doc.model().blocks[0] else {
        panic!("table");
    };
    assert_eq!(
        table.rows[0].cells[0].shading,
        Some(Color::rgb(0xAA, 0xBB, 0xCC))
    );
    assert_eq!(table.rows[0].cells[0].valign, rwml::VCell::Center);
    // A cell's own declarations still win.
    assert_eq!(
        table.rows[0].cells[1].shading,
        Some(Color::rgb(0xDD, 0xEE, 0xFF))
    );
    assert_eq!(table.rows[0].cells[1].valign, rwml::VCell::Bottom);
}

#[test]
fn row_conditional_table_styles_supply_cell_presentation() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="ConditionalCells">
                    <w:tblPr>
                        <w:tblStyleRowBandSize w:val="1"/>
                        <w:tblCellMar><w:start w:w="50" w:type="dxa"/></w:tblCellMar>
                    </w:tblPr>
                    <w:tblStylePr w:type="wholeTable">
                        <w:tblPr><w:tblCellMar><w:top w:w="100" w:type="dxa"/></w:tblCellMar></w:tblPr>
                        <w:tcPr>
                            <w:tcMar><w:top w:w="120" w:type="dxa"/><w:start w:w="110" w:type="dxa"/></w:tcMar>
                            <w:shd w:val="clear" w:fill="010203"/>
                            <w:vAlign w:val="center"/>
                            <w:tcW w:w="1000" w:type="pct"/>
                        </w:tcPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="band1Horz">
                        <w:tblPr><w:tblCellMar><w:top w:w="200" w:type="dxa"/></w:tblCellMar></w:tblPr>
                        <w:tcPr>
                            <w:tcMar><w:top w:w="220" w:type="dxa"/><w:start w:w="210" w:type="dxa"/></w:tcMar>
                            <w:shd w:val="clear" w:fill="112233"/>
                            <w:vAlign w:val="bottom"/>
                            <w:tcW w:w="1500" w:type="pct"/>
                        </w:tcPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="band2Horz">
                        <w:tblPr><w:tblCellMar><w:top w:w="300" w:type="dxa"/></w:tblCellMar></w:tblPr>
                        <w:tcPr>
                            <w:tcMar><w:top w:w="320" w:type="dxa"/><w:start w:w="310" w:type="dxa"/></w:tcMar>
                            <w:shd w:val="clear" w:fill="445566"/>
                            <w:vAlign w:val="top"/>
                            <w:tcW w:w="2000" w:type="pct"/>
                        </w:tcPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        <w:tcMar><w:top w:w="420" w:type="dxa"/><w:bottom w:w="410" w:type="dxa"/></w:tcMar>
                        <w:shd w:val="clear" w:fill="778899"/>
                        <w:vAlign w:val="center"/>
                        <w:tcW w:w="2500" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><w:tcPr>
                        <w:tcMar><w:top w:w="520" w:type="dxa"/><w:bottom w:w="510" w:type="dxa"/></w:tcMar>
                        <w:shd w:val="clear" w:fill="AABBCC"/>
                        <w:vAlign w:val="bottom"/>
                        <w:tcW w:w="3000" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr>
                    <w:tblStyle w:val="ConditionalCells"/>
                    <w:tblLook w:firstRow="1" w:lastRow="1" w:noHBand="0"/>
                </w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>first</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:p><w:r><w:t>band two</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr>
                        <w:tcMar><w:start w:w="710" w:type="dxa"/></w:tcMar>
                        <w:shd w:val="clear" w:fill="F0E0D0"/>
                        <w:vAlign w:val="center"/>
                        <w:tcW w:w="4500" w:type="pct"/>
                    </w:tcPr><w:p><w:r><w:t>direct</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:p><w:r><w:t>last</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("conditional cell presentation .docx opens");
    let Block::Table(table) = &doc.model().blocks[0] else {
        panic!("table");
    };

    let expected = [
        (
            CellMargins {
                top: 420,
                right: 115,
                bottom: 410,
                left: 210,
            },
            Color::rgb(0x77, 0x88, 0x99),
            rwml::VCell::Center,
            0.5,
        ),
        (
            CellMargins {
                top: 320,
                right: 115,
                bottom: 0,
                left: 310,
            },
            Color::rgb(0x44, 0x55, 0x66),
            rwml::VCell::Top,
            0.4,
        ),
        (
            CellMargins {
                top: 220,
                right: 115,
                bottom: 0,
                left: 710,
            },
            Color::rgb(0xF0, 0xE0, 0xD0),
            rwml::VCell::Center,
            0.9,
        ),
        (
            CellMargins {
                top: 520,
                right: 115,
                bottom: 510,
                left: 310,
            },
            Color::rgb(0xAA, 0xBB, 0xCC),
            rwml::VCell::Bottom,
            0.6,
        ),
    ];
    for (row, (margins, shading, valign, width_pct)) in table.rows.iter().zip(expected) {
        let cell = &row.cells[0];
        assert_eq!(cell.margins, Some(margins));
        assert_eq!(cell.shading, Some(shading));
        assert_eq!(cell.valign, valign);
        assert_eq!(cell.width_pct, Some(width_pct));
    }

    let converted = doc.to_docx();
    let reopened = Document::open(&converted).expect("converted conditional cells reopen");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("reopened table");
    };
    for (source_row, reopened_row) in table.rows.iter().zip(&reopened_table.rows) {
        let source = &source_row.cells[0];
        let reopened = &reopened_row.cells[0];
        assert_eq!(reopened.margins, source.margins);
        assert_eq!(reopened.shading, source.shading);
        assert_eq!(reopened.valign, source.valign);
        assert_eq!(reopened.width_pct, source.width_pct);
    }
}

#[test]
fn first_column_conditional_table_style_supplies_cell_presentation() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="ColumnCells">
                    <w:tblStylePr w:type="firstCol"><w:tcPr>
                        <w:tcMar><w:top w:w="123" w:type="dxa"/><w:start w:w="222" w:type="dxa"/></w:tcMar>
                        <w:shd w:val="clear" w:fill="123456"/>
                        <w:vAlign w:val="bottom"/>
                        <w:tcW w:w="1250" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr>
                    <w:tblStyle w:val="ColumnCells"/>
                    <w:tblLook w:firstColumn="1" w:noHBand="1" w:noVBand="1"/>
                </w:tblPr><w:tr>
                    <w:tc><w:p><w:r><w:t>first</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:r><w:t>last</w:t></w:r></w:p></w:tc>
                </w:tr></w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("first-column conditional style opens");
    let Block::Table(table) = &doc.model().blocks[0] else {
        panic!("table");
    };

    assert_eq!(
        table.rows[0].cells[0].shading,
        Some(Color::rgb(0x12, 0x34, 0x56))
    );
    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 123,
            right: 115,
            bottom: 0,
            left: 222,
        })
    );
    assert_eq!(table.rows[0].cells[0].valign, rwml::VCell::Bottom);
    assert_eq!(table.rows[0].cells[0].width_pct, Some(0.25));
    assert_eq!(table.rows[0].cells[1].shading, None);

    let reopened = Document::open(&doc.to_docx()).expect("first-column conversion reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("reopened table");
    };
    let source = &table.rows[0].cells[0];
    let reopened = &reopened_table.rows[0].cells[0];
    assert_eq!(reopened.shading, source.shading);
    assert_eq!(reopened.margins, source.margins);
    assert_eq!(reopened.valign, source.valign);
    assert_eq!(reopened.width_pct, source.width_pct);
    assert_eq!(reopened_table.rows[0].cells[1].shading, None);
}

#[test]
fn direct_cells_can_clear_conditional_shading_and_percentage_width() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Base">
                    <w:tblStylePr w:type="wholeTable"><w:tcPr>
                        <w:shd w:fill="112233"/>
                        <w:tcW w:w="2500" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr><w:tblStyle w:val="Base"/></w:tblPr><w:tr>
                    <w:tc><w:tcPr>
                        <w:shd w:val="nil" w:fill="AABBCC"/>
                        <w:tcW w:w="1440" w:type="dxa"/>
                    </w:tcPr><w:p><w:r><w:t>nil and dxa</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr>
                        <w:shd w:fill="auto"/>
                        <w:tcW w:w="Infinity" w:type="pct"/>
                    </w:tcPr><w:p><w:r><w:t>auto and invalid</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:r><w:t>inherits</w:t></w:r></w:p></w:tc>
                </w:tr></w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("conditional suppression .docx opens");
    let Block::Table(table) = &doc.model().blocks[0] else {
        panic!("table");
    };

    for cell in &table.rows[0].cells[..2] {
        assert_eq!(cell.shading, None);
        assert_eq!(cell.width_pct, None);
    }
    assert_eq!(
        table.rows[0].cells[2].shading,
        Some(Color::rgb(0x11, 0x22, 0x33))
    );
    assert_eq!(table.rows[0].cells[2].width_pct, Some(0.5));

    let reopened = Document::open(&doc.to_docx()).expect("suppression conversion reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("reopened table");
    };
    assert_eq!(reopened_table.rows[0].cells[0].shading, None);
    assert_eq!(reopened_table.rows[0].cells[0].width_pct, None);
}

#[test]
fn conditional_cell_presentation_respects_inheritance_mce_direct_precedence_and_merges() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
                <w:style w:type="table" w:styleId="Base">
                    <w:tblPr>
                        <w:tblStyleRowBandSize w:val="1"/>
                        <w:tblCellMar><w:start w:w="50" w:type="dxa"/></w:tblCellMar>
                    </w:tblPr>
                    <w:tblStylePr w:type="wholeTable">
                        <w:tblPr><w:tblCellMar><w:top w:w="100" w:type="dxa"/></w:tblCellMar></w:tblPr>
                        <w:tcPr>
                            <w:shd w:val="clear" w:fill="101010"/>
                            <w:vAlign w:val="center"/>
                            <w:tcW w:w="1000" w:type="pct"/>
                        </w:tcPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="band1Horz"><w:tcPr>
                        <w:tcMar><w:start w:w="200" w:type="dxa"/></w:tcMar>
                        <w:shd w:val="clear" w:fill="202020"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        <w:tcMar><w:bottom w:w="300" w:type="dxa"/></w:tcMar>
                        <w:shd w:val="clear" w:fill="303030"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="Derived">
                    <w:basedOn w:val="Base"/>
                    <w:tblStylePr w:type="wholeTable"><w:tblPr><w:tblCellMar>
                        <w:top w:w="150" w:type="dxa"/>
                    </w:tblCellMar></w:tblPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstRow">
                        <w:tblPrChange><w:tblPr><w:tblCellMar>
                            <w:top w:w="901" w:type="dxa"/>
                        </w:tblCellMar></w:tblPr></w:tblPrChange>
                        <w:unknown><w:tcPr><w:shd w:fill="BADBAD"/></w:tcPr></w:unknown>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w"><w:tcPr>
                                <w:tcMar><w:end w:w="400" w:type="dxa"/></w:tcMar>
                                <w:shd w:val="clear" w:fill="404040"/>
                                <w:tcPrChange><w:tcPr><w:shd w:fill="EEEEEE"/></w:tcPr></w:tcPrChange>
                            </w:tcPr></mc:Choice>
                            <mc:Fallback><w:tcPr>
                                <w:tcMar><w:end w:w="902" w:type="dxa"/></w:tcMar>
                                <w:shd w:fill="FAFAFA"/>
                            </w:tcPr></mc:Fallback>
                        </mc:AlternateContent>
                        <w:tcPr><w:vAlign w:val="bottom"/></w:tcPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><mc:AlternateContent>
                        <mc:Choice Requires="w"/>
                        <mc:Fallback><w:tcPr><w:shd w:fill="FFFFFF"/></w:tcPr></mc:Fallback>
                    </mc:AlternateContent></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr>
                    <w:tblStyle w:val="Derived"/><w:bidiVisual/>
                    <w:tblCellMar>
                        <w:top w:w="500" w:type="dxa"/><w:start w:w="600" w:type="dxa"/>
                    </w:tblCellMar>
                </w:tblPr>
                    <w:tr>
                        <w:tblPrEx><w:tblCellMar><w:bottom w:w="700" w:type="dxa"/></w:tblCellMar></w:tblPrEx>
                        <w:trPr><w:cnfStyle w:firstRow="1" w:oddHBand="1"/></w:trPr>
                        <w:tc><w:tcPr>
                        <w:vMerge w:val="restart"/>
                        <w:tcMar><w:start w:w="800" w:type="dxa"/></w:tcMar>
                    </w:tcPr><w:p><w:r><w:t>restart</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr>
                        <w:vMerge/>
                        <w:tcMar><w:start w:w="999" w:type="dxa"/></w:tcMar>
                        <w:shd w:fill="FFFFFF"/>
                    </w:tcPr><w:p><w:r><w:t>continuation</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:tbl><w:tblPr>
                    <w:tblStyle w:val="Derived"/>
                    <w:tblLook w:firstRow="1" w:lastRow="1" w:noHBand="1"/>
                </w:tblPr><w:tr><w:tc><w:p><w:r><w:t>one row</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("conditional precedence .docx opens");
    let model = doc.model();
    let tables: Vec<_> = model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .collect();

    let merged = &tables[0].rows[0].cells[0];
    assert_eq!(merged.row_span, 2);
    assert_eq!(merged.text(), "restart");
    assert_eq!(
        merged.margins,
        Some(CellMargins {
            top: 150,
            right: 800,
            bottom: 700,
            left: 400,
        })
    );
    assert_eq!(merged.shading, Some(Color::rgb(0x40, 0x40, 0x40)));
    assert_eq!(merged.valign, rwml::VCell::Bottom);
    assert_eq!(merged.width_pct, Some(0.2));

    let one_row = &tables[1].rows[0].cells[0];
    assert_eq!(
        one_row.margins,
        Some(CellMargins {
            top: 150,
            right: 400,
            bottom: 300,
            left: 50,
        })
    );
    assert_eq!(one_row.shading, Some(Color::rgb(0x40, 0x40, 0x40)));
    assert_eq!(one_row.valign, rwml::VCell::Bottom);
    assert_eq!(one_row.width_pct, Some(0.2));
}

/// A table style's layout algorithm and visual direction reach the model when
/// the table declares neither.
#[test]
fn table_styles_supply_layout_and_direction() {
    let content_types = content_types(true);
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/_rels/document.xml.rels", document_rels(true)),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Rtl">
                    <w:tblPr>
                        <w:tblLayout w:type="fixed"/>
                        <w:bidiVisual/>
                    </w:tblPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Rtl"/></w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>inherits</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr>
                        <w:tblStyle w:val="Rtl"/>
                        <w:tblLayout w:type="autofit"/>
                        <w:bidiVisual w:val="false"/>
                    </w:tblPr>
                    <w:tr><w:tc><w:p><w:r><w:t>overrides</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("styled layout .docx opens");
    let tables: Vec<_> = doc
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table.clone()),
            _ => None,
        })
        .collect();

    assert!(tables[0].fixed_layout);
    assert!(tables[0].bidi_visual);
    // Explicit off values still win over the style.
    assert!(!tables[1].fixed_layout);
    assert!(!tables[1].bidi_visual);
}

/// A paragraph style may declare list membership; paragraphs using it are list
/// items even when they carry no `w:numPr` of their own.
#[test]
fn paragraph_styles_supply_list_membership() {
    let content_types = content_types(true).replace(
        "</Types>",
        r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#,
    );
    let bytes = docx_fixture(&[
        ("[Content_Types].xml", &content_types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rIdNum" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNum w:abstractNumId="0">
                    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
                    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/></w:lvl>
                </w:abstractNum>
                <w:num w:numId="3"><w:abstractNumId w:val="0"/></w:num>
            </w:numbering>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="paragraph" w:styleId="ListItem">
                    <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="3"/></w:numPr></w:pPr>
                </w:style>
            </w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:pStyle w:val="ListItem"/></w:pPr><w:r><w:t>from style</w:t></w:r></w:p>
                <w:p><w:pPr><w:pStyle w:val="ListItem"/>
                    <w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr>
                </w:pPr><w:r><w:t>direct wins</w:t></w:r></w:p>
                <w:p><w:r><w:t>plain</w:t></w:r></w:p>
            </w:body></w:document>"#,
        ),
    ]);
    let doc = Document::open(&bytes).expect("styled list .docx opens");
    let paragraphs: Vec<_> = doc
        .model()
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(p) => Some(p.clone()),
            _ => None,
        })
        .collect();

    let styled = paragraphs[0].props.list.as_ref().expect("style list info");
    assert_eq!(styled.level, 1);
    assert!(!styled.ordered);
    let direct = paragraphs[1].props.list.as_ref().expect("direct list info");
    assert_eq!(direct.level, 0);
    assert!(direct.ordered);
    assert!(paragraphs[2].props.list.is_none());
}
