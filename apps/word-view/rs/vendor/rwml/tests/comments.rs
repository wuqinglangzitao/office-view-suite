#![cfg(feature = "docx")]

use std::io::Write;

use rwml::Document;

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

fn unzip_parts(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut parts = std::collections::BTreeMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        use std::io::Read;
        file.read_to_end(&mut bytes).unwrap();
        parts.insert(file.name().to_string(), bytes);
    }
    parts
}

fn plain_docx() -> Vec<u8> {
    docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn split_run_docx() -> Vec<u8> {
    docx_fixture(&[
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hel</w:t></w:r><w:r><w:t>lo</w:t></w:r><w:r><w:t>!</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn commented_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id=" 7 "/><w:r><w:t>Hello</w:t></w:r><w:commentRangeEnd w:id=" 7 "/><w:r><w:commentReference w:id=" 7 "/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id=" 7 " w:author=" Reviewer " w:initials=" RV " w:date=" 2026-06-24T00:00:00Z "><w:p><w:r><w:t>First </w:t></w:r><w:r><w:t>note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ])
}

fn alternate_content_commented_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:commentRangeStart w:id="7"/><w:r><w:t>Hello </w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Box</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>Box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t> Tail</w:t></w:r><w:commentRangeEnd w:id="7"/><w:r><w:commentReference w:id="7"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml"><w:comment w:id="7" w:author="Reviewer"><w:p><w:r><w:t>Comment </w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Box</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>Box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t> Tail</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ])
}

fn alternate_content_comment_entries_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Selected anchor</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="w14"><w:comment w:id="1" w:author="Reviewer"><w:p><w:r><w:t>Choice comment</w:t></w:r></w:p></w:comment></mc:Choice><mc:Fallback><w:comment w:id="9" w:author="Fallback"><w:p><w:r><w:t>Fallback comment</w:t></w:r></w:p></w:comment></mc:Fallback></mc:AlternateContent></w:comments>"#,
        ),
    ])
}

fn comments_with_blank_ids_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Valid</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p><w:p><w:commentRangeStart w:id=" "/><w:r><w:t>Blank</w:t></w:r><w:commentRangeEnd w:id=" "/><w:r><w:commentReference w:id=" "/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="1" w:author="Reviewer"><w:p><w:r><w:t>Valid comment</w:t></w:r></w:p></w:comment><w:comment w:id=" " w:author="Reviewer"><w:p><w:r><w:t>Blank comment</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ])
}

fn alternate_content_threaded_comments_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.ms-word.commentsExt+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Threaded anchor</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="w14"><w:comment w:id="1" w:author="Reviewer"><w:p w14:paraId="11111111"><w:r><w:t>Choice parent</w:t></w:r></w:p></w:comment><w:comment w:id="2" w:author="Approver"><w:p w14:paraId="22222222"><w:r><w:t>Choice reply</w:t></w:r></w:p></w:comment></mc:Choice><mc:Fallback><w:comment w:id="1" w:author="Fallback"><w:p w14:paraId="aaaaaaaa"><w:r><w:t>Fallback parent</w:t></w:r></w:p></w:comment><w:comment w:id="2" w:author="Fallback"><w:p w14:paraId="bbbbbbbb"><w:r><w:t>Fallback reply</w:t></w:r></w:p></w:comment></mc:Fallback></mc:AlternateContent></w:comments>"#,
        ),
        (
            "word/commentsExtended.xml",
            r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w15:commentEx w15:paraId="11111111" w15:done="0"/><w15:commentEx w15:paraId="22222222" w15:paraIdParent="11111111" w15:done="0"/></w15:commentsEx>"#,
        ),
    ])
}

fn threaded_comments_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.ms-word.commentsExt+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Reviewed clause</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:comment w:id="1" w:author="Reviewer"><w:p w14:paraId="11111111"><w:r><w:t>Original note</w:t></w:r></w:p></w:comment><w:comment w:id="2" w:author="Approver"><w:p w14:paraId="22222222"><w:r><w:t>Reply note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
        (
            "word/commentsExtended.xml",
            r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w15:commentEx w15:paraId="11111111" w15:done="0"/><w15:commentEx w15:paraId="22222222" w15:paraIdParent="11111111" w15:done="0"/></w15:commentsEx>"#,
        ),
    ])
}

fn resolved_comments_extended_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.ms-word.commentsExt+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="0"/><w:commentRangeStart w:id="1"/><w:commentRangeStart w:id="2"/><w:r><w:t>Reviewed clause</w:t></w:r><w:commentRangeEnd w:id="0"/><w:commentRangeEnd w:id="1"/><w:commentRangeEnd w:id="2"/><w:r><w:commentReference w:id="0"/></w:r><w:r><w:commentReference w:id="1"/></w:r><w:r><w:commentReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:comment w:id="0" w:author="Reviewer"><w:p w14:paraId="11111111"><w:r><w:t>done note</w:t></w:r></w:p></w:comment><w:comment w:id="1" w:author="Reviewer"><w:p w14:paraId="22222222"><w:r><w:t>open note</w:t></w:r></w:p></w:comment><w:comment w:id="2" w:author="Reviewer"><w:p w14:paraId="33333333"><w:r><w:t>no-ex note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
        (
            "word/commentsExtended.xml",
            r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w15:commentEx w15:paraId="11111111" w15:done="1"/><w15:commentEx w15:paraId="22222222" w15:done="0"/></w15:commentsEx>"#,
        ),
    ])
}

