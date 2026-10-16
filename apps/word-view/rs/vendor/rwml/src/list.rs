//! List autonumber reconstruction. Parses the list-definition table (`PlfLst`)
//! and the format-override table (`PlfLfo`), then computes each list
//! paragraph's label (`1.`, `1.1`, `가.`, `(1)` …) the way Word renders it.
//!
//! Reference: [MS-DOC] 2.9.200 PlfLfo, 2.9.131 LFO, 2.9.132 LFOData,
//! 2.9.133 LFOLVL, 2.9.149 LVL, 2.9.150 LVLF, 2.9.337 Xst; [MS-OSHARED]
//! 2.2.1.3 MSONFC.

use std::collections::HashMap;

use crate::numfmt;
use crate::util::{u16le, u32le};

/// Max UTF-16 units kept from a list level's number template (`LVL.xst`). Real templates are
/// a handful of placeholders + separators; this bounds a crafted huge one so per-paragraph
/// label generation can't amplify into O(template × paragraphs) output.
const MAX_XST_LEN: usize = 256;

/// One list level: how to format its number and the surrounding template.
#[derive(Debug, Clone, Default)]
struct Level {
    nfc: u8,
    start: i32,
    /// Whether every placeholder uses Arabic, except an original ArabicLZ value.
    legal: bool,
    /// `ilvlRestartLim` when `fNoRestart` is set.
    restart_limit: Option<u8>,
    /// 1-based positions in `xst` of each level's number placeholder (0-term).
    rgbxch_nums: [u8; 9],
    /// Character after the number: 0 = tab, 1 = space, 2 = nothing.
    ixch_follow: u8,
    /// The number template: literal UTF-16 chars + placeholders (value = level).
    xst: Vec<u16>,
}

/// A list definition: a stable id and its 1 (simple) or 9 levels.
#[derive(Debug, Clone)]
struct ListDef {
    lsid: i32,
    simple: bool,
    levels: Vec<Level>,
}

#[derive(Debug, Clone, Default)]
struct LevelOverride {
    start_at: Option<i32>,
    formatting: Option<Level>,
}

#[derive(Debug, Clone, Default)]
struct ListFormatOverride {
    lsid: i32,
    /// Sparse because most LFOs have no overrides. Duplicate level records are
    /// retained in source order and the last valid record wins.
    levels: Vec<(u8, LevelOverride)>,
}

impl ListFormatOverride {
    fn level(&self, index: usize) -> Option<&LevelOverride> {
        self.levels
            .iter()
            .rev()
            .find_map(|(level, value)| (usize::from(*level) == index).then_some(value))
    }
}

/// Parsed list tables, ready to drive a [`Numberer`].
#[derive(Debug, Default)]
pub(crate) struct Lists {
    defs: Vec<ListDef>,
    /// `ilfo` (1-based) → fixed LFO identity plus parallel LFOData overrides.
    lfos: Vec<ListFormatOverride>,
}

impl Lists {
    pub(crate) fn is_empty(&self) -> bool {
        self.defs.is_empty() || self.lfos.is_empty()
    }
}

/// Parse the list tables from the table stream. Returns empty (not an error) on
/// absence or malformation — list rendering then simply does nothing.
pub(crate) fn parse(
    table: &[u8],
    fc_lst: usize,
    lcb_lst: usize,
    fc_lfo: usize,
    lcb_lfo: usize,
) -> Lists {
    Lists {
        defs: parse_plf_lst(table, fc_lst, lcb_lst),
        lfos: parse_plf_lfo(table, fc_lfo, lcb_lfo),
    }
}

fn parse_plf_lst(table: &[u8], fc: usize, lcb: usize) -> Vec<ListDef> {
    if lcb < 2 {
        return Vec::new();
    }
    let Some(blob) = table.get(fc..fc.saturating_add(lcb)) else {
        return Vec::new();
    };
    let clst = i16::from_le_bytes([blob[0], blob[1]]);
    if clst <= 0 {
        return Vec::new();
    }
    let clst = clst as usize;
    // LSTF headers (28 bytes each) follow cLst.
    let mut headers = Vec::with_capacity(clst);
    for i in 0..clst {
        let off = 2 + i * 28;
        let Some(lsid) = u32le(blob, off).map(|v| v as i32) else {
            return Vec::new();
        };
        let simple = blob.get(off + 26).copied().unwrap_or(0) & 0x01 != 0;
        headers.push((lsid, simple));
    }
    // The LVL array is appended immediately after the PlfLst blob.
    let mut cur = fc + lcb;
    let mut defs = Vec::with_capacity(clst);
    for (lsid, simple) in headers {
        let nlvl = if simple { 1 } else { 9 };
        let mut levels = Vec::with_capacity(nlvl);
        for _ in 0..nlvl {
            match parse_lvl(table, &mut cur) {
                Some(lvl) => levels.push(lvl),
                None => return defs, // truncated — keep what parsed cleanly
            }
        }
        defs.push(ListDef {
            lsid,
            simple,
            levels,
        });
    }
    defs
}

