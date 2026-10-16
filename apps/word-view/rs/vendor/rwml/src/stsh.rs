//! Style sheet (STSH) parsing for paragraph style identity and the bounded
//! paragraph-property subsets consumed by the legacy reader.
//!
//! The STSH (FIB `fcStshf`, pair index 1, in the table stream) is an `STSHI`
//! header followed by one `LPStd` per style. Each `STD` carries a built-in style
//! id (`sti`), a style kind (`sgc`), the base style it inherits from
//! (`istdBase`), a display name, and length-prefixed UPX property differences.
//! A paragraph's `istd` (from its PAPX) indexes this array; the heading level is
//! derived from `sti` (1–9 = Heading 1–9), the base-style chain, or the localized
//! name (`Heading N` / `제목 N`). Paragraph styles also resolve the bounded
//! layout, indent, spacing, pagination, flat-color shading, and custom-tab SPRM
//! subsets through the same base chain.
//!
//! Reference: [MS-DOC] 2.9.271 (STSH), 2.9.272 (STSHI), 2.9.135 (LPStd),
//! 2.9.270 (STD), 2.9.270.1 (StdfBase), 2.9.276 (sti).

use crate::papx::{
    scan_paragraph_style_overrides, ParagraphIndentOverrides, ParagraphLayoutOverrides,
    ParagraphPagination, ParagraphShading, ParagraphSpacingOverrides, ParagraphStyleOverrides,
};
#[cfg(any(feature = "docx", feature = "render"))]
use crate::papx::{scan_paragraph_style_tab_changes, LegacyTabStop, ParagraphTabStopChanges};
use crate::util::u16le;

/// One style's identity (enough to resolve a heading level + name).
#[derive(Debug, Clone, Default)]
struct StyleDescription {
    sti: u16,
    sgc: u8,
    istd_base: u16,
    name: String,
}

struct ParsedStyle {
    description: StyleDescription,
    paragraph: Option<ParsedParagraphStyleProperties>,
}

struct ParsedParagraphStyleProperties {
    properties: ParagraphStyleOverrides,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stop_changes: Option<ParagraphTabStopChanges>,
}

/// The parsed stylesheet: per-`istd` heading, name, and bounded paragraph properties.
#[derive(Debug, Default)]
pub(crate) struct StyleSheet {
    heading: Vec<Option<u8>>,
    names: Vec<String>,
    layout: Vec<ParagraphLayoutOverrides>,
    indent: Vec<ParagraphIndentOverrides>,
    spacing: Vec<ParagraphSpacingOverrides>,
    pagination: Vec<ParagraphPagination>,
    shading: Vec<Option<ParagraphShading>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: Vec<Option<Vec<LegacyTabStop>>>,
}

impl StyleSheet {
    /// The heading level (1–9) for a paragraph style index, or `None` for body.
    pub(crate) fn heading_level(&self, istd: u16) -> Option<u8> {
        self.heading.get(istd as usize).copied().flatten()
    }

    /// The display name for a style index (e.g. `Heading 1`, `제목 1`), if known.
    pub(crate) fn name(&self, istd: u16) -> Option<&str> {
        self.names
            .get(istd as usize)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    /// Layout properties resolved through the paragraph style's base chain.
    pub(crate) fn paragraph_layout(&self, istd: u16) -> ParagraphLayoutOverrides {
        self.layout.get(istd as usize).copied().unwrap_or_default()
    }

    /// Indent properties resolved through the paragraph style's base chain.
    pub(crate) fn paragraph_indent(&self, istd: u16) -> ParagraphIndentOverrides {
        self.indent.get(istd as usize).copied().unwrap_or_default()
    }

    /// Spacing properties resolved through the paragraph style's base chain.
    pub(crate) fn paragraph_spacing(&self, istd: u16) -> ParagraphSpacingOverrides {
        self.spacing.get(istd as usize).copied().unwrap_or_default()
    }

    /// Pagination properties resolved through the paragraph style's base chain.
    pub(crate) fn paragraph_pagination(&self, istd: u16) -> ParagraphPagination {
        self.pagination
            .get(istd as usize)
            .copied()
            .unwrap_or_default()
    }

    /// Shading resolved through the paragraph style's base chain.
    pub(crate) fn paragraph_shading(&self, istd: u16) -> Option<ParagraphShading> {
        self.shading.get(istd as usize).copied().flatten()
    }

    /// Custom tab stops resolved through the paragraph style's base chain.
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) fn paragraph_tab_stops(&self, istd: u16) -> Option<&[LegacyTabStop]> {
        if self.tab_stops.is_empty() && istd == 0 {
            return Some(&[]);
        }
        self.tab_stops.get(istd as usize).and_then(Option::as_deref)
    }

    #[cfg(test)]
    pub(crate) fn from_test_pagination(pagination: Vec<ParagraphPagination>) -> Self {
        let len = pagination.len();
        Self {
            heading: vec![None; len],
            names: vec![String::new(); len],
            layout: vec![ParagraphLayoutOverrides::default(); len],
            indent: vec![ParagraphIndentOverrides::default(); len],
            spacing: vec![ParagraphSpacingOverrides::default(); len],
            pagination,
            shading: vec![None; len],
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops: vec![Some(Vec::new()); len],
        }
    }

