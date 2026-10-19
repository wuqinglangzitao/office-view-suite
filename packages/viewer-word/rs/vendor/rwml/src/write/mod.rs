//! Writing the document model back out to a file — currently the modern `.docx`
//! (OOXML WordprocessingML) serializer, the inverse of the `docx` reader. Behind
//! the same `docx` cargo feature (it reuses the `zip` dependency; no extra deps).
//!
//! This is the round-trip / unification half of the crate: read `.doc` *or*
//! `.docx` into one [`crate::DocModel`], then emit a clean, Office-openable
//! `.docx`. The mapping mirrors the reader exactly (`outlineLvl` ⇄ heading level,
//! `numId` ⇄ list, `gridSpan`/`vMerge` ⇄ table merges, `a:blip` ⇄ image, hyperlink
//! relationships), so `read → write → read` preserves the model's structure.

mod docx;
mod opc;

pub(crate) use docx::{to_docx, to_docx_with_source_hints, try_to_docx, SourceWriteHints};

fn is_xml_legal_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || matches!(
            c as u32,
            0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
        )
}

/// Escape text content for an XML element body (`&`, `<`, `>`).
pub(crate) fn esc_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            // Drop XML-1.0-illegal scalar values so untrusted text can never emit
            // malformed XML.
            c if !is_xml_legal_char(c) => {}
            c => o.push(c),
        }
    }
    o
}

/// Escape an XML attribute value (`&`, `<`, `>`, `"`), dropping XML-illegal
/// control characters.
pub(crate) fn esc_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if !is_xml_legal_char(c) => {}
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::{esc_attr, esc_text};

    #[test]
    fn escapers_strip_xml_illegal_controls() {
        // Vertical tab / form feed / unit separator are illegal in XML 1.0 and
        // must be dropped (so a crafted .doc URL or text can't emit malformed XML).
        assert_eq!(esc_text("a\u{0B}b\u{1F}c"), "abc");
        assert_eq!(esc_attr("u\u{0C}r\u{01}l"), "url");
        // The standard entities are escaped.
        assert_eq!(esc_text("a&<>b"), "a&amp;&lt;&gt;b");
        assert_eq!(esc_attr("\"x\"&"), "&quot;x&quot;&amp;");
        // Tab / newline / CR are legal and preserved.
        assert_eq!(esc_text("a\tb\nc\r"), "a\tb\nc\r");
    }

    #[test]
    fn escapers_strip_xml_forbidden_scalars() {
        assert_eq!(esc_text("a\u{FFFE}b\u{FFFF}c"), "abc");
        assert_eq!(esc_attr("a\u{FFFE}\"b\u{FFFF}"), "a&quot;b");
    }
}