fn alternate_content_comments_extended_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.ms-word.commentsExt+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Threaded anchor</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:comment w:id="1" w:author="Reviewer"><w:p w14:paraId="11111111"><w:r><w:t>Selected parent</w:t></w:r></w:p></w:comment><w:comment w:id="2" w:author="Approver"><w:p w14:paraId="22222222"><w:r><w:t>Reply</w:t></w:r></w:p></w:comment><w:comment w:id="3" w:author="Fallback"><w:p w14:paraId="33333333"><w:r><w:t>Fallback parent</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
        (
            "word/commentsExtended.xml",
            r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="w15"><w15:commentEx w15:paraId="22222222" w15:paraIdParent="11111111" w15:done="0"/></mc:Choice><mc:Fallback><w15:commentEx w15:paraId="22222222" w15:paraIdParent="33333333" w15:done="0"/></mc:Fallback></mc:AlternateContent></w15:commentsEx>"#,
        ),
    ])
}

fn revision_wrapped_commented_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="7"/><w:r><w:t>Direct anchor</w:t></w:r><w:commentRangeEnd w:id="7"/><w:r><w:commentReference w:id="7"/></w:r></w:p><w:ins w:id="20" w:author="Editor"><w:p><w:commentRangeStart w:id="8"/><w:r><w:t>Inserted anchor</w:t></w:r><w:commentRangeEnd w:id="8"/><w:r><w:commentReference w:id="8"/></w:r></w:p></w:ins><w:moveTo w:id="21" w:author="Editor"><w:p><w:commentRangeStart w:id="9"/><w:r><w:t>Moved-to anchor</w:t></w:r><w:commentRangeEnd w:id="9"/><w:r><w:commentReference w:id="9"/></w:r></w:p></w:moveTo><w:del w:id="22" w:author="Editor"><w:p><w:commentRangeStart w:id="10"/><w:r><w:delText>Deleted anchor</w:delText></w:r><w:commentRangeEnd w:id="10"/><w:r><w:commentReference w:id="10"/></w:r></w:p></w:del><w:moveFrom w:id="23" w:author="Editor"><w:p><w:commentRangeStart w:id="11"/><w:r><w:delText>Moved-from anchor</w:delText></w:r><w:commentRangeEnd w:id="11"/><w:r><w:commentReference w:id="11"/></w:r></w:p></w:moveFrom></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="7" w:author="Reviewer"><w:p><w:r><w:t>Direct note</w:t></w:r></w:p></w:comment><w:comment w:id="8" w:author="Reviewer"><w:p><w:r><w:t>Inserted note</w:t></w:r></w:p></w:comment><w:comment w:id="9" w:author="Reviewer"><w:p><w:r><w:t>Moved-to note</w:t></w:r></w:p></w:comment><w:comment w:id="10" w:author="Reviewer"><w:p><w:r><w:t>Deleted note</w:t></w:r></w:p></w:comment><w:comment w:id="11" w:author="Reviewer"><w:p><w:r><w:t>Moved-from note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ])
}

fn note_commented_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/endnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:p><w:commentRangeStart w:id="7"/><w:r><w:t>Foot anchor</w:t></w:r><w:commentRangeEnd w:id="7"/><w:r><w:commentReference w:id="7"/></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
        (
            "word/endnotes.xml",
            r#"<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:id="2"><w:p><w:commentRangeStart w:id="8"/><w:r><w:t>End anchor</w:t></w:r><w:commentRangeEnd w:id="8"/><w:r><w:commentReference w:id="8"/></w:r></w:p></w:endnote></w:endnotes>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="7" w:author="Reviewer"><w:p><w:r><w:t>Foot note</w:t></w:r></w:p></w:comment><w:comment w:id="8" w:author="Reviewer"><w:p><w:r><w:t>End note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ])
}

fn alternate_content_note_anchor_docx() -> Vec<u8> {
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
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><mc:AlternateContent><mc:Choice Requires="wps"><w:r><w:t>Choice anchor</w:t></w:r></mc:Choice><mc:Fallback><w:r><w:t>Fallback anchor</w:t></w:r></mc:Fallback></mc:AlternateContent><w:r><w:t> tail</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/footnotes.xml",
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:id="1"><w:p><w:r><w:t>Foot body</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
        ),
    ])
}

#[test]
fn docx_comment_replies_use_comments_extended_parent_ids() {
    let doc = Document::open(&threaded_comments_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id, "1");
    assert_eq!(comments[0].parent_comment_id, None);
    assert_eq!(comments[1].id, "2");
    assert_eq!(comments[1].parent_comment_id.as_deref(), Some("1"));
    assert_eq!(comments[1].text, "Reply note");
}