/// Parse one variable-size `LVL` starting at `*cur`, advancing the cursor.
fn parse_lvl(table: &[u8], cur: &mut usize) -> Option<Level> {
    let lvlf_end = cur.checked_add(28)?;
    let lvlf = table.get(*cur..lvlf_end)?;
    let start = i32::from_le_bytes(lvlf[0..4].try_into().ok()?);
    let nfc = lvlf[4];
    // [MS-DOC] 2.9.150: flags byte bits 2/3 carry fLegal/fNoRestart.
    let legal = lvlf[5] & (1 << 2) != 0;
    let restart_limit = (lvlf[5] & (1 << 3) != 0).then_some(lvlf[26]);
    let mut rgbxch_nums = [0u8; 9];
    rgbxch_nums.copy_from_slice(&lvlf[6..15]);
    let ixch_follow = lvlf[15];
    let cb_grpprl_chpx = lvlf[24] as usize;
    let cb_grpprl_papx = lvlf[25] as usize;
    *cur = lvlf_end;
    // grpprlPapx (sized by cbGrpprlPapx) then grpprlChpx (sized by cbGrpprlChpx).
    let grpprl_end = cur
        .checked_add(cb_grpprl_papx)?
        .checked_add(cb_grpprl_chpx)?;
    table.get(*cur..grpprl_end)?;
    *cur = grpprl_end;
    // Xst: cch (u16) then cch UTF-16 chars.
    let cch = u16le(table, *cur)? as usize;
    let xst_start = cur.checked_add(2)?;
    let xst_end = xst_start.checked_add(cch.checked_mul(2)?)?;
    let xst_bytes = table.get(xst_start..xst_end)?;
    // A real number template is tiny (≤ a few dozen units). Cap how many we read+store so a
    // crafted huge `cch` can't make every list paragraph render/insert a giant label —
    // O(template × paragraphs) output amplification. Still advance the cursor by the full
    // declared `cch` so subsequent levels stay aligned.
    let take = cch.min(MAX_XST_LEN);
    let mut xst = Vec::with_capacity(take);
    for i in 0..take {
        xst.push(u16le(xst_bytes, i * 2)?);
    }
    *cur = xst_end;
    Some(Level {
        nfc,
        start,
        legal,
        restart_limit,
        rgbxch_nums,
        ixch_follow,
        xst,
    })
}

/// Parse the fixed `rgLfo` array and its parallel variable `rgLfoData` array.
fn parse_plf_lfo(table: &[u8], fc: usize, lcb: usize) -> Vec<ListFormatOverride> {
    if lcb < 4 {
        return Vec::new();
    }
    let Some(blob) = table.get(fc..fc.saturating_add(lcb)) else {
        return Vec::new();
    };
    let lfo_mac = u32le(blob, 0).unwrap_or(0) as usize;
    if lfo_mac > 1 << 16 {
        return Vec::new();
    }
    let Some(headers_end) = lfo_mac
        .checked_mul(16)
        .and_then(|bytes| 4usize.checked_add(bytes))
    else {
        return Vec::new();
    };
    if blob.get(4..headers_end).is_none() {
        return Vec::new();
    }

    let mut counts = Vec::new();
    let mut out = Vec::new();
    if counts.try_reserve_exact(lfo_mac).is_err() || out.try_reserve_exact(lfo_mac).is_err() {
        return Vec::new();
    }
    for i in 0..lfo_mac {
        let off = 4 + i * 16;
        let Some(lsid) = u32le(blob, off) else {
            return Vec::new();
        };
        counts.push(blob[off + 12] as usize);
        out.push(ListFormatOverride {
            lsid: lsid as i32,
            ..ListFormatOverride::default()
        });
    }

    let mut cur = headers_end;
    for (index, count) in counts.into_iter().enumerate() {
        if count > 9 {
            break;
        }
        let Some(cp_end) = cur.checked_add(4) else {
            break;
        };
        if blob.get(cur..cp_end).is_none() {
            break;
        }
        cur = cp_end;

        let mut levels = Vec::with_capacity(count);
        let mut complete = true;
        for _ in 0..count {
            let Some(fixed_end) = cur.checked_add(8) else {
                complete = false;
                break;
            };
            let Some(fixed) = blob.get(cur..fixed_end) else {
                complete = false;
                break;
            };
            let start_at = i32::from_le_bytes([fixed[0], fixed[1], fixed[2], fixed[3]]);
            let flags = u32::from_le_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]);
            cur = fixed_end;

            let level_index = (flags & 0x0F) as usize;
            let has_start = flags & (1 << 4) != 0;
            let has_formatting = flags & (1 << 5) != 0;
            let formatting = if has_formatting {
                let Some(level) = parse_lvl(blob, &mut cur) else {
                    complete = false;
                    break;
                };
                Some(level)
            } else {
                None
            };

            if level_index >= 9 {
                continue;
            }
            levels.push((
                level_index as u8,
                LevelOverride {
                    start_at: (!has_formatting && has_start && (0..=0x7FFF).contains(&start_at))
                        .then_some(start_at),
                    formatting,
                },
            ));
        }
        if !complete {
            break;
        }
        out[index].levels = levels;
    }
    out
}