    /// Parse the STSH from the table stream. Returns an empty stylesheet (not an
    /// error) on absence or header malformation; malformed style slots degrade
    /// independently.
    pub(crate) fn parse(table: &[u8], fc_stshf: usize, lcb_stshf: usize) -> StyleSheet {
        let empty = StyleSheet::default();
        if lcb_stshf < 8 {
            return empty;
        }
        let Some(stsh) = table.get(fc_stshf..fc_stshf.saturating_add(lcb_stshf)) else {
            return empty;
        };
        let (Some(cb_stshi), Some(cstd), Some(cb_std_base)) =
            (u16le(stsh, 0), u16le(stsh, 2), u16le(stsh, 4))
        else {
            return empty;
        };
        let base_len = match cb_std_base {
            10 | 18 => cb_std_base as usize,
            _ => return empty,
        };

        let mut p = 2usize.saturating_add(cb_stshi as usize);
        let mut descs: Vec<Option<StyleDescription>> = Vec::with_capacity(cstd as usize);
        let mut local_properties = Vec::with_capacity(cstd as usize);
        #[cfg(any(feature = "docx", feature = "render"))]
        let mut local_tab_stop_changes = Vec::with_capacity(cstd as usize);
        for _ in 0..cstd {
            let Some(cb_std) = u16le(stsh, p) else { break };
            p += 2;
            if cb_std == 0 {
                descs.push(None); // empty slot still consumes an istd index
                local_properties.push(None);
                #[cfg(any(feature = "docx", feature = "render"))]
                local_tab_stop_changes.push(None);
                continue;
            }
            let cb_std = cb_std as usize;
            let Some(std) = stsh.get(p..p.saturating_add(cb_std)) else {
                break;
            };
            p += cb_std;
            let istd = descs.len() as u16;
            let parsed = parse_std(std, base_len, istd);
            local_properties.push(
                parsed
                    .as_ref()
                    .and_then(|style| style.paragraph.as_ref())
                    .map(|paragraph| paragraph.properties),
            );
            #[cfg(any(feature = "docx", feature = "render"))]
            local_tab_stop_changes.push(
                parsed
                    .as_ref()
                    .and_then(|style| style.paragraph.as_ref())
                    .and_then(|paragraph| paragraph.tab_stop_changes.clone()),
            );
            descs.push(parsed.map(|style| style.description));
        }

        let n = descs.len();
        let mut heading = vec![None; n];
        let mut names = vec![String::new(); n];
        let mut layout = vec![ParagraphLayoutOverrides::default(); n];
        let mut indent = vec![ParagraphIndentOverrides::default(); n];
        let mut spacing = vec![ParagraphSpacingOverrides::default(); n];
        let mut pagination = vec![ParagraphPagination::default(); n];
        let mut shading = vec![None; n];
        #[cfg(any(feature = "docx", feature = "render"))]
        let tab_stops = resolve_all_tab_stops(&descs, &local_tab_stop_changes);
        // Per-style cycle guard by epoch: `visited[i]` is the pass (`gen`) that last touched
        // style `i`. "Clearing" between styles is just a fresh `gen` (O(1)) — refilling the
        // whole buffer each pass was O(n) per style = O(n^2) writes (≈4.3e9 for cstd=65535),
        // a CPU DoS on a crafted `.doc`. `gen` starts at 1 so 0 reads as never-visited; `n`
        // is bounded by `cstd` (u16), so `gen = istd + 1` never overflows `u32`.
        let mut visited = vec![0u32; n];
        let mut layout_visited = vec![0u32; n];
        let mut indent_visited = vec![0u32; n];
        let mut spacing_visited = vec![0u32; n];
        let mut pagination_visited = vec![0u32; n];
        let mut shading_visited = vec![0u32; n];
        for istd in 0..n {
            let gen = istd as u32 + 1;
            heading[istd] = resolve_level(&descs, istd, &mut visited, gen, 0);
            layout[istd] =
                resolve_layout(&descs, &local_properties, istd, &mut layout_visited, gen, 0)
                    .unwrap_or_default();
            indent[istd] =
                resolve_indent(&descs, &local_properties, istd, &mut indent_visited, gen, 0)
                    .unwrap_or_default();
            spacing[istd] = resolve_spacing(
                &descs,
                &local_properties,
                istd,
                &mut spacing_visited,
                gen,
                0,
            )
            .unwrap_or_default();
            pagination[istd] = resolve_pagination(
                &descs,
                &local_properties,
                istd,
                &mut pagination_visited,
                gen,
                0,
            )
            .unwrap_or_default();
            shading[istd] = resolve_shading(
                &descs,
                &local_properties,
                istd,
                &mut shading_visited,
                gen,
                0,
            )
            .flatten();
            if let Some(d) = &descs[istd] {
                names[istd] = d.name.clone();
            }
        }
        StyleSheet {
            heading,
            names,
            layout,
            indent,
            spacing,
            pagination,
            shading,
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops,
        }
    }
}

fn parse_std(std: &[u8], base_len: usize, istd: u16) -> Option<ParsedStyle> {
    let sti = u16le(std, 0)? & 0x0FFF;
    let grf = u16le(std, 2)?;
    let sgc = (grf & 0x000F) as u8;
    let istd_base = (grf >> 4) & 0x0FFF;
    let cupx = (u16le(std, 4)? & 0x000F) as u8;
    let has_original_style = base_len == 18 && u16le(std, 10)? & 0x1000 != 0;
    let (name, grlp_offset) = parse_xstz(std, base_len)?;
    let paragraph = if sgc == 1 {
        parse_paragraph_style_properties(std.get(grlp_offset..)?, cupx, has_original_style, istd)
    } else {
        None
    };
    Some(ParsedStyle {
        description: StyleDescription {
            sti,
            sgc,
            istd_base,
            name,
        },
        paragraph,
    })
}

fn parse_xstz(std: &[u8], offset: usize) -> Option<(String, usize)> {
    let cch = u16le(std, offset)? as usize;
    let chars_start = offset.checked_add(2)?;
    let chars_len = cch.checked_mul(2)?;
    let chars_end = chars_start.checked_add(chars_len)?;
    let terminator_end = chars_end.checked_add(2)?;
    let chars = std.get(chars_start..chars_end)?;
    if u16le(std, chars_end)? != 0 {
        return None;
    }
    Some((utf16le(chars), terminator_end))
}

fn parse_paragraph_style_properties(
    grlp: &[u8],
    cupx: u8,
    has_original_style: bool,
    istd: u16,
) -> Option<ParsedParagraphStyleProperties> {
    if !matches!((cupx, has_original_style), (2, false) | (3, true)) {
        return None;
    }
    let (papx, mut offset) = read_lp_upx(grlp, 0)?;
    if papx.len() < 2 || u16le(papx, 0)? != istd {
        return None;
    }
    let properties = scan_paragraph_style_overrides(&papx[2..])?;
    #[cfg(any(feature = "docx", feature = "render"))]
    let tab_stop_changes = scan_paragraph_style_tab_changes(&papx[2..]);

    let (_, next) = read_lp_upx(grlp, offset)?;
    offset = next;
    if cupx == 3 {
        let cb = u16le(grlp, offset)? as usize;
        let start = offset.checked_add(2)?;
        let end = start.checked_add(cb)?;
        validate_revision_paragraph_upx(grlp.get(start..end)?)?;
        offset = end;
    }
    (offset == grlp.len()).then_some(ParsedParagraphStyleProperties {
        properties,
        #[cfg(any(feature = "docx", feature = "render"))]
        tab_stop_changes,
    })
}

fn validate_revision_paragraph_upx(data: &[u8]) -> Option<()> {
    let (_, after_papx) = read_lp_upx(data, 8)?;
    let (_, end) = read_lp_upx(data, after_papx)?;
    (end == data.len()).then_some(())
}

fn read_lp_upx(data: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    let cb = u16le(data, offset)? as usize;
    let start = offset.checked_add(2)?;
    let end = start.checked_add(cb)?;
    let payload = data.get(start..end)?;
    let next = end.checked_add(cb & 1)?;
    if cb & 1 != 0 && data.get(end).copied()? != 0 {
        return None;
    }
    data.get(end..next)?;
    Some((payload, next))
}

/// Maximum base-style chain depth followed before giving up. Far deeper than any real style
/// hierarchy, but it bounds the recursion so a crafted `.doc` with a long `basedOn` chain
/// can't overflow the stack (the `visited` guard already breaks cycles; this caps a long
/// acyclic chain, whose length is otherwise only bounded by the attacker-controlled count).
const MAX_STYLE_BASE_DEPTH: usize = 64;

