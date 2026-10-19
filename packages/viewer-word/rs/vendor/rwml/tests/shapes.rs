#![cfg(feature = "docx")]

use std::{fs, io::Write, path::Path};

use rwml::{
    Block, Color, Document, ShapeDistance, ShapeEffectExtent, ShapeExtent, ShapePoint,
    ShapePosition, ShapeWrapping,
};

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

fn public_corpus_docx(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus/public")
        .join(relative);
    fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before anchor </w:t></w:r><w:r><w:drawing><wp:anchor simplePos="0" relativeHeight=" 251659264 " behindDoc="0" layoutInCell="1" locked="0" allowOverlap="1" distT=" 120 " distB="240" distL="360" distR="480"><wp:positionH relativeFrom=" column "><wp:posOffset>91440</wp:posOffset></wp:positionH><wp:positionV relativeFrom=" paragraph "><wp:align>top</wp:align></wp:positionV><wp:extent cx=" 914400 " cy="457200"/><wp:wrapSquare wrapText=" bothSides " distT="9144" distB=" 18288 " distL="27432" distR="36576"/><wp:docPr id=" 7 " name=" Float one " descr=" A floating object "/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Shape</w:t><w:noBreakHyphen/><w:t>body</w:t><w:softHyphen/><w:t>soft</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After anchor</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_symbol_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> anchor </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="17" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="17" name="Symbol float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Shape </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_expanded_anchor_markers_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Lead</w:t><w:tab></w:tab><w:t>Tab</w:t><w:br></w:br><w:t>Line</w:t><w:br w:type="page"></w:br><w:t>Page</w:t><w:noBreakHyphen></w:noBreakHyphen><w:t>Hard</w:t><w:softHyphen></w:softHyphen><w:t>Soft </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="117" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="117" name="Expanded marker float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_expanded_text_markers_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="118" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="118" name="Expanded marker body"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Shape</w:t><w:tab></w:tab><w:t>Tab</w:t><w:br></w:br><w:t>Line</w:t><w:br w:type="page"></w:br><w:t>Page</w:t><w:noBreakHyphen></w:noBreakHyphen><w:t>Hard</w:t><w:softHyphen></w:softHyphen><w:t>Soft</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_computed_simple_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="18" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="18" name="Computed field float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" QUOTE &quot;Fresh shape&quot; "><w:r><w:t>stale shape</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_computed_complex_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="31" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="31" name="Complex field float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh complex shape&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex shape</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn complex_field_direct_text_box_result_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Outer shape&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:drawing><wp:anchor relativeHeight="52" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="52" name="Direct result float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>stale result box</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_nested_complex_field_inside_simple_result_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="39" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="39" name="Nested complex field float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" CUSTOM outer "><w:r><w:t>Prefix </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Inner target&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale inner target</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> Suffix</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_document_info_field_text_docx() -> Vec<u8> {
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
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Quarter Plan</dc:title></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="19" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="19" name="Document info float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" TITLE "><w:r><w:t>stale title</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_revision_number_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="20" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="20" name="Revision float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" REVNUM "><w:r><w:t>stale revision</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_bookmark_formula_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="7" w:name="InvoiceSubtotal"/><w:r><w:t>42</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="21" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="21" name="Formula float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" = InvoiceSubtotal + 8 "><w:r><w:t>stale formula</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_table_formula_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:drawing><wp:anchor relativeHeight="42" behindDoc="0" layoutInCell="1"><wp:extent cx="914400" cy="457200"/><wp:docPr id="42" name="Table formula float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" = SUM(LEFT) "><w:r><w:t>stale shape table formula</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_page_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>stale prior page</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="43" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="43" name="PAGE float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>stale shape page</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_page_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" PAGEREF ShapeTarget \p "><w:r><w:t>stale prior page ref</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:lastRenderedPageBreak/><w:t>Page two lead.</w:t></w:r></w:p><w:p><w:bookmarkStart w:id="44" w:name="ShapeTarget"/><w:r><w:t>Shape target</w:t></w:r><w:bookmarkEnd w:id="44"/></w:p><w:p><w:r><w:drawing><wp:anchor relativeHeight="44" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="44" name="PAGEREF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" PAGEREF ShapeTarget \p "><w:r><w:t>stale shape page ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_section_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale prior section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>Second section </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="45" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="45" name="SECTION float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale shape section</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_anchor_section_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale prior section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>Second section </w:t></w:r><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale anchor section</w:t></w:r></w:fldSimple><w:r><w:t> anchor </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="46" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="46" name="SECTION anchor float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale shape section</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_note_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" NOTEREF FootOne \p "><w:r><w:t>stale prior note ref</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Target note </w:t></w:r><w:bookmarkStart w:id="46" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="46"/></w:p><w:p><w:r><w:t>After note </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="46" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="46" name="NOTEREF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" NOTEREF FootOne \p "><w:r><w:t>stale shape note ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_bookmark_if_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="8" w:name="InvoiceTier"/><w:r><w:t>Gold</w:t></w:r><w:bookmarkEnd w:id="8"/></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="22" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="22" name="IF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" IF InvoiceTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale bookmark if</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_bookmark_merge_control_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="9" w:name="Gate"/><w:r><w:t>Ready</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="23" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="23" name="Merge control float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" NEXTIF Gate = &quot;Ready&quot; "><w:r><w:t>stale bookmark nextif</w:t></w:r></w:fldSimple><w:r><w:t>gate body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_set_backed_if_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="24" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="24" name="SET-backed IF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" SET ClientTier &quot;Gold&quot; "><w:r><w:t>cached shape set</w:t></w:r></w:fldSimple><w:fldSimple w:instr=" IF ClientTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale shape set if</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_complex_set_backed_if_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="38" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="38" name="Complex SET-backed IF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SET ClientTier "Gold" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>cached complex shape set</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:fldSimple w:instr=" IF ClientTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale complex shape set if</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="10" w:name="CaptionText"/><w:r><w:t>Figure 1</w:t></w:r><w:bookmarkEnd w:id="10"/></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="25" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="25" name="REF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" REF CaptionText "><w:r><w:t>stale ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_ref_note_mark_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:bookmarkStart w:id="47" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="47"/></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="47" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="47" name="REF note float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" REF FootOne \f "><w:r><w:t>stale shape note mark ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_relative_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" REF ShapeTarget \p "><w:r><w:t>stale prior relative ref</w:t></w:r></w:fldSimple></w:p><w:p><w:bookmarkStart w:id="48" w:name="ShapeTarget"/><w:r><w:t>Target text</w:t></w:r><w:bookmarkEnd w:id="48"/></w:p><w:p><w:r><w:t>After target </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="48" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="48" name="REF relative float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" REF ShapeTarget \p "><w:r><w:t>stale shape relative ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_direct_relative_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" ShapeDirectTarget \p "><w:r><w:t>stale prior direct relative ref</w:t></w:r></w:fldSimple></w:p><w:p><w:bookmarkStart w:id="50" w:name="ShapeDirectTarget"/><w:r><w:t>Direct target text</w:t></w:r><w:bookmarkEnd w:id="50"/></w:p><w:p><w:r><w:t>After direct target </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="50" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="50" name="Direct REF relative float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" ShapeDirectTarget \p "><w:r><w:t>stale shape direct relative ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_numbered_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="42"/></w:numPr></w:pPr><w:bookmarkStart w:id="49" w:name="ShapeClause"/><w:r><w:t>Numbered clause</w:t></w:r><w:bookmarkEnd w:id="49"/></w:p><w:p><w:fldSimple w:instr=" REF ShapeClause \n "><w:r><w:t>stale body number</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>After clause </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="49" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="49" name="REF number float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" REF ShapeClause \n \p "><w:r><w:t>stale shape number relative</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/numbering.xml",
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="9"><w:lvl w:ilvl="0"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="42"><w:abstractNumId w:val="9"/></w:num></w:numbering>"#,
        ),
    ])
}

