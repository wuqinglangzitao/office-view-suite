//! Piece decoding, control-mark handling, and line normalization.

use encoding_rs::{
    Encoding, BIG5, EUC_KR, GBK, SHIFT_JIS, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254,
    WINDOWS_1255, WINDOWS_1256, WINDOWS_1258, WINDOWS_874,
};

use crate::clx::Piece;
use crate::list::Numberer;
use crate::papx::PapxTable;

/// Map a Windows ANSI codepage number to its `encoding_rs` codec. Used to
/// decode compressed (8-bit) pieces in the document's language codepage rather
/// than blindly as cp1252.
pub(crate) fn encoding_for_codepage(cp: u16) -> &'static Encoding {
    match cp {
        949 => EUC_KR, // Windows-949 / UHC (covers all 11,172 Hangul)
        932 => SHIFT_JIS,
        936 => GBK,
        950 => BIG5,
        1251 => WINDOWS_1251,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        874 => WINDOWS_874,
        1258 => WINDOWS_1258,
        _ => WINDOWS_1252,
    }
}

/// Output of [`decode_pieces`]: a CP-aligned stream (`raw`, for sub-document
/// slicing) and the full render (`labeled`, with reconstructed list autonumbers).
pub(crate) struct Decoded {
    /// Decoded char stream aligned 1:1 with Word's CP space (no generated text).
    pub raw: String,
    /// Same stream plus reconstructed list-autonumber prefixes (e.g. `1.\t`).
    pub labeled: String,
}

/// Decode every piece (in CP order) into the document's char stream.
///
/// Uncompressed pieces are UTF-16LE (Korean body text); `fCompressed` pieces are
/// 8-bit in the document's ANSI codepage (`enc`). Paragraph/cell marks are
/// resolved against `papx`: table cell marks (`0x07`) become a tab between cells
/// and a newline at the row terminator; each list paragraph gets its computed
/// autonumber (`numberer`) prefixed — in `labeled` only, so `raw` stays aligned
/// with Word's CP counts for sub-document slicing.
pub(crate) fn decode_pieces(
    word: &[u8],
    pieces: &[Piece],
    enc: &'static Encoding,
    papx: &PapxTable,
    numberer: &mut Numberer<'_>,
) -> Decoded {
    let mut d = Decoded {
        raw: String::new(),
        labeled: String::new(),
    };
    let mut para_start = 0usize; // byte index in `labeled` of the current paragraph

    // Bound cumulative decoded bytes: valid pieces partition the stream (total ≤ word.len()),
    // but a crafted table with overlapping pieces re-decodes it per piece — O(pieces × word.len())
    // memory/CPU. The per-piece `.min(word.len())` clamp can't catch overlap, so stop at a budget.
    let budget = word.len().saturating_add(16);
    let mut consumed = 0usize;
    for p in pieces {
        if p.cch == 0 {
            continue;
        }
        if consumed >= budget {
            break;
        }
        if p.compressed {
            let end = p.fc.saturating_add(p.cch).min(word.len());
            let Some(slice) = word.get(p.fc..end) else {
                continue;
            };
            consumed = consumed.saturating_add(slice.len());
            let mut buf: Vec<u8> = Vec::new();
            for (j, &b) in slice.iter().enumerate() {
                if b == 0x07 || b == 0x0D {
                    flush_bytes(&mut d, &mut buf, enc);
                    emit_mark(
                        &mut d,
                        &mut para_start,
                        papx,
                        numberer,
                        (p.fc + j) as u32,
                        b,
                    );
                } else {
                    buf.push(b);
                }
            }
            flush_bytes(&mut d, &mut buf, enc);
        } else {
            let byte_len = p.cch.saturating_mul(2);
            let end = p.fc.saturating_add(byte_len).min(word.len());
            let Some(slice) = word.get(p.fc..end) else {
                continue;
            };
            consumed = consumed.saturating_add(slice.len());
            let mut buf: Vec<u16> = Vec::new();
            for (i, c) in slice.chunks_exact(2).enumerate() {
                let u = u16::from_le_bytes([c[0], c[1]]);
                if u == 0x0007 || u == 0x000D {
                    flush_units(&mut d, &mut buf);
                    emit_mark(
                        &mut d,
                        &mut para_start,
                        papx,
                        numberer,
                        (p.fc + i * 2) as u32,
                        u as u8,
                    );
                } else {
                    buf.push(u);
                }
            }
            flush_units(&mut d, &mut buf);
        }
    }
    d
}

fn flush_bytes(d: &mut Decoded, buf: &mut Vec<u8>, enc: &'static Encoding) {
    if !buf.is_empty() {
        let s = enc.decode(buf).0;
        d.raw.push_str(&s);
        d.labeled.push_str(&s);
        buf.clear();
    }
}

fn flush_units(d: &mut Decoded, buf: &mut Vec<u16>) {
    if !buf.is_empty() {
        let s = String::from_utf16_lossy(buf);
        d.raw.push_str(&s);
        d.labeled.push_str(&s);
        buf.clear();
    }
}

