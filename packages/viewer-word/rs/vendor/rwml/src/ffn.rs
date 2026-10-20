//! `SttbfFfn` — the font-name table ([MS-DOC] 2.9.272), resolving a CHPX font
//! index (`sprmCRgFtc0`) to a family name for the rich model.
//!
//! This is a non-extended STTB with byte-sized FFN record lengths. Each UTF-16
//! name begins after the 39-byte fixed prefix, independently of stream alignment.

use crate::util::u16le;

/// Parse `SttbfFfn` into font names indexed by font index (`ftc`). Returns an
/// empty list on absence/malformation — fonts then fall back to defaults.
pub(crate) fn parse(table: &[u8], fc: usize, lcb: usize) -> Vec<String> {
    let mut out = Vec::new();
    let Some(data) = table.get(fc..fc.saturating_add(lcb)) else {
        return out;
    };
    if data.len() < 4 || u16le(data, 2) != Some(0) {
        return out;
    }
    let count = u16le(data, 0).unwrap_or(0) as usize;
    if count > 0x7FF0 {
        return out;
    }
    let mut pos = 4usize;
    for _ in 0..count {
        let Some(&length) = data.get(pos) else { break };
        pos += 1;
        let elem_len = length as usize;
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

/// Read the primary name; preserve an empty slot when its record is malformed.
fn ffn_name(elem: &[u8]) -> String {
    let Some(name) = elem.get(39..) else {
        return String::new();
    };
    if name.len() % 2 != 0 {
        return String::new();
    }
    let units: Vec<u16> = name
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let Some(end) = units.iter().position(|&unit| unit == 0) else {
        return String::new();
    };
    String::from_utf16(&units[..end]).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn font_record(name: &str, alternate: Option<&str>) -> Vec<u8> {
        // FFN starts after the STTB's one-byte length: its fixed part is 39 bytes.
        let mut record = vec![0; 39];
        record[0] = 4;
        record[1..3].copy_from_slice(&400u16.to_le_bytes());
        if alternate.is_some() {
            record[4] = (name.encode_utf16().count() + 1) as u8;
        }
        for text in std::iter::once(name).chain(alternate) {
            for unit in text.encode_utf16().chain(std::iter::once(0)) {
                record.extend_from_slice(&unit.to_le_bytes());
            }
        }
        let mut result = vec![record.len() as u8];
        result.extend(record);
        result
    }

    #[test]
    fn reads_non_extended_ffn_at_the_defined_unaligned_name_offset() {
        let mut data = vec![3, 0, 0, 0];
        data.extend(font_record("Times New Roman", None));
        data.extend(font_record("宋体", None));
        data.extend(font_record("仿宋_GB2312", Some("仿宋")));
        assert_eq!(
            parse(&data, 0, data.len()),
            ["Times New Roman", "宋体", "仿宋_GB2312"]
        );
    }

    #[test]
    fn malformed_font_names_do_not_shift_the_following_font_index() {
        let mut data = vec![2, 0, 0, 0];
        let mut bad = font_record("Broken", None);
        let len = bad.len();
        bad[len - 2..].copy_from_slice(&65u16.to_le_bytes());
        data.extend(bad);
        data.extend(font_record("宋体", None));
        assert_eq!(parse(&data, 0, data.len()), ["", "宋体"]);
    }

    #[test]
    fn extracts_utf16_font_name() {
        let mut d = vec![1, 0, 0, 0];
        d.extend(font_record("맑은 고딕", None));
        let names = parse(&d, 0, d.len());
        assert_eq!(names, vec!["맑은 고딕".to_string()]);
        assert_eq!(super::name_of(&names, 0).as_deref(), Some("맑은 고딕"));
    }

    #[test]
    fn primary_name_wins_over_a_longer_alternate() {
        let mut d = vec![1, 0, 0, 0];
        d.extend(font_record("宋体", Some("Longer Alternate Name")));
        assert_eq!(parse(&d, 0, d.len()), ["宋体"]);
    }

    #[test]
    fn empty_or_garbage_yields_no_names() {
        assert!(parse(&[], 0, 0).is_empty());
        assert!(parse(&[0xFF, 0xFF, 0, 0], 0, 4).is_empty());
        assert_eq!(parse(&[1, 0, 0, 0, 2, 0, 0], 0, 7), [""]);
    }
}