fn floating_shape_legacy_form_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Prior A"/><w:listEntry w:val="Prior B"/></w:ddList></w:ffData><w:r><w:t>stale prior option</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="26" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="26" name="Legacy form float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Shape A"/><w:listEntry w:val="Shape B"/></w:ddList></w:ffData><w:r><w:t>stale shape option</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_anchor_legacy_form_after_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Direct A"/><w:listEntry w:val="Direct B"/></w:ddList></w:ffData><w:r><w:t>stale direct option</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="52" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="52" name="Anchor after legacy form float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Shape A"/><w:listEntry w:val="Shape B"/></w:ddList></w:ffData><w:r><w:t>stale shape option</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="After A"/><w:listEntry w:val="After B"/></w:ddList></w:ffData><w:r><w:t>stale after option</w:t></w:r></w:fldSimple><w:r><w:t> after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_text_form_current_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Prior A"/><w:listEntry w:val="Prior B"/></w:ddList></w:ffData><w:r><w:t>stale prior option</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="37" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="37" name="Text form current float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Shape Default"/></w:textInput></w:ffData><w:r><w:t>Shape typed</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Shape A"/><w:listEntry w:val="Shape B"/></w:ddList></w:ffData><w:r><w:t>stale shape option</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_complex_legacy_form_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Prior A"/><w:listEntry w:val="Prior B"/></w:ddList></w:ffData><w:r><w:t>stale prior option</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="35" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="35" name="Complex legacy form float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Shape A"/><w:listEntry w:val="Shape B"/></w:ddList></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMDROPDOWN </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex shape option</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_complex_text_form_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Prior A"/><w:listEntry w:val="Prior B"/></w:ddList></w:ffData><w:r><w:t>stale prior option</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="36" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="36" name="Complex text form float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:textInput><w:default w:val="Shape Default"/></w:textInput></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMTEXT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Shape Typed</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_sequence_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>stale prior seq</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="27" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="27" name="Sequence float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>stale shape seq</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_sequence_heading_reset_field_text_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter One</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" SEQ Figure \s 1 "><w:r><w:t>stale body heading seq</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Shape anchor </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="51" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="51" name="SEQ heading float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" SEQ Figure \s 1 "><w:r><w:t>stale shape heading seq</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_autonum_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Auto </w:t></w:r><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale prior autonum</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="28" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="28" name="Autonum float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Auto </w:t></w:r><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale shape autonum</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_listnum_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>List </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault "><w:r><w:t>stale prior listnum</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="29" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="29" name="Listnum float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>List </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault "><w:r><w:t>stale shape listnum</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_complex_sequence_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Figure </w:t></w:r><w:fldSimple w:instr=" SEQ Figure "><w:r><w:t>stale prior seq</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="32" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="32" name="Complex sequence float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Figure </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SEQ Figure </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex shape seq</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_complex_autonum_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Auto </w:t></w:r><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale prior autonum</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="33" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="33" name="Complex autonum float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Auto </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> AUTONUM </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex shape autonum</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_complex_listnum_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>List </w:t></w:r><w:fldSimple w:instr=" LISTNUM NumberDefault "><w:r><w:t>stale prior listnum</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="34" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="34" name="Complex listnum float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>List </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> LISTNUM NumberDefault </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale complex shape listnum</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_toc_entry_marker_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="30" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="30" name="TOC entry float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" TC &quot;Hidden shape&quot; "><w:r><w:t>stale tc marker</w:t></w:r></w:fldSimple><w:r><w:t>body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_toc_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Risks</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="3"/></w:pPr><w:r><w:t>Excluded Detail</w:t></w:r></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="35" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="35" name="TOC float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" TOC \o &quot;1-2&quot; "><w:r><w:t>stale shape toc</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn floating_shape_style_ref_field_text_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale prior style ref</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Intro Heading</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Shape Heading</w:t></w:r></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="36" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="36" name="STYLEREF float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale shape style ref</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn header_footer_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:p><w:r><w:t>Header before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="77" behindDoc="0"><wp:positionH relativeFrom="page"><wp:posOffset>91440</wp:posOffset></wp:positionH><wp:extent cx="914400" cy="457200"/><wp:docPr id="77" name="Header float" descr="Header shape"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Header shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>after</w:t></w:r></w:p></w:hdr>"#,
        ),
    ])
}

