#![cfg(feature = "docx")]

use std::io::Write;

use rwml::{Document, RevisionKind, RevisionView};

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

fn revised_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id=" 1 " w:author=" Alice " w:date=" 2026-06-24T01:00:00Z "><w:r><w:t>added</w:t></w:r></w:ins><w:del w:id="2" w:author="Bob"><w:r><w:delText>removed</w:delText></w:r></w:del><w:moveFrom w:id="3"><w:r><w:delText>from</w:delText></w:r></w:moveFrom><w:moveTo w:id="4"><w:r><w:t>to</w:t></w:r></w:moveTo></w:p></w:body></w:document>"#,
        ),
    ])
}

fn block_level_revised_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:ins w:id="10" w:author="Alice"><w:p><w:r><w:t>Inserted block</w:t></w:r></w:p></w:ins><w:moveTo w:id="11" w:author="Casey"><w:p><w:r><w:t>Moved-to block</w:t></w:r></w:p></w:moveTo><w:del w:id="12" w:author="Bob"><w:p><w:r><w:delText>Deleted block</w:delText></w:r></w:p></w:del><w:moveFrom w:id="13" w:author="Dana"><w:p><w:r><w:delText>Moved-from block</w:delText></w:r></w:p></w:moveFrom><w:p><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn multi_paragraph_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:ins w:id="20" w:author="Alice"><w:p><w:r><w:t>First</w:t></w:r></w:p><w:p><w:r><w:t>Second</w:t></w:r></w:p></w:ins><w:p><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn simple_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="30" w:author="Alice"><w:fldSimple w:instr=" QUOTE &quot;Fresh added&quot; "><w:r><w:t>stale added</w:t></w:r></w:fldSimple></w:ins><w:del w:id="31" w:author="Bob"><w:fldSimple w:instr=" QUOTE &quot;Fresh removed&quot; "><w:r><w:delText>stale removed</w:delText></w:r></w:fldSimple></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn complex_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="32" w:author="Alice"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh added&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale added</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:ins><w:del w:id="33" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh removed&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale removed</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn dynamic_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="34" w:author="Alice"><w:fldSimple w:instr=" IF 1 = 1 &quot;Fresh added&quot; &quot;stale branch&quot; "><w:r><w:t>stale added</w:t></w:r></w:fldSimple></w:ins><w:del w:id="35" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> IF 2 &gt; 1 &quot;Fresh removed&quot; &quot;stale branch&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale removed</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn local_field_bookmark_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="36" w:author="Alice"><w:fldSimple w:instr=" SET Client &quot;Acme&quot; "/><w:fldSimple w:instr=" IF Client = &quot;Acme&quot; &quot;Fresh added&quot; &quot;stale branch&quot; "><w:r><w:t>stale added</w:t></w:r></w:fldSimple></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn display_action_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="37" w:author="Alice"><w:fldSimple w:instr=" SYMBOL 183 \f Symbol "><w:r><w:t>stale insert symbol</w:t></w:r></w:fldSimple></w:ins><w:del w:id="38" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MACROBUTTON RunReport &quot;Fresh removed&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale action text</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn marker_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="39" w:author="Alice"><w:fldSimple w:instr=" TC &quot;Inserted entry&quot; "><w:r><w:t>stale insert marker</w:t></w:r></w:fldSimple><w:r><w:t>added</w:t></w:r></w:ins><w:del w:id="40" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> XE &quot;Deleted index&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete marker</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText>removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn numbering_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="41" w:author="Alice"><w:fldSimple w:instr=" SEQ RevisionItem "><w:r><w:t>stale insert sequence one</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SEQ RevisionItem </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale insert sequence two</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:ins><w:del w:id="42" w:author="Bob"><w:fldSimple w:instr=" LISTNUM NumberDefault "><w:r><w:delText>stale delete list one</w:delText></w:r></w:fldSimple><w:r><w:delText> </w:delText></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> LISTNUM NumberDefault </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete list two</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn document_info_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="43" w:author="Alice"><w:fldSimple w:instr=" TITLE "><w:r><w:t>stale insert title</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="44" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> TITLE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete title</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn revision_number_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="45" w:author="Alice"><w:fldSimple w:instr=" REVNUM "><w:r><w:t>stale insert revision</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="46" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> REVNUM </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete revision</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn section_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale direct section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:ins w:id="47" w:author="Alice"><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale insert section</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn legacy_form_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Direct A"/><w:listEntry w:val="Direct B"/></w:ddList></w:ffData><w:r><w:t>stale direct option</w:t></w:r></w:fldSimple></w:p><w:p><w:ins w:id="48" w:author="Alice"><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Insert A"/><w:listEntry w:val="Insert B"/></w:ddList></w:ffData><w:r><w:t>stale insert option</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn text_form_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Direct default"/></w:textInput></w:ffData><w:r><w:t>Direct typed</w:t></w:r></w:fldSimple></w:p><w:p><w:ins w:id="49" w:author="Alice"><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Insert default"/></w:textInput></w:ffData><w:r><w:t>Insert typed</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn complex_text_form_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Direct default"/></w:textInput></w:ffData><w:r><w:t>Direct typed</w:t></w:r></w:fldSimple></w:p><w:p><w:ins w:id="50" w:author="Alice"><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:textInput><w:default w:val="Insert default"/></w:textInput></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMTEXT \*Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Insert typed</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn document_bookmark_formula_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceSubtotal"/><w:r><w:t>42</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:ins w:id="47" w:author="Alice"><w:fldSimple w:instr=" = InvoiceSubtotal + 8 "><w:r><w:t>stale insert formula</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="48" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> = InvoiceSubtotal + 8 </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete formula</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn document_bookmark_conditional_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceTier"/><w:r><w:t>Gold</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:ins w:id="49" w:author="Alice"><w:fldSimple w:instr=" IF InvoiceTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale insert if</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="50" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> COMPARE InvoiceTier = &quot;Gold&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete compare</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn document_bookmark_ref_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="ClauseText"/><w:r><w:t>clause one</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:ins w:id="51" w:author="Alice"><w:fldSimple w:instr=" REF ClauseText \* Upper "><w:r><w:t>stale insert ref</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="52" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> ClauseText \* FirstCap </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete direct ref</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn note_ref_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:ins w:id="53" w:author="Alice"><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>stale insert note</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="54" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> FTNREF FootOne </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete note</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn toc_field_revision_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:ins w:id="55" w:author="Alice"><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; "><w:r><w:t>stale insert toc</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="56" w:author="Bob"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> TOC \o &quot;1-1&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale delete toc</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn style_ref_field_revision_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Body Heading</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale earlier body style ref</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Revision Heading</w:t></w:r></w:p><w:p><w:ins w:id="57" w:author="Alice"><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale insert style ref</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins><w:del w:id="58" w:author="Bob"><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:delText>stale delete style ref</w:delText></w:r></w:fldSimple><w:r><w:delText> removed</w:delText></w:r></w:del></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Move Heading</w:t></w:r></w:p><w:p><w:moveTo w:id="59" w:author="Casey"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> STYLEREF &quot;heading 1&quot; \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale move-to style ref</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> moved</w:t></w:r></w:moveTo><w:moveFrom w:id="60" w:author="Dana"><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> STYLEREF &quot;heading 1&quot; \* Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:delText>stale move-from style ref</w:delText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:delText> original</w:delText></w:r></w:moveFrom></w:p></w:body></w:document>"#,
        ),
    ])
}