#[test]
fn docx_recovers_comment_resolved_done_state() {
    let doc = Document::open(&resolved_comments_extended_docx()).expect("fixture opens");
    let comments = doc.comments();

    let done = comments.iter().find(|c| c.id == "0").expect("comment 0");
    let open = comments.iter().find(|c| c.id == "1").expect("comment 1");
    let no_ex = comments.iter().find(|c| c.id == "2").expect("comment 2");
    assert_eq!(done.resolved, Some(true));
    assert_eq!(open.resolved, Some(false));
    assert_eq!(no_ex.resolved, None);

    // A document without a commentsExtended part leaves resolved unknown.
    let plain = Document::open(&commented_docx()).expect("fixture opens");
    assert_eq!(plain.comments()[0].resolved, None);
}

#[test]
fn docx_comments_are_extracted() {
    let doc = Document::open(&commented_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, "7");
    assert_eq!(comments[0].author.as_deref(), Some("Reviewer"));
    assert_eq!(comments[0].initials.as_deref(), Some("RV"));
    assert_eq!(comments[0].date.as_deref(), Some("2026-06-24T00:00:00Z"));
    assert_eq!(comments[0].text, "First note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Hello")
    );
}

#[test]
fn docx_comments_ignore_alternate_content_duplicate_branch_text() {
    let doc = Document::open(&alternate_content_commented_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Comment Box Tail");
    assert_eq!(comments[0].text.matches("Box").count(), 1);
    let anchor = comments[0].anchor.as_ref().expect("comment anchor");
    assert_eq!(anchor.text, "Hello Box Tail");
    assert_eq!(anchor.text.matches("Box").count(), 1);
}

#[test]
fn docx_comment_entries_use_single_alternate_content_branch() {
    let doc = Document::open(&alternate_content_comment_entries_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, "1");
    assert_eq!(comments[0].author.as_deref(), Some("Reviewer"));
    assert_eq!(comments[0].text, "Choice comment");
    assert_eq!(
        comments[0]
            .anchor
            .as_ref()
            .map(|anchor| anchor.text.as_str()),
        Some("Selected anchor")
    );
}

#[test]
fn docx_comment_reply_threading_uses_selected_alternate_content_branch() {
    let doc = Document::open(&alternate_content_threaded_comments_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id, "1");
    assert_eq!(comments[0].parent_comment_id, None);
    assert_eq!(comments[0].text, "Choice parent");
    assert_eq!(comments[1].id, "2");
    assert_eq!(comments[1].parent_comment_id.as_deref(), Some("1"));
    assert_eq!(comments[1].text, "Choice reply");
}

#[test]
fn docx_comment_reply_threading_uses_selected_comments_extended_alternate_content_branch() {
    let doc = Document::open(&alternate_content_comments_extended_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].id, "1");
    assert_eq!(comments[0].parent_comment_id, None);
    assert_eq!(comments[1].id, "2");
    assert_eq!(comments[1].parent_comment_id.as_deref(), Some("1"));
    assert_eq!(comments[2].id, "3");
    assert_eq!(comments[2].parent_comment_id, None);
}

#[test]
fn docx_comments_ignore_blank_ids() {
    let doc = Document::open(&comments_with_blank_ids_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, "1");
    assert_eq!(comments[0].text, "Valid comment");
    assert_eq!(
        comments[0]
            .anchor
            .as_ref()
            .map(|anchor| anchor.text.as_str()),
        Some("Valid")
    );
}

#[test]
fn docx_comment_anchors_follow_accepted_revision_view() {
    let doc = Document::open(&revision_wrapped_commented_docx()).expect("fixture opens");

    assert_eq!(
        doc.main_text(),
        "Direct anchor\nInserted anchor\nMoved-to anchor"
    );
    let comments = doc.comments();
    assert_eq!(comments.len(), 5);
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Direct anchor")
    );
    assert_eq!(
        comments[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Inserted anchor")
    );
    assert_eq!(
        comments[2].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Moved-to anchor")
    );
    assert!(comments[3].anchor.is_none());
    assert!(comments[4].anchor.is_none());
}

#[test]
fn docx_comment_anchor_ignores_deleted_range_markers() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="1"/><w:r><w:t>A</w:t></w:r><w:del w:id="2"><w:commentRangeEnd w:id="1"/></w:del><w:r><w:t>B</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="1"><w:p><w:r><w:t>Note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);

    let doc = Document::open(&fixture).expect("fixture opens");
    assert_eq!(doc.main_text(), "AB");
    assert_eq!(
        doc.comments()[0]
            .anchor
            .as_ref()
            .map(|anchor| anchor.text.as_str()),
        Some("AB")
    );
}

#[test]
fn docx_note_comment_anchors_are_exposed() {
    let doc = Document::open(&note_commented_docx()).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].text, "Foot note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Foot anchor")
    );
    assert_eq!(comments[1].text, "End note");
    assert_eq!(
        comments[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("End anchor")
    );
}

#[test]
fn docx_note_anchors_use_single_alternate_content_branch() {
    let doc = Document::open(&alternate_content_note_anchor_docx()).expect("fixture opens");
    let notes = doc.notes();

    assert_eq!(doc.main_text(), "Choice anchor tail");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "Foot body");
    assert_eq!(
        notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
        Some("Choice anchor tail")
    );
}

