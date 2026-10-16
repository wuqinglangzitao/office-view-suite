//! `SttbfFfn` — the font-name table ([MS-DOC] 2.9.272), resolving a CHPX font
//! index (`sprmCRgFtc0`) to a family name for the rich model.
//!
//! `SttbfFfn` is an STTB whose elements are `FFN` structures. The STTB may be
//! "extended" (a leading `0xFFFF`, then 2-byte `cchData` and 2-byte chars) or
//! plain (2-byte `cData`, then 1-byte `cchData`). Either way, each FFN's
//! `xszFfn` name is **UTF-16** sitting past a binary fixed prefix
//! (flags/wWeight/chs/panose/fs); we locate it by taking the longest
//! null-terminated run of name-like UTF-16 units in the element.

use crate::util::u16le;

/// Parse `SttbfFfn` into font names indexed by font index (`ftc`). Returns an
/// empty list on absence/malformation — fonts then fall back to defaults.
pub(crate) fn parse(table: &[u8], fc: usize, lcb: usize) -> Vec<String> {
    let mut out = Vec::new();
    let Some(data) = table.get(fc..fc.saturating_add(lcb)) else {
        return out;
    };
    if data.len() < 6 {
        return out;
    }
    // Extended STTB iff the first u16 is the 0xFFFF marker.
    let extended = u16le(data, 0) == Some(0xFFFF);
    let (mut pos, count) = if extended {
        (6usize, u16le(data, 2).unwrap_or(0) as usize) // skip fExtend, cData, cbExtra
    } else {
        (4usize, u16le(data, 0).unwrap_or(0) as usize) // skip cData, cbExtra
    };
    let cb_char = if extended { 2 } else { 1 };
    for _ in 0..count.min(8192) {
        let cch = if extended {
            let Some(c) = u16le(data, pos) else { break };
            pos += 2;
            c as usize
        } else {
            let Some(&c) = data.get(pos) else { break };
            pos += 1;
            c as usize
        };
        let elem_len = cch.saturating_mul(cb_char);
        let Some(elem) = data.get(pos..pos.saturating_add(elem_len)) else {
            break;
        };
        out.push(ffn_name(elem));
        pos = pos.saturating_add(elem_len);
    }
    out
}

/// The font name for index `ftc`, if present and non-empty.
pub(crate) fn name_of(names: &[String], ftc: u16) -> Option<String> {
    names.get(ftc as usize).filter(|s| !s.is_empty()).cloned()
}

/// `true` for a UTF-16 unit that plausibly belongs to a font name (ASCII
/// printable, Hangul, kana, or CJK), so the scan skips the binary FFN prefix.
fn name_like(u: u16) -> bool {
    let c = u as u32;
    c == 0x20
        || (0x21..=0x7E).contains(&c)
        || (0x3040..=0x30FF).contains(&c) // kana
        || (0x3130..=0x318F).contains(&c) // Hangul compatibility jamo
        || (0x4E00..=0x9FFF).contains(&c) // CJK unified
        || (0xAC00..=0xD7A3).contains(&c) // Hangul syllables
        || (0xF900..=0xFAFF).contains(&c) // CJK compatibility ideographs
}

/// Extract `xszFfn` (UTF-16) from one FFN element by taking the longest
/// null-terminated run of name-like units — the font name, never the small
/// binary prefix values.
fn ffn_name(elem: &[u8]) -> String {
    let units: Vec<u16> = elem
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let mut best = String::new();
    let mut best_len = 0usize; // char count of `best`, tracked so we never recount it
    let mut i = 0;
    while i < units.len() {
        if !name_like(units[i]) {
            i += 1;
            continue;
        }
        let mut j = i;
        let mut s = String::new();
        let mut s_len = 0usize;
        while j < units.len() && units[j] != 0 && name_like(units[j]) {
            if let Some(ch) = char::from_u32(units[j] as u32) {
                s.push(ch);
                s_len += 1;
            }
            j += 1;
        }
        let terminated = j < units.len() && units[j] == 0;
        // Compare tracked lengths, not `best.chars().count()` per candidate — recounting the
        // current best made one long name + many short runs O(L×K) within a bounded element.
        if terminated && s_len > best_len {
            best = s;
            best_len = s_len;
        }
        i = j + 1;
    }
    best.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::parse;

    /// A synthetic extended `SttbfFfn` with one UTF-16 font name past a 40-byte
    /// binary FFN prefix — proves the name extractor when names are Unicode.
    #[test]
    fn extracts_utf16_font_name() {
        let mut d = vec![0xFF, 0xFF, 1, 0, 0, 0]; // fExtend, cData=1, cbExtra=0
        d.extend_from_slice(&26u16.to_le_bytes()); // cchData = 26 chars (52 bytes)
        d.extend(std::iter::repeat_n(0u8, 40)); // FFN fixed prefix
        for u in "맑은 고딕".encode_utf16() {
            d.extend_from_slice(&u.to_le_bytes());
        }
        d.extend_from_slice(&[0, 0]); // NUL terminator
        let names = parse(&d, 0, d.len());
        assert_eq!(names, vec!["맑은 고딕".to_string()]);
        assert_eq!(super::name_of(&names, 0).as_deref(), Some("맑은 고딕"));
    }

    #[test]
    fn longest_terminated_name_wins_among_many_runs() {
        // One FFN element with several NUL-terminated name-like runs; the longest must win.
        // (Regression for the O(L×K) scan that recounted the best string per candidate.)
        let mut d = vec![0xFF, 0xFF, 1, 0, 0, 0]; // fExtend, cData=1, cbExtra=0
        let content: Vec<u16> = "AB\u{0}LONGER\u{0}C\u{0}".encode_utf16().collect();
        d.extend_from_slice(&(content.len() as u16).to_le_bytes()); // cchData
        for u in &content {
            d.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(parse(&d, 0, d.len()), vec!["LONGER".to_string()]);
    }

    #[test]
    fn empty_or_garbage_yields_no_names() {
        assert!(parse(&[], 0, 0).is_empty());
        // Binary prefix bytes must never surface as a (garbage) name.
        assert!(parse(
            &[0xFF, 0xFF, 1, 0, 0, 0, 4, 0, 0x0B, 0, 5, 0, 3, 0, 2, 0],
            0,
            16
        )[0]
        .is_empty());
    }
}