fn note_floating_shape_docx() -> Vec<u8> {
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
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:footnote w:id="1"><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="61"><wp:extent cx="914400" cy="457200"/><wp:docPr id="61" name="Note direct"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Direct shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:del w:id="62"><w:r><w:drawing><wp:anchor relativeHeight="62"><wp:extent cx="914400" cy="457200"/><wp:docPr id="62" name="Note deleted"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Deleted shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r></w:del><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor relativeHeight="63"><wp:extent cx="914400" cy="457200"/><wp:docPr id="63" name="Note choice"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Choice shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:drawing><wp:anchor relativeHeight="64"><wp:extent cx="914400" cy="457200"/><wp:docPr id="64" name="Note fallback"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Fallback shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></mc:Fallback></mc:AlternateContent></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

fn sdt_wrapped_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before block</w:t></w:r></w:p><w:sdt><w:sdtPr></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Wrapped before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="1" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="11" name="Wrapped float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Wrapped shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Wrapped after</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:r><w:t>After block</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn custom_xml_wrapped_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before block</w:t></w:r></w:p><w:customXml w:element="record" w:uri="urn:rwml:test"><w:customXmlPr><w:attr w:name="kind" w:val="fixture"/></w:customXmlPr><w:p><w:r><w:t>Custom before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="4" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="14" name="Custom XML float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Custom XML shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Custom after</w:t></w:r></w:p></w:customXml><w:p><w:r><w:t>After block</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn smart_tag_wrapped_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before block</w:t></w:r></w:p><w:smartTag><w:p><w:r><w:t>Smart before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="2" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="12" name="Smart float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Smart shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Smart after</w:t></w:r></w:p></w:smartTag><w:p><w:r><w:t>After block</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn inserted_wrapped_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before block</w:t></w:r></w:p><w:ins w:id="20" w:author="Editor"><w:p><w:r><w:t>Inserted before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="3" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:docPr id="13" name="Inserted float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Inserted shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Inserted after</w:t></w:r></w:p></w:ins><w:p><w:r><w:t>After block</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn revision_wrapped_floating_shapes_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Direct before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="1"><wp:extent cx="914400" cy="457200"/><wp:docPr id="31" name="Direct float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Direct body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Direct after</w:t></w:r></w:p><w:ins w:id="32" w:author="Editor"><w:p><w:r><w:t>Inserted before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="2"><wp:extent cx="914400" cy="457200"/><wp:docPr id="32" name="Inserted float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Inserted body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Inserted after</w:t></w:r></w:p></w:ins><w:moveTo w:id="33" w:author="Editor"><w:p><w:r><w:t>Moved-to before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="3"><wp:extent cx="914400" cy="457200"/><wp:docPr id="33" name="Moved-to float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Moved-to body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Moved-to after</w:t></w:r></w:p></w:moveTo><w:del w:id="34" w:author="Editor"><w:p><w:r><w:delText>Deleted before </w:delText></w:r><w:r><w:drawing><wp:anchor relativeHeight="4"><wp:extent cx="914400" cy="457200"/><wp:docPr id="34" name="Deleted float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Deleted body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:delText>Deleted after</w:delText></w:r></w:p></w:del><w:moveFrom w:id="35" w:author="Editor"><w:p><w:r><w:delText>Moved-from before </w:delText></w:r><w:r><w:drawing><wp:anchor relativeHeight="5"><wp:extent cx="914400" cy="457200"/><wp:docPr id="35" name="Moved-from float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Moved-from body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:delText>Moved-from after</w:delText></w:r></w:p></w:moveFrom></w:body></w:document>"#,
        ),
    ])
}

fn alternate_content_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Before alternate </w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor relativeHeight="41"><wp:extent cx="914400" cy="457200"/><wp:docPr id="41" name="Choice float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Choice shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:drawing><wp:anchor relativeHeight="42"><wp:extent cx="914400" cy="457200"/><wp:docPr id="42" name="Fallback float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Fallback shape body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>After alternate</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn block_alternate_content_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><mc:AlternateContent><mc:Choice Requires="wps"><w:p><w:r><w:t>Choice before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="71"><wp:extent cx="914400" cy="457200"/><wp:docPr id="71" name="Choice block float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Choice block body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Choice after</w:t></w:r></w:p></mc:Choice><mc:Fallback><w:p><w:r><w:t>Fallback before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="72"><wp:extent cx="914400" cy="457200"/><wp:docPr id="72" name="Fallback block float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Fallback block body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Fallback after</w:t></w:r></w:p></mc:Fallback></mc:AlternateContent></w:body></w:document>"#,
        ),
    ])
}

fn preset_geometry_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body><w:p><w:r><w:t>Shape before </w:t></w:r><w:r><w:drawing><wp:anchor simplePos="1" relativeHeight="42" behindDoc="0"><wp:simplePos x=" 182880 " y="274320"/><wp:extent cx="914400" cy="457200"/><wp:effectExtent l="9144" t=" 18288 " r="27432" b="36576"/><wp:docPr id="21" name="Rounded box"/><wps:wsp><wps:spPr><a:solidFill><a:srgbClr val=" FF8800 "/></a:solidFill><a:ln><a:solidFill><a:srgbClr val=" 003366 "/></a:solidFill></a:ln><a:prstGeom prst=" roundRect "><a:avLst/></a:prstGeom></wps:spPr></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Shape after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn wrap_polygon_floating_shape_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><w:body><w:p><w:r><w:t>Polygon before </w:t></w:r><w:r><w:drawing><wp:anchor relativeHeight="88" behindDoc="0"><wp:extent cx="914400" cy="457200"/><wp:wrapTight wrapText=" largest " distT="100" distB="200" distL="300" distR="400"><wp:wrapPolygon edited="0"><wp:start x="0" y="0"/><wp:lineTo x="914400" y="0"/><wp:lineTo x="914400" y="457200"/><wp:lineTo x="0" y="457200"/></wp:wrapPolygon></wp:wrapTight><wp:docPr id="88" name="Polygon float"/><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Polygon body</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></wp:anchor></w:drawing></w:r><w:r><w:t>Polygon after</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