fn style_ref_file_size_source_revision_docx() -> Vec<u8> {
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
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:fldSimple w:instr=" FILESIZE "><w:r><w:t>cached file size heading</w:t></w:r></w:fldSimple></w:p><w:p><w:ins w:id="61" w:author="Alice"><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale revision style ref</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn style_ref_note_numbering_source_revision_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#,
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
            "word/settings.xml",
            r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnotePr><w:numStart w:val="4"/><w:numFmt w:val="upperRoman"/></w:footnotePr></w:settings>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Topic </w:t></w:r><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>cached source note</w:t></w:r></w:fldSimple><w:r><w:t> Scope</w:t></w:r></w:p><w:p><w:ins w:id="62" w:author="Alice"><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale revision style ref</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn style_ref_invalid_utf8_styles_revision_docx() -> Vec<u8> {
    let mut styles =
        br#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#
            .to_vec();
    styles.push(0x80);
    styles.extend_from_slice(
        br#"<w:style w:type="paragraph" w:styleId="CustomHeading"><w:name w:val="Custom Heading"/></w:style></w:styles>"#,
    );
    docx_fixture_bytes(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/styles.xml", &styles),
        (
            "word/document.xml",
            br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="CustomHeading"/></w:pPr><w:r><w:t>Source </w:t></w:r><w:fldSimple w:instr=" QUOTE &quot;Fresh&quot; "><w:r><w:t>stale source</w:t></w:r></w:fldSimple></w:p><w:p><w:ins w:id="63" w:author="Alice"><w:fldSimple w:instr=" STYLEREF &quot;Custom Heading&quot; "><w:r><w:t>stale revision style ref</w:t></w:r></w:fldSimple><w:r><w:t> added</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn note_revised_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Main</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:id="1"><w:p><w:ins w:id="7" w:author="Editor" w:date="2026-06-24T00:00:00Z"><w:r><w:t>Foot added</w:t></w:r></w:ins><w:del w:id="8"><w:r><w:delText>Foot removed</w:delText></w:r></w:del></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="2"><w:p><w:moveFrom w:id="9"><w:r><w:delText>End moved from</w:delText></w:r></w:moveFrom><w:moveTo w:id="10"><w:r><w:t>End moved to</w:t></w:r></w:moveTo></w:p></w:endnote></w:endnotes>"#,
        ),
    ])
}