#[test]
fn docx_comments_preserve_inline_markers_in_text_and_anchor() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="4"/><w:r><w:t>Hello</w:t><w:tab/><w:t>anchor</w:t><w:br/><w:t>no</w:t><w:noBreakHyphen/><w:t>break</w:t><w:softHyphen/><w:t>soft</w:t></w:r><w:commentRangeEnd w:id="4"/><w:r><w:commentReference w:id="4"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="4" w:author="Reviewer"><w:p><w:r><w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:noBreakHyphen/><w:t>2</w:t><w:softHyphen/><w:t>soft</w:t><w:tab/><w:t>Cell</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Line 1\nLine-2\u{00ad}soft\tCell");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Hello\tanchor\nno-break\u{00ad}soft")
    );
}

#[test]
fn docx_comments_preserve_page_break_markers_in_text_and_anchor() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="4"/><w:r><w:t>Before</w:t><w:br w:type="page"/><w:t>After</w:t></w:r><w:commentRangeEnd w:id="4"/><w:r><w:commentReference w:id="4"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="4" w:author="Reviewer"><w:p><w:r><w:t>Line 1</w:t><w:br w:type="page"></w:br><w:t>Line 2</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Line 1\u{000C}Line 2");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Before\u{000C}After")
    );
}

#[test]
fn docx_comment_anchor_preserves_paragraph_boundaries() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="4"/><w:r><w:t>First</w:t></w:r></w:p><w:p><w:r><w:t>Second</w:t></w:r><w:commentRangeEnd w:id="4"/><w:r><w:commentReference w:id="4"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="4" w:author="Reviewer"><w:p><w:r><w:t>Spanning</w:t></w:r></w:p><w:p><w:r><w:t>note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "First\nSecond");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Spanning\nnote");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("First\nSecond")
    );
}

#[test]
fn docx_comments_use_computed_simple_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="4"/><w:fldSimple w:instr=" QUOTE &quot;Fresh anchor&quot; "><w:r><w:t>stale anchor</w:t></w:r></w:fldSimple><w:commentRangeEnd w:id="4"/><w:r><w:commentReference w:id="4"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="4" w:author="Reviewer"><w:p><w:fldSimple w:instr=" QUOTE &quot;Fresh note&quot; "><w:r><w:t>stale note</w:t></w:r></w:fldSimple><w:r><w:t> body</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Fresh anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Fresh note body");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh anchor")
    );
}

#[test]
fn docx_comments_use_computed_complex_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="5"/><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh anchor&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale anchor</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:commentRangeEnd w:id="5"/><w:r><w:commentReference w:id="5"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="5" w:author="Reviewer"><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> QUOTE &quot;Fresh note&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale note</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Fresh anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Fresh note body");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh anchor")
    );
}

#[test]
fn docx_comments_use_computed_dynamic_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="6"/><w:fldSimple w:instr=" IF 1 = 1 &quot;Fresh anchor&quot; &quot;stale branch&quot; "><w:r><w:t>stale anchor</w:t></w:r></w:fldSimple><w:commentRangeEnd w:id="6"/><w:r><w:commentReference w:id="6"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="6" w:author="Reviewer"><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> IF 2 &gt; 1 &quot;Fresh note&quot; &quot;stale branch&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale note</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> body</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Fresh anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Fresh note body");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Fresh anchor")
    );
}

#[test]
fn docx_comments_use_local_field_bookmarks_in_dynamic_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="8"/><w:r><w:t>Selected</w:t></w:r><w:commentRangeEnd w:id="8"/><w:r><w:commentReference w:id="8"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="8" w:author="Reviewer"><w:p><w:fldSimple w:instr=" SET Client &quot;Acme&quot; "/><w:fldSimple w:instr=" IF Client = &quot;Acme&quot; &quot;Fresh note&quot; &quot;stale branch&quot; "><w:r><w:t>stale note</w:t></w:r></w:fldSimple><w:r><w:t> tail</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Fresh note tail");
}

#[test]
fn docx_comments_use_supported_display_and_action_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="9"/><w:fldSimple w:instr=" SYMBOL 183 \f Symbol "><w:r><w:t>stale anchor symbol</w:t></w:r></w:fldSimple><w:commentRangeEnd w:id="9"/><w:r><w:commentReference w:id="9"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="9" w:author="Reviewer"><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> MACROBUTTON RunReport &quot;Fresh note&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale action text</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> tail</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "•");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Fresh note tail");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("•")
    );
}

#[test]
fn docx_comments_hide_supported_toc_and_index_marker_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="10"/><w:fldSimple w:instr=" TC &quot;Anchor entry&quot; "><w:r><w:t>stale anchor marker</w:t></w:r></w:fldSimple><w:r><w:t>Selected</w:t></w:r><w:commentRangeEnd w:id="10"/><w:r><w:commentReference w:id="10"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="10" w:author="Reviewer"><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> XE &quot;Comment index&quot; </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale comment marker</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Selected");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Selected")
    );
}

