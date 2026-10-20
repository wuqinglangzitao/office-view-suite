//! `.docx` numbering (`word/numbering.xml`) → per `(numId, ilvl)` ordered/bullet
//! and the autonumber **label** (`1.`, `a)`, `1.1`, `i.` …), computed from each
//! level's `<w:numFmt>`, `<w:lvlText>` pattern, and `<w:start>` with live
//! per-level counters maintained in document order.
//!
//! `<w:num w:numId>` points at a `<w:abstractNum w:abstractNumId>` whose
//! `<w:lvl w:ilvl>` carries `numFmt` (decimal/lowerLetter/lowerRoman/…; bullet/
//! none ⇒ unordered), `lvlText` (e.g. `%1.`), and `start`.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::fields::computed_run_symbol_char;
use super::xml_text::{skip_alternate_content_branch, skip_subtree, AlternateContentBranchState};
use super::{attr_local, attr_local_trimmed, attr_u32, attr_u8, local};

/// One numbering level's resolved formatting.
#[derive(Debug, Clone)]
struct Level {
    ordered: bool,
    num_fmt: String,
    lvl_text: String,
    marker_font: Option<String>,
    start: u32,
}

impl Default for Level {
    fn default() -> Self {
        Level {
            ordered: true,
            num_fmt: "decimal".into(),
            lvl_text: String::new(),
            marker_font: None,
            start: 1,
        }
    }
}

/// Parsed numbering: `numId → abstractNumId` and per-abstract level formats.
#[derive(Debug, Default)]
pub(crate) struct Numbering {
    num_to_abstract: HashMap<String, String>,
    /// `abstractNumId → (ilvl → Level)`.
    abstract_levels: HashMap<String, HashMap<u8, Level>>,
}

impl Numbering {
    fn levels(&self, num_id: &str) -> Option<&HashMap<u8, Level>> {
        self.abstract_levels.get(self.num_to_abstract.get(num_id)?)
    }

    /// `Some(true)` = numbered, `Some(false)` = bullet, for `(numId, ilvl)`.
    /// `None` when `numId` isn't a known list; defaults to ordered when the list
    /// exists but the specific level's format is unknown.
    pub(crate) fn ordered(&self, num_id: &str, ilvl: u8) -> Option<bool> {
        let levels = self.levels(num_id)?;
        Some(levels.get(&ilvl).map(|l| l.ordered).unwrap_or(true))
    }

    /// Font declared for the rendered list marker at `(numId, ilvl)`.
    pub(crate) fn marker_font(&self, num_id: &str, ilvl: u8) -> Option<&str> {
        self.levels(num_id)?.get(&ilvl)?.marker_font.as_deref()
    }

    /// Advance `counters` for this list item and format its autonumber label
    /// (e.g. `1.`, `a)`, `1.1`), or a bullet level's declared literal glyph.
    /// Returns `None` when a bullet level declares none (the caller supplies its
    /// own) or for an unknown list.
    pub(crate) fn label(&self, num_id: &str, ilvl: u8, counters: &mut [u32; 9]) -> Option<String> {
        let levels = self.levels(num_id)?;
        let i = ilvl.min(8) as usize;
        let lvl = levels.get(&ilvl).cloned().unwrap_or_default();
        // Advance this level (seed at `start` on first use), reset deeper levels.
        if counters[i] == 0 {
            counters[i] = lvl.start.max(1);
        } else {
            counters[i] += 1;
        }
        for c in counters.iter_mut().skip(i + 1) {
            *c = 0;
        }
        if !lvl.ordered {
            // A bullet level's `lvlText` is the glyph the document asks for.
            // Only a literal one counts: a `%N` pattern is autonumber syntax,
            // and an absent one leaves the caller its synthesized bullet.
            let glyph = lvl.lvl_text.trim();
            if glyph.is_empty() || glyph.contains('%') {
                return None;
            }
            return Some(
                glyph
                    .chars()
                    .map(|ch| {
                        let code = ch as u32;
                        // Numbering uses the same legacy font codes as w:sym.
                        // Already-Unicode markers must not be reduced to a byte.
                        if code <= 0xff || (0xf000..=0xf0ff).contains(&code) {
                            computed_run_symbol_char(
                                lvl.marker_font.as_deref(),
                                &format!("{code:X}"),
                            )
                            .unwrap_or(ch)
                        } else {
                            ch
                        }
                    })
                    .collect(),
            );
        }
        let pattern = if lvl.lvl_text.is_empty() {
            format!("%{}.", i + 1)
        } else {
            lvl.lvl_text.clone()
        };
        Some(expand(&pattern, levels, counters))
    }

