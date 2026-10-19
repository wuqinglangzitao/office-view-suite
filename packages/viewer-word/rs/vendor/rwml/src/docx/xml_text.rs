//! Shared XML text helpers for `.docx` parts.

use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::Reader;

use super::{is_page_break_type, local};

#[derive(Default)]
pub(crate) struct AlternateContentBranchState {
    took_branch: bool,
}

pub(crate) fn skip_alternate_content_branch(
    stack: &mut [AlternateContentBranchState],
    name: &[u8],
) -> bool {
    if !matches!(name, b"Choice" | b"Fallback") {
        return false;
    }
    let Some(state) = stack.last_mut() else {
        return false;
    };
    if state.took_branch {
        true
    } else {
        state.took_branch = true;
        false
    }
}

pub(crate) fn skip_subtree(r: &mut Reader<&[u8]>) {
    let mut depth = 1usize;
    loop {
        match r.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// Read text content through the current element's end.
///
/// `unescape` resolves standard XML entities but errors on unknown/custom
/// entities. In that case keep the raw text verbatim rather than dropping the
/// node or resolving external entities.
pub(crate) fn read_text(r: &mut Reader<&[u8]>) -> String {
    let mut s = String::new();
    loop {
        match r.read_event() {
            Ok(Event::Text(t)) => match t.decode().ok().map(|c| c.into_owned()) {
                Some(c) => s.push_str(&c),
                None => s.push_str(&String::from_utf8_lossy(t.into_inner().as_ref())),
            },
            Ok(Event::GeneralRef(reference)) => match resolve_reference(&reference) {
                Some(value) => s.push_str(&value),
                None => {
                    // Preserve unknown general entities exactly as the pre-0.41 text
                    // unescape fallback did; never attempt external entity resolution.
                    s.push('&');
                    s.push_str(&String::from_utf8_lossy(reference.as_ref()));
                    s.push(';');
                }
            },
            Ok(Event::CData(t)) => s.push_str(&String::from_utf8_lossy(t.into_inner().as_ref())),
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    s
}

/// Resolve numeric and the five predefined XML entity references. Unknown general
/// entities intentionally remain unresolved: OOXML does not define a DTD resolver.
pub(crate) fn resolve_reference(reference: &BytesRef<'_>) -> Option<String> {
    if let Some(character) = reference.resolve_char_ref().ok()? {
        return Some(character.to_string());
    }
    let name = reference.decode().ok()?;
    quick_xml::escape::resolve_predefined_entity(&name).map(str::to_owned)
}

pub(crate) fn read_i64_text(r: &mut Reader<&[u8]>) -> Option<i64> {
    read_text(r).trim().parse().ok()
}

pub(crate) fn inline_marker_text(e: &BytesStart<'_>) -> Option<&'static str> {
    match local(e.name().as_ref()) {
        b"tab" => Some("\t"),
        b"br" => {
            if is_page_break_type(e) {
                Some("\u{000C}")
            } else {
                Some("\n")
            }
        }
        b"cr" => Some("\n"),
        b"noBreakHyphen" => Some("-"),
        b"softHyphen" => Some("\u{00ad}"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{read_i64_text, read_text};
    use quick_xml::events::Event;
    use quick_xml::Reader;

    #[test]
    fn read_text_unescapes_cdata_and_keeps_unknown_entities_raw() {
        let mut r = Reader::from_str("<w:t>A &amp; B<![CDATA[ <C> ]]>&unknown;</w:t>");
        assert!(matches!(r.read_event(), Ok(Event::Start(_))));

        assert_eq!(read_text(&mut r), "A & B <C> &unknown;");
    }

    #[test]
    fn read_i64_text_trims_ooxml_text() {
        let mut r = Reader::from_str("<wp:posOffset> 91440 </wp:posOffset>");
        assert!(matches!(r.read_event(), Ok(Event::Start(_))));

        assert_eq!(read_i64_text(&mut r), Some(91_440));
    }
}