#[test]
fn docx_comments_use_supported_source_order_numbering_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="11"/><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale anchor one</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> AUTONUM </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale anchor two</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="11"/><w:r><w:commentReference w:id="11"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="11" w:author="Reviewer"><w:p><w:fldSimple w:instr=" AUTONUM "><w:r><w:t>stale comment one</w:t></w:r></w:fldSimple><w:r><w:t> </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> AUTONUM </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>stale comment two</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "1 2 Anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "1 2 note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("1 2 Anchor")
    );
}

#[test]
fn docx_comments_use_supported_document_info_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Side Table Plan</dc:title></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="12"/><w:fldSimple w:instr=" TITLE "><w:r><w:t>stale anchor title</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="12"/><w:r><w:commentReference w:id="12"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="12" w:author="Reviewer"><w:p><w:fldSimple w:instr=" TITLE "><w:r><w:t>stale comment title</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Side Table Plan Anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Side Table Plan note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Side Table Plan Anchor")
    );
}

#[test]
fn docx_comments_use_supported_revision_number_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "docProps/core.xml",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"><cp:revision>17</cp:revision></cp:coreProperties>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="13"/><w:fldSimple w:instr=" REVNUM "><w:r><w:t>stale anchor revision</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="13"/><w:r><w:commentReference w:id="13"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="13" w:author="Reviewer"><w:p><w:fldSimple w:instr=" REVNUM "><w:r><w:t>stale comment revision</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "17 Anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "17 note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("17 Anchor")
    );
}

#[test]
fn docx_comments_use_style_ref_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="14"/><w:r><w:t>Anchor</w:t></w:r><w:commentRangeEnd w:id="14"/><w:r><w:commentReference w:id="14"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="14" w:author="Reviewer"><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Comment Heading</w:t></w:r></w:p><w:p><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale comment style ref</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Comment Heading\nComment Heading note");
}

#[test]
fn docx_comment_anchor_uses_computed_section_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale prior section</w:t></w:r></w:fldSimple></w:p><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:commentRangeStart w:id="19"/><w:r><w:t>Second section </w:t></w:r><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale anchor section</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="19"/><w:r><w:commentReference w:id="19"/></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="19" w:author="Reviewer"><w:p><w:r><w:t>Section note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Section note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Second section 2 Anchor")
    );
}

#[test]
fn docx_comment_anchor_uses_legacy_form_dropdown_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Direct A"/><w:listEntry w:val="Direct B"/></w:ddList></w:ffData><w:r><w:t>stale direct option</w:t></w:r></w:fldSimple></w:p><w:p><w:commentRangeStart w:id="20"/><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Anchor A"/><w:listEntry w:val="Anchor B"/></w:ddList></w:ffData><w:r><w:t>stale anchor option</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="20"/><w:r><w:commentReference w:id="20"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="20" w:author="Reviewer"><w:p><w:r><w:t>Dropdown note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Direct B\nAnchor B Anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Dropdown note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Anchor B Anchor")
    );
}

#[test]
fn docx_comment_anchor_uses_text_form_current_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Direct default"/></w:textInput></w:ffData><w:r><w:t>Direct typed</w:t></w:r></w:fldSimple></w:p><w:p><w:commentRangeStart w:id="21"/><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Anchor default"/></w:textInput></w:ffData><w:r><w:t>Anchor typed</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="21"/><w:r><w:commentReference w:id="21"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="21" w:author="Reviewer"><w:p><w:r><w:t>Text form note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "DIRECT TYPED\nANCHOR TYPED Anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Text form note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("ANCHOR TYPED Anchor")
    );
}

#[test]
fn docx_comment_anchor_uses_style_ref_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/styles.xml",
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Anchor Heading</w:t></w:r></w:p><w:p><w:commentRangeStart w:id="22"/><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale anchor style ref</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="22"/><w:r><w:commentReference w:id="22"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="22" w:author="Reviewer"><w:p><w:r><w:t>Style note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(doc.main_text(), "Anchor Heading\nAnchor Heading Anchor");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Style note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Anchor Heading Anchor")
    );
}

#[test]
fn docx_comments_use_computed_section_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="20"/><w:r><w:t>First anchor</w:t></w:r><w:commentRangeEnd w:id="20"/><w:r><w:commentReference w:id="20"/></w:r></w:p><w:p><w:commentRangeStart w:id="21"/><w:r><w:t>Second anchor</w:t></w:r><w:commentRangeEnd w:id="21"/><w:r><w:commentReference w:id="21"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="20" w:author="Reviewer"><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale first comment section</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment><w:comment w:id="21" w:author="Reviewer"><w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/></w:sectPr></w:pPr></w:p><w:p><w:fldSimple w:instr=" SECTION "><w:r><w:t>stale second comment section</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].text, "1 note");
    assert_eq!(comments[1].text, "2 note");
}

#[test]
fn docx_comments_use_legacy_form_dropdown_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="22"/><w:r><w:t>Anchor</w:t></w:r><w:commentRangeEnd w:id="22"/><w:r><w:commentReference w:id="22"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="22" w:author="Reviewer"><w:p><w:fldSimple w:instr=" FORMDROPDOWN "><w:ffData><w:ddList><w:result w:val="1"/><w:listEntry w:val="Alpha"/><w:listEntry w:val="Beta"/></w:ddList></w:ffData><w:r><w:t>stale dropdown</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Beta note");
}