    /// Format the current list item using all available ancestor counters,
    /// matching REF `\w` full-context numbering such as `1.a.i`.
    pub(crate) fn full_context_label(
        &self,
        num_id: &str,
        ilvl: u8,
        counters: &[u32; 9],
    ) -> Option<String> {
        let levels = self.levels(num_id)?;
        let max = ilvl.min(8);
        let mut parts = Vec::new();
        for level in 0..=max {
            let count = counters[level as usize];
            if count == 0 {
                return None;
            }
            let lvl = levels.get(&level).cloned().unwrap_or_default();
            if !lvl.ordered {
                return None;
            }
            parts.push(format_num(count, &lvl.num_fmt));
        }
        (!parts.is_empty()).then(|| parts.join("."))
    }
}

/// Expand an `lvlText` pattern, replacing `%1`..`%9` with the corresponding
/// level's counter formatted in that level's `numFmt`.
fn expand(pattern: &str, levels: &HashMap<u8, Level>, counters: &[u32; 9]) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if let Some(d) = chars.peek().and_then(|d| d.to_digit(10)) {
                chars.next();
                if (1..=9).contains(&d) {
                    let k = (d - 1) as usize;
                    let fmt = levels
                        .get(&(k as u8))
                        .map(|l| l.num_fmt.as_str())
                        .unwrap_or("decimal");
                    out.push_str(&format_num(counters[k].max(1), fmt));
                    continue;
                }
                out.push('%');
                out.push(char::from_digit(d, 10).unwrap_or('?'));
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Format a 1-based counter in a `w:numFmt` style. Unknown/CJK formats fall back
/// to decimal (the legacy `.doc` path handles Korean autonumber styles).
fn format_num(n: u32, fmt: &str) -> String {
    match fmt {
        "decimalZero" => format!("{n:02}"),
        "lowerLetter" => alpha(n, 'a'),
        "upperLetter" => alpha(n, 'A'),
        "lowerRoman" => roman(n).to_lowercase(),
        "upperRoman" => roman(n),
        // 中文/日文计数(小写:一二三)
        "chineseCounting"
        | "chineseCountingThousand"
        | "japaneseCounting"
        | "japaneseDigitalTenThousand"
        | "taiwaneseCounting"
        | "taiwaneseCountingThousand"
        | "koreanCounting" => chinese(n, false),
        // 中文大写(壹贰叁)
        "chineseLegalSimplified" | "japaneseLegalSimplified" => chinese(n, true),
        // 甲乙丙丁…
        "ideographTraditional" | "ideographZodiac" | "chineseUnsealedNumeral" | "japaneseIroha" => {
            ideograph(n)
        }
        // 全角数字
        "decimalFullWidth" | "decimalFullWidth2" => fullwidth_digits(n),
        _ => n.to_string(),
    }
}

/// 简体中文数字(小写一二三 / 大写壹贰叁),支持 1..=9999
fn chinese(n: u32, upper: bool) -> String {
    if n == 0 {
        return "零".to_string();
    }
    const LOWER: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    const UPPER: [&str; 10] = ["", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    let d = if upper { &UPPER } else { &LOWER };
    let units: [&str; 5] = ["", "十", "百", "千", "万"];
    let mut s = String::new();
    let mut rest = n.min(9999);
    let mut pos = 4i32;
    let mut started = false;
    let mut need_zero = false;
    while pos >= 0 {
        let p = 10u32.pow(pos as u32);
        let digit = (rest / p) as usize;
        rest %= p;
        if digit > 0 {
            if need_zero {
                s.push('零');
                need_zero = false;
            }
            // 一十 → 十(仅当它是首位)
            if !(digit == 1 && pos == 1 && s.is_empty()) {
                s.push_str(d[digit]);
            }
            s.push_str(units[pos as usize]);
            started = true;
        } else if started {
            need_zero = true;
        }
        pos -= 1;
    }
    s
}

/// 甲乙丙丁…(11→甲,12→乙 循环)
fn ideograph(n: u32) -> String {
    const STEM: [char; 10] = ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'];
    if n == 0 {
        return String::new();
    }
    STEM[((n - 1) % 10) as usize].to_string()
}

/// 全角数字
fn fullwidth_digits(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap_or(c))
        .collect()
}

/// Spreadsheet-style letters: 1→a, 26→z, 27→aa (base = `'a'` or `'A'`).
fn alpha(mut n: u32, base: char) -> String {
    if n == 0 {
        return base.to_string();
    }
    let mut s = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        s.push((base as u8 + rem as u8) as char);
        n = (n - 1) / 26;
    }
    s.iter().rev().collect()
}

