//! Runtime editing of `<p:txBody>` text frames on imported decks.
//!
//! python-pptx parity:
//! - `Shape.has_text_frame` (predicate on the parsed shape) lives in
//!   [`super::slide::ParsedShape::has_text_frame`].
//! - `Shape.text_frame.text = "…"` is exposed here as
//!   [`super::editor::PresentationEditor::set_shape_text`] (clears existing
//!   paragraphs, sets the first).
//! - `TextFrame.add_paragraph()` is exposed here as
//!   [`super::editor::PresentationEditor::add_paragraph`].
//! - `Paragraph.level` / `Paragraph.font.bold` are applied at write time
//!   via [`super::editor::PresentationEditor::add_paragraph_with_format`].
//!
//! The implementation works at the byte level on the slide's XML part
//! because `XmlElement` (xmlchemy) is read-only. String operations are safe
//! because `<p:sp>`, `<p:txBody>`, `<a:p>`, and `</a:p>` / `</p:txBody>` /
//! `</p:sp>` are well-formed, never nested, and emitted by ppt-rs itself —
//! so there's no risk of matching the wrong element.
//!
//! Convention: when the slide XML has no `<p:txBody>` for the target shape,
//! mutation methods return [`crate::exc::PptxError::NotFound`] rather than
//! silently creating one. Use [`crate::exc::PptxError`] (via `Result`) at
//! the public API surface — see `PresentationEditor::set_shape_text` etc.

use super::slide::SlideParser;

/// Builder for `<a:p>` paragraph XML used by runtime text edits.
///
/// Mirrors the generator's `Run::to_xml` / paragraph format so emitted XML
/// matches what `SlideXmlBuilder` would have produced. Avoids the 1800-point
/// default — runtime edits default to 1800 (18pt) which is the visual
/// default for body text.
#[derive(Debug, Clone)]
pub struct ParagraphEdit<'a> {
    pub text: &'a str,
    pub level: u32,
    pub bold: bool,
    pub italic: bool,
    pub font_size_pts: u32,
}

impl<'a> ParagraphEdit<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            level: 0,
            bold: false,
            italic: false,
            font_size_pts: 18,
        }
    }

    pub fn with_level(mut self, level: u32) -> Self {
        self.level = level.min(8);
        self
    }

    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    pub fn with_font_size(mut self, pts: u32) -> Self {
        self.font_size_pts = pts.max(8);
        self
    }

    /// Emit the `<a:p>…</a:p>` XML for this paragraph.
    pub fn to_xml(&self) -> String {
        let lvl_attr = if self.level > 0 {
            format!(r#" lvl="{}""#, self.level)
        } else {
            String::new()
        };
        let bold = if self.bold { "1" } else { "0" };
        let italic = if self.italic { "1" } else { "0" };
        // OOXML font size is in hundredths of a point (sz="1800" = 18pt).
        let sz = self.font_size_pts * 100;
        let escaped = escape_xml(self.text);
        format!(
            r#"<a:p><a:pPr{lvl_attr}><a:defRPr/></a:pPr><a:r><a:rPr lang="en-US" sz="{sz}" b="{bold}" i="{italic}" dirty="0"/><a:t>{escaped}</a:t></a:r></a:p>"#,
        )
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Find the byte range of the `shape_idx`-th `<p:sp>` element (0-based) in
/// `slide_xml`. Returns `(start_offset, end_offset_inclusive_of_close_tag)`.
///
/// Uses a depth counter so nested `<p:sp>` (none in practice, but the
/// schema permits) are skipped correctly. Returns `None` if the index is
/// out of range.
///
/// Iterates by **char boundaries**, not bytes — slide XML frequently
/// contains non-ASCII text (e.g. `•` in `Bullet 1`, `&` entities). Walking
/// byte-by-byte and slicing `&slide_xml[i..]` panics with "not a char
/// boundary" when `i` lands inside a multi-byte char. When no tag prefix
/// matches, we advance `i` by the size of the current char (a single
/// `char_indices()` step).
pub(crate) fn nth_sp_range(slide_xml: &str, shape_idx: usize) -> Option<(usize, usize)> {
    const OPEN_SHORT: &str = "<p:sp>";
    const OPEN_ATTR: &str = "<p:sp ";
    const CLOSE: &str = "</p:sp>";

    let mut depth: usize = 0;
    let mut count: usize = 0;
    let mut start: Option<usize> = None;

    let mut i = 0;
    while i < slide_xml.len() {
        let rest = &slide_xml[i..];
        if rest.starts_with(OPEN_SHORT) || rest.starts_with(OPEN_ATTR) {
            if depth == 0 && count == shape_idx {
                start = Some(i);
            }
            depth += 1;
            i += OPEN_SHORT.len();
        } else if rest.starts_with(CLOSE) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                if let Some(s) = start {
                    return Some((s, i + CLOSE.len()));
                }
                count += 1;
            }
            i += CLOSE.len();
        } else {
            // Advance past the current char so we land on the next char
            // boundary. `char_indices()`'s first pair is at byte 0 of `rest`
            // (the current char); advancing by its length skips it.
            let cur_char_len = rest
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            i += cur_char_len;
        }
    }
    None
}