/// Handle a paragraph-ending mark (`0x0D` paragraph or `0x07` cell): prefix the
/// list autonumber (into `labeled`), then emit the separator char (tab between
/// table cells, newline otherwise).
fn emit_mark(
    d: &mut Decoded,
    para_start: &mut usize,
    papx: &PapxTable,
    numberer: &mut Numberer<'_>,
    fc: u32,
    mark: u8,
) {
    if !numberer.is_empty() {
        let (ilfo, ilvl) = papx.list_at(fc);
        if ilfo > 0 {
            if let Some(label) = numberer.label(ilfo, ilvl) {
                d.labeled.insert_str(*para_start, &label);
            }
        }
    }
    let ch = if mark == 0x07 && !papx.is_empty() {
        let (in_table, row_end) = papx.at(fc);
        if in_table && !row_end {
            '\t'
        } else {
            '\n'
        }
    } else {
        '\n'
    };
    d.raw.push(ch);
    d.labeled.push(ch);
    *para_start = d.labeled.len();
}

/// Convert Word control marks to plain text.
///
///   * `0x0D`/`0x0B`/`0x07`/`0x0C`/`0x0E` → newline (paragraph / line / cell /
///     page / column break)
///   * `0x1E` non-breaking hyphen → visible `-` (keeps identifiers like
///     `2024-1234` intact); `0x1F` optional hyphen dropped (zero-width)
///   * `0xA0` non-breaking space → normalized to a regular space
///   * field markers `0x13`/`0x14`/`0x15` removed, field *text* kept (matches
///     POI `WordExtractor.getText()`, which keeps HYPERLINK/PAGE field text)
///   * NUL and other C0 controls dropped
pub(crate) fn strip_controls(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\u{13}' | '\u{14}' | '\u{15}' => {}
            // 0x09 tab is real content (kept). 0x07 cell marks are already
            // resolved to tab/newline during piece decoding (see `cell_mark`).
            '\t' => out.push('\t'),
            '\r' | '\u{0B}' | '\u{0C}' | '\u{0E}' | '\n' => out.push('\n'),
            '\u{1E}' => out.push('-'),
            '\u{A0}' => out.push(' '),
            '\u{00}' => {}
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// Collapse newlines, trim each line, drop empty lines and consecutive
/// duplicate *prose* lines (table rows are preserved — see below).
pub(crate) fn normalize_lines(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = Vec::new();
    for line in unified.split('\n') {
        // Trim trailing whitespace (including the spurious last-cell tab) and
        // leading spaces, but keep a leading tab — it is an empty first table
        // cell whose column position would otherwise be lost.
        let trimmed_end = if line.chars().all(|ch| ch == '\t') {
            line
        } else {
            line.trim_end()
        };
        let trimmed = trimmed_end.trim_start_matches(' ');
        if trimmed.is_empty() {
            continue;
        }
        // Drop consecutive duplicate prose, but never collapse table rows or
        // numeric-only lines such as repeated PAGE/PAGEREF field results.
        if !trimmed.contains('\t')
            && !is_numeric_field_line(trimmed)
            && lines.last() == Some(&trimmed)
        {
            continue;
        }
        lines.push(trimmed);
    }
    lines.join("\n")
}

fn is_numeric_field_line(line: &str) -> bool {
    let mut saw_digit = false;
    for ch in line.chars() {
        if ch.is_ascii_digit() {
            saw_digit = true;
        } else if !matches!(ch, ' ' | '\t' | '-' | '\u{2010}' | '\u{2011}' | '/' | '\\') {
            return false;
        }
    }
    saw_digit
}

/// A run of decoded text → final normalized plain text.
pub(crate) fn finalize(raw: &str) -> String {
    normalize_lines(&strip_controls(raw))
}

pub(crate) fn has_indexable(text: &str) -> bool {
    text.chars()
        .any(|c| c.is_alphanumeric() || ('가'..='힣').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_duplicate_table_rows_but_collapses_prose() {
        // Repeated table rows are real data — never dropped.
        assert_eq!(normalize_lines("X\tY\nX\tY\n"), "X\tY\nX\tY");
        // Repeated numeric field-result lines are also real content.
        assert_eq!(normalize_lines("2\n2\n"), "2\n2");
        // Repeated prose lines still collapse.
        assert_eq!(normalize_lines("hi\nhi\n"), "hi");
    }

    #[test]
    fn preserves_leading_empty_cell_drops_trailing_tab() {
        // Empty first cell keeps its column; spurious last-cell tab is dropped.
        assert_eq!(normalize_lines("\tB\t\n"), "\tB");
        assert_eq!(normalize_lines("A\tB\t\n"), "A\tB");
        assert_eq!(normalize_lines("\t\n-\n"), "\t\n-");
        // Leading prose spaces are still trimmed.
        assert_eq!(normalize_lines("   hello  \n"), "hello");
    }

    #[test]
    fn overlapping_pieces_decode_is_bounded() {
        // A small word stream, but 1000 pieces ALL pointing at the same region (fc=0, full
        // length) — the overlap amplification a crafted piece table uses. Without the
        // cumulative budget this decodes 1000 × 50 = 50_000 chars; the budget (~word.len())
        // bounds it to roughly the stream size, with no hang/panic.
        let word = b"A\x00".repeat(50); // 100 bytes = 50 UTF-16 'A'
        let pieces: Vec<Piece> = (0..1000)
            .map(|_| Piece {
                cch: 50,
                fc: 0,
                compressed: false,
                prm: 0,
            })
            .collect();
        let lists = crate::list::parse(&[], 0, 0, 0, 0);
        let papx = crate::papx::parse(&[], &[], 0, 0);
        let mut numberer = Numberer::new(&lists);
        let d = decode_pieces(&word, &pieces, WINDOWS_1252, &papx, &mut numberer);
        assert!(
            d.raw.chars().count() < 1000,
            "overlapping-piece decode was not bounded: {} chars",
            d.raw.chars().count()
        );
    }
}