/// Roman numerals (uppercase), clamped to a sane range.
fn roman(mut n: u32) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    const VALS: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for &(v, sym) in VALS {
        while n >= v {
            s.push_str(sym);
            n -= v;
        }
    }
    s
}

/// Parse `word/numbering.xml`. Returns empty on absence/malformation.
pub(crate) fn parse(xml: &str) -> Numbering {
    let mut r = Reader::from_str(xml);
    let mut nb = Numbering::default();
    let mut cur_abstract: Option<String> = None;
    let mut cur_ilvl: Option<u8> = None;
    let mut cur_num: Option<String> = None;
    let mut cur_override_ilvl: Option<u8> = None;
    let mut in_override_level = false;
    let mut overrides: HashMap<String, HashMap<u8, LevelOverride>> = HashMap::new();
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) =>
            {
                skip_subtree(&mut r);
            }
            Ok(Event::Empty(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) => {}
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.push(AlternateContentBranchState::default());
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"lvlOverride" => {
                    cur_override_ilvl = attr_u8(&e, b"ilvl");
                    if let (Some(num), Some(ilvl)) = (cur_num.as_ref(), cur_override_ilvl) {
                        overrides
                            .entry(num.clone())
                            .or_default()
                            .entry(ilvl)
                            .or_default();
                    }
                }
                b"startOverride" => {
                    if let (Some(num), Some(ilvl), Some(v)) =
                        (cur_num.as_ref(), cur_override_ilvl, attr_u32(&e, b"val"))
                    {
                        overrides
                            .entry(num.clone())
                            .or_default()
                            .entry(ilvl)
                            .or_default()
                            .start = Some(v);
                    }
                }
                b"abstractNum" => {
                    cur_abstract = attr_local_trimmed(&e, b"abstractNumId");
                    cur_ilvl = None;
                }
                b"lvl" => {
                    cur_ilvl = attr_u8(&e, b"ilvl");
                    if let (Some(num), Some(ilvl)) = (cur_num.as_ref(), cur_override_ilvl) {
                        // A replacement level definition inside `w:lvlOverride`.
                        in_override_level = true;
                        overrides
                            .entry(num.clone())
                            .or_default()
                            .entry(ilvl)
                            .or_default()
                            .level
                            .get_or_insert_with(Level::default);
                    } else {
                        set_level(&mut nb, &cur_abstract, cur_ilvl, &e, |_, _| {});
                    }
                }
                b"numFmt" => {
                    let apply = |l: &mut Level, e: &BytesStart<'_>| {
                        if let Some(v) = attr_local(e, b"val") {
                            let value = v.trim();
                            l.ordered = value != "bullet" && value != "none";
                            l.num_fmt = value.to_string();
                        }
                    };
                    if in_override_level {
                        if let Some(l) =
                            override_level_mut(&mut overrides, &cur_num, cur_override_ilvl)
                        {
                            apply(l, &e);
                        }
                    } else {
                        set_level(&mut nb, &cur_abstract, cur_ilvl, &e, apply);
                    }
                }
                b"lvlText" => {
                    let apply = |l: &mut Level, e: &BytesStart<'_>| {
                        if let Some(v) = attr_local_trimmed(e, b"val") {
                            l.lvl_text = v;
                        }
                    };
                    if in_override_level {
                        if let Some(l) =
                            override_level_mut(&mut overrides, &cur_num, cur_override_ilvl)
                        {
                            apply(l, &e);
                        }
                    } else {
                        set_level(&mut nb, &cur_abstract, cur_ilvl, &e, apply);
                    }
                }
                b"rFonts" => {
                    let apply = |l: &mut Level, e: &BytesStart<'_>| {
                        l.marker_font = attr_local_trimmed(e, b"ascii")
                            .or_else(|| attr_local_trimmed(e, b"hAnsi"))
                            .or_else(|| attr_local_trimmed(e, b"eastAsia"))
                            .or_else(|| attr_local_trimmed(e, b"cs"));
                    };
                    if in_override_level {
                        if let Some(l) =
                            override_level_mut(&mut overrides, &cur_num, cur_override_ilvl)
                        {
                            apply(l, &e);
                        }
                    } else {
                        set_level(&mut nb, &cur_abstract, cur_ilvl, &e, apply);
                    }
                }
                b"start" => {
                    let apply = |l: &mut Level, e: &BytesStart<'_>| {
                        if let Some(v) = attr_u32(e, b"val") {
                            l.start = v;
                        }
                    };
                    if in_override_level {
                        if let Some(l) =
                            override_level_mut(&mut overrides, &cur_num, cur_override_ilvl)
                        {
                            apply(l, &e);
                        }
                    } else {
                        set_level(&mut nb, &cur_abstract, cur_ilvl, &e, apply);
                    }
                }
                b"num" => cur_num = attr_local_trimmed(&e, b"numId"),
                b"abstractNumId" => {
                    if let (Some(num), Some(val)) =
                        (cur_num.as_ref(), attr_local_trimmed(&e, b"val"))
                    {
                        nb.num_to_abstract.insert(num.clone(), val);
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"abstractNum" => cur_abstract = None,
                b"num" => {
                    cur_num = None;
                    cur_override_ilvl = None;
                }
                b"lvl" => {
                    cur_ilvl = None;
                    in_override_level = false;
                }
                b"lvlOverride" => cur_override_ilvl = None,
                b"AlternateContent" => {
                    alternate_content_stack.pop();
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    // Materialize each overridden instance as its own level set, so lookups
    // stay a plain `numId -> levels` map.
    for (num_id, per_level) in overrides {
        let Some(abstract_id) = nb.num_to_abstract.get(&num_id).cloned() else {
            continue;
        };
        let mut levels = nb
            .abstract_levels
            .get(&abstract_id)
            .cloned()
            .unwrap_or_default();
        for (ilvl, over) in per_level {
            let entry = levels.entry(ilvl).or_default();
            if let Some(level) = over.level {
                *entry = level;
            }
            if let Some(start) = over.start {
                entry.start = start;
            }
        }
        let key = format!("\u{0}override:{num_id}");
        nb.abstract_levels.insert(key.clone(), levels);
        nb.num_to_abstract.insert(num_id, key);
    }
    nb
}

/// A `w:num`'s per-level override: a restart value, a replacement definition,
/// or both.
#[derive(Debug, Default, Clone)]
struct LevelOverride {
    start: Option<u32>,
    level: Option<Level>,
}

fn override_level_mut<'a>(
    overrides: &'a mut HashMap<String, HashMap<u8, LevelOverride>>,
    num: &Option<String>,
    ilvl: Option<u8>,
) -> Option<&'a mut Level> {
    let num = num.as_ref()?;
    overrides.get_mut(num)?.get_mut(&ilvl?)?.level.as_mut()
}