/// Stateful list numberer: advances shared-list counters in paragraph order and
/// renders each label.
#[derive(Debug)]
pub(crate) struct Numberer<'a> {
    lists: &'a Lists,
    /// `lsid` → index into `lists.defs`, built once. Replaces a per-paragraph
    /// linear scan of every list definition (`def_for`), which a crafted `.doc`
    /// with many list defs + many list paragraphs turned into O(paragraphs × defs)
    /// work at `Document::open` (CPU DoS). First definition wins, matching the
    /// previous `iter().find`.
    lsid_index: HashMap<i32, usize>,
    counters: HashMap<i32, [i32; 9]>,
    seen: HashMap<i32, [bool; 9]>,
    instance_seen: HashMap<u16, [bool; 9]>,
}

impl<'a> Numberer<'a> {
    pub(crate) fn new(lists: &'a Lists) -> Self {
        let mut lsid_index = HashMap::with_capacity(lists.defs.len());
        for (i, d) in lists.defs.iter().enumerate() {
            lsid_index.entry(d.lsid).or_insert(i);
        }
        Numberer {
            lists,
            lsid_index,
            counters: HashMap::new(),
            seen: HashMap::new(),
            instance_seen: HashMap::new(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }

    fn list_for(&self, ilfo: u16) -> Option<(&'a ListDef, &'a ListFormatOverride)> {
        let lfo = self.lists.lfos.get((ilfo as usize).checked_sub(1)?)?;
        let def = self.lists.defs.get(*self.lsid_index.get(&lfo.lsid)?)?;
        Some((def, lfo))
    }

    /// Advance the counters for paragraph `(ilfo, ilvl)` and return its label
    /// (including the trailing follow character), or `None` for non-list /
    /// bullet / no-number paragraphs.
    pub(crate) fn label(&mut self, ilfo: u16, ilvl: u8) -> Option<String> {
        if ilfo == 0 {
            return None;
        }
        let (def, lfo) = self.list_for(ilfo)?;
        let ilvl = (ilvl as usize).min(8);
        let level_for = |k: usize| {
            let base_index = if def.simple { 0 } else { k };
            lfo.level(k)
                .and_then(|level| level.formatting.as_ref())
                .or_else(|| def.levels.get(base_index))
        };
        let level = level_for(ilvl)?;
        let first_instance_level = {
            let seen = self.instance_seen.entry(ilfo).or_insert([false; 9]);
            let first = !seen[ilvl];
            seen[ilvl] = true;
            first
        };

        // Update counters. The active level: start-at on first sight, else +1.
        let cnt = self.counters.entry(lfo.lsid).or_insert([0; 9]);
        let seen = self.seen.entry(lfo.lsid).or_insert([false; 9]);
        let start_at = lfo.level(ilvl).and_then(|level| level.start_at);
        if !seen[ilvl] {
            cnt[ilvl] = if first_instance_level {
                start_at.unwrap_or(level.start)
            } else {
                level.start
            };
            seen[ilvl] = true;
        } else if first_instance_level {
            if let Some(start_at) = start_at {
                cnt[ilvl] = start_at;
            } else {
                cnt[ilvl] = cnt[ilvl].saturating_add(1);
            }
        } else {
            cnt[ilvl] = cnt[ilvl].saturating_add(1);
        }
        // [MS-DOC] 2.4.6.4: a deeper sequence restarts only after a level more
        // significant than its effective ilvlRestartLim. Invalid limits use the
        // ordinary rule rather than suppressing restarts indefinitely.
        for (k, seen_k) in seen.iter_mut().enumerate().skip(ilvl + 1) {
            let restart_limit = level_for(k)
                .and_then(|level| level.restart_limit)
                .filter(|limit| usize::from(*limit) <= k)
                .map_or(k, usize::from);
            if ilvl < restart_limit {
                *seen_k = false;
            }
        }
        // Ancestor levels referenced by this level's template but not yet seen
        // are seeded to their start-at, so a deep-first paragraph renders "1.1",
        // not "0.1".
        for k in 0..ilvl {
            if !seen[k] {
                cnt[k] = level_for(k).map(|level| level.start).unwrap_or(1);
                seen[k] = true;
            }
        }
        let counters = *cnt;

        // Bullet / none → no number prefix (kept out of the indexed text).
        if level.nfc == 0x17 || level.nfc == 0xFF {
            return None;
        }

        let placeholders: &[u8] = {
            let n = level.rgbxch_nums.iter().take_while(|&&x| x != 0).count();
            &level.rgbxch_nums[..n]
        };
        let mut out = String::new();
        for (pos, &ch) in level.xst.iter().enumerate() {
            if placeholders.contains(&((pos + 1) as u8)) {
                let k = (ch as usize).min(8);
                let original_nfc = level_for(k).map(|level| level.nfc).unwrap_or(level.nfc);
                let knfc = if level.legal && original_nfc != 0x16 {
                    0x00
                } else {
                    original_nfc
                };
                out.push_str(&numfmt::format(counters[k].max(0) as u32, knfc));
            } else if let Some(c) = char::from_u32(ch as u32) {
                out.push(c);
            }
        }
        match level.ixch_follow {
            0 => out.push('\t'),
            1 => out.push(' '),
            _ => {}
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(start: i32, rgbxch_nums: [u8; 9], ixch_follow: u8, xst: Vec<u16>) -> Level {
        Level {
            nfc: 0,
            start,
            legal: false,
            restart_limit: None,
            rgbxch_nums,
            ixch_follow,
            xst,
        }
    }

    fn serialized_lvl(
        start: i32,
        nfc: u8,
        rgbxch_nums: [u8; 9],
        ixch_follow: u8,
        xst: &[u16],
    ) -> Vec<u8> {
        let mut bytes = vec![0u8; 28];
        bytes[0..4].copy_from_slice(&start.to_le_bytes());
        bytes[4] = nfc;
        bytes[6..15].copy_from_slice(&rgbxch_nums);
        bytes[15] = ixch_follow;
        bytes.extend_from_slice(&(xst.len() as u16).to_le_bytes());
        for unit in xst {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    fn start_override(level: u8, start: i32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8);
        bytes.extend_from_slice(&start.to_le_bytes());
        bytes.extend_from_slice(&(u32::from(level) | (1 << 4)).to_le_bytes());
        bytes
    }

    fn formatting_override(level: u8, outer_start: i32, replacement: Vec<u8>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + replacement.len());
        bytes.extend_from_slice(&outer_start.to_le_bytes());
        bytes.extend_from_slice(&(u32::from(level) | (1 << 4) | (1 << 5)).to_le_bytes());
        bytes.extend(replacement);
        bytes
    }

    fn parsed_simple_decimal_lists_with_lfo(lfo: Vec<u8>) -> Lists {
        let mut table = Vec::new();
        table.extend_from_slice(&1i16.to_le_bytes());
        let mut lstf = [0u8; 28];
        lstf[0..4].copy_from_slice(&42i32.to_le_bytes());
        lstf[26] = 0x01;
        table.extend_from_slice(&lstf);
        let lcb_lst = table.len();
        table.extend(serialized_lvl(
            1,
            0,
            [1, 0, 0, 0, 0, 0, 0, 0, 0],
            0,
            &[0, '.' as u16],
        ));

        let fc_lfo = table.len();
        let lcb_lfo = lfo.len();
        table.extend(lfo);

        parse(&table, 0, lcb_lst, fc_lfo, lcb_lfo)
    }

    fn parsed_simple_decimal_lists(entries: Vec<(i32, Vec<Vec<u8>>)>) -> Lists {
        let mut lfo = Vec::new();
        lfo.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (lsid, overrides) in &entries {
            let mut header = [0u8; 16];
            header[0..4].copy_from_slice(&lsid.to_le_bytes());
            header[12] = overrides.len() as u8;
            lfo.extend_from_slice(&header);
        }
        for (_, overrides) in entries {
            lfo.extend_from_slice(&0u32.to_le_bytes());
            for record in overrides {
                lfo.extend(record);
            }
        }

        parsed_simple_decimal_lists_with_lfo(lfo)
    }

    fn parsed_multilevel_decimal_lists(mut levels: Vec<Vec<u8>>, overrides: Vec<Vec<u8>>) -> Lists {
        while levels.len() < 9 {
            levels.push(serialized_lvl(1, 0, [0; 9], 2, &[]));
        }

        let mut table = Vec::new();
        table.extend_from_slice(&1i16.to_le_bytes());
        let mut lstf = [0u8; 28];
        lstf[0..4].copy_from_slice(&7i32.to_le_bytes());
        table.extend_from_slice(&lstf);
        let lcb_lst = table.len();
        for level in levels {
            table.extend(level);
        }

        let fc_lfo = table.len();
        let mut lfo = Vec::new();
        lfo.extend_from_slice(&1u32.to_le_bytes());
        let mut header = [0u8; 16];
        header[0..4].copy_from_slice(&7i32.to_le_bytes());
        header[12] = overrides.len() as u8;
        lfo.extend_from_slice(&header);
        lfo.extend_from_slice(&0u32.to_le_bytes());
        for record in overrides {
            lfo.extend(record);
        }
        let lcb_lfo = lfo.len();
        table.extend(lfo);

        parse(&table, 0, lcb_lst, fc_lfo, lcb_lfo)
    }

    fn three_level_decimal_lvls() -> Vec<Vec<u8>> {
        vec![
            serialized_lvl(1, 0, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, &[0, '.' as u16]),
            serialized_lvl(1, 0, [1, 3, 0, 0, 0, 0, 0, 0, 0], 2, &[0, '.' as u16, 1]),
            serialized_lvl(
                1,
                0,
                [1, 3, 5, 0, 0, 0, 0, 0, 0],
                2,
                &[0, '.' as u16, 1, '.' as u16, 2],
            ),
        ]
    }

    fn with_restart_limit(mut level: Vec<u8>, limit: u8) -> Vec<u8> {
        level[5] |= 1 << 3;
        level[26] = limit;
        level
    }

    fn with_legal(mut level: Vec<u8>) -> Vec<u8> {
        level[5] |= 1 << 2;
        level
    }

    /// Build a Lists with one simple decimal list ("1." template) for ilfo 1.
    fn decimal_list() -> Lists {
        Lists {
            defs: vec![ListDef {
                lsid: 42,
                simple: true,
                levels: vec![lvl(
                    1,
                    [1, 0, 0, 0, 0, 0, 0, 0, 0],
                    0,
                    vec![0x0000, '.' as u16],
                )],
            }],
            lfos: vec![ListFormatOverride {
                lsid: 42,
                ..ListFormatOverride::default()
            }],
        }
    }

    #[test]
    fn parse_lvl_caps_xst_but_advances_cursor_fully() {
        // A crafted LVL declares a huge `cch` template. We keep at most MAX_XST_LEN units
        // (so per-paragraph label generation can't amplify), but still advance the cursor by
        // the FULL declared length so subsequent levels stay aligned.
        let mut buf = vec![0u8; 28]; // LVLF: start/nfc/flags/grpprl-sizes all 0
        let cch = 1000u16;
        buf.extend_from_slice(&cch.to_le_bytes());
        buf.resize(buf.len() + cch as usize * 2, 0x41u8); // cch UTF-16 units
        let mut cur = 0usize;
        let lvl = parse_lvl(&buf, &mut cur).expect("LVL parses");
        assert_eq!(lvl.xst.len(), MAX_XST_LEN); // capped, not 1000
        assert_eq!(cur, 28 + 2 + 2 * cch as usize); // cursor advanced by full declared cch
    }

    #[test]
    fn parse_lvl_rejects_a_truncated_declared_template() {
        let mut buf = vec![0u8; 28];
        buf.extend_from_slice(&1000u16.to_le_bytes());
        buf.resize(buf.len() + MAX_XST_LEN * 2, 0x41);
        let mut cur = 0usize;

        assert!(parse_lvl(&buf, &mut cur).is_none());
    }

    #[test]
    fn lfolvl_start_override_resets_once_and_same_lsid_instances_continue() {
        let lists = parsed_simple_decimal_lists(vec![
            (42, Vec::new()),
            (42, Vec::new()),
            (42, vec![start_override(0, 9)]),
        ]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("1.\t"));
        assert_eq!(numberer.label(2, 0).as_deref(), Some("2.\t"));
        assert_eq!(numberer.label(3, 0).as_deref(), Some("9.\t"));
        assert_eq!(numberer.label(3, 0).as_deref(), Some("10.\t"));
        assert_eq!(numberer.label(1, 0).as_deref(), Some("11.\t"));
    }

    #[test]
    fn lfolvl_replacement_format_uses_embedded_lvl_and_ignores_outer_start() {
        let replacement = serialized_lvl(
            4,
            1,
            [2, 0, 0, 0, 0, 0, 0, 0, 0],
            2,
            &['(' as u16, 0, ')' as u16],
        );
        let lists =
            parsed_simple_decimal_lists(vec![(42, vec![formatting_override(0, 99, replacement)])]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("(IV)"));
        assert_eq!(numberer.label(1, 0).as_deref(), Some("(V)"));
    }

    #[test]
    fn invalid_lfolvl_starts_fall_back_to_shared_sequence() {
        let lists = parsed_simple_decimal_lists(vec![
            (42, vec![start_override(0, -1)]),
            (42, Vec::new()),
            (42, vec![start_override(0, 0x8000)]),
        ]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("1.\t"));
        assert_eq!(numberer.label(2, 0).as_deref(), Some("2.\t"));
        assert_eq!(numberer.label(3, 0).as_deref(), Some("3.\t"));
    }

    #[test]
    fn invalid_lfolvl_level_is_consumed_before_a_valid_record() {
        let lists = parsed_simple_decimal_lists(vec![(
            42,
            vec![start_override(9, 99), start_override(0, 7)],
        )]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("7.\t"));
    }

    #[test]
    fn malformed_replacement_lvl_keeps_fixed_lfo_mappings() {
        let mut lfo = Vec::new();
        lfo.extend_from_slice(&2u32.to_le_bytes());
        for count in [1u8, 0] {
            let mut header = [0u8; 16];
            header[0..4].copy_from_slice(&42i32.to_le_bytes());
            header[12] = count;
            lfo.extend_from_slice(&header);
        }
        lfo.extend_from_slice(&0u32.to_le_bytes());
        lfo.extend(formatting_override(0, 99, vec![0u8; 8]));
        lfo.extend_from_slice(&0u32.to_le_bytes());

        let lists = parsed_simple_decimal_lists_with_lfo(lfo);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("1.\t"));
        assert_eq!(numberer.label(2, 0).as_deref(), Some("2.\t"));
    }

    #[test]
    fn oversized_lfo_count_does_not_accept_a_partial_fixed_array() {
        let mut lfo = Vec::new();
        lfo.extend_from_slice(&u32::MAX.to_le_bytes());
        let mut header = [0u8; 16];
        header[0..4].copy_from_slice(&42i32.to_le_bytes());
        lfo.extend_from_slice(&header);

        let lists = parsed_simple_decimal_lists_with_lfo(lfo);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(lists.defs.len(), 1);
        assert!(lists.lfos.is_empty());
        assert_eq!(numberer.label(1, 0), None);
    }

    #[test]
    fn incomplete_lfo_fixed_array_is_rejected_before_variable_data() {
        let mut lfo = Vec::new();
        lfo.extend_from_slice(&2u32.to_le_bytes());
        let mut header = [0u8; 16];
        header[0..4].copy_from_slice(&42i32.to_le_bytes());
        lfo.extend_from_slice(&header);

        let lists = parsed_simple_decimal_lists_with_lfo(lfo);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(lists.defs.len(), 1);
        assert!(lists.lfos.is_empty());
        assert_eq!(numberer.label(1, 0), None);
    }

    #[test]
    fn def_for_resolves_correct_definition_by_lsid_among_many() {
        // ilfo 1 maps (via its LFO) to the SECOND definition's lsid. The lsid index
        // must select that def, not the first — and do so without scanning every def
        // (the prior linear `def_for` was an O(paragraphs × defs) DoS).
        let lists = Lists {
            defs: vec![
                ListDef {
                    lsid: 100,
                    simple: true,
                    levels: vec![lvl(
                        5,
                        [5, 0, 0, 0, 0, 0, 0, 0, 0],
                        0,
                        vec![0x0000, '.' as u16],
                    )],
                },
                ListDef {
                    lsid: 200,
                    simple: true,
                    levels: vec![lvl(
                        1,
                        [1, 0, 0, 0, 0, 0, 0, 0, 0],
                        0,
                        vec![0x0000, ')' as u16],
                    )],
                },
            ],
            lfos: vec![ListFormatOverride {
                lsid: 200,
                ..ListFormatOverride::default()
            }],
        };
        let mut n = Numberer::new(&lists);
        // Second def (start=1, ")" follow), not the first (would be "5.").
        assert_eq!(n.label(1, 0).as_deref(), Some("1)\t"));
    }

    #[test]
    fn numbers_a_simple_list() {
        let lists = decimal_list();
        let mut n = Numberer::new(&lists);
        assert_eq!(n.label(1, 0).as_deref(), Some("1.\t"));
        assert_eq!(n.label(1, 0).as_deref(), Some("2.\t"));
        assert_eq!(n.label(1, 0).as_deref(), Some("3.\t"));
        // Unknown ilfo / non-list → None.
        assert_eq!(n.label(0, 0), None);
        assert_eq!(n.label(99, 0), None);
    }

    #[test]
    fn distinct_lsids_keep_independent_counters() {
        let mut lists = decimal_list();
        lists.defs.push(ListDef {
            lsid: 77,
            simple: true,
            levels: vec![lvl(
                5,
                [1, 0, 0, 0, 0, 0, 0, 0, 0],
                0,
                vec![0x0000, '.' as u16],
            )],
        });
        lists.lfos.extend([
            ListFormatOverride {
                lsid: 77,
                ..ListFormatOverride::default()
            },
            ListFormatOverride {
                lsid: 42,
                ..ListFormatOverride::default()
            },
        ]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("1.\t"));
        assert_eq!(numberer.label(2, 0).as_deref(), Some("5.\t"));
        assert_eq!(numberer.label(3, 0).as_deref(), Some("2.\t"));
        assert_eq!(numberer.label(2, 0).as_deref(), Some("6.\t"));
    }

    /// ilfo 1: 9-level list, level 0 = "%0.", level 1 = "%0.%1".
    fn multilevel_list() -> Lists {
        let mut levels = vec![
            lvl(1, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, vec![0x0000, '.' as u16]),
            lvl(
                1,
                [1, 3, 0, 0, 0, 0, 0, 0, 0],
                2,
                vec![0x0000, '.' as u16, 0x0001],
            ),
        ];
        levels.resize(9, Level::default());
        Lists {
            defs: vec![ListDef {
                lsid: 7,
                simple: false,
                levels,
            }],
            lfos: vec![ListFormatOverride {
                lsid: 7,
                ..ListFormatOverride::default()
            }],
        }
    }

    #[test]
    fn multilevel_resets_deeper_counter() {
        let lists = multilevel_list();
        let mut n = Numberer::new(&lists);
        assert_eq!(n.label(1, 0).as_deref(), Some("1.")); // level 0 → 1
        assert_eq!(n.label(1, 1).as_deref(), Some("1.1")); // level 1 → 1.1
        assert_eq!(n.label(1, 1).as_deref(), Some("1.2")); // → 1.2
        assert_eq!(n.label(1, 0).as_deref(), Some("2.")); // level 0 → 2 (resets L1)
        assert_eq!(n.label(1, 1).as_deref(), Some("2.1")); // L1 restarted
    }

    #[test]
    fn lvlf_restart_limit_restarts_only_after_more_significant_level() {
        let mut levels = three_level_decimal_lvls();
        levels[2] = with_restart_limit(levels[2].clone(), 1);
        let lists = parsed_multilevel_decimal_lists(levels, Vec::new());
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("1."));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.1"));
        assert_eq!(numberer.label(1, 2).as_deref(), Some("1.1.1"));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.2"));
        assert_eq!(numberer.label(1, 2).as_deref(), Some("1.2.2"));
        assert_eq!(numberer.label(1, 0).as_deref(), Some("2."));
        assert_eq!(numberer.label(1, 2).as_deref(), Some("2.1.1"));
    }

    #[test]
    fn replacement_lvlf_restart_limit_controls_counter_reset() {
        let levels = three_level_decimal_lvls();
        let replacement = with_restart_limit(levels[2].clone(), 1);
        let lists =
            parsed_multilevel_decimal_lists(levels, vec![formatting_override(2, 99, replacement)]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 2).as_deref(), Some("1.1.1"));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.2"));
        assert_eq!(numberer.label(1, 2).as_deref(), Some("1.2.2"));
        assert_eq!(numberer.label(1, 0).as_deref(), Some("2."));
        assert_eq!(numberer.label(1, 2).as_deref(), Some("2.1.1"));
    }

    #[test]
    fn invalid_lvlf_restart_limit_uses_ordinary_restart_rule() {
        let mut levels = three_level_decimal_lvls();
        levels[2] = with_restart_limit(levels[2].clone(), 8);
        let lists = parsed_multilevel_decimal_lists(levels, Vec::new());
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 2).as_deref(), Some("1.1.1"));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.2"));
        assert_eq!(numberer.label(1, 2).as_deref(), Some("1.2.1"));
    }

    #[test]
    fn lvlf_legal_formats_current_and_inherited_placeholders_as_arabic() {
        let levels = vec![
            serialized_lvl(1, 0x01, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, &[0]),
            with_legal(serialized_lvl(
                1,
                0x04,
                [1, 3, 0, 0, 0, 0, 0, 0, 0],
                2,
                &[0, '.' as u16, 1],
            )),
        ];
        let lists = parsed_multilevel_decimal_lists(levels, Vec::new());
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 0).as_deref(), Some("I"));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.1"));
    }

    #[test]
    fn lvlf_without_legal_keeps_each_placeholder_format() {
        let levels = vec![
            serialized_lvl(1, 0x01, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, &[0]),
            serialized_lvl(1, 0x04, [1, 3, 0, 0, 0, 0, 0, 0, 0], 2, &[0, '.' as u16, 1]),
        ];
        let lists = parsed_multilevel_decimal_lists(levels, Vec::new());
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 1).as_deref(), Some("I.a"));
    }

    #[test]
    fn lvlf_legal_preserves_inherited_arabic_lz() {
        let levels = vec![
            serialized_lvl(1, 0x16, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, &[0]),
            with_legal(serialized_lvl(
                1,
                0x03,
                [1, 3, 0, 0, 0, 0, 0, 0, 0],
                2,
                &[0, '.' as u16, 1],
            )),
        ];
        let lists = parsed_multilevel_decimal_lists(levels, Vec::new());
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 1).as_deref(), Some("01.1"));
    }

    #[test]
    fn lvlf_legal_preserves_current_arabic_lz() {
        let levels = vec![
            serialized_lvl(1, 0x01, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, &[0]),
            with_legal(serialized_lvl(
                1,
                0x16,
                [1, 3, 0, 0, 0, 0, 0, 0, 0],
                2,
                &[0, '.' as u16, 1],
            )),
        ];
        let lists = parsed_multilevel_decimal_lists(levels, Vec::new());
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.01"));
    }

    #[test]
    fn replacement_lvlf_legal_controls_effective_formatting() {
        let levels = vec![
            serialized_lvl(1, 0x01, [1, 0, 0, 0, 0, 0, 0, 0, 0], 2, &[0]),
            serialized_lvl(1, 0x04, [1, 3, 0, 0, 0, 0, 0, 0, 0], 2, &[0, '.' as u16, 1]),
        ];
        let replacement = with_legal(levels[1].clone());
        let lists =
            parsed_multilevel_decimal_lists(levels, vec![formatting_override(1, 99, replacement)]);
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.1"));
    }

    #[test]
    fn lfolvl_start_override_is_not_reapplied_after_a_hierarchy_restart() {
        let mut lists = multilevel_list();
        lists.lfos[0].levels.push((
            1,
            LevelOverride {
                start_at: Some(3),
                ..LevelOverride::default()
            },
        ));
        let mut numberer = Numberer::new(&lists);

        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.3"));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("1.4"));
        assert_eq!(numberer.label(1, 0).as_deref(), Some("2."));
        assert_eq!(numberer.label(1, 1).as_deref(), Some("2.1"));
    }

    #[test]
    fn deep_first_paragraph_seeds_ancestors() {
        // First list paragraph appears at level 1 with no prior level-0 paragraph:
        // the ancestor must be seeded to its start (1), not rendered as 0.
        let lists = multilevel_list();
        let mut n = Numberer::new(&lists);
        assert_eq!(n.label(1, 1).as_deref(), Some("1.1")); // not "0.1"
    }
}