#[test]
fn docx_floating_shape_geometry_is_extracted() {
    let doc = Document::open(&floating_shape_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, "7");
    assert_eq!(shapes[0].name.as_deref(), Some("Float one"));
    assert_eq!(shapes[0].description.as_deref(), Some("A floating object"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape-body\u{00ad}soft"));
    assert_eq!(shapes[0].anchor_block_index, Some(0));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Before anchor After anchor")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Before anchor ".chars().count())
    );
    assert_eq!(
        shapes[0].extent,
        Some(ShapeExtent {
            cx_emu: 914400,
            cy_emu: 457200,
        })
    );
    assert_eq!(
        shapes[0].horizontal_position,
        Some(ShapePosition {
            relative_from: Some("column".to_string()),
            offset_emu: Some(91440),
            align: None,
        })
    );
    assert_eq!(
        shapes[0].vertical_position,
        Some(ShapePosition {
            relative_from: Some("paragraph".to_string()),
            offset_emu: None,
            align: Some("top".to_string()),
        })
    );
    assert_eq!(shapes[0].relative_height, Some(251_659_264));
    assert_eq!(shapes[0].behind_doc, Some(false));
    assert_eq!(shapes[0].layout_in_cell, Some(true));
    assert_eq!(shapes[0].locked, Some(false));
    assert_eq!(shapes[0].allow_overlap, Some(true));
    assert_eq!(
        shapes[0].distance,
        ShapeDistance {
            top_emu: Some(120),
            bottom_emu: Some(240),
            left_emu: Some(360),
            right_emu: Some(480),
        }
    );
    assert_eq!(
        shapes[0].wrapping,
        Some(ShapeWrapping {
            kind: "square".to_string(),
            text: Some("bothSides".to_string()),
            distance: ShapeDistance {
                top_emu: Some(9_144),
                bottom_emu: Some(18_288),
                left_emu: Some(27_432),
                right_emu: Some(36_576),
            },
            polygon: Vec::new(),
        })
    );
    assert_eq!(doc.report().features.floating_shapes, 1);
}

#[test]
fn docx_floating_shape_preserves_supported_symbols_in_metadata_text() {
    let doc = Document::open(&floating_shape_symbol_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Before \u{2022} anchor")
            && main_text.contains("Shape \u{2022} body"),
        "body text should preserve supported symbols before floating metadata extraction: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Symbol float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape \u{2022} body"));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Before \u{2022} anchor after")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Before \u{2022} anchor ".chars().count())
    );
}

#[test]
fn docx_floating_shape_anchor_preserves_expanded_inline_markers() {
    let doc =
        Document::open(&floating_shape_expanded_anchor_markers_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Expanded marker float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape body"));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Lead\tTab\nLine\nPage-Hard\u{00ad}Soft After")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Lead\tTab\nLine\nPage-Hard\u{00ad}Soft ".chars().count())
    );
}

#[test]
fn docx_floating_shape_text_preserves_expanded_inline_markers() {
    let doc = Document::open(&floating_shape_expanded_text_markers_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Shape\tTab\nLine\nPage-Hard\u{00ad}Soft"),
        "body text should preserve expanded markers inside shape text: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Expanded marker body"));
    assert_eq!(
        shapes[0].text.as_deref(),
        Some("Shape\tTab\nLine\nPage-Hard\u{00ad}Soft")
    );
}

#[test]
fn docx_floating_shape_metadata_uses_computed_simple_field_text() {
    let doc =
        Document::open(&floating_shape_computed_simple_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Fresh shape body") && !main_text.contains("stale shape"),
        "body text should use computed simple-field text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Computed field float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Fresh shape body"));
}

#[test]
fn docx_floating_shape_metadata_uses_computed_complex_field_text() {
    let doc =
        Document::open(&floating_shape_computed_complex_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Fresh complex shape body")
            && !main_text.contains("stale complex shape"),
        "body text should use computed complex-field text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Complex field float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Fresh complex shape body"));
}

#[test]
fn docx_complex_field_replaces_direct_text_box_result_in_body() {
    let doc = Document::open(&complex_field_direct_text_box_result_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert_eq!(main_text, "Outer shape");
    assert!(!main_text.contains("stale result box"));
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Direct result float"));
}

#[test]
fn docx_floating_shape_metadata_computes_nested_complex_fields_inside_simple_results() {
    let doc = Document::open(&floating_shape_nested_complex_field_inside_simple_result_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Prefix Inner target Suffix body")
            && !main_text.contains("stale inner target"),
        "body text should use computed nested complex field text inside shape simple-field results: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(
        shapes[0].name.as_deref(),
        Some("Nested complex field float")
    );
    assert_eq!(
        shapes[0].text.as_deref(),
        Some("Prefix Inner target Suffix body")
    );
}

#[test]
fn docx_floating_shape_metadata_uses_document_info_simple_field_text() {
    let doc =
        Document::open(&floating_shape_document_info_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Quarter Plan body") && !main_text.contains("stale title"),
        "body text should use computed document-info text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Document info float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Quarter Plan body"));
}