fn alternate_content_revised_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><mc:AlternateContent><mc:Choice Requires="w14"><w:ins w:id="1" w:author="Alice"><w:r><w:t>Choice insert</w:t></w:r></w:ins></mc:Choice><mc:Fallback><w:ins w:id="2" w:author="Bob"><w:r><w:t>Fallback insert</w:t></w:r></w:ins></mc:Fallback></mc:AlternateContent><w:ins w:id="3"><w:r><mc:AlternateContent><mc:Choice Requires="w14"><w:t>Choice inner</w:t></mc:Choice><mc:Fallback><w:t>Fallback inner</w:t></mc:Fallback></mc:AlternateContent></w:r></w:ins></w:p></w:body></w:document>"#,
        ),
    ])
}

fn inline_marker_revised_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="5"><w:r><w:t>Col1</w:t><w:tab/><w:t>Col2</w:t><w:br/><w:t>No</w:t><w:noBreakHyphen/><w:t>Break</w:t><w:softHyphen/><w:t>Soft</w:t></w:r></w:ins><w:del w:id="6"><w:r><w:delText>Old</w:delText><w:tab/><w:delText>Text</w:delText><w:cr/><w:delText>End</w:delText><w:softHyphen/><w:delText>Soft</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

fn unchanged_inline_marker_revision_view_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Lead</w:t><w:tab/><w:t>Tab</w:t><w:br/><w:t>Line</w:t><w:br w:type="page"/><w:t>Page</w:t><w:noBreakHyphen/><w:t>Hard</w:t><w:softHyphen/><w:t>Soft</w:t></w:r><w:ins w:id="7"><w:r><w:t>Added</w:t></w:r></w:ins><w:del w:id="8"><w:r><w:delText>Removed</w:delText></w:r></w:del></w:p></w:body></w:document>"#,
        ),
    ])
}

#[test]
fn docx_revisions_are_extracted() {
    let doc = Document::open(&revised_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 4);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].id.as_deref(), Some("1"));
    assert_eq!(revisions[0].author.as_deref(), Some("Alice"));
    assert_eq!(revisions[0].date.as_deref(), Some("2026-06-24T01:00:00Z"));
    assert_eq!(revisions[0].text, "added");

    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].id.as_deref(), Some("2"));
    assert_eq!(revisions[1].author.as_deref(), Some("Bob"));
    assert_eq!(revisions[1].text, "removed");

    assert_eq!(revisions[2].kind, RevisionKind::MoveFrom);
    assert_eq!(revisions[2].text, "from");
    assert_eq!(revisions[3].kind, RevisionKind::MoveTo);
    assert_eq!(revisions[3].text, "to");
}

#[test]
fn docx_revision_views_extract_accepted_original_and_annotated_text() {
    let doc = Document::open(&revised_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "addedto");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "added to"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "removed from"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+added] [-removed] [~from->] [~->to]"
    );
}

#[test]
fn docx_block_level_current_revision_paragraphs_are_accepted_in_body_model() {
    let doc = Document::open(&block_level_revised_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Before\nInserted block\nMoved-to block\nAfter"
    );
    assert!(!doc.main_text().contains("Deleted block"));
    assert!(!doc.main_text().contains("Moved-from block"));

    let blocks = &doc.model().blocks;
    assert_eq!(blocks.len(), 4);
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Before Inserted block Moved-to block After"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Before Deleted block Moved-from block After"
    );
}

#[test]
fn docx_note_revisions_are_exposed() {
    let doc = Document::open(&note_revised_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 4);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].id.as_deref(), Some("7"));
    assert_eq!(revisions[0].author.as_deref(), Some("Editor"));
    assert_eq!(revisions[0].date.as_deref(), Some("2026-06-24T00:00:00Z"));
    assert_eq!(revisions[0].text, "Foot added");

    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].id.as_deref(), Some("8"));
    assert_eq!(revisions[1].text, "Foot removed");

    assert_eq!(revisions[2].kind, RevisionKind::MoveFrom);
    assert_eq!(revisions[2].id.as_deref(), Some("9"));
    assert_eq!(revisions[2].text, "End moved from");
    assert_eq!(revisions[3].kind, RevisionKind::MoveTo);
    assert_eq!(revisions[3].id.as_deref(), Some("10"));
    assert_eq!(revisions[3].text, "End moved to");

    assert_eq!(doc.footnote_text(), "Foot added");
    assert_eq!(doc.endnote_text(), "End moved to");
}