#[test]
fn docx_comments_use_text_form_current_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="23"/><w:r><w:t>Anchor</w:t></w:r><w:commentRangeEnd w:id="23"/><w:r><w:commentReference w:id="23"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="23" w:author="Reviewer"><w:p><w:fldSimple w:instr=" FORMTEXT \*Upper "><w:ffData><w:textInput><w:default w:val="Comment default"/></w:textInput></w:ffData><w:r><w:t>Comment typed</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "COMMENT TYPED note");
}

#[test]
fn docx_comments_use_complex_text_form_current_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="24"/><w:r><w:t>Anchor</w:t></w:r><w:commentRangeEnd w:id="24"/><w:r><w:commentReference w:id="24"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="24" w:author="Reviewer"><w:p><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:textInput><w:default w:val="Comment default"/></w:textInput></w:ffData></w:fldChar></w:r><w:r><w:instrText> FORMTEXT \*Upper </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Comment typed</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "COMMENT TYPED note");
}

#[test]
fn docx_comments_use_document_bookmark_formula_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceSubtotal"/><w:r><w:t>42</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:commentRangeStart w:id="14"/><w:fldSimple w:instr=" = InvoiceSubtotal + 8 "><w:r><w:t>stale anchor formula</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="14"/><w:r><w:commentReference w:id="14"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="14" w:author="Reviewer"><w:p><w:fldSimple w:instr=" = InvoiceSubtotal + 8 "><w:r><w:t>stale comment formula</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "50 note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("50 Anchor")
    );
}

#[test]
fn docx_comments_use_document_bookmark_conditional_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceTier"/><w:r><w:t>Gold</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:commentRangeStart w:id="15"/><w:fldSimple w:instr=" COMPARE InvoiceTier = &quot;Gold&quot; "><w:r><w:t>stale anchor compare</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="15"/><w:r><w:commentReference w:id="15"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="15" w:author="Reviewer"><w:p><w:fldSimple w:instr=" IF InvoiceTier = &quot;Gold&quot; &quot;ship&quot; &quot;hold&quot; "><w:r><w:t>stale comment if</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "ship note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("1 Anchor")
    );
}

#[test]
fn docx_comments_use_document_bookmark_ref_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="InvoiceTotal"/><w:r><w:t>21</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:commentRangeStart w:id="16"/><w:fldSimple w:instr=" InvoiceTotal \* Ordinal "><w:r><w:t>stale anchor direct ref</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="16"/><w:r><w:commentReference w:id="16"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="16" w:author="Reviewer"><w:p><w:fldSimple w:instr=" REF InvoiceTotal \* CardText "><w:r><w:t>stale comment ref</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "twenty-one note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("21st Anchor")
    );
}

#[test]
fn docx_comments_use_document_note_ref_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="1" w:name="FootOne"/><w:r><w:footnoteReference w:id="1"/></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:commentRangeStart w:id="17"/><w:fldSimple w:instr=" FTNREF FootOne "><w:r><w:t>stale anchor note</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="17"/><w:r><w:commentReference w:id="17"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="17" w:author="Reviewer"><w:p><w:fldSimple w:instr=" NOTEREF FootOne "><w:r><w:t>stale comment note</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "1 note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("1 Anchor")
    );
}

#[test]
fn docx_comments_use_document_toc_field_text() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Executive Summary</w:t></w:r></w:p><w:p><w:commentRangeStart w:id="18"/><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; "><w:r><w:t>stale anchor toc</w:t></w:r></w:fldSimple><w:r><w:t> Anchor</w:t></w:r><w:commentRangeEnd w:id="18"/><w:r><w:commentReference w:id="18"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="18" w:author="Reviewer"><w:p><w:fldSimple w:instr=" TOC \o &quot;1-1&quot; "><w:r><w:t>stale comment toc</w:t></w:r></w:fldSimple><w:r><w:t> note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Executive Summary note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Executive Summary Anchor")
    );
}

#[test]
fn docx_comments_preserve_supported_symbols_in_text_and_anchor() {
    let docx = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="4"/><w:r><w:t>Anchor </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> Text</w:t></w:r><w:commentRangeEnd w:id="4"/><w:r><w:commentReference w:id="4"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="4" w:author="Reviewer"><w:p><w:r><w:t>Review </w:t><w:sym w:font="Symbol" w:char="F0B7"/><w:t> Note</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let doc = Document::open(&docx).expect("fixture opens");
    let comments = doc.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Review • Note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Anchor • Text")
    );
}

#[test]
fn set_comment_text_updates_existing_comment_body() {
    let mut doc = Document::open(&commented_docx()).expect("fixture opens");

    doc.set_comment_text("7", "Updated note")
        .expect("comment text updates");

    assert_eq!(doc.edited_parts(), ["word/comments.xml"]);
    let saved = doc.save().expect("save edited docx");
    let mut reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, "7");
    assert_eq!(comments[0].author.as_deref(), Some("Reviewer"));
    assert_eq!(comments[0].initials.as_deref(), Some("RV"));
    assert_eq!(comments[0].date.as_deref(), Some("2026-06-24T00:00:00Z"));
    assert_eq!(comments[0].text, "Updated note");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Hello")
    );
    assert!(
        reopened.set_comment_text("missing", "x").is_err(),
        "missing comment id should be an error"
    );
}