#[test]
fn docx_floating_shape_metadata_uses_revision_number_simple_field_text() {
    let doc =
        Document::open(&floating_shape_revision_number_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("12 body") && !main_text.contains("stale revision"),
        "body text should use computed revision-number text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Revision float"));
    assert_eq!(shapes[0].text.as_deref(), Some("12 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_document_bookmark_formula_text() {
    let doc =
        Document::open(&floating_shape_bookmark_formula_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("50 body") && !main_text.contains("stale formula"),
        "body text should use computed document-bookmark formula text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Formula float"));
    assert_eq!(shapes[0].text.as_deref(), Some("50 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_table_formula_text() {
    let doc =
        Document::open(&floating_shape_table_formula_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("5 body") && !main_text.contains("stale shape table formula"),
        "body text should use computed table formula text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Table formula float"));
    assert_eq!(shapes[0].text.as_deref(), Some("5 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_page_field_text() {
    let doc = Document::open(&floating_shape_page_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("1")
            && main_text.contains("Page two lead 2 body")
            && !main_text.contains("stale prior page")
            && !main_text.contains("stale shape page"),
        "body text should use computed PAGE text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("PAGE float"));
    assert_eq!(shapes[0].text.as_deref(), Some("2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_page_ref_field_text() {
    let doc = Document::open(&floating_shape_page_ref_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("on page 2")
            && main_text.contains("Shape target")
            && main_text.contains("above body")
            && !main_text.contains("stale prior page ref")
            && !main_text.contains("stale shape page ref"),
        "body text should use computed PAGEREF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("PAGEREF float"));
    assert_eq!(shapes[0].text.as_deref(), Some("above body"));
}

#[test]
fn docx_floating_shape_metadata_uses_section_field_text() {
    let doc = Document::open(&floating_shape_section_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("1")
            && main_text.contains("Second section 2 body")
            && !main_text.contains("stale prior section")
            && !main_text.contains("stale shape section"),
        "body text should use computed SECTION text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("SECTION float"));
    assert_eq!(shapes[0].text.as_deref(), Some("2 body"));
}

#[test]
fn docx_floating_shape_anchor_uses_computed_section_field_text() {
    let doc =
        Document::open(&floating_shape_anchor_section_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("SECTION anchor float"));
    assert_eq!(shapes[0].text.as_deref(), Some("2 body"));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Second section 2 anchor")
    );
}

#[test]
fn docx_floating_shape_metadata_uses_note_ref_field_text() {
    let doc = Document::open(&floating_shape_note_ref_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("below")
            && main_text.contains("After note above body")
            && !main_text.contains("stale prior note ref")
            && !main_text.contains("stale shape note ref"),
        "body text should use computed NOTEREF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("NOTEREF float"));
    assert_eq!(shapes[0].text.as_deref(), Some("above body"));
}

#[test]
fn docx_floating_shape_metadata_uses_document_bookmark_if_text() {
    let doc = Document::open(&floating_shape_bookmark_if_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("ship body") && !main_text.contains("stale bookmark if"),
        "body text should use computed document-bookmark IF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("IF float"));
    assert_eq!(shapes[0].text.as_deref(), Some("ship body"));
}

#[test]
fn docx_floating_shape_metadata_uses_document_bookmark_merge_control_text() {
    let doc = Document::open(&floating_shape_bookmark_merge_control_field_text_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("gate body") && !main_text.contains("stale bookmark nextif"),
        "body text should use computed document-bookmark merge-control text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Merge control float"));
    assert_eq!(shapes[0].text.as_deref(), Some("gate body"));
}

#[test]
fn docx_floating_shape_metadata_uses_set_backed_if_text() {
    let doc =
        Document::open(&floating_shape_set_backed_if_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("ship body")
            && !main_text.contains("cached shape set")
            && !main_text.contains("stale shape set if"),
        "body text should use shape-local SET-backed IF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("SET-backed IF float"));
    assert_eq!(shapes[0].text.as_deref(), Some("ship body"));
}

#[test]
fn docx_floating_shape_metadata_uses_complex_set_backed_if_text() {
    let doc = Document::open(&floating_shape_complex_set_backed_if_field_text_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("ship body")
            && !main_text.contains("cached complex shape set")
            && !main_text.contains("stale complex shape set if"),
        "body text should use shape-local complex SET-backed IF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(
        shapes[0].name.as_deref(),
        Some("Complex SET-backed IF float")
    );
    assert_eq!(shapes[0].text.as_deref(), Some("ship body"));
}

#[test]
fn docx_floating_shape_metadata_uses_ref_field_text() {
    let doc = Document::open(&floating_shape_ref_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Figure 1 body") && !main_text.contains("stale ref"),
        "body text should use computed REF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("REF float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Figure 1 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_ref_note_mark_field_text() {
    let doc =
        Document::open(&floating_shape_ref_note_mark_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("2 body") && !main_text.contains("stale shape note mark ref"),
        "body text should use computed REF note-mark text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("REF note float"));
    assert_eq!(shapes[0].text.as_deref(), Some("2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_relative_ref_field_text() {
    let doc =
        Document::open(&floating_shape_relative_ref_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("below")
            && main_text.contains("After target above body")
            && !main_text.contains("stale prior relative ref")
            && !main_text.contains("stale shape relative ref"),
        "body text should use computed REF relative text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("REF relative float"));
    assert_eq!(shapes[0].text.as_deref(), Some("above body"));
}

#[test]
fn docx_floating_shape_metadata_uses_direct_relative_ref_field_text() {
    let doc = Document::open(&floating_shape_direct_relative_ref_field_text_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("below")
            && main_text.contains("After direct target above body")
            && !main_text.contains("stale prior direct relative ref")
            && !main_text.contains("stale shape direct relative ref"),
        "body text should use computed direct REF relative text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Direct REF relative float"));
    assert_eq!(shapes[0].text.as_deref(), Some("above body"));
}

#[test]
fn docx_floating_shape_metadata_uses_numbered_ref_field_text() {
    let doc =
        Document::open(&floating_shape_numbered_ref_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Numbered clause")
            && main_text.contains("3")
            && main_text.contains("3 above body")
            && !main_text.contains("stale body number")
            && !main_text.contains("stale shape number relative"),
        "body text should use computed numbered REF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("REF number float"));
    assert_eq!(shapes[0].text.as_deref(), Some("3 above body"));
}

#[test]
fn docx_floating_shape_metadata_uses_legacy_form_field_text() {
    let doc = Document::open(&floating_shape_legacy_form_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Prior B")
            && main_text.contains("Shape B body")
            && !main_text.contains("stale shape option"),
        "body text should use computed legacy-form text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Legacy form float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape B body"));
}

#[test]
fn docx_floating_shape_anchor_counts_intervening_legacy_form_fields() {
    let doc = Document::open(&floating_shape_anchor_legacy_form_after_shape_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Direct B")
            && main_text.contains("Shape B body")
            && main_text.contains("After B after")
            && !main_text.contains("stale after option"),
        "body text should compute all legacy-form values in source order: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(
        shapes[0].name.as_deref(),
        Some("Anchor after legacy form float")
    );
    assert_eq!(shapes[0].text.as_deref(), Some("Shape B body"));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Before After B after")
    );
}

#[test]
fn docx_floating_shape_metadata_uses_simple_text_form_current_text() {
    let doc =
        Document::open(&floating_shape_text_form_current_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Prior B")
            && main_text.contains("SHAPE TYPED Shape B body")
            && !main_text.contains("SHAPE DEFAULT")
            && !main_text.contains("stale shape option"),
        "body text should use formatted simple text-form current text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Text form current float"));
    assert_eq!(shapes[0].text.as_deref(), Some("SHAPE TYPED Shape B body"));
}

#[test]
fn docx_floating_shape_metadata_uses_complex_legacy_form_field_text() {
    let doc = Document::open(&floating_shape_complex_legacy_form_field_text_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Prior B")
            && main_text.contains("Shape B body")
            && !main_text.contains("stale complex shape option"),
        "body text should use computed complex legacy-form text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Complex legacy form float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape B body"));
}

#[test]
fn docx_floating_shape_metadata_uses_complex_text_form_current_text() {
    let doc =
        Document::open(&floating_shape_complex_text_form_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Prior B")
            && main_text.contains("Shape Typed body")
            && !main_text.contains("Shape Default body"),
        "body text should preserve complex text-form current text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Complex text form float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape Typed body"));
}

#[test]
fn docx_floating_shape_metadata_uses_sequence_field_text() {
    let doc = Document::open(&floating_shape_sequence_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Figure 1")
            && main_text.contains("Figure 2 body")
            && !main_text.contains("stale shape seq"),
        "body text should use computed source-order sequence text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Sequence float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Figure 2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_sequence_heading_reset_field_text() {
    let doc = Document::open(&floating_shape_sequence_heading_reset_field_text_docx())
        .expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Chapter One")
            && main_text.contains("1")
            && main_text.contains("Shape anchor 2 body")
            && !main_text.contains("stale body heading seq")
            && !main_text.contains("stale shape heading seq"),
        "body text should use computed heading-reset sequence text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("SEQ heading float"));
    assert_eq!(shapes[0].text.as_deref(), Some("2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_autonum_field_text() {
    let doc = Document::open(&floating_shape_autonum_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Auto 1")
            && main_text.contains("Auto 2 body")
            && !main_text.contains("stale shape autonum"),
        "body text should use computed source-order AUTONUM text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Autonum float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Auto 2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_listnum_field_text() {
    let doc = Document::open(&floating_shape_listnum_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("List 1")
            && main_text.contains("List 2 body")
            && !main_text.contains("stale shape listnum"),
        "body text should use computed source-order LISTNUM text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Listnum float"));
    assert_eq!(shapes[0].text.as_deref(), Some("List 2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_complex_sequence_field_text() {
    let doc =
        Document::open(&floating_shape_complex_sequence_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Figure 1")
            && main_text.contains("Figure 2 body")
            && !main_text.contains("stale complex shape seq"),
        "body text should use computed complex SEQ text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Complex sequence float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Figure 2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_complex_autonum_field_text() {
    let doc =
        Document::open(&floating_shape_complex_autonum_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Auto 1")
            && main_text.contains("Auto 2 body")
            && !main_text.contains("stale complex shape autonum"),
        "body text should use computed complex AUTONUM text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Complex autonum float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Auto 2 body"));
}

#[test]
fn docx_floating_shape_metadata_uses_complex_listnum_field_text() {
    let doc =
        Document::open(&floating_shape_complex_listnum_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("List 1")
            && main_text.contains("List 2 body")
            && !main_text.contains("stale complex shape listnum"),
        "body text should use computed complex LISTNUM text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Complex listnum float"));
    assert_eq!(shapes[0].text.as_deref(), Some("List 2 body"));
}

#[test]
fn docx_floating_shape_metadata_hides_toc_entry_marker_field_text() {
    let doc =
        Document::open(&floating_shape_toc_entry_marker_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("body") && !main_text.contains("stale tc marker"),
        "body text should hide TC marker display text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("TOC entry float"));
    assert_eq!(shapes[0].text.as_deref(), Some("body"));
}

#[test]
fn docx_floating_shape_metadata_uses_toc_field_text() {
    let doc = Document::open(&floating_shape_toc_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Executive Summary\nRisks body")
            && !main_text.contains("stale shape toc"),
        "body text should use computed TOC text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("TOC float"));
    assert_eq!(
        shapes[0].text.as_deref(),
        Some("Executive Summary\n  Risks body")
    );
}

#[test]
fn docx_floating_shape_metadata_uses_style_ref_field_text() {
    let doc = Document::open(&floating_shape_style_ref_field_text_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();
    let main_text = doc.main_text();

    assert!(
        main_text.contains("Intro Heading")
            && main_text.contains("Shape Heading body")
            && !main_text.contains("stale shape style ref"),
        "body text should use computed STYLEREF text inside shapes: {main_text:?}"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("STYLEREF float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Shape Heading body"));
}

#[test]
fn docx_floating_shape_recovers_wrap_polygon_points() {
    let doc = Document::open(&wrap_polygon_floating_shape_docx()).expect("fixture opens");
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");
    let wrapping = shape.wrapping.as_ref().expect("wrap metadata");

    assert_eq!(shape.name.as_deref(), Some("Polygon float"));
    assert_eq!(shape.text.as_deref(), Some("Polygon body"));
    assert_eq!(wrapping.kind, "tight");
    assert_eq!(wrapping.text.as_deref(), Some("largest"));
    assert_eq!(
        wrapping.distance,
        ShapeDistance {
            top_emu: Some(100),
            bottom_emu: Some(200),
            left_emu: Some(300),
            right_emu: Some(400),
        }
    );
    assert_eq!(
        wrapping.polygon,
        vec![
            ShapePoint { x_emu: 0, y_emu: 0 },
            ShapePoint {
                x_emu: 914_400,
                y_emu: 0,
            },
            ShapePoint {
                x_emu: 914_400,
                y_emu: 457_200,
            },
            ShapePoint {
                x_emu: 0,
                y_emu: 457_200,
            },
        ]
    );
    assert_eq!(doc.report().features.floating_shapes, 1);
}

#[test]
fn docx_header_footer_floating_shapes_are_exposed() {
    let doc = Document::open(&header_footer_floating_shape_docx()).expect("fixture opens");

    assert!(doc.header_text().contains("Header shape body"));
    let shapes = doc.floating_shapes();
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, "77");
    assert_eq!(shapes[0].name.as_deref(), Some("Header float"));
    assert_eq!(shapes[0].description.as_deref(), Some("Header shape"));
    assert_eq!(shapes[0].text.as_deref(), Some("Header shape body"));
    assert_eq!(shapes[0].anchor_block_index, None);
    assert_eq!(shapes[0].anchor_text, None);
    assert_eq!(
        shapes[0].horizontal_position,
        Some(ShapePosition {
            relative_from: Some("page".to_string()),
            offset_emu: Some(91440),
            align: None,
        })
    );
    assert_eq!(
        shapes[0].extent,
        Some(ShapeExtent {
            cx_emu: 914400,
            cy_emu: 457200,
        })
    );
}

#[test]
fn docx_note_floating_shapes_are_exposed_with_current_policy() {
    let doc = Document::open(&note_floating_shape_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].id, "61");
    assert_eq!(shapes[0].name.as_deref(), Some("Note direct"));
    assert_eq!(shapes[0].text.as_deref(), Some("Direct shape body"));
    assert_eq!(shapes[0].anchor_block_index, None);
    assert_eq!(shapes[0].anchor_text, None);
    assert_eq!(shapes[1].id, "63");
    assert_eq!(shapes[1].name.as_deref(), Some("Note choice"));
    assert_eq!(shapes[1].text.as_deref(), Some("Choice shape body"));
    assert_eq!(shapes[1].anchor_block_index, None);
    assert_eq!(shapes[1].anchor_text, None);
    assert_eq!(doc.report().features.floating_shapes, 2);
}

#[test]
fn docx_floating_shape_recovers_textless_preset_geometry() {
    let doc = Document::open(&preset_geometry_floating_shape_docx()).expect("fixture opens");
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");

    assert_eq!(shape.name.as_deref(), Some("Rounded box"));
    assert_eq!(shape.text, None);
    assert_eq!(shape.preset_geometry.as_deref(), Some("roundRect"));
    assert_eq!(shape.fill_color, Some(Color::rgb(0xFF, 0x88, 0x00)));
    assert_eq!(shape.outline_color, Some(Color::rgb(0x00, 0x33, 0x66)));
    assert_eq!(shape.simple_position_enabled, Some(true));
    assert_eq!(
        shape.simple_position,
        Some(ShapePoint {
            x_emu: 182_880,
            y_emu: 274_320,
        })
    );
    assert_eq!(
        shape.effect_extent,
        Some(ShapeEffectExtent {
            left_emu: 9_144,
            top_emu: 18_288,
            right_emu: 27_432,
            bottom_emu: 36_576,
        })
    );
    assert_eq!(
        shape.anchor_text.as_deref(),
        Some("Shape before Shape after")
    );
    assert_eq!(
        shape.anchor_char_offset,
        Some("Shape before ".chars().count())
    );
}

#[test]
fn docx_floating_shape_anchor_survives_body_level_content_control() {
    let doc = Document::open(&sdt_wrapped_floating_shape_docx()).expect("fixture opens");
    let model = doc.model();
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");

    assert_eq!(model.blocks.len(), 3);
    let Block::Paragraph(wrapped) = &model.blocks[1] else {
        panic!("wrapped content-control paragraph should become a body block");
    };
    assert!(
        wrapped.text().contains("Wrapped shape body"),
        "body parser should still expose text-bearing shape body: {:?}",
        wrapped.text()
    );
    assert_eq!(shape.anchor_block_index, Some(1));
    assert_eq!(
        shape.anchor_text.as_deref(),
        Some("Wrapped before Wrapped after")
    );
    assert_eq!(
        shape.anchor_char_offset,
        Some("Wrapped before ".chars().count())
    );
}

#[test]
fn docx_floating_shape_anchor_survives_body_level_custom_xml() {
    let doc = Document::open(&custom_xml_wrapped_floating_shape_docx()).expect("fixture opens");
    let model = doc.model();
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");

    assert_eq!(model.blocks.len(), 3);
    let Block::Paragraph(wrapped) = &model.blocks[1] else {
        panic!("customXml paragraph should become a body block");
    };
    assert!(
        wrapped.text().contains("Custom XML shape body"),
        "body parser should still expose text-bearing customXml shape body: {:?}",
        wrapped.text()
    );
    assert_eq!(shape.anchor_block_index, Some(1));
    assert_eq!(shape.name.as_deref(), Some("Custom XML float"));
    assert_eq!(shape.text.as_deref(), Some("Custom XML shape body"));
    assert_eq!(
        shape.anchor_text.as_deref(),
        Some("Custom before Custom after")
    );
    assert_eq!(
        shape.anchor_char_offset,
        Some("Custom before ".chars().count())
    );
}

#[test]
fn docx_floating_shape_anchor_survives_body_level_smart_tag() {
    let doc = Document::open(&smart_tag_wrapped_floating_shape_docx()).expect("fixture opens");
    let model = doc.model();
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");

    assert_eq!(model.blocks.len(), 3);
    let Block::Paragraph(wrapped) = &model.blocks[1] else {
        panic!("smartTag paragraph should become a body block");
    };
    assert!(
        wrapped.text().contains("Smart shape body"),
        "body parser should still expose text-bearing smartTag shape body: {:?}",
        wrapped.text()
    );
    assert_eq!(shape.anchor_block_index, Some(1));
    assert_eq!(
        shape.anchor_text.as_deref(),
        Some("Smart before Smart after")
    );
    assert_eq!(
        shape.anchor_char_offset,
        Some("Smart before ".chars().count())
    );
}

#[test]
fn docx_floating_shape_anchor_survives_body_level_insertion() {
    let doc = Document::open(&inserted_wrapped_floating_shape_docx()).expect("fixture opens");
    let model = doc.model();
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");

    assert_eq!(model.blocks.len(), 3);
    let Block::Paragraph(inserted) = &model.blocks[1] else {
        panic!("inserted paragraph should become an accepted body block");
    };
    assert!(
        inserted.text().contains("Inserted shape body"),
        "body parser should still expose text-bearing inserted shape body: {:?}",
        inserted.text()
    );
    assert_eq!(shape.anchor_block_index, Some(1));
    assert_eq!(
        shape.anchor_text.as_deref(),
        Some("Inserted before Inserted after")
    );
    assert_eq!(
        shape.anchor_char_offset,
        Some("Inserted before ".chars().count())
    );
}

#[test]
fn docx_floating_shapes_follow_accepted_revision_view() {
    let doc = Document::open(&revision_wrapped_floating_shapes_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(
        doc.main_text(),
        "Direct before Direct bodyDirect after\nInserted before Inserted bodyInserted after\nMoved-to before Moved-to bodyMoved-to after"
    );
    assert_eq!(shapes.len(), 3);
    assert_eq!(shapes[0].name.as_deref(), Some("Direct float"));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Direct before Direct after")
    );
    assert_eq!(shapes[1].name.as_deref(), Some("Inserted float"));
    assert_eq!(
        shapes[1].anchor_text.as_deref(),
        Some("Inserted before Inserted after")
    );
    assert_eq!(shapes[2].name.as_deref(), Some("Moved-to float"));
    assert_eq!(
        shapes[2].anchor_text.as_deref(),
        Some("Moved-to before Moved-to after")
    );
}

#[test]
fn docx_floating_shape_alternate_content_uses_single_branch() {
    let doc = Document::open(&alternate_content_floating_shape_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(
        doc.main_text(),
        "Before alternate Choice shape bodyAfter alternate"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, "41");
    assert_eq!(shapes[0].name.as_deref(), Some("Choice float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Choice shape body"));
    assert_eq!(shapes[0].relative_height, Some(41));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Before alternate After alternate")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Before alternate ".chars().count())
    );
}

#[test]
fn docx_block_alternate_content_floating_shape_keeps_anchor() {
    let doc =
        Document::open(&block_alternate_content_floating_shape_docx()).expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(
        doc.main_text(),
        "Choice before Choice block bodyChoice after"
    );
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, "71");
    assert_eq!(shapes[0].name.as_deref(), Some("Choice block float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Choice block body"));
    assert_eq!(shapes[0].anchor_block_index, Some(0));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Choice before Choice after")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Choice before ".chars().count())
    );
}

#[test]
fn public_corpus_altcontent_anchor_fixture_recovers_single_floating_shape() {
    let doc = Document::open(&public_corpus_docx(
        "synthetic/floating_altcontent_anchor.docx",
    ))
    .expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, "410");
    assert_eq!(shapes[0].name.as_deref(), Some("Public choice float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Public choice body"));
    assert_eq!(shapes[0].relative_height, Some(410));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Public alternate before.  Public alternate after.")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Public alternate before. ".chars().count())
    );
}

#[test]
fn public_corpus_z_order_fixture_recovers_behind_doc_and_relative_height_order() {
    let doc = Document::open(&public_corpus_docx("synthetic/floating_z_order_pair.docx"))
        .expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].name.as_deref(), Some("Behind float"));
    assert_eq!(shapes[0].behind_doc, Some(true));
    assert_eq!(shapes[0].relative_height, Some(100));
    assert_eq!(shapes[1].name.as_deref(), Some("Front float"));
    assert_eq!(shapes[1].behind_doc, Some(false));
    assert_eq!(shapes[1].relative_height, Some(200));
}

#[test]
fn public_corpus_wrap_fixture_recovers_anchor_and_wrap_distances() {
    let doc = Document::open(&public_corpus_docx("synthetic/floating_wrap_policy.docx"))
        .expect("fixture opens");
    let shape = doc
        .floating_shapes()
        .into_iter()
        .next()
        .expect("floating shape extracted");
    let wrapping = shape.wrapping.as_ref().expect("wrap metadata");

    assert_eq!(shape.name.as_deref(), Some("Public wrap float"));
    assert_eq!(
        shape.distance,
        ShapeDistance {
            top_emu: Some(111),
            bottom_emu: Some(222),
            left_emu: Some(333),
            right_emu: Some(444),
        }
    );
    assert_eq!(wrapping.kind, "tight");
    assert_eq!(wrapping.text.as_deref(), Some("bothSides"));
    assert_eq!(
        wrapping.distance,
        ShapeDistance {
            top_emu: Some(555),
            bottom_emu: Some(666),
            left_emu: Some(777),
            right_emu: Some(888),
        }
    );
    assert_eq!(wrapping.polygon.len(), 5);
}

#[test]
fn public_corpus_text_bearing_fixture_recovers_body_and_anchor_text() {
    let doc = Document::open(&public_corpus_docx("synthetic/floating_text_bearing.docx"))
        .expect("fixture opens");
    let shapes = doc.floating_shapes();

    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].name.as_deref(), Some("Public text float"));
    assert_eq!(shapes[0].text.as_deref(), Some("Public text body"));
    assert_eq!(shapes[0].anchor_block_index, Some(0));
    assert_eq!(
        shapes[0].anchor_text.as_deref(),
        Some("Containing block before.  Containing block after.")
    );
    assert_eq!(
        shapes[0].anchor_char_offset,
        Some("Containing block before. ".chars().count())
    );
}

#[cfg(feature = "render")]
#[test]
fn docx_floating_shape_render_draws_preview_overlay_and_keeps_warning() {
    let doc = Document::open(&floating_shape_docx()).expect("fixture opens");
    let plain = rwml::render_pdf(&doc.model());
    let rendered = doc.to_pdf_with_report();

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert!(
        rendered.pdf.len() > plain.len(),
        "floating-shape overlay should add visible preview content"
    );
    assert_eq!(rendered.report.unsupported.floating_shapes, 1);
    assert!(rendered.report.warnings.iter().any(|warning| matches!(
        warning,
        rwml::RenderWarning::FloatingShapePlaceholderOnly { count: 1 }
    )));
}