/// Resolve a style's heading level: by built-in `sti`, then up the base chain,
/// then by name. `visited` guards against base-style cycles; `depth` bounds an acyclic chain.
fn resolve_level(
    descs: &[Option<StyleDescription>],
    istd: usize,
    visited: &mut [u32],
    gen: u32,
    depth: usize,
) -> Option<u8> {
    if depth > MAX_STYLE_BASE_DEPTH
        || istd >= descs.len()
        || visited.get(istd).copied().unwrap_or(gen) == gen
    {
        return None;
    }
    visited[istd] = gen;
    let Some(d) = &descs[istd] else { return None };
    if (1..=9).contains(&d.sti) {
        return Some(d.sti as u8);
    }
    if d.sti == 0 {
        return None; // Normal
    }
    // Custom style based on a heading inherits its level.
    if d.sgc == 1 && d.istd_base != 0x0FFF && d.istd_base as usize != istd {
        if let Some(n) = resolve_level(descs, d.istd_base as usize, visited, gen, depth + 1) {
            return Some(n);
        }
    }
    heading_from_name(&d.name)
}

#[cfg(any(feature = "docx", feature = "render"))]
fn resolve_all_tab_stops(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphTabStopChanges>],
) -> Vec<Option<Vec<LegacyTabStop>>> {
    let mut states = vec![0u8; descs.len()];
    let mut resolved = vec![None; descs.len()];
    for istd in 0..descs.len() {
        let _ = resolve_tab_stops(descs, local, istd, &mut states, &mut resolved, 0);
    }
    resolved
}

#[cfg(any(feature = "docx", feature = "render"))]
fn resolve_tab_stops(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphTabStopChanges>],
    istd: usize,
    states: &mut [u8],
    resolved: &mut [Option<Vec<LegacyTabStop>>],
    depth: usize,
) -> Option<Vec<LegacyTabStop>> {
    if depth > MAX_STYLE_BASE_DEPTH || istd >= descs.len() {
        return None;
    }
    match states.get(istd).copied()? {
        1 => return None,
        2 => return resolved.get(istd)?.clone(),
        _ => {}
    }
    states[istd] = 1;
    let result = (|| {
        let description = descs.get(istd)?.as_ref()?;
        if description.sgc != 1 {
            return None;
        }
        let changes = local.get(istd)?.as_ref()?;
        let inherited = if description.istd_base == 0x0FFF {
            Vec::new()
        } else {
            resolve_tab_stops(
                descs,
                local,
                description.istd_base as usize,
                states,
                resolved,
                depth + 1,
            )?
        };
        changes.apply(&inherited)
    })();
    states[istd] = 2;
    resolved[istd] = result.clone();
    result
}

fn resolve_pagination(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphStyleOverrides>],
    istd: usize,
    visited: &mut [u32],
    gen: u32,
    depth: usize,
) -> Option<ParagraphPagination> {
    if depth > MAX_STYLE_BASE_DEPTH
        || istd >= descs.len()
        || visited.get(istd).copied().unwrap_or(gen) == gen
    {
        return None;
    }
    visited[istd] = gen;
    let description = descs.get(istd)?.as_ref()?;
    if description.sgc != 1 {
        return None;
    }
    let overrides = local.get(istd).copied().flatten()?.pagination;
    let inherited = if description.istd_base == 0x0FFF {
        ParagraphPagination::default()
    } else {
        resolve_pagination(
            descs,
            local,
            description.istd_base as usize,
            visited,
            gen,
            depth + 1,
        )?
    };
    Some(inherited.apply(overrides))
}

fn resolve_layout(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphStyleOverrides>],
    istd: usize,
    visited: &mut [u32],
    gen: u32,
    depth: usize,
) -> Option<ParagraphLayoutOverrides> {
    if depth > MAX_STYLE_BASE_DEPTH
        || istd >= descs.len()
        || visited.get(istd).copied().unwrap_or(gen) == gen
    {
        return None;
    }
    visited[istd] = gen;
    let description = descs.get(istd)?.as_ref()?;
    if description.sgc != 1 {
        return None;
    }
    let overrides = local.get(istd).copied().flatten()?.layout;
    let inherited = if description.istd_base == 0x0FFF {
        ParagraphLayoutOverrides::default()
    } else {
        resolve_layout(
            descs,
            local,
            description.istd_base as usize,
            visited,
            gen,
            depth + 1,
        )?
    };
    Some(inherited.apply(overrides))
}

fn resolve_indent(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphStyleOverrides>],
    istd: usize,
    visited: &mut [u32],
    gen: u32,
    depth: usize,
) -> Option<ParagraphIndentOverrides> {
    if depth > MAX_STYLE_BASE_DEPTH
        || istd >= descs.len()
        || visited.get(istd).copied().unwrap_or(gen) == gen
    {
        return None;
    }
    visited[istd] = gen;
    let description = descs.get(istd)?.as_ref()?;
    if description.sgc != 1 {
        return None;
    }
    let overrides = local.get(istd).copied().flatten()?.indent;
    let inherited = if description.istd_base == 0x0FFF {
        ParagraphIndentOverrides::default()
    } else {
        resolve_indent(
            descs,
            local,
            description.istd_base as usize,
            visited,
            gen,
            depth + 1,
        )?
    };
    Some(inherited.apply(overrides))
}

fn resolve_spacing(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphStyleOverrides>],
    istd: usize,
    visited: &mut [u32],
    gen: u32,
    depth: usize,
) -> Option<ParagraphSpacingOverrides> {
    if depth > MAX_STYLE_BASE_DEPTH
        || istd >= descs.len()
        || visited.get(istd).copied().unwrap_or(gen) == gen
    {
        return None;
    }
    visited[istd] = gen;
    let description = descs.get(istd)?.as_ref()?;
    if description.sgc != 1 {
        return None;
    }
    let overrides = local.get(istd).copied().flatten()?.spacing;
    let inherited = if description.istd_base == 0x0FFF {
        ParagraphSpacingOverrides::default()
    } else {
        resolve_spacing(
            descs,
            local,
            description.istd_base as usize,
            visited,
            gen,
            depth + 1,
        )?
    };
    Some(inherited.apply(overrides))
}

fn resolve_shading(
    descs: &[Option<StyleDescription>],
    local: &[Option<ParagraphStyleOverrides>],
    istd: usize,
    visited: &mut [u32],
    gen: u32,
    depth: usize,
) -> Option<Option<ParagraphShading>> {
    if depth > MAX_STYLE_BASE_DEPTH
        || istd >= descs.len()
        || visited.get(istd).copied().unwrap_or(gen) == gen
    {
        return None;
    }
    visited[istd] = gen;
    let description = descs.get(istd)?.as_ref()?;
    if description.sgc != 1 {
        return None;
    }
    let overrides = local.get(istd).copied().flatten()?.shading;
    let inherited = if description.istd_base == 0x0FFF {
        None
    } else {
        resolve_shading(
            descs,
            local,
            description.istd_base as usize,
            visited,
            gen,
            depth + 1,
        )?
    };
    Some(overrides.or(inherited))
}