#[test]
fn docx_revision_text_preserves_paragraph_boundaries() {
    let doc = Document::open(&multi_paragraph_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "Before\nFirst\nSecond\nAfter");
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "First\nSecond");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Before First\nSecond After"
    );
}

#[test]
fn docx_revision_text_uses_computed_simple_field_text() {
    let doc = Document::open(&simple_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "Fresh added");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Fresh added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Fresh removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Fresh added"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Fresh removed"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+Fresh added] [-Fresh removed]"
    );
}

#[test]
fn docx_revision_text_uses_computed_complex_field_text() {
    let doc = Document::open(&complex_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "Fresh added");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Fresh added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Fresh removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Fresh added"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Fresh removed"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+Fresh added] [-Fresh removed]"
    );
}

#[test]
fn docx_revision_text_uses_computed_dynamic_field_text() {
    let doc = Document::open(&dynamic_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "Fresh added");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Fresh added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Fresh removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Fresh added"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Fresh removed"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+Fresh added] [-Fresh removed]"
    );
}

#[test]
fn docx_revision_text_uses_local_field_bookmarks_in_dynamic_field_text() {
    let doc = Document::open(&local_field_bookmark_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "Fresh added");
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Fresh added");
}

#[test]
fn docx_revision_text_uses_supported_display_and_action_field_text() {
    let doc = Document::open(&display_action_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "•");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "•");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Fresh removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "•"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Fresh removed"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+•] [-Fresh removed]"
    );
}

#[test]
fn docx_revision_text_hides_supported_toc_and_index_marker_field_text() {
    let doc = Document::open(&marker_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "added");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "added"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "removed"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+added] [-removed]"
    );
}

#[test]
fn docx_revision_text_uses_supported_source_order_numbering_field_text() {
    let doc = Document::open(&numbering_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "1 2");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "1 2");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "1 2");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "1 2"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "1 2"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+1 2] [-1 2]"
    );
}

#[test]
fn docx_revision_text_uses_supported_document_info_field_text() {
    let doc = Document::open(&document_info_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "Side Table Plan added");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Side Table Plan added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Side Table Plan removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Side Table Plan removed"
    );
}

#[test]
fn docx_revision_text_uses_supported_revision_number_field_text() {
    let doc = Document::open(&revision_number_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(doc.main_text(), "17 added");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "17 added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "17 removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "17 removed"
    );
}

#[test]
fn docx_revision_text_uses_computed_section_field_text() {
    let doc = Document::open(&section_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "2 added");
}

#[test]
fn docx_revision_text_uses_legacy_form_dropdown_field_text() {
    let doc = Document::open(&legacy_form_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Insert B added");
}

#[test]
fn docx_revision_text_uses_text_form_current_field_text() {
    let doc = Document::open(&text_form_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    let main_text = doc.main_text();
    assert!(
        main_text.contains("DIRECT TYPED")
            && main_text.contains("INSERT TYPED added")
            && !main_text.contains("Direct default")
            && !main_text.contains("Insert default"),
        "main text should use formatted text-form current text: {main_text:?}"
    );
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "INSERT TYPED added");
}

#[test]
fn docx_revision_text_uses_complex_text_form_current_field_text() {
    let doc = Document::open(&complex_text_form_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    let main_text = doc.main_text();
    assert!(
        main_text.contains("DIRECT TYPED")
            && main_text.contains("INSERT TYPED added")
            && !main_text.contains("Direct default")
            && !main_text.contains("Insert default"),
        "main text should use formatted complex text-form current text: {main_text:?}"
    );
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "INSERT TYPED added");
}

#[test]
fn docx_revision_text_uses_document_bookmark_formula_field_text() {
    let doc =
        Document::open(&document_bookmark_formula_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "50 added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "50 removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "42 50 removed"
    );
}

#[test]
fn docx_revision_text_uses_document_bookmark_conditional_field_text() {
    let doc = Document::open(&document_bookmark_conditional_field_revision_docx())
        .expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "ship added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "1 removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Gold 1 removed"
    );
}