#[test]
fn set_comment_text_preserves_first_run_properties() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="7"/><w:r><w:t>Hello</w:t></w:r><w:commentRangeEnd w:id="7"/><w:r><w:commentReference w:id="7"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="7" w:author="Reviewer"><w:p><w:r><w:rPr><w:b/><w:color w:val="FF0000"/></w:rPr><w:t>Styled</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t> tail</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");

    doc.set_comment_text("7", "Updated")
        .expect("comment text updates");

    let saved = doc.save().expect("save edited docx");
    let comments_xml = String::from_utf8(unzip_parts(&saved)["word/comments.xml"].clone()).unwrap();
    assert!(
        comments_xml.contains(r#"<w:rPr><w:b/><w:color w:val="FF0000"/></w:rPr>"#),
        "first run properties were dropped: {comments_xml}"
    );
    assert!(
        comments_xml.contains(r#"<w:rPr><w:i/></w:rPr>"#),
        "later run properties were dropped: {comments_xml}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.comments()[0].text, "Updated");
}

#[test]
fn set_comment_text_skips_deleted_comment_body_text() {
    let fixture = docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="7"/><w:r><w:t>Hello</w:t></w:r><w:commentRangeEnd w:id="7"/><w:r><w:commentReference w:id="7"/></w:r></w:p></w:body></w:document>"#,
        ),
        (
            "word/comments.xml",
            r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="7" w:author="Reviewer"><w:p><w:del w:id="1"><w:r><w:delText>OLD</w:delText></w:r></w:del><w:moveFrom w:id="2"><w:r><w:t>OLD</w:t></w:r></w:moveFrom><w:r><w:t>OLD</w:t></w:r></w:p></w:comment></w:comments>"#,
        ),
    ]);
    let mut doc = Document::open(&fixture).expect("fixture opens");
    assert_eq!(doc.comments()[0].text, "OLD");

    doc.set_comment_text("7", "Updated")
        .expect("comment text updates");

    let saved = doc.save().expect("save edited docx");
    let comments_xml = String::from_utf8(unzip_parts(&saved)["word/comments.xml"].clone()).unwrap();
    assert!(
        comments_xml.contains(r#"<w:del w:id="1"><w:r><w:delText>OLD</w:delText></w:r></w:del>"#),
        "deleted comment text changed: {comments_xml}"
    );
    assert!(
        comments_xml.contains(r#"<w:moveFrom w:id="2"><w:r><w:t>OLD</w:t></w:r></w:moveFrom>"#),
        "moved-from comment text changed: {comments_xml}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(reopened.comments()[0].text, "Updated");
}

#[test]
fn set_comment_text_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&commented_docx()).expect("fixture opens");

    doc.set_comment_text("7", "Line 1\nLine\t2")
        .expect("comment text updates");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let comments = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    assert!(
        comments.contains(r#"<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"#),
        "updated comment text should encode tabs and breaks as WML markers: {comments}"
    );
    assert!(
        !comments.contains("Line 1\nLine\t2"),
        "updated comment text should not keep raw tab/break characters in one w:t: {comments}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Line 1\nLine\t2");
}

#[test]
fn add_comment_on_text_creates_comments_part_relationship_and_anchor() {
    let mut doc = Document::open(&plain_docx()).expect("fixture opens");

    let id = doc
        .add_comment_on_text("Hello", "Note <one> & two", "Reviewer")
        .expect("comment added");

    assert_eq!(id, "0");
    assert_eq!(
        doc.edited_parts(),
        [
            "[Content_Types].xml",
            "word/_rels/document.xml.rels",
            "word/comments.xml",
            "word/document.xml"
        ]
    );
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let comments = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();

    let start = body.find(r#"<w:commentRangeStart"#).unwrap_or(usize::MAX);
    let anchor = body.find(r#"<w:t>Hello</w:t>"#).unwrap_or(usize::MAX);
    let end = body.find(r#"<w:commentRangeEnd"#).unwrap_or(usize::MAX);
    let reference = body.find(r#"<w:commentReference"#).unwrap_or(usize::MAX);
    assert!(
        start < anchor && anchor < end && end < reference && body.contains(r#"w:id="0""#),
        "body anchor missing or out of order: {body}"
    );
    assert!(
        comments.contains(r#"<w:comment "#)
            && comments.contains(r#"w:id="0""#)
            && comments.contains(r#"w:author="Reviewer""#),
        "comment metadata missing: {comments}"
    );
    assert!(
        comments.contains(r#"<w:t>Note &lt;one&gt; &amp; two</w:t>"#),
        "comment text not escaped: {comments}"
    );
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml""#
        ),
        "comments relationship missing: {rels}"
    );
    assert!(
        ct.contains(r#"PartName="/word/comments.xml""#)
            && ct.contains("wordprocessingml.comments+xml"),
        "comments content type missing: {ct}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, "0");
    assert_eq!(comments[0].author.as_deref(), Some("Reviewer"));
    assert_eq!(comments[0].text, "Note <one> & two");
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Hello")
    );
}

#[test]
fn add_comment_on_text_preserves_edge_whitespace() {
    let mut doc = Document::open(&plain_docx()).expect("fixture opens");

    doc.add_comment_on_text("Hello", " Padded note ", "Reviewer")
        .expect("comment added");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let comments = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    assert!(
        comments.contains(r#"<w:t xml:space="preserve"> Padded note </w:t>"#),
        "comment text should preserve edge whitespace: {comments}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, " Padded note ");
}

#[test]
fn add_comment_on_text_writes_tabs_and_breaks_as_markers() {
    let mut doc = Document::open(&plain_docx()).expect("fixture opens");

    doc.add_comment_on_text("Hello", "Line 1\nLine\t2", "Reviewer")
        .expect("comment added");

    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let comments = String::from_utf8(parts["word/comments.xml"].clone()).unwrap();
    assert!(
        comments.contains(r#"<w:t>Line 1</w:t><w:br/><w:t>Line</w:t><w:tab/><w:t>2</w:t>"#),
        "comment text should encode tabs and breaks as WML markers: {comments}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Line 1\nLine\t2");
}

#[test]
fn add_comment_on_text_can_anchor_across_adjacent_runs() {
    let mut doc = Document::open(&split_run_docx()).expect("fixture opens");

    let id = doc
        .add_comment_on_text("Hello", "Split note", "Reviewer")
        .expect("comment added across split runs");

    assert_eq!(id, "0");
    let saved = doc.save().expect("save edited docx");
    let parts = unzip_parts(&saved);
    let body = String::from_utf8(parts["word/document.xml"].clone()).unwrap();

    let start = body.find(r#"<w:commentRangeStart"#).unwrap_or(usize::MAX);
    let first = body.find(r#"<w:t>Hel</w:t>"#).unwrap_or(usize::MAX);
    let second = body.find(r#"<w:t>lo</w:t>"#).unwrap_or(usize::MAX);
    let end = body.find(r#"<w:commentRangeEnd"#).unwrap_or(usize::MAX);
    let reference = body.find(r#"<w:commentReference"#).unwrap_or(usize::MAX);
    let tail = body.find(r#"<w:t>!</w:t>"#).unwrap_or(usize::MAX);
    assert!(
        start < first && first < second && second < end && end < reference && reference < tail,
        "split-run comment anchor missing or misplaced: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();
    assert_eq!(comments.len(), 1);
    assert_eq!(
        comments[0].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Hello")
    );
}

#[test]
fn add_comment_on_text_anchors_across_accepted_revision_wrappers() {
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

    doc.add_comment_on_text("Hello", "Revision note", "Reviewer")
        .expect("comment added across accepted revision wrappers");

    let saved = doc.save().expect("save edited docx");
    let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
    let start = body.find("<w:commentRangeStart").unwrap_or(usize::MAX);
    let first = body.find("<w:t>Hel</w:t>").unwrap_or(usize::MAX);
    let second = body.find("<w:t>l</w:t>").unwrap_or(usize::MAX);
    let third = body.find("<w:t>o</w:t>").unwrap_or(usize::MAX);
    let end = body.find("<w:commentRangeEnd").unwrap_or(usize::MAX);
    let reference = body.find("<w:commentReference").unwrap_or(usize::MAX);
    let tail = body.find("<w:t>!</w:t>").unwrap_or(usize::MAX);
    assert!(
        start < first
            && first < second
            && second < third
            && third < end
            && end < reference
            && reference < tail,
        "revision-spanning comment anchor missing or misplaced: {body}"
    );

    let reopened = Document::open(&saved).expect("reopen edited docx");
    assert_eq!(
        reopened.comments()[0]
            .anchor
            .as_ref()
            .map(|anchor| anchor.text.as_str()),
        Some("Hello")
    );
}

#[test]
fn add_comment_on_text_uses_next_comment_id_and_preserves_existing_comments() {
    let mut doc = Document::open(&commented_docx()).expect("fixture opens");

    let id = doc
        .add_comment_on_text("Hello", "Second note", "Reviewer 2")
        .expect("comment added");

    assert_eq!(id, "8");
    assert_eq!(
        doc.edited_parts(),
        ["word/comments.xml", "word/document.xml"]
    );
    let saved = doc.save().expect("save edited docx");
    let reopened = Document::open(&saved).expect("reopen edited docx");
    let comments = reopened.comments();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id, "7");
    assert_eq!(comments[0].text, "First note");
    assert_eq!(comments[1].id, "8");
    assert_eq!(comments[1].author.as_deref(), Some("Reviewer 2"));
    assert_eq!(comments[1].text, "Second note");
    assert_eq!(
        comments[1].anchor.as_ref().map(|a| a.text.as_str()),
        Some("Hello")
    );

    let mut missing = reopened;
    assert!(
        missing
            .add_comment_on_text("Missing anchor", "Nope", "Reviewer")
            .is_err(),
        "missing anchor text should be an error"
    );
}
