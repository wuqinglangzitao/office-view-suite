//! A minimal Open Packaging Conventions (OPC) writer — assembles the
//! `[Content_Types].xml`, `_rels/*.rels`, and part streams of an OOXML package
//! (`.docx`/`.xlsx`) into a ZIP. Shared by the Word writer (and reusable by a
//! future Excel writer), so the packaging rules live in one place.

use std::io::Write;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::esc_attr;

const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// One relationship entry of a `_rels/*.rels` part.
pub(crate) struct Rel {
    /// Relationship id, referenced from the body (`r:id`, `r:embed`).
    pub id: String,
    /// Relationship type URI.
    pub rel_type: String,
    /// Target part path or external URL.
    pub target: String,
    /// `true` ⇒ `TargetMode="External"` (hyperlinks); `false` ⇒ internal part.
    pub external: bool,
}

/// Accumulates the parts, content types, and relationship files of an OOXML
/// package, then serializes everything to ZIP bytes.
pub(crate) struct Package {
    parts: Vec<(String, Vec<u8>)>,
    defaults: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
    rels: Vec<(String, Vec<Rel>)>,
}

impl Package {
    /// A new package, pre-seeded with the mandatory `rels` and `xml` defaults.
    pub(crate) fn new() -> Self {
        Package {
            parts: Vec::new(),
            defaults: vec![
                ("rels".into(), CT_RELS.into()),
                ("xml".into(), "application/xml".into()),
            ],
            overrides: Vec::new(),
            rels: Vec::new(),
        }
    }

    /// Register a default content type for a file extension (e.g. `png`). Idempotent.
    pub(crate) fn add_default(&mut self, ext: &str, content_type: &str) {
        if !self.defaults.iter().any(|(e, _)| e == ext) {
            self.defaults
                .push((ext.to_string(), content_type.to_string()));
        }
    }

    /// Add a part at `path` (no leading slash). `content_type` `Some` ⇒ an
    /// `<Override>` is emitted; `None` ⇒ it relies on the extension `<Default>`.
    pub(crate) fn add_part(&mut self, path: &str, content_type: Option<&str>, bytes: Vec<u8>) {
        if let Some(ct) = content_type {
            self.overrides.push((format!("/{path}"), ct.to_string()));
        }
        self.parts.push((path.to_string(), bytes));
    }

    /// Add a `_rels/*.rels` file at `rels_path` with the given relationships.
    pub(crate) fn add_rels(&mut self, rels_path: &str, rels: Vec<Rel>) {
        self.rels.push((rels_path.to_string(), rels));
    }

    /// Serialize the whole package to ZIP bytes. Fallible only on the (in practice
    /// unreachable) in-memory ZIP write error, which the public `try_write_docx` /
    /// `save` APIs surface instead of yielding empty bytes.
    pub(crate) fn try_into_zip(self) -> std::io::Result<Vec<u8>> {
        let mut zw = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();

        // [Content_Types].xml
        let mut ct = String::new();
        ct.push_str(XML_DECL);
        ct.push_str(&format!(r#"<Types xmlns="{CT_NS}">"#));
        for (ext, c) in &self.defaults {
            ct.push_str(&format!(
                r#"<Default Extension="{}" ContentType="{}"/>"#,
                esc_attr(ext),
                esc_attr(c)
            ));
        }
        for (pn, c) in &self.overrides {
            ct.push_str(&format!(
                r#"<Override PartName="{}" ContentType="{}"/>"#,
                esc_attr(pn),
                esc_attr(c)
            ));
        }
        ct.push_str("</Types>");
        zw.start_file("[Content_Types].xml", opt)?;
        zw.write_all(ct.as_bytes())?;

        // _rels/*.rels files
        for (path, rels) in &self.rels {
            let mut x = String::new();
            x.push_str(XML_DECL);
            x.push_str(&format!(r#"<Relationships xmlns="{REL_NS}">"#));
            for rel in rels {
                x.push_str(&format!(
                    r#"<Relationship Id="{}" Type="{}" Target="{}""#,
                    esc_attr(&rel.id),
                    esc_attr(&rel.rel_type),
                    esc_attr(&rel.target)
                ));
                if rel.external {
                    x.push_str(r#" TargetMode="External""#);
                }
                x.push_str("/>");
            }
            x.push_str("</Relationships>");
            zw.start_file(path.as_str(), opt)?;
            zw.write_all(x.as_bytes())?;
        }

        // content parts
        for (path, bytes) in &self.parts {
            zw.start_file(path.as_str(), opt)?;
            zw.write_all(bytes)?;
        }

        Ok(zw.finish()?.into_inner())
    }
}