/// Locate the byte range of the inner content (between `<p:txBody>` and
/// `</p:txBody>`) within a `<p:sp>` substring. Returns `(inner_start, inner_end)`
/// byte offsets relative to `sp_xml`.
pub(crate) fn txbody_range(sp_xml: &str) -> Option<(usize, usize)> {
    let start_tag = "<p:txBody>";
    let end_tag = "</p:txBody>";
    let start = sp_xml.find(start_tag)? + start_tag.len();
    let end = sp_xml[start..].find(end_tag).map(|o| start + o)?;
    Some((start, end))
}

/// Count existing `<a:p>` paragraphs inside a txBody — used to decide where
/// to insert on `add_paragraph` (we append before `</p:txBody>`, no count
/// needed, but kept for diagnostics).
pub fn count_paragraphs_in_txbody(sp_xml: &str) -> usize {
    let Some((start, end)) = txbody_range(sp_xml) else {
        return 0;
    };
    sp_xml[start..end].matches("<a:p>").count()
        + sp_xml[start..end].matches("<a:p ").count()
}

/// Read a slide's XML, return the inner paragraphs of the nth shape's txBody
/// (read-only — used by tests/diagnostics).
pub fn read_paragraphs(slide_xml: &str, shape_idx: usize) -> Vec<String> {
    let Some((sp_start, sp_end)) = nth_sp_range(slide_xml, shape_idx) else {
        return Vec::new();
    };
    let sp_xml = &slide_xml[sp_start..sp_end];
    let Some((tb_start, tb_end)) = txbody_range(sp_xml) else {
        return Vec::new();
    };
    let inner = &sp_xml[tb_start..tb_end];
    // Wrap the inner paragraphs in a `<p:txBody>` so the parser's
    // `find_descendant("txBody")` resolves. The parser only walks into
    // `<p:sp>` children of `<p:spTree>` — that's the schema shape we
    // reproduce here.
    let wrapped = format!(
        r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="999" name="X"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:txBody>{inner}</p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
    );
    let parsed = SlideParser::parse(&wrapped);
    parsed
        .map(|s| {
            s.shapes
                .into_iter()
                .flat_map(|sh| sh.paragraphs.into_iter().map(|p| p.text()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
        <p:txBody><a:p><a:r><a:t>Original Title</a:t></a:r></a:p></p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="3" name="Body"/><p:cNvSpPr/><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr>
        <p:txBody>
          <a:p><a:r><a:t>Bullet 1</a:t></a:r></a:p>
          <a:p><a:r><a:t>Bullet 2</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

    #[test]
    fn paragraph_edit_emits_clean_xml() {
        let p = ParagraphEdit::new("Hello").with_bold(true).with_font_size(24);
        let xml = p.to_xml();
        assert!(xml.starts_with("<a:p>"));
        assert!(xml.contains("Hello"));
        assert!(xml.contains(r#"b="1""#));
        assert!(xml.contains(r#"sz="2400""#));
    }

    #[test]
    fn paragraph_edit_escapes_special_chars() {
        let p = ParagraphEdit::new("a < b & c > d");
        let xml = p.to_xml();
        assert!(xml.contains("&lt;"));
        assert!(xml.contains("&amp;"));
        assert!(xml.contains("&gt;"));
        assert!(!xml.contains("a < b"));
    }

    #[test]
    fn paragraph_edit_with_level_sets_lvl_attr() {
        let p = ParagraphEdit::new("Nested").with_level(2);
        let xml = p.to_xml();
        assert!(xml.contains(r#"lvl="2""#));
    }

    #[test]
    fn paragraph_edit_no_level_omits_lvl_attr() {
        let p = ParagraphEdit::new("Top level");
        let xml = p.to_xml();
        assert!(!xml.contains("lvl="));
    }

    #[test]
    fn nth_sp_range_first() {
        let (start, end) = nth_sp_range(SAMPLE, 0).unwrap();
        let sp = &SAMPLE[start..end];
        assert!(sp.contains("Original Title"));
        assert!(!sp.contains("Bullet 1"));
    }

    #[test]
    fn nth_sp_range_second() {
        let (start, end) = nth_sp_range(SAMPLE, 1).unwrap();
        let sp = &SAMPLE[start..end];
        assert!(sp.contains("Bullet 1"));
        assert!(!sp.contains("Original Title"));
    }

    #[test]
    fn nth_sp_range_out_of_bounds() {
        assert!(nth_sp_range(SAMPLE, 5).is_none());
    }

    #[test]
    fn read_paragraphs_returns_each_paragraph_text() {
        let paras = read_paragraphs(SAMPLE, 1);
        assert_eq!(paras, vec!["Bullet 1", "Bullet 2"]);
    }

    #[test]
    fn txbody_range_finds_inner_offsets() {
        let (sp_start, sp_end) = nth_sp_range(SAMPLE, 1).unwrap();
        let sp_xml = &SAMPLE[sp_start..sp_end];
        let (start, end) = txbody_range(sp_xml).unwrap();
        let inner = &sp_xml[start..end];
        assert!(inner.contains("Bullet 1"));
        assert!(inner.contains("Bullet 2"));
        assert!(!inner.contains("<p:txBody>"));
        assert!(!inner.contains("</p:txBody>"));
    }
}