/// Apply a mutation to the current `(abstract, ilvl)` level, creating it if new.
fn set_level(
    nb: &mut Numbering,
    abs: &Option<String>,
    ilvl: Option<u8>,
    e: &BytesStart<'_>,
    f: impl FnOnce(&mut Level, &BytesStart<'_>),
) {
    if let (Some(abs), Some(ilvl)) = (abs.as_ref(), ilvl) {
        let lvl = nb
            .abstract_levels
            .entry(abs.clone())
            .or_default()
            .entry(ilvl)
            .or_default();
        f(lvl, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_numid_to_ordered_or_bullet() {
        let xml = r#"<w:numbering>
            <w:abstractNum w:abstractNumId="0">
                <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
                <w:lvl w:ilvl="1"><w:numFmt w:val="lowerLetter"/></w:lvl>
            </w:abstractNum>
            <w:abstractNum w:abstractNumId="1">
                <w:lvl w:ilvl="0"><w:numFmt w:val=" bullet "/></w:lvl>
            </w:abstractNum>
            <w:abstractNum w:abstractNumId="2">
                <w:lvl w:ilvl="0"><w:numFmt w:val=" none "/></w:lvl>
            </w:abstractNum>
            <w:num w:numId="5"><w:abstractNumId w:val="0"/></w:num>
            <w:num w:numId="6"><w:abstractNumId w:val="1"/></w:num>
            <w:num w:numId="7"><w:abstractNumId w:val="2"/></w:num>
        </w:numbering>"#;
        let nb = parse(xml);
        assert_eq!(nb.ordered("5", 0), Some(true));
        assert_eq!(nb.ordered("5", 1), Some(true));
        assert_eq!(nb.ordered("6", 0), Some(false));
        assert_eq!(nb.ordered("7", 0), Some(false));
        assert_eq!(nb.ordered("99", 0), None);
    }

    #[test]
    fn bullet_levels_keep_their_declared_glyph() {
        // A bullet level's `lvlText` is the character the document asks for;
        // only a level that declares none falls back to a synthesized bullet.
        let xml = r#"<w:numbering>
            <w:abstractNum w:abstractNumId="0">
                <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/><w:lvlText w:val="○"/></w:lvl>
                <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
                <w:lvl w:ilvl="2"><w:numFmt w:val="bullet"/><w:lvlText w:val="%3."/></w:lvl>
            </w:abstractNum>
            <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
        </w:numbering>"#;
        let nb = parse(xml);
        let mut c = [0u32; 9];
        assert_eq!(nb.label("1", 0, &mut c).as_deref(), Some("\u{25CB}"));
        assert_eq!(nb.label("1", 1, &mut c), None);
        // A placeholder pattern is not a literal glyph, so it stays a fallback.
        assert_eq!(nb.label("1", 2, &mut c), None);
    }

    #[test]
    fn bullet_levels_keep_their_declared_marker_font() {
        let xml = r#"<w:numbering>
            <w:abstractNum w:abstractNumId="0">
                <w:lvl w:ilvl="0">
                    <w:numFmt w:val="bullet"/>
                    <w:lvlText w:val=""/>
                    <w:rPr><w:rFonts w:ascii="Wingdings" w:hAnsi="Wingdings"/></w:rPr>
                </w:lvl>
            </w:abstractNum>
            <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
        </w:numbering>"#;
        let nb = parse(xml);

        assert_eq!(nb.marker_font("1", 0), Some("Wingdings"));
    }

    #[test]
    fn bullet_levels_decode_legacy_symbol_codes_without_changing_unicode() {
        let cases = [
            ("Symbol", "\u{f0b7}", "•"),
            ("Symbol", "\u{00b7}", "•"),
            ("Wingdings", "\u{f0d8}", "➢"),
            ("Wingdings", "\u{f06e}", "■"),
            ("Wingdings", "\u{f0fc}", "✓"),
            ("Symbol", "•", "•"),
            ("Symbol", "○", "○"),
            ("Symbol", "α", "α"),
            ("Wingdings", "➢", "➢"),
            ("Courier New", "o", "o"),
            ("Arial", "\u{00b7}", "\u{00b7}"),
            ("Custom Icons", "\u{f0b7}", "\u{f0b7}"),
            ("Symbol", "\u{f041}•", "Α•"),
            ("Symbol", "\u{e0b7}", "\u{e0b7}"),
            ("Wingdings", "\u{f000}", "\u{f000}"),
        ];
        for (font, glyph, expected) in cases {
            let xml = format!(
                r#"<w:numbering><w:abstractNum w:abstractNumId="0">
                <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/><w:lvlText w:val="{glyph}"/>
                <w:rPr><w:rFonts w:ascii="{font}"/></w:rPr></w:lvl>
                </w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
            );
            let nb = parse(&xml);
            assert_eq!(
                nb.label("1", 0, &mut [0; 9]).as_deref(),
                Some(expected),
                "font={font}, glyph={glyph:?}"
            );
            assert_eq!(nb.marker_font("1", 0), Some(font));
        }
    }

    #[test]
    fn list_instances_honor_their_level_overrides() {
        // A `w:num` may override its abstract numbering per level: a restart
        // value, or a full replacement level definition.
        let xml = r#"<w:numbering>
            <w:abstractNum w:abstractNumId="0">
                <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
                <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%2."/></w:lvl>
            </w:abstractNum>
            <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
            <w:num w:numId="2">
                <w:abstractNumId w:val="0"/>
                <w:lvlOverride w:ilvl="0"><w:startOverride w:val="5"/></w:lvlOverride>
            </w:num>
            <w:num w:numId="3">
                <w:abstractNumId w:val="0"/>
                <w:lvlOverride w:ilvl="0">
                    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%1)"/></w:lvl>
                </w:lvlOverride>
            </w:num>
        </w:numbering>"#;
        let nb = parse(xml);

        // The unoverridden instance is unaffected.
        let mut c = [0u32; 9];
        assert_eq!(nb.label("1", 0, &mut c).as_deref(), Some("1."));

        // A start override seeds the counter on first use.
        let mut c = [0u32; 9];
        assert_eq!(nb.label("2", 0, &mut c).as_deref(), Some("5."));
        assert_eq!(nb.label("2", 0, &mut c).as_deref(), Some("6."));

        // A replacement level definition changes format and text.
        let mut c = [0u32; 9];
        assert_eq!(nb.label("3", 0, &mut c).as_deref(), Some("a)"));
        // Levels the override does not mention keep the abstract definition.
        assert_eq!(nb.label("3", 1, &mut c).as_deref(), Some("1."));
    }

    #[test]
    fn formats_multi_level_labels() {
        let xml = r#"<w:numbering>
            <w:abstractNum w:abstractNumId="0">
                <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
                <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2)"/></w:lvl>
                <w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val=" lowerRoman "/><w:lvlText w:val="%1.%2.%3"/></w:lvl>
            </w:abstractNum>
            <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
            <w:abstractNum w:abstractNumId="9"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/></w:lvl></w:abstractNum>
            <w:num w:numId="2"><w:abstractNumId w:val="9"/></w:num>
        </w:numbering>"#;
        let nb = parse(xml);
        let mut c = [0u32; 9];
        assert_eq!(nb.label("1", 0, &mut c).as_deref(), Some("1."));
        assert_eq!(nb.label("1", 0, &mut c).as_deref(), Some("2."));
        assert_eq!(nb.label("1", 1, &mut c).as_deref(), Some("a)"));
        assert_eq!(nb.label("1", 1, &mut c).as_deref(), Some("b)"));
        assert_eq!(nb.label("1", 2, &mut c).as_deref(), Some("2.b.i"));
        assert_eq!(nb.label("1", 0, &mut c).as_deref(), Some("3."));
        // A declared bullet glyph is the label; the caller no longer guesses.
        assert_eq!(nb.label("2", 0, &mut c).as_deref(), Some("•"));
    }

    #[test]
    fn uses_single_alternate_content_branch() {
        let xml = r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <mc:AlternateContent>
                <mc:Choice Requires="w14">
                    <w:abstractNum w:abstractNumId="1">
                        <w:lvl w:ilvl="0"><w:start w:val="5"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
                    </w:abstractNum>
                    <w:num w:numId="10"><w:abstractNumId w:val="1"/></w:num>
                </mc:Choice>
                <mc:Fallback>
                    <w:abstractNum w:abstractNumId="9">
                        <w:lvl w:ilvl="0"><w:start w:val="9"/><w:numFmt w:val="upperRoman"/><w:lvlText w:val="%1."/></w:lvl>
                    </w:abstractNum>
                    <w:num w:numId="10"><w:abstractNumId w:val="9"/></w:num>
                </mc:Fallback>
            </mc:AlternateContent>
        </w:numbering>"#;
        let nb = parse(xml);
        let mut counters = [0u32; 9];

        assert_eq!(nb.label("10", 0, &mut counters).as_deref(), Some("5."));
    }
}