#[test]
fn docx_revision_text_uses_document_bookmark_ref_field_text() {
    let doc = Document::open(&document_bookmark_ref_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "CLAUSE ONE added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Clause one removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "clause one Clause one removed"
    );
}

#[test]
fn docx_revision_text_uses_document_note_ref_field_text() {
    let doc = Document::open(&note_ref_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "1 added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "1 removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "1 removed"
    );
}

#[test]
fn docx_revision_text_uses_document_toc_field_text() {
    let doc = Document::open(&toc_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Executive Summary added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Executive Summary removed");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Executive Summary Executive Summary removed"
    );
}

#[test]
fn docx_revision_text_uses_current_style_ref_field_text() {
    let doc = Document::open(&style_ref_field_revision_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 4);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Revision Heading added");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "stale delete style ref removed");
    assert_eq!(revisions[2].kind, RevisionKind::MoveTo);
    assert_eq!(revisions[2].text, "MOVE HEADING moved");
    assert_eq!(revisions[3].kind, RevisionKind::MoveFrom);
    assert_eq!(revisions[3].text, "stale move-from style ref original");
}

#[test]
fn docx_revision_view_uses_current_style_ref_field_text() {
    let doc = Document::open(&style_ref_field_revision_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Body Heading stale earlier body style ref Revision Heading Revision Heading added Move Heading MOVE HEADING moved"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Body Heading stale earlier body style ref Revision Heading stale delete style ref removed Move Heading stale move-from style ref original"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "Body Heading stale earlier body style ref Revision Heading [+Revision Heading added] [-stale delete style ref removed] Move Heading [~->MOVE HEADING moved] [~stale move-from style ref original->]"
    );
}

#[test]
fn docx_revision_view_style_ref_source_fields_use_source_file_size() {
    let fixture = style_ref_file_size_source_revision_docx();
    let expected_size = fixture.len().to_string();
    let doc = Document::open(&fixture).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        format!("{expected_size}\n{expected_size} added")
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        format!("cached file size heading {expected_size} added")
    );
}

#[test]
fn docx_revision_view_style_ref_source_note_refs_use_settings_numbering() {
    let doc =
        Document::open(&style_ref_note_numbering_source_revision_docx()).expect("fixture opens");

    assert_eq!(doc.main_text(), "Topic IV Scope\nTopic IV Scope added");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Topic  cached source note  Scope Topic IV Scope added"
    );
}

#[test]
fn docx_revision_view_matches_strict_open_context_for_invalid_utf8_styles() {
    let doc =
        Document::open(&style_ref_invalid_utf8_styles_revision_docx()).expect("fixture opens");

    assert_eq!(doc.revisions()[0].text, "stale revision style ref added");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Source  stale source stale revision style ref added"
    );
}

#[test]
fn docx_revisions_use_first_alternate_content_branch() {
    let doc = Document::open(&alternate_content_revised_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].id.as_deref(), Some("1"));
    assert_eq!(revisions[0].author.as_deref(), Some("Alice"));
    assert_eq!(revisions[0].text, "Choice insert");

    assert_eq!(revisions[1].kind, RevisionKind::Insertion);
    assert_eq!(revisions[1].id.as_deref(), Some("3"));
    assert_eq!(revisions[1].text, "Choice inner");

    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Choice insert Choice inner"
    );
    assert_eq!(doc.main_text_with_revision_view(RevisionView::Original), "");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "[+Choice insert] [+Choice inner]"
    );
}

#[test]
fn docx_revision_text_preserves_inline_markers() {
    let doc = Document::open(&inline_marker_revised_docx()).expect("fixture opens");
    let revisions = doc.revisions();

    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].kind, RevisionKind::Insertion);
    assert_eq!(revisions[0].text, "Col1\tCol2\nNo-Break\u{00ad}Soft");
    assert_eq!(revisions[1].kind, RevisionKind::Deletion);
    assert_eq!(revisions[1].text, "Old\tText\nEnd\u{00ad}Soft");
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Col1\tCol2\nNo-Break\u{00ad}Soft"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Old\tText\nEnd\u{00ad}Soft"
    );
}

#[test]
fn docx_revision_views_preserve_unchanged_inline_markers() {
    let doc = Document::open(&unchanged_inline_marker_revision_view_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Accepted),
        "Lead\tTab\nLine\nPage-Hard\u{00ad}Soft Added"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Original),
        "Lead\tTab\nLine\nPage-Hard\u{00ad}Soft Removed"
    );
    assert_eq!(
        doc.main_text_with_revision_view(RevisionView::Annotated),
        "Lead\tTab\nLine\nPage-Hard\u{00ad}Soft [+Added] [-Removed]"
    );
}