/// `Heading N` (any case) or Korean `제목 N` → the digit `N` (1–9). Shared with
/// the `.docx` style resolver (`w:styleId` / `w:name` use the same conventions).
pub(crate) fn heading_from_name(name: &str) -> Option<u8> {
    let t = name.trim();
    if let Some(rest) = t.strip_prefix("제목") {
        if let Ok(n) = rest.trim().parse::<u8>() {
            if (1..=9).contains(&n) {
                return Some(n);
            }
        }
    }
    let lower = t.to_lowercase();
    if let Some(rest) = lower.strip_prefix("heading") {
        if let Ok(n) = rest.trim().parse::<u8>() {
            if (1..=9).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

fn utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;
    #[cfg(any(feature = "docx", feature = "render"))]
    use crate::model::{TabAlignment, TabLeader, TabStop};
    use crate::papx::{ParagraphJustification, ParagraphLineSpacing};

    fn paragraph_style_std(
        base_len: usize,
        istd: u16,
        base: u16,
        name: &str,
        properties: &[(u16, u8)],
        revision_marked: bool,
    ) -> Vec<u8> {
        let mut grpprl = Vec::with_capacity(properties.len() * 3);
        for &(sprm, value) in properties {
            grpprl.extend_from_slice(&sprm.to_le_bytes());
            grpprl.push(value);
        }
        paragraph_style_std_grpprl(base_len, istd, base, name, &grpprl, revision_marked)
    }

    fn paragraph_style_std_grpprl(
        base_len: usize,
        istd: u16,
        base: u16,
        name: &str,
        properties: &[u8],
        revision_marked: bool,
    ) -> Vec<u8> {
        let mut std = vec![0u8; base_len];
        let sti: u16 = if istd == 0 { 0 } else { 0x0FFE };
        std[0..2].copy_from_slice(&sti.to_le_bytes());
        std[2..4].copy_from_slice(&(1 | ((base & 0x0FFF) << 4)).to_le_bytes());
        let cupx: u16 = if revision_marked { 3 } else { 2 };
        std[4..6].copy_from_slice(&cupx.to_le_bytes());
        if revision_marked && base_len == 18 {
            std[10..12].copy_from_slice(&0x1000u16.to_le_bytes());
        }

        let name_units: Vec<u16> = name.encode_utf16().collect();
        std.extend_from_slice(&(name_units.len() as u16).to_le_bytes());
        for unit in name_units {
            std.extend_from_slice(&unit.to_le_bytes());
        }
        std.extend_from_slice(&0u16.to_le_bytes());

        let mut papx = Vec::with_capacity(2 + properties.len());
        papx.extend_from_slice(&istd.to_le_bytes());
        papx.extend_from_slice(properties);
        std.extend_from_slice(&(papx.len() as u16).to_le_bytes());
        std.extend_from_slice(&papx);
        if papx.len() % 2 == 1 {
            std.push(0);
        }
        std.extend_from_slice(&0u16.to_le_bytes());

        if revision_marked {
            let mut revision = vec![0u8; 8];
            revision.extend_from_slice(&0u16.to_le_bytes());
            revision.extend_from_slice(&0u16.to_le_bytes());
            std.extend_from_slice(&(revision.len() as u16).to_le_bytes());
            std.extend_from_slice(&revision);
        }
        let cb_std = std.len() as u16;
        std[6..8].copy_from_slice(&cb_std.to_le_bytes());
        std
    }

    fn paragraph_shd80_sprm(foreground: u8, background: u8, pattern: u8) -> Vec<u8> {
        let value =
            u16::from(foreground) | (u16::from(background) << 5) | (u16::from(pattern) << 10);
        let mut sprm = Vec::from(0x442Du16.to_le_bytes());
        sprm.extend_from_slice(&value.to_le_bytes());
        sprm
    }

    fn paragraph_shd_sprm(
        foreground: Option<Color>,
        background: Option<Color>,
        pattern: u16,
    ) -> Vec<u8> {
        let mut sprm = Vec::from(0xC64Du16.to_le_bytes());
        sprm.push(10);
        for color in [foreground, background] {
            if let Some(color) = color {
                sprm.extend_from_slice(&[color.r, color.g, color.b, 0]);
            } else {
                sprm.extend_from_slice(&[0, 0, 0, 0xFF]);
            }
        }
        sprm.extend_from_slice(&pattern.to_le_bytes());
        sprm
    }

    fn stylesheet(base_len: usize, styles: &[Option<Vec<u8>>]) -> Vec<u8> {
        let mut stsh = vec![0u8; 20];
        stsh[0..2].copy_from_slice(&18u16.to_le_bytes());
        stsh[2..4].copy_from_slice(&(styles.len() as u16).to_le_bytes());
        stsh[4..6].copy_from_slice(&(base_len as u16).to_le_bytes());
        stsh[6..8].copy_from_slice(&1u16.to_le_bytes());
        for style in styles {
            match style {
                Some(std) => {
                    stsh.extend_from_slice(&(std.len() as u16).to_le_bytes());
                    stsh.extend_from_slice(std);
                }
                None => stsh.extend_from_slice(&0u16.to_le_bytes()),
            }
        }
        stsh
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_tab_sprm(deletions: &[i16], additions: &[(i16, u8)]) -> Vec<u8> {
        let mut operand = Vec::new();
        operand.push(deletions.len() as u8);
        for &position in deletions {
            operand.extend_from_slice(&position.to_le_bytes());
        }
        operand.push(additions.len() as u8);
        for &(position, _) in additions {
            operand.extend_from_slice(&position.to_le_bytes());
        }
        for &(_, descriptor) in additions {
            operand.push(descriptor);
        }
        let mut grpprl = Vec::from(0xC60Du16.to_le_bytes());
        grpprl.push(operand.len() as u8);
        grpprl.extend_from_slice(&operand);
        grpprl
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn paragraph_style_tabs_resolve_base_chains_and_isolate_invalid_styles() {
        let normal = legacy_tab_sprm(&[], &[(720, 0x09), (1440, 0x12)]);
        let child = legacy_tab_sprm(&[720], &[(1080, 0x19)]);
        let mut invalid = Vec::from(0x2406u16.to_le_bytes());
        invalid.push(1);
        invalid.extend(legacy_tab_sprm(&[], &[(1800, 0x06)]));
        let bytes = stylesheet(
            10,
            &[
                Some(paragraph_style_std_grpprl(
                    10, 0, 0x0FFF, "Normal", &normal, false,
                )),
                Some(paragraph_style_std_grpprl(10, 1, 0, "Child", &child, false)),
                Some(paragraph_style_std_grpprl(
                    10,
                    2,
                    1,
                    "Grandchild",
                    &[],
                    false,
                )),
                Some(paragraph_style_std_grpprl(10, 3, 4, "CycleA", &[], false)),
                Some(paragraph_style_std_grpprl(10, 4, 3, "CycleB", &[], false)),
                Some(paragraph_style_std_grpprl(
                    10, 5, 0, "Invalid", &invalid, false,
                )),
            ],
        );
        let styles = StyleSheet::parse(&bytes, 0, bytes.len());
        let tabs = |istd| {
            styles.paragraph_tab_stops(istd).map(|stops| {
                stops
                    .iter()
                    .copied()
                    .map(LegacyTabStop::to_model)
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            tabs(0).unwrap(),
            vec![
                TabStop {
                    position_pt: 36.0,
                    alignment: TabAlignment::Center,
                    leader: TabLeader::Dot,
                },
                TabStop {
                    position_pt: 72.0,
                    alignment: TabAlignment::Right,
                    leader: TabLeader::Hyphen,
                },
            ]
        );
        let expected_child = vec![
            TabStop {
                position_pt: 54.0,
                alignment: TabAlignment::Center,
                leader: TabLeader::Underscore,
            },
            TabStop {
                position_pt: 72.0,
                alignment: TabAlignment::Right,
                leader: TabLeader::Hyphen,
            },
        ];
        assert_eq!(tabs(1).unwrap(), expected_child);
        assert_eq!(tabs(2).unwrap(), expected_child);
        assert_eq!(tabs(3), None);
        assert_eq!(tabs(4), None);
        assert_eq!(tabs(5), None);
        assert!(styles.paragraph_pagination(5).keep_next);
    }

    fn parsed_pagination(
        base_len: usize,
        styles: &[Option<Vec<u8>>],
        istd: u16,
    ) -> ParagraphPagination {
        let stsh = stylesheet(base_len, styles);
        StyleSheet::parse(&stsh, 0, stsh.len()).paragraph_pagination(istd)
    }

    fn parsed_layout(
        base_len: usize,
        styles: &[Option<Vec<u8>>],
        istd: u16,
    ) -> ParagraphLayoutOverrides {
        let stsh = stylesheet(base_len, styles);
        StyleSheet::parse(&stsh, 0, stsh.len()).paragraph_layout(istd)
    }

    fn parsed_indent(
        base_len: usize,
        styles: &[Option<Vec<u8>>],
        istd: u16,
    ) -> ParagraphIndentOverrides {
        let stsh = stylesheet(base_len, styles);
        StyleSheet::parse(&stsh, 0, stsh.len()).paragraph_indent(istd)
    }

    fn parsed_spacing(
        base_len: usize,
        styles: &[Option<Vec<u8>>],
        istd: u16,
    ) -> ParagraphSpacingOverrides {
        let stsh = stylesheet(base_len, styles);
        StyleSheet::parse(&stsh, 0, stsh.len()).paragraph_spacing(istd)
    }

    fn parsed_shading(
        base_len: usize,
        styles: &[Option<Vec<u8>>],
        istd: u16,
    ) -> Option<ParagraphShading> {
        let stsh = stylesheet(base_len, styles);
        StyleSheet::parse(&stsh, 0, stsh.len()).paragraph_shading(istd)
    }

    #[test]
    fn resolves_paragraph_layout_inheritance_for_both_std_sizes() {
        for base_len in [10, 18] {
            let styles = vec![
                Some(paragraph_style_std(
                    base_len,
                    0,
                    0x0FFF,
                    "Normal",
                    &[(0x2441, 1), (0x2461, 0)],
                    false,
                )),
                Some(paragraph_style_std(
                    base_len,
                    1,
                    0,
                    "Parent",
                    &[(0x2403, 0)],
                    false,
                )),
                Some(paragraph_style_std(
                    base_len,
                    2,
                    1,
                    "Child",
                    &[(0x2441, 0)],
                    false,
                )),
            ];
            assert_eq!(
                parsed_layout(base_len, &styles, 1),
                ParagraphLayoutOverrides {
                    bidi: Some(true),
                    justification: Some(ParagraphJustification::PhysicalLeft),
                }
            );
            assert_eq!(
                parsed_layout(base_len, &styles, 2),
                ParagraphLayoutOverrides {
                    bidi: Some(false),
                    justification: Some(ParagraphJustification::PhysicalLeft),
                }
            );
        }
    }

    #[test]
    fn resolves_paragraph_indent_inheritance_for_both_std_sizes() {
        for base_len in [10, 18] {
            let styles = vec![
                Some(paragraph_style_std_grpprl(
                    base_len,
                    0,
                    0x0FFF,
                    "Normal",
                    &[
                        0x5E, 0x84, 0xD0, 0x02, // logical left = 720
                        0x5D, 0x84, 0xA0, 0x05, // logical right = 1440
                        0x60, 0x84, 0x98, 0xFE, // hanging indent = -360
                    ],
                    false,
                )),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    1,
                    0,
                    "Parent",
                    &[
                        0x5D, 0x84, 0xE8, 0x03, // logical right = 1000
                    ],
                    false,
                )),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    2,
                    1,
                    "Child",
                    &[
                        0x5E, 0x84, 0x30, 0xFD, // logical left = -720
                        0x60, 0x84, 0xF0, 0x00, // first line = 240
                    ],
                    false,
                )),
            ];
            assert_eq!(
                parsed_indent(base_len, &styles, 1),
                ParagraphIndentOverrides {
                    logical_left_twips: Some(720),
                    logical_right_twips: Some(1000),
                    nest_twips: None,
                    first_line_twips: Some(-360),
                }
            );
            assert_eq!(
                parsed_indent(base_len, &styles, 2),
                ParagraphIndentOverrides {
                    logical_left_twips: Some(-720),
                    logical_right_twips: Some(1000),
                    nest_twips: None,
                    first_line_twips: Some(240),
                }
            );
        }
    }

    #[test]
    fn resolves_paragraph_spacing_inheritance_for_both_std_sizes() {
        for base_len in [10, 18] {
            let styles = vec![
                Some(paragraph_style_std_grpprl(
                    base_len,
                    0,
                    0x0FFF,
                    "Normal",
                    &[
                        0x13, 0xA4, 0xF0, 0x00, // before = 240
                        0x14, 0xA4, 0x78, 0x00, // after = 120
                        0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
                    ],
                    false,
                )),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    1,
                    0,
                    "Parent",
                    &[
                        0x14, 0xA4, 0x3C, 0x00, // after = 60
                        0x12, 0x64, 0x40, 0x84, 0x00, 0x00, // exact spacing
                    ],
                    false,
                )),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    2,
                    1,
                    "Child",
                    &[
                        0x13, 0xA4, 0x00, 0x00, // before = 0
                        0x12, 0x64, 0xE0, 0x01, 0x01, 0x00, // 2 lines
                    ],
                    false,
                )),
            ];
            assert_eq!(
                parsed_spacing(base_len, &styles, 1),
                ParagraphSpacingOverrides {
                    before_twips: Some(240),
                    after_twips: Some(60),
                    line: Some(ParagraphLineSpacing::ExactTwips(31_680)),
                }
            );
            assert_eq!(
                parsed_spacing(base_len, &styles, 2),
                ParagraphSpacingOverrides {
                    before_twips: Some(0),
                    after_twips: Some(60),
                    line: Some(ParagraphLineSpacing::ProportionalTwips(480)),
                }
            );
        }
    }

    #[test]
    fn resolves_paragraph_shading_inheritance_for_both_std_sizes() {
        let inherited = Color::rgb(0x18, 0x52, 0x86);
        let recovered = Color::rgb(0x24, 0x68, 0xAC);
        for base_len in [10, 18] {
            let styles = vec![
                Some(paragraph_style_std_grpprl(
                    base_len,
                    0,
                    0x0FFF,
                    "Normal",
                    &paragraph_shd_sprm(None, Some(inherited), 0),
                    false,
                )),
                Some(paragraph_style_std(base_len, 1, 0, "Inherited", &[], false)),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    2,
                    1,
                    "Replacement",
                    &paragraph_shd80_sprm(6, 0, 1),
                    false,
                )),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    3,
                    2,
                    "Suppressed",
                    &paragraph_shd_sprm(None, None, 0),
                    false,
                )),
                Some(paragraph_style_std_grpprl(
                    base_len,
                    4,
                    3,
                    "Recovered",
                    &paragraph_shd_sprm(None, Some(recovered), 0),
                    false,
                )),
            ];

            assert_eq!(
                parsed_shading(base_len, &styles, 1),
                Some(ParagraphShading::Flat(inherited))
            );
            assert_eq!(
                parsed_shading(base_len, &styles, 2),
                Some(ParagraphShading::Flat(Color::rgb(0xFF, 0, 0)))
            );
            assert_eq!(
                parsed_shading(base_len, &styles, 3),
                Some(ParagraphShading::Unrepresentable)
            );
            assert_eq!(
                parsed_shading(base_len, &styles, 4),
                Some(ParagraphShading::Flat(recovered))
            );
        }
    }

    #[test]
    fn paragraph_style_layout_is_source_ordered_and_strict() {
        let styles = vec![Some(paragraph_style_std(
            10,
            0,
            0x0FFF,
            "Normal",
            &[
                (0x2441, 1),
                (0x2441, 2),
                (0x2441, 0),
                (0x2403, 2),
                (0x2461, 0),
                (0x2461, 10),
            ],
            false,
        ))];
        assert_eq!(
            parsed_layout(10, &styles, 0),
            ParagraphLayoutOverrides {
                bidi: Some(false),
                justification: Some(ParagraphJustification::LogicalStart),
            }
        );
    }

    #[test]
    fn resolves_paragraph_pagination_inheritance_for_both_std_sizes() {
        for base_len in [10, 18] {
            let styles = vec![
                Some(paragraph_style_std(
                    base_len,
                    0,
                    0x0FFF,
                    "Normal",
                    &[(0x2405, 1), (0x2431, 0)],
                    false,
                )),
                Some(paragraph_style_std(
                    base_len,
                    1,
                    0,
                    "Parent",
                    &[(0x2406, 1), (0x2407, 1)],
                    false,
                )),
                Some(paragraph_style_std(
                    base_len,
                    2,
                    1,
                    "Child",
                    &[(0x2405, 0), (0x2407, 0)],
                    false,
                )),
            ];
            assert_eq!(
                parsed_pagination(base_len, &styles, 2),
                ParagraphPagination {
                    keep_next: true,
                    keep_lines: false,
                    page_break_before: false,
                    widow_control: false,
                }
            );
        }
    }

    #[test]
    fn paragraph_style_pagination_is_last_sprm_wins() {
        let styles = vec![Some(paragraph_style_std(
            10,
            0,
            0x0FFF,
            "Normal",
            &[
                (0x2405, 1),
                (0x2405, 0),
                (0x2407, 0),
                (0x2407, 1),
                (0x2431, 0),
                (0x2431, 1),
            ],
            false,
        ))];
        assert_eq!(
            parsed_pagination(10, &styles, 0),
            ParagraphPagination {
                keep_next: false,
                keep_lines: false,
                page_break_before: true,
                widow_control: true,
            }
        );
    }

    #[test]
    fn revision_marked_style_uses_current_paragraph_upx() {
        let styles = vec![Some(paragraph_style_std_grpprl(
            18,
            0,
            0x0FFF,
            "Normal",
            &[
                0x06, 0x24, 0x01, // keep next
                0x31, 0x24, 0x00, // widow control off
                0x41, 0x24, 0x01, // RTL
                0x61, 0x24, 0x02, // logical end
                0x5E, 0x84, 0xD0, 0x02, // logical left = 720
                0x5D, 0x84, 0xA0, 0x05, // logical right = 1440
                0x60, 0x84, 0x98, 0xFE, // hanging indent = -360
                0x2D, 0x44, 0xE0, 0x00, // yellow clear shading
            ],
            true,
        ))];
        assert_eq!(
            parsed_pagination(18, &styles, 0),
            ParagraphPagination {
                keep_next: true,
                keep_lines: false,
                page_break_before: false,
                widow_control: false,
            }
        );
        assert_eq!(
            parsed_layout(18, &styles, 0),
            ParagraphLayoutOverrides {
                bidi: Some(true),
                justification: Some(ParagraphJustification::LogicalEnd),
            }
        );
        assert_eq!(
            parsed_indent(18, &styles, 0),
            ParagraphIndentOverrides {
                logical_left_twips: Some(720),
                logical_right_twips: Some(1440),
                nest_twips: None,
                first_line_twips: Some(-360),
            }
        );
        assert_eq!(
            parsed_shading(18, &styles, 0),
            Some(ParagraphShading::Flat(Color::rgb(0xFF, 0xFF, 0)))
        );
    }

    #[test]
    fn malformed_paragraph_style_upx_falls_back_without_partial_values() {
        let valid = paragraph_style_std_grpprl(
            10,
            0,
            0x0FFF,
            "Normal",
            &[
                0x05, 0x24, 0x01, // keep lines
                0x41, 0x24, 0x01, // RTL
                0x07, 0x24, 0x01, // page break before
                0x13, 0xA4, 0xF0, 0x00, // before = 240
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
                0x2D, 0x44, 0xE0, 0x00, // yellow clear shading
            ],
            false,
        );
        let grlp = 10 + 2 + "Normal".encode_utf16().count() * 2 + 2;

        let mut cases = Vec::new();

        let mut wrong_kind = valid.clone();
        wrong_kind[2..4].copy_from_slice(&(2u16 | (0x0FFFu16 << 4)).to_le_bytes());
        cases.push(wrong_kind);

        let mut wrong_cupx = valid.clone();
        wrong_cupx[4..6].copy_from_slice(&1u16.to_le_bytes());
        cases.push(wrong_cupx);

        let mut wrong_istd = valid.clone();
        wrong_istd[grlp + 2..grlp + 4].copy_from_slice(&1u16.to_le_bytes());
        cases.push(wrong_istd);

        let mut nonzero_padding = valid.clone();
        let cb_upx = u16le(&nonzero_padding, grlp).unwrap() as usize;
        nonzero_padding[grlp + 2 + cb_upx] = 1;
        cases.push(nonzero_padding);

        let mut oversized_upx = valid.clone();
        oversized_upx[grlp..grlp + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        cases.push(oversized_upx);

        let mut truncated_sprm = valid.clone();
        truncated_sprm.remove(grlp + 2 + 4);
        cases.push(truncated_sprm);

        let mut truncated_chpx = valid.clone();
        let chpx_len = truncated_chpx.len();
        truncated_chpx[chpx_len - 2..].copy_from_slice(&1u16.to_le_bytes());
        cases.push(truncated_chpx);

        let mut missing_name_terminator = valid;
        missing_name_terminator[grlp - 2..grlp].copy_from_slice(&1u16.to_le_bytes());
        cases.push(missing_name_terminator);

        for std in cases {
            let stsh = stylesheet(10, &[Some(std)]);
            let parsed = StyleSheet::parse(&stsh, 0, stsh.len());
            assert_eq!(
                parsed.paragraph_pagination(0),
                ParagraphPagination::default()
            );
            assert_eq!(
                parsed.paragraph_layout(0),
                ParagraphLayoutOverrides::default()
            );
            assert_eq!(
                parsed.paragraph_indent(0),
                ParagraphIndentOverrides::default()
            );
            assert_eq!(
                parsed.paragraph_spacing(0),
                ParagraphSpacingOverrides::default()
            );
            assert_eq!(parsed.paragraph_shading(0), None);
        }
    }

    #[test]
    fn invalid_base_chains_do_not_apply_partial_style_values() {
        let properties = &[(0x2407, 1), (0x2441, 1)];
        let out_of_range = vec![Some(paragraph_style_std(
            10, 0, 7, "BadBase", properties, false,
        ))];
        assert_eq!(
            parsed_pagination(10, &out_of_range, 0),
            ParagraphPagination::default()
        );
        assert_eq!(
            parsed_layout(10, &out_of_range, 0),
            ParagraphLayoutOverrides::default()
        );
        let out_of_range_spacing = vec![Some(paragraph_style_std_grpprl(
            10,
            0,
            7,
            "BadBase",
            &[
                0x13, 0xA4, 0xF0, 0x00, // before = 240
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
            ],
            false,
        ))];
        assert_eq!(
            parsed_spacing(10, &out_of_range_spacing, 0),
            ParagraphSpacingOverrides::default()
        );
        let out_of_range_indent = vec![Some(paragraph_style_std_grpprl(
            10,
            0,
            7,
            "BadBase",
            &[0x5E, 0x84, 0xD0, 0x02],
            false,
        ))];
        assert_eq!(
            parsed_indent(10, &out_of_range_indent, 0),
            ParagraphIndentOverrides::default()
        );

        let empty_base = vec![
            None,
            Some(paragraph_style_std(
                10,
                1,
                0,
                "EmptyBase",
                properties,
                false,
            )),
        ];
        assert_eq!(
            parsed_pagination(10, &empty_base, 1),
            ParagraphPagination::default()
        );
        assert_eq!(
            parsed_layout(10, &empty_base, 1),
            ParagraphLayoutOverrides::default()
        );

        let cycle = vec![
            Some(paragraph_style_std(10, 0, 1, "CycleA", properties, false)),
            Some(paragraph_style_std(10, 1, 0, "CycleB", &[], false)),
        ];
        assert_eq!(
            parsed_pagination(10, &cycle, 0),
            ParagraphPagination::default()
        );
        assert_eq!(
            parsed_layout(10, &cycle, 0),
            ParagraphLayoutOverrides::default()
        );
        let spacing_cycle = vec![
            Some(paragraph_style_std_grpprl(
                10,
                0,
                1,
                "CycleA",
                &[
                    0x13, 0xA4, 0xF0, 0x00, // before = 240
                    0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
                ],
                false,
            )),
            Some(paragraph_style_std(10, 1, 0, "CycleB", &[], false)),
        ];
        assert_eq!(
            parsed_spacing(10, &spacing_cycle, 0),
            ParagraphSpacingOverrides::default()
        );
        let indent_cycle = vec![
            Some(paragraph_style_std_grpprl(
                10,
                0,
                1,
                "CycleA",
                &[0x5E, 0x84, 0xD0, 0x02],
                false,
            )),
            Some(paragraph_style_std(10, 1, 0, "CycleB", &[], false)),
        ];
        assert_eq!(
            parsed_indent(10, &indent_cycle, 0),
            ParagraphIndentOverrides::default()
        );
    }

    #[test]
    fn invalid_base_chains_do_not_apply_partial_style_shading() {
        let shading = paragraph_shd80_sprm(0, 7, 0);
        let out_of_range = vec![Some(paragraph_style_std_grpprl(
            10, 0, 7, "BadBase", &shading, false,
        ))];
        assert_eq!(parsed_shading(10, &out_of_range, 0), None);

        let empty_base = vec![
            None,
            Some(paragraph_style_std_grpprl(
                10,
                1,
                0,
                "EmptyBase",
                &shading,
                false,
            )),
        ];
        assert_eq!(parsed_shading(10, &empty_base, 1), None);

        let cycle = vec![
            Some(paragraph_style_std_grpprl(
                10, 0, 1, "CycleA", &shading, false,
            )),
            Some(paragraph_style_std(10, 1, 0, "CycleB", &[], false)),
        ];
        assert_eq!(parsed_shading(10, &cycle, 0), None);
    }

    #[test]
    fn paragraph_property_base_chain_has_a_depth_bound() {
        let mut styles = Vec::new();
        styles.push(Some(paragraph_style_std_grpprl(
            10,
            0,
            0x0FFF,
            "Root",
            &[
                0x07, 0x24, 0x01, // page break before
                0x41, 0x24, 0x01, // RTL
                0x5E, 0x84, 0xD0, 0x02, // logical left = 720
                0x13, 0xA4, 0xF0, 0x00, // before = 240
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
                0x2D, 0x44, 0xE0, 0x00, // yellow clear shading
            ],
            false,
        )));
        for istd in 1..=MAX_STYLE_BASE_DEPTH + 2 {
            styles.push(Some(paragraph_style_std(
                10,
                istd as u16,
                (istd - 1) as u16,
                "Derived",
                &[],
                false,
            )));
        }
        assert_eq!(
            parsed_pagination(10, &styles, (styles.len() - 1) as u16),
            ParagraphPagination::default()
        );
        assert_eq!(
            parsed_layout(10, &styles, (styles.len() - 1) as u16),
            ParagraphLayoutOverrides::default()
        );
        assert_eq!(
            parsed_indent(10, &styles, (styles.len() - 1) as u16),
            ParagraphIndentOverrides::default()
        );
        assert_eq!(
            parsed_spacing(10, &styles, (styles.len() - 1) as u16),
            ParagraphSpacingOverrides::default()
        );
        assert_eq!(
            parsed_shading(10, &styles, MAX_STYLE_BASE_DEPTH as u16),
            Some(ParagraphShading::Flat(Color::rgb(0xFF, 0xFF, 0)))
        );
        assert_eq!(
            parsed_shading(10, &styles, (MAX_STYLE_BASE_DEPTH + 1) as u16),
            None
        );
        assert_eq!(parsed_shading(10, &styles, (styles.len() - 1) as u16), None);
    }

    #[test]
    fn invalid_std_base_size_and_revision_shape_fall_back() {
        let styles = vec![Some(paragraph_style_std_grpprl(
            10,
            0,
            0x0FFF,
            "Normal",
            &[
                0x07, 0x24, 0x01, // page break before
                0x13, 0xA4, 0xF0, 0x00, // before = 240
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
            ],
            false,
        ))];
        let malformed_header = stylesheet(12, &styles);
        let parsed = StyleSheet::parse(&malformed_header, 0, malformed_header.len());
        assert_eq!(
            parsed.paragraph_pagination(0),
            ParagraphPagination::default()
        );
        assert_eq!(
            parsed.paragraph_spacing(0),
            ParagraphSpacingOverrides::default()
        );

        let invalid_revision = paragraph_style_std_grpprl(
            10,
            0,
            0x0FFF,
            "Normal",
            &[
                0x07, 0x24, 0x01, // page break before
                0x13, 0xA4, 0xF0, 0x00, // before = 240
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
            ],
            true,
        );
        let invalid_revision = stylesheet(10, &[Some(invalid_revision)]);
        let parsed = StyleSheet::parse(&invalid_revision, 0, invalid_revision.len());
        assert_eq!(
            parsed.paragraph_pagination(0),
            ParagraphPagination::default()
        );
        assert_eq!(
            parsed.paragraph_spacing(0),
            ParagraphSpacingOverrides::default()
        );
    }

    #[test]
    fn heading_name_matching() {
        assert_eq!(heading_from_name("Heading 1"), Some(1));
        assert_eq!(heading_from_name("heading3"), Some(3));
        assert_eq!(heading_from_name("제목 2"), Some(2));
        assert_eq!(heading_from_name("제목3"), Some(3));
        assert_eq!(heading_from_name("Normal"), None);
        assert_eq!(heading_from_name("본문"), None);
    }

    #[test]
    fn resolve_by_sti_and_base_chain() {
        let descs = vec![
            Some(StyleDescription {
                sti: 0,
                sgc: 1,
                istd_base: 0x0FFF,
                name: "Normal".into(),
            }),
            Some(StyleDescription {
                sti: 1,
                sgc: 1,
                istd_base: 0,
                name: "Heading 1".into(),
            }),
            // custom style (high sti) based on Heading 1 (istd 1) → level 1.
            Some(StyleDescription {
                sti: 0x0FFE,
                sgc: 1,
                istd_base: 1,
                name: "MyHead".into(),
            }),
            // custom style by name only.
            Some(StyleDescription {
                sti: 0x0FFE,
                sgc: 1,
                istd_base: 0x0FFF,
                name: "제목 4".into(),
            }),
        ];
        let n = descs.len();
        let lvl = |i| resolve_level(&descs, i, &mut vec![0u32; n], 1, 0);
        assert_eq!(lvl(0), None);
        assert_eq!(lvl(1), Some(1));
        assert_eq!(lvl(2), Some(1));
        assert_eq!(lvl(3), Some(4));
    }

    #[test]
    fn cycle_guard() {
        // A↔B base cycle must terminate, not stack-overflow.
        let descs = vec![
            Some(StyleDescription {
                sti: 0x0FFE,
                sgc: 1,
                istd_base: 1,
                name: String::new(),
            }),
            Some(StyleDescription {
                sti: 0x0FFE,
                sgc: 1,
                istd_base: 0,
                name: String::new(),
            }),
        ];
        assert_eq!(resolve_level(&descs, 0, &mut [0u32; 2], 1, 0), None);
    }

    #[test]
    fn long_acyclic_base_chain_does_not_stack_overflow() {
        // A crafted .doc can declare a very long acyclic basedOn chain (style i based on
        // i+1). The `visited` guard breaks cycles but not depth, so without the depth cap the
        // recursion would blow the stack. With it, this terminates and yields the bottom
        // style's heading name without panicking.
        let n = 200_000usize;
        let mut descs: Vec<Option<StyleDescription>> = (0..n)
            .map(|i| {
                Some(StyleDescription {
                    sti: 0x0FFE,
                    sgc: 1,
                    istd_base: (i + 1) as u16, // chain upward; last points past the end
                    name: String::new(),
                })
            })
            .collect();
        // Make the deepest style a named heading so a (hypothetical) full walk would resolve.
        descs[n - 1] = Some(StyleDescription {
            sti: 0x0FFE,
            sgc: 1,
            istd_base: 0x0FFF,
            name: "Heading 3".into(),
        });
        // Must not panic / overflow the stack; the depth cap stops the walk early → None.
        let mut visited = vec![0u32; n];
        assert_eq!(resolve_level(&descs, 0, &mut visited, 1, 0), None);
    }

    #[test]
    fn huge_style_count_resolves_in_linear_time() {
        // Crafted STSH: cbStshi=6, cstd=65535, cbStdBase=10, namesWritten=0, then 65535 empty
        // (cbStd=0) style slots — ~128 KiB. Resolving every style's heading must be O(n); the
        // old per-style `visited.fill(false)` was O(n²) (≈4.3e9 writes) = a CPU DoS at open.
        let cstd = 65535u16;
        let mut stsh = Vec::new();
        stsh.extend_from_slice(&6u16.to_le_bytes()); // cbStshi → records start at byte 8
        stsh.extend_from_slice(&cstd.to_le_bytes()); // cstd
        stsh.extend_from_slice(&10u16.to_le_bytes()); // cbStdBase
        stsh.extend_from_slice(&0u16.to_le_bytes()); // flags (namesWritten = 0)
        stsh.resize(stsh.len() + cstd as usize * 2, 0u8); // 65535 × cbStd=0
        let ss = StyleSheet::parse(&stsh, 0, stsh.len());
        // All slots empty ⇒ no headings; the point is it terminates fast (no O(n²) work).
        assert_eq!(ss.heading.len(), cstd as usize);
        assert!(ss.heading.iter().all(|h| h.is_none()));
    }
}
