//! Paragraph-property (PAPX) reading — the minimum needed to reconstruct table
//! structure and source paragraph layout: table membership/termination, list
//! and style references, direct pagination and BiDi/justification controls,
//! bounded flat-color shading and custom-tab changes, and row properties.
//!
//! The `PlcfBtePapx` bin table (FIB `fcPlcfBtePapx`, in the table stream) points
//! to 512-byte **PAPX FKP** pages in the `WordDocument` stream. Each FKP maps FC
//! ranges to a `grpprl` (a list of `sprm` property modifiers); we scan those for
//! `sprmPFInTable` (0x2416) and `sprmPFTtp` (0x2417).
//!
//! Reference: [MS-DOC] 2.8.25 (PlcBtePapx), 2.9.137 (PapxInFkp), 2.6.2 (sprm).

use crate::chpx::pchg_tabs_operand_len;
use crate::model::Color;
#[cfg(any(feature = "docx", feature = "render"))]
use crate::model::{TabAlignment, TabLeader, TabStop, MAX_TAB_STOPS};
use crate::table::TableDef;
use crate::util::{u16le, u32le};

const FKP_SIZE: usize = 512;
/// Upper bound on PAPX bin-table page iterations *and* accumulated entries — far above any
/// real `.doc` (a 1000-page document has well under this many paragraph property runs), but
/// it bounds a crafted bin-table that would otherwise amplify into billions of entries.
const MAX_FKP_ENTRIES: usize = 1 << 20;

// A test-lowerable copy of the cap so the over-cap break can be exercised on a tiny fixture
// instead of a multi-million-entry one. Production always uses `MAX_FKP_ENTRIES`.
#[cfg(test)]
thread_local! {
    static TEST_MAX_FKP: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_FKP_ENTRIES) };
}
#[cfg(test)]
fn set_test_max_fkp(n: usize) {
    TEST_MAX_FKP.with(|c| c.set(n));
}
fn max_fkp_entries() -> usize {
    #[cfg(test)]
    {
        TEST_MAX_FKP.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_FKP_ENTRIES
    }
}
const SPRM_P_ISTD: u16 = 0x4600; // direct istd override (2-byte)
const SPRM_P_ISTD_PERMUTE: u16 = 0xC601; // conditional paragraph-style remap
#[cfg(any(feature = "docx", feature = "render"))]
const SPRM_P_CHG_TABS_PAPX: u16 = 0xC60D;
const SPRM_P_JC_80: u16 = 0x2403; // physical paragraph justification (1-byte)
const SPRM_P_FKEEP: u16 = 0x2405;
const SPRM_P_FKEEP_FOLLOW: u16 = 0x2406;
const SPRM_P_FPAGE_BREAK_BEFORE: u16 = 0x2407;
const SPRM_P_FIN_TABLE: u16 = 0x2416;
const SPRM_P_FTTP: u16 = 0x2417;
const SPRM_P_FWIDOW_CONTROL: u16 = 0x2431;
const SPRM_P_F_BIDI: u16 = 0x2441;
const SPRM_P_JC: u16 = 0x2461; // logical paragraph justification (1-byte)
const SPRM_P_DYA_LINE: u16 = 0x6412; // LSPD (4-byte)
const SPRM_P_DYA_BEFORE: u16 = 0xA413; // unsigned twips
const SPRM_P_DYA_AFTER: u16 = 0xA414; // unsigned twips
const SPRM_P_CHG_TABS: u16 = 0xC615;
const SPRM_P_SHD_80: u16 = 0x442D; // Shd80 (2-byte palette/pattern)
const SPRM_P_SHD: u16 = 0xC64D; // SHDOperand (cb + 10-byte Shd)
const SPRM_P_DXA_RIGHT: u16 = 0x845D; // logical right indent (signed twips)
const SPRM_P_DXA_LEFT: u16 = 0x845E; // logical left indent (signed twips)
const SPRM_P_NEST: u16 = 0x465F; // additive logical left indent (signed twips)
const SPRM_P_DXA_LEFT_1: u16 = 0x8460; // logical first-line offset (signed twips)
const SPRM_P_OUT_LVL: u16 = 0x2640; // outline level 0..8, 9 = body (1-byte)
const SPRM_P_ILVL: u16 = 0x260A;
const SPRM_T_FCANT_SPLIT_90: u16 = 0x3403;
const SPRM_T_TABLE_HEADER: u16 = 0x3404; // row repeats as a header (1-byte)
const SPRM_T_FCANT_SPLIT: u16 = 0x3466;
const SPRM_P_ILFO: u16 = 0x460B;
const SPRM_T_F_BIDI: u16 = 0x560B;
const SPRM_T_F_BIDI_90: u16 = 0x5664;
const SPRM_T_TABLE_BORDERS_80: u16 = 0xD605;
const SPRM_T_DEF_TABLE: u16 = 0xD608;
const SPRM_T_TLP: u16 = 0x740A;
const SPRM_T_TABLE_BORDERS: u16 = 0xD613;
const SPRM_T_BRC_TOP_CV: u16 = 0xD61A;
const SPRM_T_BRC_LEFT_CV: u16 = 0xD61B;
const SPRM_T_BRC_BOTTOM_CV: u16 = 0xD61C;
const SPRM_T_BRC_RIGHT_CV: u16 = 0xD61D;
const SPRM_T_SET_BRC_80: u16 = 0xD620;
const SPRM_T_INSERT: u16 = 0x7621;
const SPRM_T_DELETE: u16 = 0x5622;
const SPRM_T_MERGE: u16 = 0x5624;
const SPRM_T_SPLIT: u16 = 0x5625;
const SPRM_T_VERT_MERGE: u16 = 0xD62B;
const SPRM_T_SET_BRC: u16 = 0xD62F;
const SPRM_T_CELL_SPACING_DEFAULT: u16 = 0xD633;
const SPRM_T_ISTD: u16 = 0x563A;
const SPRM_T_CELL_BRC_TYPE: u16 = 0xD662;
const SPRM_T_WALL: u16 = 0x3668;
const SPRM_T_CNF: u16 = 0xD66A;
const SPRM_T_CELL_BRC_TOP_STYLE: u16 = 0xD47F;
const SPRM_T_CELL_BRC_BOTTOM_STYLE: u16 = 0xD680;
const SPRM_T_CELL_BRC_LEFT_STYLE: u16 = 0xD681;
const SPRM_T_CELL_BRC_RIGHT_STYLE: u16 = 0xD682;
const SPRM_T_CELL_BRC_INSIDE_H_STYLE: u16 = 0xD683;
const SPRM_T_CELL_BRC_INSIDE_V_STYLE: u16 = 0xD684;
const SPRM_T_CELL_BRC_TL2BR_STYLE: u16 = 0xD685;
const SPRM_T_CELL_BRC_TR2BL_STYLE: u16 = 0xD686;

/// Direct paragraph pagination resolved against the MS-DOC format defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParagraphPagination {
    pub(crate) keep_next: bool,
    pub(crate) keep_lines: bool,
    pub(crate) page_break_before: bool,
    pub(crate) widow_control: bool,
}

impl Default for ParagraphPagination {
    fn default() -> Self {
        Self {
            keep_next: false,
            keep_lines: false,
            page_break_before: false,
            widow_control: true,
        }
    }
}

/// Sparse paragraph pagination modifiers. `None` means the source did not
/// specify that property; this distinction is required when direct PAPX
/// formatting overrides an inherited paragraph style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ParagraphPaginationOverrides {
    pub(crate) keep_next: Option<bool>,
    pub(crate) keep_lines: Option<bool>,
    pub(crate) page_break_before: Option<bool>,
    pub(crate) widow_control: Option<bool>,
}

impl ParagraphPagination {
    pub(crate) fn apply(self, overrides: ParagraphPaginationOverrides) -> Self {
        Self {
            keep_next: overrides.keep_next.unwrap_or(self.keep_next),
            keep_lines: overrides.keep_lines.unwrap_or(self.keep_lines),
            page_break_before: overrides
                .page_break_before
                .unwrap_or(self.page_break_before),
            widow_control: overrides.widow_control.unwrap_or(self.widow_control),
        }
    }
}

impl From<ParagraphPagination> for ParagraphPaginationOverrides {
    fn from(value: ParagraphPagination) -> Self {
        Self {
            keep_next: Some(value.keep_next),
            keep_lines: Some(value.keep_lines),
            page_break_before: Some(value.page_break_before),
            widow_control: Some(value.widow_control),
        }
    }
}

/// Direction-independent representation of the bounded legacy justification
/// values that the shared physical-alignment model can preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParagraphJustification {
    PhysicalLeft,
    Center,
    PhysicalRight,
    Justify,
    LogicalStart,
    LogicalEnd,
    UnsupportedIndented,
}

/// Sparse direct paragraph layout modifiers. Logical justification remains
/// unresolved until assembly knows the final paragraph direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ParagraphLayoutOverrides {
    pub(crate) bidi: Option<bool>,
    pub(crate) justification: Option<ParagraphJustification>,
}

impl ParagraphLayoutOverrides {
    pub(crate) fn apply(self, overrides: Self) -> Self {
        Self {
            bidi: overrides.bidi.or(self.bidi),
            justification: overrides.justification.or(self.justification),
        }
    }
}

/// Sparse modern logical twip indents from direct paragraph formatting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ParagraphIndentOverrides {
    pub(crate) logical_left_twips: Option<i16>,
    pub(crate) logical_right_twips: Option<i16>,
    pub(crate) nest_twips: Option<i16>,
    pub(crate) first_line_twips: Option<i16>,
}

impl ParagraphIndentOverrides {
    pub(crate) fn apply(self, overrides: Self) -> Self {
        Self {
            logical_left_twips: overrides.logical_left_twips.or(self.logical_left_twips),
            logical_right_twips: overrides.logical_right_twips.or(self.logical_right_twips),
            nest_twips: overrides.nest_twips.or(self.nest_twips),
            first_line_twips: overrides.first_line_twips.or(self.first_line_twips),
        }
    }
}

/// The bounded legacy line-spacing forms retained by the parser.
///
/// Exact, minimum, and zero-multiplier LSPD values cannot be expressed by
/// `Spacing::line_pct`, but their distinct source semantics are retained for
/// the renderer-only sidecar. The sentinel still has to override an inherited
/// multiplier instead of silently retaining it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParagraphLineSpacing {
    ProportionalTwips(u16),
    ExactTwips(u16),
    AtLeastTwips(u16),
    Unrepresentable,
}

/// Sparse legacy paragraph-spacing modifiers in source units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ParagraphSpacingOverrides {
    pub(crate) before_twips: Option<u16>,
    pub(crate) after_twips: Option<u16>,
    pub(crate) line: Option<ParagraphLineSpacing>,
}

impl ParagraphSpacingOverrides {
    pub(crate) fn apply(self, overrides: Self) -> Self {
        Self {
            before_twips: overrides.before_twips.or(self.before_twips),
            after_twips: overrides.after_twips.or(self.after_twips),
            line: overrides.line.or(self.line),
        }
    }
}

/// A paragraph-shading result representable by the shared model.
///
/// `Unrepresentable` is an override, not absence: a later patterned, automatic,
/// nil, invalid, or malformed value must suppress an earlier flat color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParagraphShading {
    Flat(Color),
    Unrepresentable,
}

impl ParagraphShading {
    pub(crate) fn flat_color(self) -> Option<Color> {
        match self {
            Self::Flat(color) => Some(color),
            Self::Unrepresentable => None,
        }
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegacyTabStop {
    position_twips: i16,
    alignment: TabAlignment,
    leader: TabLeader,
}

#[cfg(any(feature = "docx", feature = "render"))]
impl LegacyTabStop {
    pub(crate) fn to_model(self) -> TabStop {
        TabStop {
            position_pt: f32::from(self.position_twips) / 20.0,
            alignment: self.alignment,
            leader: self.leader,
        }
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum LegacyTabDeletion {
    Nearby(i16),
    Range {
        center_twips: i16,
        radius_twips: i32,
    },
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyTabOperation {
    deletions: Vec<LegacyTabDeletion>,
    additions: Vec<LegacyTabStop>,
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParagraphTabStopChanges {
    valid: bool,
    operations: Vec<LegacyTabOperation>,
}

#[cfg(any(feature = "docx", feature = "render"))]
impl Default for ParagraphTabStopChanges {
    fn default() -> Self {
        Self {
            valid: true,
            operations: Vec::new(),
        }
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
impl ParagraphTabStopChanges {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn invalidate(&mut self) {
        self.valid = false;
        self.operations.clear();
    }

    fn push_operand(&mut self, sprm: u16, operand: &[u8]) {
        if !self.valid {
            return;
        }
        let Some(operation) = parse_legacy_tab_operation(sprm, operand) else {
            self.invalidate();
            return;
        };
        self.operations.push(operation);
    }

    pub(crate) fn apply(&self, inherited: &[LegacyTabStop]) -> Option<Vec<LegacyTabStop>> {
        if !self.valid || inherited.len() > MAX_TAB_STOPS {
            return None;
        }
        let mut resolved = inherited.to_vec();
        if resolved.windows(2).any(|pair| {
            pair[0].position_twips >= pair[1].position_twips || pair[0].position_twips < 0
        }) || resolved.last().is_some_and(|stop| stop.position_twips < 0)
        {
            return None;
        }

        for operation in &self.operations {
            for deletion in &operation.deletions {
                resolved.retain(|stop| {
                    let delta = (i32::from(stop.position_twips)
                        - i32::from(match deletion {
                            LegacyTabDeletion::Nearby(center) => *center,
                            LegacyTabDeletion::Range { center_twips, .. } => *center_twips,
                        }))
                    .abs();
                    let radius = match deletion {
                        LegacyTabDeletion::Nearby(_) => 25,
                        LegacyTabDeletion::Range { radius_twips, .. } => *radius_twips,
                    };
                    delta > radius
                });
            }
            for addition in &operation.additions {
                match resolved
                    .binary_search_by_key(&addition.position_twips, |stop| stop.position_twips)
                {
                    Ok(index) => resolved[index] = *addition,
                    Err(index) => resolved.insert(index, *addition),
                }
            }
            if resolved.len() > MAX_TAB_STOPS {
                return None;
            }
        }
        Some(resolved)
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
fn parse_legacy_tab_operation(sprm: u16, operand: &[u8]) -> Option<LegacyTabOperation> {
    let cb = usize::from(*operand.first()?);
    if !(2..=u8::MAX as usize).contains(&cb) {
        return None;
    }
    let mut offset = 1usize;
    let deletion_count = usize::from(*operand.get(offset)?);
    if deletion_count > 64 {
        return None;
    }
    offset = offset.checked_add(1)?;

    let mut deletion_centers = Vec::with_capacity(deletion_count);
    for _ in 0..deletion_count {
        let center = strict_xas(operand.get(offset..offset.checked_add(2)?)?)?;
        if deletion_centers
            .last()
            .is_some_and(|previous| *previous >= center)
        {
            return None;
        }
        deletion_centers.push(center);
        offset = offset.checked_add(2)?;
    }

    let deletions = match sprm {
        SPRM_P_CHG_TABS_PAPX => deletion_centers
            .into_iter()
            .map(LegacyTabDeletion::Nearby)
            .collect(),
        SPRM_P_CHG_TABS => {
            let mut parsed = Vec::with_capacity(deletion_count);
            for center_twips in deletion_centers {
                let stored = i16::from_le_bytes(
                    operand
                        .get(offset..offset.checked_add(2)?)?
                        .try_into()
                        .ok()?,
                );
                let decoded = i32::from(stored).checked_sub(1)?;
                if !(-31_679..=31_681).contains(&decoded) {
                    return None;
                }
                parsed.push(LegacyTabDeletion::Range {
                    center_twips,
                    radius_twips: decoded.max(25),
                });
                offset = offset.checked_add(2)?;
            }
            parsed
        }
        _ => return None,
    };

    let addition_count = usize::from(*operand.get(offset)?);
    if addition_count > 64 {
        return None;
    }
    offset = offset.checked_add(1)?;
    let mut addition_positions = Vec::with_capacity(addition_count);
    for _ in 0..addition_count {
        let position = strict_xas(operand.get(offset..offset.checked_add(2)?)?)?;
        if position < 0
            || addition_positions
                .last()
                .is_some_and(|previous| *previous >= position)
        {
            return None;
        }
        addition_positions.push(position);
        offset = offset.checked_add(2)?;
    }
    let mut additions = Vec::with_capacity(addition_count);
    for position_twips in addition_positions {
        let descriptor = *operand.get(offset)?;
        offset = offset.checked_add(1)?;
        let alignment = match descriptor & 0x07 {
            0 => TabAlignment::Left,
            1 => TabAlignment::Center,
            2 => TabAlignment::Right,
            3 => TabAlignment::Decimal,
            4 => TabAlignment::Bar,
            _ => return None,
        };
        let leader = if alignment == TabAlignment::Bar {
            TabLeader::None
        } else {
            match (descriptor >> 3) & 0x07 {
                0 | 7 => TabLeader::None,
                1 => TabLeader::Dot,
                2 => TabLeader::Hyphen,
                3 => TabLeader::Underscore,
                4 => TabLeader::Heavy,
                5 => TabLeader::MiddleDot,
                _ => return None,
            }
        };
        additions.push(LegacyTabStop {
            position_twips,
            alignment,
            leader,
        });
    }

    let expected_remainder = offset.checked_sub(1)?;
    if offset != operand.len()
        || (sprm != SPRM_P_CHG_TABS || cb != u8::MAX as usize) && cb != expected_remainder
    {
        return None;
    }
    Some(LegacyTabOperation {
        deletions,
        additions,
    })
}

/// Sparse paragraph properties carried by one paragraph-style PAPX.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ParagraphStyleOverrides {
    pub(crate) layout: ParagraphLayoutOverrides,
    pub(crate) indent: ParagraphIndentOverrides,
    pub(crate) pagination: ParagraphPaginationOverrides,
    pub(crate) spacing: ParagraphSpacingOverrides,
    pub(crate) shading: Option<ParagraphShading>,
}

/// Per-paragraph properties over an FC range `[fc_start, fc_lim)`.
#[derive(Debug, Clone, Default)]
struct PapEntry {
    fc_lim: u32,
    in_table: bool,
    ttp: bool,
    /// `ilfo` — list-format-override index (1-based). 0 = not a list paragraph.
    ilfo: u16,
    /// `ilvl` — list level (0-based).
    ilvl: u8,
    /// `istd` — paragraph style index (into the STSH), for heading resolution.
    istd: u16,
    /// `sprmPOutLvl` operand (0..8 = outline levels 1..9, 9 = body), if present.
    outlvl: Option<u8>,
    /// Sparse direct paragraph direction and justification modifiers.
    layout: ParagraphLayoutOverrides,
    /// Sparse direct modern logical paragraph indents.
    indent: ParagraphIndentOverrides,
    /// Sparse direct legacy paragraph spacing.
    spacing: ParagraphSpacingOverrides,
    /// Sparse direct paragraph shading.
    shading: Option<ParagraphShading>,
    /// Sparse direct paragraph pagination modifiers.
    pagination: ParagraphPaginationOverrides,
    /// Ordered direct custom-tab changes, unavailable when the local PAPX is malformed.
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stop_changes: ParagraphTabStopChanges,
    /// Row repeats as a table header (`sprmTTableHeader`).
    table_header: bool,
    /// Resolved `sprmTFCantSplit` / `sprmTFCantSplit90` row property.
    table_cant_split: bool,
    /// Resolved direct table direction from `sprmTFBiDi` / `sprmTFBiDi90`.
    table_bidi_visual: bool,
    /// Parsed `sprmTDefTable` row definition — present only on TTP paragraphs.
    table_def: Option<TableDef>,
}

/// Per-paragraph properties scanned out of one grpprl.
#[derive(Debug, Clone, Default)]
struct Pap {
    in_table: bool,
    ttp: bool,
    ilfo: u16,
    ilvl: u8,
    istd: u16,
    outlvl: Option<u8>,
    layout: ParagraphLayoutOverrides,
    indent: ParagraphIndentOverrides,
    spacing: ParagraphSpacingOverrides,
    shading: Option<ParagraphShading>,
    pagination: ParagraphPaginationOverrides,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stop_changes: ParagraphTabStopChanges,
    table_header: bool,
    table_cant_split_90: Option<bool>,
    table_cant_split: Option<bool>,
    table_bidi: Option<bool>,
    table_bidi_90: Option<bool>,
}

impl Pap {
    fn resolved_cant_split(&self) -> bool {
        self.table_cant_split
            .or(self.table_cant_split_90)
            .unwrap_or(false)
    }

    fn resolved_table_bidi_visual(&self) -> bool {
        self.table_bidi.unwrap_or(false) || self.table_bidi_90.unwrap_or(false)
    }
}

/// All paragraphs' properties, sorted by FC, for point lookup by a mark's FC.
#[derive(Debug, Default)]
pub(crate) struct PapxTable {
    entries: Vec<PapEntry>,
}

impl PapxTable {
    /// The paragraph whose mark sits at byte offset `fc` (the first entry whose
    /// `fc_lim > fc`, since entries are sorted by `fc_lim`).
    fn entry_at(&self, fc: u32) -> Option<&PapEntry> {
        let i = self.entries.partition_point(|e| e.fc_lim <= fc);
        self.entries.get(i)
    }

    /// Table state of the paragraph at `fc`: `(in_table, is_row_end)`.
    pub(crate) fn at(&self, fc: u32) -> (bool, bool) {
        self.entry_at(fc)
            .map(|e| (e.in_table, e.ttp))
            .unwrap_or((false, false))
    }

    /// List membership of the paragraph at `fc`: `(ilfo, ilvl)` — `ilfo` 0 means
    /// not a list paragraph.
    pub(crate) fn list_at(&self, fc: u32) -> (u16, u8) {
        self.entry_at(fc)
            .map(|e| (e.ilfo, e.ilvl))
            .unwrap_or((0, 0))
    }

    /// Style identity and outline level of the paragraph at `fc`.
    pub(crate) fn style_at(&self, fc: u32) -> (u16, Option<u8>) {
        self.entry_at(fc)
            .map(|e| (e.istd, e.outlvl))
            .unwrap_or((0, None))
    }

    /// Sparse direct paragraph direction and justification modifiers at `fc`.
    pub(crate) fn paragraph_layout_overrides_at(&self, fc: u32) -> ParagraphLayoutOverrides {
        self.entry_at(fc).map(|e| e.layout).unwrap_or_default()
    }

    /// Sparse direct modern logical paragraph indents at `fc`.
    pub(crate) fn paragraph_indent_overrides_at(&self, fc: u32) -> ParagraphIndentOverrides {
        self.entry_at(fc).map(|e| e.indent).unwrap_or_default()
    }

    /// Sparse direct legacy paragraph spacing at `fc`.
    pub(crate) fn paragraph_spacing_overrides_at(&self, fc: u32) -> ParagraphSpacingOverrides {
        self.entry_at(fc).map(|e| e.spacing).unwrap_or_default()
    }

    /// Sparse direct paragraph-shading state at `fc`.
    pub(crate) fn paragraph_shading_at(&self, fc: u32) -> Option<ParagraphShading> {
        self.entry_at(fc).and_then(|entry| entry.shading)
    }

    /// Direct paragraph pagination controls at `fc`, with MS-DOC defaults.
    #[cfg(test)]
    pub(crate) fn paragraph_pagination_at(&self, fc: u32) -> ParagraphPagination {
        ParagraphPagination::default().apply(self.paragraph_pagination_overrides_at(fc))
    }

    /// Sparse direct paragraph pagination controls at `fc`.
    pub(crate) fn paragraph_pagination_overrides_at(
        &self,
        fc: u32,
    ) -> ParagraphPaginationOverrides {
        self.entry_at(fc).map(|e| e.pagination).unwrap_or_default()
    }

    /// Resolve direct custom-tab operations against the paragraph style's tab set.
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) fn resolve_paragraph_tab_stops_at(
        &self,
        fc: u32,
        inherited: &[LegacyTabStop],
    ) -> Option<Vec<LegacyTabStop>> {
        self.entry_at(fc)
            .map(|entry| entry.tab_stop_changes.apply(inherited))
            .unwrap_or_else(|| Some(inherited.to_vec()))
    }

    /// The `sprmTDefTable` row definition for the row ending at `fc` (TTP), if any.
    pub(crate) fn table_def_at(&self, fc: u32) -> Option<&TableDef> {
        self.entry_at(fc).and_then(|e| e.table_def.as_ref())
    }

    /// Whether the row ending at `fc` repeats as a table header.
    pub(crate) fn table_header_at(&self, fc: u32) -> bool {
        self.entry_at(fc).map(|e| e.table_header).unwrap_or(false)
    }

    /// Whether the row ending at `fc` must not split across a page boundary.
    pub(crate) fn table_cant_split_at(&self, fc: u32) -> bool {
        self.entry_at(fc)
            .map(|e| e.table_cant_split)
            .unwrap_or(false)
    }

    /// Whether the row ending at `fc` uses visual right-to-left table ordering.
    pub(crate) fn table_bidi_visual_at(&self, fc: u32) -> bool {
        self.entry_at(fc)
            .map(|e| e.table_bidi_visual)
            .unwrap_or(false)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn from_test_entries(entries: &[(u32, bool, bool, bool)]) -> Self {
        Self {
            entries: entries
                .iter()
                .map(|&(fc_lim, in_table, ttp, table_cant_split)| PapEntry {
                    fc_lim,
                    in_table,
                    ttp,
                    table_cant_split,
                    ..PapEntry::default()
                })
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_entries_with_pagination(
        entries: &[(u32, bool, bool, bool, ParagraphPagination)],
    ) -> Self {
        Self {
            entries: entries
                .iter()
                .map(
                    |&(fc_lim, in_table, ttp, table_cant_split, pagination)| PapEntry {
                        fc_lim,
                        in_table,
                        ttp,
                        table_cant_split,
                        pagination: pagination.into(),
                        ..PapEntry::default()
                    },
                )
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_entries_with_style_pagination(
        entries: &[(u32, bool, bool, bool, u16, ParagraphPaginationOverrides)],
    ) -> Self {
        Self {
            entries: entries
                .iter()
                .map(
                    |&(fc_lim, in_table, ttp, table_cant_split, istd, pagination)| PapEntry {
                        fc_lim,
                        in_table,
                        ttp,
                        table_cant_split,
                        istd,
                        pagination,
                        ..PapEntry::default()
                    },
                )
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_entries_with_table_bidi(
        entries: &[(u32, bool, bool, bool, bool)],
    ) -> Self {
        Self {
            entries: entries
                .iter()
                .map(
                    |&(fc_lim, in_table, ttp, table_cant_split, table_bidi_visual)| PapEntry {
                        fc_lim,
                        in_table,
                        ttp,
                        table_cant_split,
                        table_bidi_visual,
                        ..PapEntry::default()
                    },
                )
                .collect(),
        }
    }

    #[cfg(all(test, any(feature = "docx", feature = "render")))]
    pub(crate) fn from_test_entries_with_tab_grpprls(entries: &[(u32, &[u8])]) -> Self {
        Self {
            entries: entries
                .iter()
                .map(|&(fc_lim, grpprl)| PapEntry {
                    fc_lim,
                    tab_stop_changes: scan_paragraph_style_tab_changes(grpprl).unwrap_or_else(
                        || {
                            let mut changes = ParagraphTabStopChanges::default();
                            changes.invalidate();
                            changes
                        },
                    ),
                    ..PapEntry::default()
                })
                .collect(),
        }
    }
}

/// Parse the PAPX bin table + FKP pages into a [`PapxTable`]. Returns an empty
/// table (not an error) if the structures are absent or malformed — table
/// reconstruction then simply degrades to the plain-paragraph rendering.
pub(crate) fn parse(word: &[u8], table: &[u8], fc_plcf: usize, lcb_plcf: usize) -> PapxTable {
    let mut entries = Vec::new();
    if lcb_plcf < 4 {
        return PapxTable { entries };
    }
    let Some(plc) = table.get(fc_plcf..fc_plcf.saturating_add(lcb_plcf)) else {
        return PapxTable { entries };
    };
    // PlcBtePapx: (n+1) FCs then n PnFkpPapx (4 bytes each). n = (lcb-4)/8.
    // Both the page count and the accumulated entries are bounded: a crafted .doc can make
    // `n` huge and point every page number at the same valid FKP page, amplifying a small
    // table into billions of entries (a memory/CPU DoS). Cap iterations and break once the
    // cumulative entry budget is hit (mirrors the OPC/xmltree/ffn caps elsewhere).
    let cap = max_fkp_entries();
    let n = ((plc.len().saturating_sub(4)) / 8).min(cap);
    let pn_base = 4 * (n + 1);
    for i in 0..n {
        if entries.len() >= cap {
            break;
        }
        let Some(pn_raw) = u32le(plc, pn_base + i * 4) else {
            break;
        };
        let page = (pn_raw & 0x003F_FFFF) as usize; // low 22 bits = page number
        let off = page.saturating_mul(FKP_SIZE);
        parse_fkp(word, off, &mut entries);
    }
    entries.sort_by_key(|e| e.fc_lim);
    PapxTable { entries }
}

/// Parse one 512-byte PAPX FKP at `page_off`, appending its paragraphs.
fn parse_fkp(word: &[u8], page_off: usize, out: &mut Vec<PapEntry>) {
    let Some(page) = word.get(page_off..page_off + FKP_SIZE) else {
        return;
    };
    let crun = page[FKP_SIZE - 1] as usize;
    if crun == 0 || 4 * (crun + 1) + 13 * crun >= FKP_SIZE {
        return;
    }
    for i in 0..crun {
        let fc_lim = match u32le(page, 4 * (i + 1)) {
            Some(v) => v,
            None => break,
        };
        // BxPap[i]: bOffset(1) + PHE(12); papx at bOffset*2 within the page.
        let bx_off = 4 * (crun + 1) + i * 13;
        let b_offset = page.get(bx_off).copied().unwrap_or(0) as usize;
        let (pap, table_def) = if b_offset == 0 {
            (Pap::default(), None)
        } else {
            parse_papx(page, b_offset * 2)
        };
        out.push(PapEntry {
            fc_lim,
            in_table: pap.in_table,
            ttp: pap.ttp,
            ilfo: pap.ilfo,
            ilvl: pap.ilvl,
            istd: pap.istd,
            outlvl: pap.outlvl,
            layout: pap.layout,
            indent: pap.indent,
            spacing: pap.spacing,
            shading: pap.shading,
            pagination: pap.pagination,
            table_header: pap.table_header,
            table_cant_split: pap.resolved_cant_split(),
            table_bidi_visual: pap.resolved_table_bidi_visual(),
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stop_changes: pap.tab_stop_changes,
            table_def,
        });
    }
}

/// Read a `PapxInFkp` at `off` within an FKP and scan its grpprl, returning the
/// scalar properties plus any `sprmTDefTable` row definition.
fn parse_papx(page: &[u8], off: usize) -> (Pap, Option<TableDef>) {
    let Some(&cb) = page.get(off) else {
        return (Pap::default(), None);
    };
    // GrpprlAndIstd = istd(2) + grpprl. Size depends on whether cb is 0.
    let (data_off, data_len) = if cb != 0 {
        (off + 1, (cb as usize) * 2 - 1)
    } else {
        let cb2 = page.get(off + 1).copied().unwrap_or(0) as usize;
        (off + 2, cb2 * 2)
    };
    if data_len < 2 {
        return (Pap::default(), None);
    }
    // The leading u16 of GrpprlAndIstd is the paragraph style index (istd); the
    // grpprl follows. A grpprl-level sprmPIstd overrides it.
    let istd = u16le(page, data_off).unwrap_or(0);
    match page.get(data_off + 2..data_off + data_len) {
        Some(gp) => scan_grpprl(gp, istd),
        None => (
            Pap {
                istd,
                ..Pap::default()
            },
            None,
        ),
    }
}

fn blocks_coherent_table_border_projection(sprm: u16) -> bool {
    matches!(
        sprm,
        SPRM_T_TLP
            | SPRM_T_BRC_TOP_CV
            | SPRM_T_BRC_LEFT_CV
            | SPRM_T_BRC_BOTTOM_CV
            | SPRM_T_BRC_RIGHT_CV
            | SPRM_T_SET_BRC_80
            | SPRM_T_INSERT
            | SPRM_T_DELETE
            | SPRM_T_MERGE
            | SPRM_T_SPLIT
            | SPRM_T_VERT_MERGE
            | SPRM_T_SET_BRC
            | SPRM_T_CELL_SPACING_DEFAULT
            | SPRM_T_ISTD
            | SPRM_T_CELL_BRC_TYPE
            | SPRM_T_WALL
            | SPRM_T_CNF
            | SPRM_T_CELL_BRC_TOP_STYLE
            | SPRM_T_CELL_BRC_BOTTOM_STYLE
            | SPRM_T_CELL_BRC_LEFT_STYLE
            | SPRM_T_CELL_BRC_RIGHT_STYLE
            | SPRM_T_CELL_BRC_INSIDE_H_STYLE
            | SPRM_T_CELL_BRC_INSIDE_V_STYLE
            | SPRM_T_CELL_BRC_TL2BR_STYLE
            | SPRM_T_CELL_BRC_TR2BL_STYLE
    )
}

fn touches_coherent_table_border_projection(sprm: u16) -> bool {
    matches!(
        sprm,
        SPRM_T_DEF_TABLE | SPRM_T_TABLE_BORDERS_80 | SPRM_T_TABLE_BORDERS
    ) || blocks_coherent_table_border_projection(sprm)
}

fn block_table_border_projection(
    table_def: &mut Option<TableDef>,
    blocked_without_definition: &mut bool,
) {
    *blocked_without_definition = true;
    if let Some(definition) = table_def.as_mut() {
        definition.block_border_projection();
    }
}

/// Walk a grpprl, extracting table flags, list (`ilfo`/`ilvl`), style index,
/// outline level, layout, indent, spacing, shading, and direct pagination.
/// Stops on an unsizeable or truncated sprm.
fn scan_grpprl(gp: &[u8], istd: u16) -> (Pap, Option<TableDef>) {
    let mut pap = Pap {
        istd,
        ..Pap::default()
    };
    let mut table_def = None;
    let mut border_projection_blocked = false;
    let mut pos = 0;
    while pos + 2 <= gp.len() {
        let Some(sprm) = u16le(gp, pos) else { break };
        let op = pos + 2;
        let Some(len) = operand_len(sprm, gp, op) else {
            #[cfg(any(feature = "docx", feature = "render"))]
            pap.tab_stop_changes.invalidate();
            if is_paragraph_shading_sprm(sprm) {
                pap.shading = Some(ParagraphShading::Unrepresentable);
            }
            if touches_coherent_table_border_projection(sprm) {
                block_table_border_projection(&mut table_def, &mut border_projection_blocked);
            }
            break;
        };
        let Some(operand_end) = op.checked_add(len) else {
            #[cfg(any(feature = "docx", feature = "render"))]
            pap.tab_stop_changes.invalidate();
            if is_paragraph_shading_sprm(sprm) {
                pap.shading = Some(ParagraphShading::Unrepresentable);
            }
            if touches_coherent_table_border_projection(sprm) {
                block_table_border_projection(&mut table_def, &mut border_projection_blocked);
            }
            break;
        };
        if gp.get(op..operand_end).is_none() {
            #[cfg(any(feature = "docx", feature = "render"))]
            pap.tab_stop_changes.invalidate();
            if is_paragraph_shading_sprm(sprm) {
                pap.shading = Some(ParagraphShading::Unrepresentable);
            }
            if touches_coherent_table_border_projection(sprm) {
                block_table_border_projection(&mut table_def, &mut border_projection_blocked);
            }
            break;
        }
        if apply_pagination_sprm(&mut pap.pagination, sprm, &gp[op..operand_end]) {
            pos = operand_end;
            continue;
        }
        if apply_layout_sprm(&mut pap.layout, sprm, &gp[op..operand_end]) {
            pos = operand_end;
            continue;
        }
        if apply_indent_sprm(&mut pap.indent, sprm, &gp[op..operand_end]) {
            pos = operand_end;
            continue;
        }
        if apply_spacing_sprm(&mut pap.spacing, sprm, &gp[op..operand_end]) {
            pos = operand_end;
            continue;
        }
        if apply_shading_sprm(&mut pap.shading, sprm, &gp[op..operand_end]) {
            pos = operand_end;
            continue;
        }
        match sprm {
            SPRM_P_ISTD => {
                if let Some(new_istd) = u16le(gp, op) {
                    apply_paragraph_style_to_modeled_properties(&mut pap, new_istd);
                }
            }
            SPRM_P_ISTD_PERMUTE => {
                if let Some(new_istd) = permuted_istd(&gp[op..operand_end], pap.istd) {
                    apply_paragraph_style_to_modeled_properties(&mut pap, new_istd);
                }
            }
            #[cfg(any(feature = "docx", feature = "render"))]
            SPRM_P_CHG_TABS_PAPX | SPRM_P_CHG_TABS => pap
                .tab_stop_changes
                .push_operand(sprm, &gp[op..operand_end]),
            SPRM_P_FIN_TABLE => pap.in_table = gp.get(op).copied().unwrap_or(0) != 0,
            SPRM_P_FTTP => pap.ttp = gp.get(op).copied().unwrap_or(0) != 0,
            SPRM_P_OUT_LVL => pap.outlvl = Some(gp.get(op).copied().unwrap_or(9)),
            SPRM_T_FCANT_SPLIT_90 => {
                pap.table_cant_split_90 = Some(gp.get(op).copied().unwrap_or(0) != 0);
            }
            SPRM_T_TABLE_HEADER => pap.table_header = gp.get(op).copied().unwrap_or(0) != 0,
            SPRM_T_FCANT_SPLIT => {
                pap.table_cant_split = Some(gp.get(op).copied().unwrap_or(0) != 0);
            }
            SPRM_T_F_BIDI => {
                if let Some(value) = strict_bool16(&gp[op..operand_end]) {
                    pap.table_bidi = Some(value);
                }
            }
            SPRM_T_F_BIDI_90 => {
                if let Some(value) = strict_bool16(&gp[op..operand_end]) {
                    pap.table_bidi_90 = Some(value);
                }
            }
            SPRM_P_ILVL => pap.ilvl = gp.get(op).copied().unwrap_or(0),
            SPRM_P_ILFO => pap.ilfo = u16le(gp, op).unwrap_or(0),
            SPRM_T_DEF_TABLE => {
                if let Some(operand) = gp.get(op..op + len) {
                    if let Some(mut definition) = TableDef::parse(operand) {
                        if border_projection_blocked {
                            definition.block_border_projection();
                        }
                        table_def = Some(definition);
                    } else {
                        block_table_border_projection(
                            &mut table_def,
                            &mut border_projection_blocked,
                        );
                    }
                }
            }
            SPRM_T_TABLE_BORDERS_80 => {
                let operand = &gp[op..operand_end];
                let valid = TableDef::table_borders80_operand_is_valid(operand);
                let applied = table_def
                    .as_mut()
                    .is_some_and(|definition| definition.apply_table_borders80(operand));
                if !valid || !applied {
                    block_table_border_projection(&mut table_def, &mut border_projection_blocked);
                }
            }
            SPRM_T_TABLE_BORDERS => {
                let operand = &gp[op..operand_end];
                let valid = TableDef::table_borders_operand_is_valid(operand);
                let applied = table_def
                    .as_mut()
                    .is_some_and(|definition| definition.apply_table_borders(operand));
                if !valid || !applied {
                    block_table_border_projection(&mut table_def, &mut border_projection_blocked);
                }
            }
            _ if blocks_coherent_table_border_projection(sprm) => {
                block_table_border_projection(&mut table_def, &mut border_projection_blocked);
            }
            _ => {}
        }
        pos = operand_end;
    }
    #[cfg(any(feature = "docx", feature = "render"))]
    if pos != gp.len() {
        pap.tab_stop_changes.invalidate();
    }
    (pap, table_def)
}

fn apply_paragraph_style_to_modeled_properties(pap: &mut Pap, istd: u16) {
    pap.istd = istd;
    pap.ilfo = 0;
    pap.ilvl = 0;
    pap.outlvl = (1..=9).contains(&istd).then_some((istd - 1) as u8);
    pap.layout = ParagraphLayoutOverrides::default();
    pap.indent = ParagraphIndentOverrides::default();
    pap.spacing = ParagraphSpacingOverrides::default();
    pap.shading = None;
    pap.pagination = ParagraphPaginationOverrides::default();
    #[cfg(any(feature = "docx", feature = "render"))]
    pap.tab_stop_changes.clear();
    // Table membership and row properties are intentionally preserved by a
    // paragraph-style change. Style-derived layout, indent, spacing, shading,
    // and pagination are resolved during assembly; style-derived list values
    // remain outside this scanner.
}

fn permuted_istd(operand: &[u8], current_istd: u16) -> Option<u16> {
    let declared_len = usize::from(*operand.first()?);
    if operand.len() != declared_len.checked_add(1)? {
        return None;
    }

    // SPPOperand = cb, fLong, istdFirst, istdLast, rgIstdPermute.
    let first = u16le(operand, 2)?;
    let last = u16le(operand, 4)?;
    let count = usize::from(last.checked_sub(first)?).checked_add(1)?;
    let expected_len = 6usize.checked_add(count.checked_mul(2)?)?;
    if operand.len() != expected_len || !(first..=last).contains(&current_istd) {
        return None;
    }

    let index = usize::from(current_istd - first);
    u16le(operand, 6 + index * 2)
}

fn apply_pagination_sprm(
    pagination: &mut ParagraphPaginationOverrides,
    sprm: u16,
    operand: &[u8],
) -> bool {
    let Some(value) = operand.first().map(|value| *value != 0) else {
        return false;
    };
    match sprm {
        SPRM_P_FKEEP => pagination.keep_lines = Some(value),
        SPRM_P_FKEEP_FOLLOW => pagination.keep_next = Some(value),
        SPRM_P_FPAGE_BREAK_BEFORE => pagination.page_break_before = Some(value),
        SPRM_P_FWIDOW_CONTROL => pagination.widow_control = Some(value),
        _ => return false,
    }
    true
}

fn apply_layout_sprm(layout: &mut ParagraphLayoutOverrides, sprm: u16, operand: &[u8]) -> bool {
    let Some(&value) = operand.first() else {
        return false;
    };
    match sprm {
        SPRM_P_F_BIDI => {
            if value <= 1 {
                layout.bidi = Some(value == 1);
            }
        }
        SPRM_P_JC_80 => {
            if let Some(justification) = physical_justification(value) {
                layout.justification = Some(justification);
            }
        }
        SPRM_P_JC => {
            if let Some(justification) = logical_justification(value) {
                layout.justification = Some(justification);
            }
        }
        _ => return false,
    }
    true
}

fn apply_indent_sprm(indent: &mut ParagraphIndentOverrides, sprm: u16, operand: &[u8]) -> bool {
    let target = match sprm {
        SPRM_P_DXA_RIGHT => &mut indent.logical_right_twips,
        SPRM_P_DXA_LEFT => &mut indent.logical_left_twips,
        SPRM_P_NEST => &mut indent.nest_twips,
        SPRM_P_DXA_LEFT_1 => &mut indent.first_line_twips,
        _ => return false,
    };
    if let Some(value) = strict_xas(operand) {
        *target = Some(value);
    }
    true
}

fn apply_style_indent_sprm(
    indent: &mut ParagraphIndentOverrides,
    sprm: u16,
    operand: &[u8],
) -> bool {
    if sprm == SPRM_P_NEST {
        return false;
    }
    apply_indent_sprm(indent, sprm, operand)
}

fn apply_spacing_sprm(spacing: &mut ParagraphSpacingOverrides, sprm: u16, operand: &[u8]) -> bool {
    match sprm {
        SPRM_P_DYA_BEFORE => {
            if let Some(value) = strict_unsigned_yas(operand) {
                spacing.before_twips = Some(value);
            }
        }
        SPRM_P_DYA_AFTER => {
            if let Some(value) = strict_unsigned_yas(operand) {
                spacing.after_twips = Some(value);
            }
        }
        SPRM_P_DYA_LINE => {
            if let Some(value) = strict_line_spacing(operand) {
                spacing.line = Some(value);
            }
        }
        _ => return false,
    }
    true
}

fn is_paragraph_shading_sprm(sprm: u16) -> bool {
    matches!(sprm, SPRM_P_SHD_80 | SPRM_P_SHD)
}

fn apply_shading_sprm(shading: &mut Option<ParagraphShading>, sprm: u16, operand: &[u8]) -> bool {
    let parsed = match sprm {
        SPRM_P_SHD_80 => parse_shd80(operand),
        SPRM_P_SHD => parse_shd_operand(operand),
        _ => return false,
    };
    *shading = Some(
        parsed
            .map(ParagraphShading::Flat)
            .unwrap_or(ParagraphShading::Unrepresentable),
    );
    true
}

fn parse_shd80(operand: &[u8]) -> Option<Color> {
    let value = u16le(operand, 0)?;
    if operand.len() != 2 || value == u16::MAX {
        return None;
    }
    let foreground = explicit_ico_color((value & 0x1F) as u8)?;
    let background = explicit_ico_color(((value >> 5) & 0x1F) as u8)?;
    let pattern = (value >> 10) & 0x3F;
    flat_shading_color(foreground, background, pattern)
}

fn parse_shd_operand(operand: &[u8]) -> Option<Color> {
    if operand.len() != 11 || operand.first().copied() != Some(10) {
        return None;
    }
    let foreground = explicit_colorref(operand.get(1..5)?)?;
    let background = explicit_colorref(operand.get(5..9)?)?;
    let pattern = u16le(operand, 9)?;
    flat_shading_color(foreground, background, pattern)
}

fn explicit_ico_color(value: u8) -> Option<Option<Color>> {
    match value {
        0 => Some(None),
        1..=16 => crate::chpx::ico_color(value).map(Some),
        _ => None,
    }
}

fn explicit_colorref(bytes: &[u8]) -> Option<Option<Color>> {
    let [red, green, blue, f_auto]: [u8; 4] = bytes.try_into().ok()?;
    match f_auto {
        0 => Some(Some(Color::rgb(red, green, blue))),
        0xFF => Some(None),
        _ => None,
    }
}

fn flat_shading_color(
    foreground: Option<Color>,
    background: Option<Color>,
    pattern: u16,
) -> Option<Color> {
    let valid_pattern = matches!(pattern, 0x0000..=0x0019 | 0x0023..=0x003D);
    if !valid_pattern {
        return None;
    }
    match pattern {
        0 => background,
        1 => foreground,
        _ if foreground.is_some() && foreground == background => foreground,
        _ => None,
    }
}

fn physical_justification(value: u8) -> Option<ParagraphJustification> {
    match value {
        0 => Some(ParagraphJustification::PhysicalLeft),
        1 => Some(ParagraphJustification::Center),
        2 => Some(ParagraphJustification::PhysicalRight),
        3..=5 => Some(ParagraphJustification::Justify),
        _ => None,
    }
}

fn logical_justification(value: u8) -> Option<ParagraphJustification> {
    match value {
        0 => Some(ParagraphJustification::LogicalStart),
        1 => Some(ParagraphJustification::Center),
        2 => Some(ParagraphJustification::LogicalEnd),
        3..=5 | 7..=9 => Some(ParagraphJustification::Justify),
        6 => Some(ParagraphJustification::UnsupportedIndented),
        _ => None,
    }
}

fn strict_bool16(operand: &[u8]) -> Option<bool> {
    match u16le(operand, 0)? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn strict_xas(operand: &[u8]) -> Option<i16> {
    let value = u16le(operand, 0)? as i16;
    (-31_680..=31_680).contains(&value).then_some(value)
}

fn strict_unsigned_yas(operand: &[u8]) -> Option<u16> {
    let value = u16le(operand, 0)?;
    (value <= 0x7BC0).then_some(value)
}

fn strict_line_spacing(operand: &[u8]) -> Option<ParagraphLineSpacing> {
    let dya_line = u16le(operand, 0)?;
    let multiple = u16le(operand, 2)?;
    if multiple > 1 {
        return None;
    }
    match (dya_line, multiple) {
        (0x8440..=u16::MAX, _) => Some(ParagraphLineSpacing::ExactTwips(u16::MAX - dya_line + 1)),
        (1..=0x7BC0, 0) => Some(ParagraphLineSpacing::AtLeastTwips(dya_line)),
        (1..=0x7BC0, 1) => Some(ParagraphLineSpacing::ProportionalTwips(dya_line)),
        (0, _) => Some(ParagraphLineSpacing::Unrepresentable),
        _ => None,
    }
}

/// Strictly scan a style `grpprlPapx` for the bounded paragraph-property
/// subsets modeled by the legacy reader. A malformed modifier invalidates the
/// local style payload instead of applying a partial prefix.
pub(crate) fn scan_paragraph_style_overrides(gp: &[u8]) -> Option<ParagraphStyleOverrides> {
    let mut layout = ParagraphLayoutOverrides::default();
    let mut indent = ParagraphIndentOverrides::default();
    let mut pagination = ParagraphPaginationOverrides::default();
    let mut spacing = ParagraphSpacingOverrides::default();
    let mut shading = None;
    let mut pos = 0;
    while pos < gp.len() {
        let sprm = u16le(gp, pos)?;
        let op = pos.checked_add(2)?;
        let len = operand_len(sprm, gp, op)?;
        let operand_end = op.checked_add(len)?;
        let operand = gp.get(op..operand_end)?;
        apply_layout_sprm(&mut layout, sprm, operand);
        apply_style_indent_sprm(&mut indent, sprm, operand);
        apply_pagination_sprm(&mut pagination, sprm, operand);
        apply_spacing_sprm(&mut spacing, sprm, operand);
        apply_shading_sprm(&mut shading, sprm, operand);
        pos = operand_end;
    }
    Some(ParagraphStyleOverrides {
        layout,
        indent,
        pagination,
        spacing,
        shading,
    })
}

/// Strictly retain ordered custom-tab changes from one paragraph-style PAPX.
#[cfg(any(feature = "docx", feature = "render"))]
pub(crate) fn scan_paragraph_style_tab_changes(gp: &[u8]) -> Option<ParagraphTabStopChanges> {
    let mut changes = ParagraphTabStopChanges::default();
    let mut pos = 0usize;
    while pos < gp.len() {
        let sprm = u16le(gp, pos)?;
        let op = pos.checked_add(2)?;
        let len = operand_len(sprm, gp, op)?;
        let operand_end = op.checked_add(len)?;
        let operand = gp.get(op..operand_end)?;
        if matches!(sprm, SPRM_P_CHG_TABS_PAPX | SPRM_P_CHG_TABS) {
            changes.push_operand(sprm, operand);
            if !changes.valid {
                return None;
            }
        }
        pos = operand_end;
    }
    Some(changes)
}

/// Operand length for a sprm, from its `spra` field ([MS-DOC] 2.2.5).
fn operand_len(sprm: u16, data: &[u8], op: usize) -> Option<usize> {
    match (sprm >> 13) & 0x7 {
        0 | 1 => Some(1),
        2 | 4 | 5 => Some(2),
        3 => Some(4),
        7 => Some(3),
        6 => {
            if sprm == SPRM_T_DEF_TABLE {
                // [MS-DOC] 2.9.349: the leading u16 `cb` is the remainder length
                // PLUS ONE, so total operand = cb-field(2) + (cb-1) = cb + 1.
                let cb = u16le(data, op)? as usize;
                (cb != 0).then_some(1 + cb)
            } else if sprm == SPRM_P_CHG_TABS {
                pchg_tabs_operand_len(data, op)
            } else {
                Some(1 + *data.get(op)? as usize)
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Table, TableBorderSide, TableBorderStyle};
    use crate::table::{self, CellBuild, RowBuild};

    fn table_def_sprm() -> Vec<u8> {
        table_def_sprm_with_tc80([0; 4])
    }

    fn table_def_sprm_with_tc80(border: [u8; 4]) -> Vec<u8> {
        let mut sprm = vec![
            0x08, 0xD6, 0x30, 0x00, // sprmTDefTable, cb=48
            0x02, // two cells
            0x00, 0x00, 0xE8, 0x03, 0xD0, 0x07, // boundaries 0..1000..2000
        ];
        for _ in 0..2 {
            sprm.extend_from_slice(&[0u8; 4]);
            for _ in 0..4 {
                sprm.extend_from_slice(&border);
            }
        }
        sprm
    }

    fn table_borders80_sprm(size: u8, style: u8, ico: u8) -> Vec<u8> {
        let mut sprm = vec![0x05, 0xD6, 0x18];
        for _ in 0..6 {
            sprm.extend_from_slice(&[size, style, ico, 0]);
        }
        sprm
    }

    fn table_borders_sprm(size: u8, style: u8, color: Color) -> Vec<u8> {
        let mut sprm = vec![0x13, 0xD6, 0x30];
        for _ in 0..6 {
            sprm.extend_from_slice(&[color.r, color.g, color.b, 0, size, style, 0, 0]);
        }
        sprm
    }

    fn paragraph_shd80_sprm(foreground: u8, background: u8, pattern: u8) -> Vec<u8> {
        let value =
            u16::from(foreground) | (u16::from(background) << 5) | (u16::from(pattern) << 10);
        let mut sprm = Vec::from(SPRM_P_SHD_80.to_le_bytes());
        sprm.extend_from_slice(&value.to_le_bytes());
        sprm
    }

    fn paragraph_shd_sprm(foreground: [u8; 4], background: [u8; 4], pattern: u16) -> Vec<u8> {
        let mut sprm = Vec::from(SPRM_P_SHD.to_le_bytes());
        sprm.push(10);
        sprm.extend_from_slice(&foreground);
        sprm.extend_from_slice(&background);
        sprm.extend_from_slice(&pattern.to_le_bytes());
        sprm
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_tab_sprm(
        sprm: u16,
        deletions: &[(i16, i16)],
        additions: &[(i16, u8)],
        cb_override: Option<u8>,
    ) -> Vec<u8> {
        let mut operand = Vec::new();
        operand.push(deletions.len() as u8);
        for &(position, _) in deletions {
            operand.extend_from_slice(&position.to_le_bytes());
        }
        if sprm == SPRM_P_CHG_TABS {
            for &(_, close) in deletions {
                operand.extend_from_slice(&close.to_le_bytes());
            }
        }
        operand.push(additions.len() as u8);
        for &(position, _) in additions {
            operand.extend_from_slice(&position.to_le_bytes());
        }
        for &(_, descriptor) in additions {
            operand.push(descriptor);
        }
        let mut grpprl = Vec::from(sprm.to_le_bytes());
        grpprl.push(cb_override.unwrap_or(operand.len() as u8));
        grpprl.extend_from_slice(&operand);
        grpprl
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_tab(
        position_twips: i16,
        alignment: TabAlignment,
        leader: TabLeader,
    ) -> LegacyTabStop {
        LegacyTabStop {
            position_twips,
            alignment,
            leader,
        }
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_tab_descriptors_map_all_supported_alignments_and_leaders() {
        let grpprl = legacy_tab_sprm(
            SPRM_P_CHG_TABS_PAPX,
            &[],
            &[
                (100, 0x00),
                (200, 0x09),
                (300, 0x12),
                (400, 0x1B),
                (500, 0x20),
                (600, 0x28),
                (700, 0x38),
                (800, 0x2C),
            ],
            None,
        );
        let changes = scan_paragraph_style_tab_changes(&grpprl).unwrap();
        assert_eq!(
            changes.apply(&[]).unwrap(),
            vec![
                legacy_tab(100, TabAlignment::Left, TabLeader::None),
                legacy_tab(200, TabAlignment::Center, TabLeader::Dot),
                legacy_tab(300, TabAlignment::Right, TabLeader::Hyphen),
                legacy_tab(400, TabAlignment::Decimal, TabLeader::Underscore),
                legacy_tab(500, TabAlignment::Left, TabLeader::Heavy),
                legacy_tab(600, TabAlignment::Left, TabLeader::MiddleDot),
                legacy_tab(700, TabAlignment::Left, TabLeader::None),
                legacy_tab(800, TabAlignment::Bar, TabLeader::None),
            ]
        );
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_tab_deletions_apply_exact_radii_and_source_order() {
        let inherited = [100, 125, 126, 300, 350, 351, 600, 625, 626]
            .map(|position| legacy_tab(position, TabAlignment::Left, TabLeader::None));
        let mut grpprl = legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[(100, 0)], &[(200, 0x09)], None);
        grpprl.extend(legacy_tab_sprm(
            SPRM_P_CHG_TABS,
            &[(300, 51), (600, 1)],
            &[(400, 0x12)],
            None,
        ));
        let (pap, _) = scan_grpprl(&grpprl, 0);
        assert_eq!(
            pap.tab_stop_changes.apply(&inherited).unwrap(),
            vec![
                legacy_tab(126, TabAlignment::Left, TabLeader::None),
                legacy_tab(200, TabAlignment::Center, TabLeader::Dot),
                legacy_tab(351, TabAlignment::Left, TabLeader::None),
                legacy_tab(400, TabAlignment::Right, TabLeader::Hyphen),
                legacy_tab(626, TabAlignment::Left, TabLeader::None),
            ]
        );

        let sentinel = legacy_tab_sprm(SPRM_P_CHG_TABS, &[], &[(720, 0x1B)], Some(u8::MAX));
        let (pap, _) = scan_grpprl(&sentinel, 0);
        assert_eq!(
            pap.tab_stop_changes.apply(&[]).unwrap(),
            vec![legacy_tab(
                720,
                TabAlignment::Decimal,
                TabLeader::Underscore
            )]
        );
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_tab_style_switch_discards_earlier_direct_changes() {
        let mut grpprl = legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &[(500, 0x00)], None);
        grpprl.extend_from_slice(&SPRM_P_ISTD.to_le_bytes());
        grpprl.extend_from_slice(&1u16.to_le_bytes());
        grpprl.extend(legacy_tab_sprm(
            SPRM_P_CHG_TABS_PAPX,
            &[],
            &[(900, 0x09)],
            None,
        ));
        let (pap, _) = scan_grpprl(&grpprl, 0);
        assert_eq!(pap.istd, 1);
        assert_eq!(
            pap.tab_stop_changes
                .apply(&[legacy_tab(720, TabAlignment::Right, TabLeader::Hyphen)])
                .unwrap(),
            vec![
                legacy_tab(720, TabAlignment::Right, TabLeader::Hyphen),
                legacy_tab(900, TabAlignment::Center, TabLeader::Dot),
            ]
        );
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_tab_malformed_or_unsupported_operands_are_all_or_nothing() {
        let malformed = [
            legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &[(200, 0x00), (100, 0x00)], None),
            legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &[(-1, 0x00)], None),
            legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &[(100, 0x06)], None),
            legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &[(100, 0x30)], None),
            legacy_tab_sprm(SPRM_P_CHG_TABS, &[], &[(100, 0x00)], Some(1)),
            legacy_tab_sprm(SPRM_P_CHG_TABS, &[], &[(100, 0x00)], Some(7)),
            vec![0x0D, 0xC6, 0x02, 65, 0],
        ];
        for grpprl in malformed {
            let (pap, _) = scan_grpprl(&grpprl, 0);
            assert!(pap.tab_stop_changes.apply(&[]).is_none(), "{grpprl:?}");
            assert!(scan_paragraph_style_tab_changes(&grpprl).is_none());
        }

        let mut trailing = legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &[(100, 0x00)], None);
        trailing.push(0xFF);
        let (pap, _) = scan_grpprl(&trailing, 0);
        assert!(pap.tab_stop_changes.apply(&[]).is_none());
        assert!(scan_paragraph_style_tab_changes(&trailing).is_none());

        let mut over_bound = Vec::new();
        for batch in 0..=4 {
            let additions = (0..64)
                .map(|index| ((batch * 1000 + index + 1) as i16, 0x00))
                .collect::<Vec<_>>();
            over_bound.extend(legacy_tab_sprm(SPRM_P_CHG_TABS_PAPX, &[], &additions, None));
        }
        let (pap, _) = scan_grpprl(&over_bound, 0);
        assert!(pap.tab_stop_changes.apply(&[]).is_none());
        assert!(scan_paragraph_style_tab_changes(&over_bound)
            .unwrap()
            .apply(&[])
            .is_none());

        let mut bounded_result = Vec::new();
        let repeated_deletions = (-64..0)
            .map(|position| (position, 0))
            .collect::<Vec<(i16, i16)>>();
        for _ in 0..=4 {
            bounded_result.extend(legacy_tab_sprm(
                SPRM_P_CHG_TABS_PAPX,
                &repeated_deletions,
                &[],
                None,
            ));
        }
        let changes = scan_paragraph_style_tab_changes(&bounded_result).unwrap();
        assert_eq!(changes.apply(&[]), Some(Vec::new()));
    }

    fn table_from_grpprl(grpprl: &[u8]) -> Table {
        let (_, definition) = scan_grpprl(grpprl, 0);
        let definition = definition.expect("test grpprl must retain a table definition");
        table::build(vec![
            RowBuild {
                cells: vec![CellBuild::default(), CellBuild::default()],
                def: Some(definition.clone()),
                header: false,
            },
            RowBuild {
                cells: vec![CellBuild::default(), CellBuild::default()],
                def: Some(definition),
                header: false,
            },
        ])
        .table
    }

    fn assert_no_projected_table_borders(table: &Table) {
        for side in [
            TableBorderSide::Top,
            TableBorderSide::Left,
            TableBorderSide::Bottom,
            TableBorderSide::Right,
            TableBorderSide::InsideHorizontal,
            TableBorderSide::InsideVertical,
        ] {
            assert_eq!(table.border_colors.get(side).or(table.border_color), None);
            assert_eq!(
                table.border_sizes.get(side).or(table.border_size_eighths),
                None
            );
            assert_eq!(table.border_styles.get(side).or(table.border_style), None);
        }
    }

    #[test]
    fn scans_table_flags() {
        // grpprl with sprmPFInTable=1 then sprmPFTtp=1.
        let (p, _) = scan_grpprl(&[0x16, 0x24, 0x01, 0x17, 0x24, 0x01], 0);
        assert!(p.in_table && p.ttp);
        // a 2-byte-operand sprm (spra=2, e.g. 0x4400) then fInTable.
        let (p2, _) = scan_grpprl(&[0x00, 0x44, 0xAA, 0xBB, 0x16, 0x24, 0x01], 0);
        assert!(p2.in_table && !p2.ttp);
    }

    #[test]
    fn resolves_modern_and_compatibility_table_row_no_split_properties() {
        let (default, _) = scan_grpprl(&[], 0);
        let (compatibility, _) = scan_grpprl(&[0x03, 0x34, 0x01], 0);
        let (modern, _) = scan_grpprl(&[0x66, 0x34, 0x01], 0);

        assert!(!default.resolved_cant_split());
        assert!(compatibility.resolved_cant_split());
        assert!(modern.resolved_cant_split());

        // `sprmTFCantSplit` supersedes `sprmTFCantSplit90` regardless of the
        // physical order in the grpprl.
        for grpprl in [
            [0x03, 0x34, 0x01, 0x66, 0x34, 0x00],
            [0x66, 0x34, 0x00, 0x03, 0x34, 0x01],
        ] {
            let (properties, _) = scan_grpprl(&grpprl, 0);
            assert!(!properties.resolved_cant_split());
        }
        for grpprl in [
            [0x03, 0x34, 0x00, 0x66, 0x34, 0x01],
            [0x66, 0x34, 0x01, 0x03, 0x34, 0x00],
        ] {
            let (properties, _) = scan_grpprl(&grpprl, 0);
            assert!(properties.resolved_cant_split());
        }
    }

    #[test]
    fn resolves_direct_table_bidi_properties_strictly() {
        let (default, _) = scan_grpprl(&[], 0);
        let (modern, _) = scan_grpprl(&[0x0B, 0x56, 0x01, 0x00], 0);
        let (compatibility, _) = scan_grpprl(&[0x64, 0x56, 0x01, 0x00], 0);
        assert!(!default.resolved_table_bidi_visual());
        assert!(modern.resolved_table_bidi_visual());
        assert!(compatibility.resolved_table_bidi_visual());

        for grpprl in [
            [0x0B, 0x56, 0x01, 0x00, 0x64, 0x56, 0x00, 0x00],
            [0x64, 0x56, 0x00, 0x00, 0x0B, 0x56, 0x01, 0x00],
            [0x0B, 0x56, 0x00, 0x00, 0x64, 0x56, 0x01, 0x00],
            [0x64, 0x56, 0x01, 0x00, 0x0B, 0x56, 0x00, 0x00],
        ] {
            let (properties, _) = scan_grpprl(&grpprl, 0);
            assert!(properties.resolved_table_bidi_visual());
        }

        let (modern_last_off, _) =
            scan_grpprl(&[0x0B, 0x56, 0x01, 0x00, 0x0B, 0x56, 0x00, 0x00], 0);
        let (compatibility_last_on, _) =
            scan_grpprl(&[0x64, 0x56, 0x00, 0x00, 0x64, 0x56, 0x01, 0x00], 0);
        assert!(!modern_last_off.resolved_table_bidi_visual());
        assert!(compatibility_last_on.resolved_table_bidi_visual());

        let (invalid_after_valid, _) =
            scan_grpprl(&[0x0B, 0x56, 0x01, 0x00, 0x0B, 0x56, 0x02, 0x00], 0);
        let (only_invalid, _) = scan_grpprl(&[0x64, 0x56, 0xFF, 0xFF], 0);
        let (truncated_after_valid, _) =
            scan_grpprl(&[0x0B, 0x56, 0x01, 0x00, 0x64, 0x56, 0x01], 0);
        assert!(invalid_after_valid.resolved_table_bidi_visual());
        assert!(!only_invalid.resolved_table_bidi_visual());
        assert!(truncated_after_valid.resolved_table_bidi_visual());
    }

    #[test]
    fn direct_table_border_formats_apply_in_source_order() {
        let compatibility = table_borders80_sprm(4, 0x03, 2);
        let modern_color = Color::rgb(0x31, 0x72, 0xB4);
        let modern = table_borders_sprm(10, 0x06, modern_color);

        let mut modern_last = table_def_sprm();
        modern_last.extend_from_slice(&compatibility);
        modern_last.extend_from_slice(&modern);
        let modern_table = table_from_grpprl(&modern_last);
        assert_eq!(
            modern_table
                .border_sizes
                .get(TableBorderSide::Top)
                .or(modern_table.border_size_eighths),
            Some(10)
        );
        assert_eq!(
            modern_table
                .border_colors
                .get(TableBorderSide::Top)
                .or(modern_table.border_color),
            Some(modern_color)
        );
        assert_eq!(
            modern_table
                .border_styles
                .get(TableBorderSide::Top)
                .or(modern_table.border_style),
            Some(TableBorderStyle::Dotted)
        );

        let mut compatibility_last = table_def_sprm();
        compatibility_last.extend_from_slice(&modern);
        compatibility_last.extend_from_slice(&compatibility);
        let compatibility_table = table_from_grpprl(&compatibility_last);
        assert_eq!(
            compatibility_table
                .border_sizes
                .get(TableBorderSide::Top)
                .or(compatibility_table.border_size_eighths),
            Some(4)
        );
        assert_eq!(
            compatibility_table
                .border_colors
                .get(TableBorderSide::Top)
                .or(compatibility_table.border_color),
            Some(Color::rgb(0, 0, 0xFF))
        );
        assert_eq!(
            compatibility_table
                .border_styles
                .get(TableBorderSide::Top)
                .or(compatibility_table.border_style),
            Some(TableBorderStyle::Double)
        );
    }

    #[test]
    fn pre_definition_row_borders_do_not_project_over_default_tc80() {
        let mut grpprl = table_borders80_sprm(8, 0x01, 6);
        grpprl.extend_from_slice(&table_def_sprm());

        assert_no_projected_table_borders(&table_from_grpprl(&grpprl));
    }

    #[test]
    fn pre_definition_row_borders_do_not_bypass_later_tc80_cell_properties() {
        let mut grpprl = table_borders80_sprm(8, 0x01, 6);
        grpprl.extend_from_slice(&table_def_sprm_with_tc80([8, 0x01, 6, 0]));

        assert_no_projected_table_borders(&table_from_grpprl(&grpprl));
    }

    #[test]
    fn nil_unsupported_override_and_malformed_border_values_suppress_projection() {
        let positive = table_borders80_sprm(8, 0x01, 6);
        let mut nil = positive.clone();
        nil[4] = 0;
        let mut unsupported = positive.clone();
        unsupported[4] = 0x14;

        let mut cases = Vec::new();
        for later in [nil, unsupported] {
            let mut grpprl = table_def_sprm();
            grpprl.extend_from_slice(&positive);
            grpprl.extend_from_slice(&later);
            cases.push(grpprl);
        }

        let mut cell_override = table_def_sprm();
        cell_override.extend_from_slice(&positive);
        cell_override.extend_from_slice(&[
            0x20, 0xD6, 0x07, // sprmTSetBrc80, cb=7
            0, 2, 0x0F, 8, 0x01, 6, 0,
        ]);
        cases.push(cell_override);

        let mut wrong_size = table_def_sprm();
        wrong_size.extend_from_slice(&positive);
        wrong_size.extend_from_slice(&[0x05, 0xD6, 0x17]);
        wrong_size.extend_from_slice(&[0u8; 23]);
        cases.push(wrong_size);

        let mut truncated = table_def_sprm();
        truncated.extend_from_slice(&positive);
        truncated.extend_from_slice(&[0x13, 0xD6, 0x30, 0, 0, 0, 0]);
        cases.push(truncated);

        for grpprl in cases {
            assert_no_projected_table_borders(&table_from_grpprl(&grpprl));
        }
    }

    #[test]
    fn resolves_direct_paragraph_pagination_properties_and_defaults() {
        let (default, _) = scan_grpprl(&[], 0);
        assert_eq!(default.pagination, ParagraphPaginationOverrides::default());

        let (enabled, _) = scan_grpprl(
            &[
                0x05, 0x24, 0x01, // sprmPFKeep
                0x06, 0x24, 0x01, // sprmPFKeepFollow
                0x07, 0x24, 0x01, // sprmPFPageBreakBefore
                0x31, 0x24, 0x00, // sprmPFWidowControl
            ],
            0,
        );
        assert_eq!(
            enabled.pagination,
            ParagraphPaginationOverrides {
                keep_next: Some(true),
                keep_lines: Some(true),
                page_break_before: Some(true),
                widow_control: Some(false),
            }
        );

        let (last_value_wins, _) = scan_grpprl(
            &[
                0x05, 0x24, 0x01, 0x05, 0x24, 0x00, 0x06, 0x24, 0x00, 0x06, 0x24, 0x01, 0x07, 0x24,
                0x01, 0x07, 0x24, 0x00, 0x31, 0x24, 0x00, 0x31, 0x24, 0x01,
            ],
            0,
        );
        assert_eq!(
            last_value_wins.pagination,
            ParagraphPaginationOverrides {
                keep_next: Some(true),
                keep_lines: Some(false),
                page_break_before: Some(false),
                widow_control: Some(true),
            }
        );

        let (truncated, _) = scan_grpprl(
            &[
                0x05, 0x24, 0x01, // valid keep-lines survives
                0x31, 0x24, // truncated widow-control operand is ignored
            ],
            0,
        );
        assert_eq!(
            truncated.pagination,
            ParagraphPaginationOverrides {
                keep_lines: Some(true),
                ..ParagraphPaginationOverrides::default()
            }
        );

        assert!(scan_paragraph_style_overrides(&[0x05, 0x24, 0x01, 0x31, 0x24]).is_none());
        assert_eq!(
            scan_paragraph_style_overrides(&[
                0x5E, 0x84, 0x34, 0x12, // unrelated two-byte indent operand
                0x0D, 0xC6, 0x02, 0x00, 0x00, // unrelated variable tab operand
                0x07, 0x24, 0x01,
            ]),
            Some(ParagraphStyleOverrides {
                indent: ParagraphIndentOverrides {
                    logical_left_twips: Some(0x1234),
                    ..ParagraphIndentOverrides::default()
                },
                pagination: ParagraphPaginationOverrides {
                    page_break_before: Some(true),
                    ..ParagraphPaginationOverrides::default()
                },
                ..ParagraphStyleOverrides::default()
            })
        );
    }

    #[test]
    fn resolves_bounded_paragraph_spacing_in_strict_source_order() {
        let (default, _) = scan_grpprl(&[], 0);
        assert_eq!(default.spacing, ParagraphSpacingOverrides::default());

        let (source_ordered, _) = scan_grpprl(
            &[
                0x13, 0xA4, 0xC8, 0x00, // before = 200
                0x13, 0xA4, 0x00, 0x7D, // invalid before = 32000
                0x13, 0xA4, 0x00, 0x00, // before = 0
                0x14, 0xA4, 0x64, 0x00, // after = 100
                0x14, 0xA4, 0x00, 0x7D, // invalid after = 32000
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
                0x12, 0x64, 0xE0, 0x01, 0x02, 0x00, // invalid multiplier
                0x12, 0x64, 0xE0, 0x01, 0x01, 0x00, // 2 lines
            ],
            0,
        );
        assert_eq!(
            source_ordered.spacing,
            ParagraphSpacingOverrides {
                before_twips: Some(0),
                after_twips: Some(100),
                line: Some(ParagraphLineSpacing::ProportionalTwips(480)),
            }
        );

        let (inclusive_boundaries, _) = scan_grpprl(
            &[
                0x13, 0xA4, 0xC0, 0x7B, // maximum valid before = 0x7BC0
                0x13, 0xA4, 0xC1, 0x7B, // immediate invalid before
                0x14, 0xA4, 0xC0, 0x7B, // maximum valid after = 0x7BC0
                0x14, 0xA4, 0xC1, 0x7B, // immediate invalid after
                0x12, 0x64, 0xC0, 0x7B, 0x01, 0x00, // maximum proportional LSPD
                0x12, 0x64, 0xC1, 0x7B, 0x01, 0x00, // invalid LSPD gap
            ],
            0,
        );
        assert_eq!(
            inclusive_boundaries.spacing,
            ParagraphSpacingOverrides {
                before_twips: Some(0x7BC0),
                after_twips: Some(0x7BC0),
                line: Some(ParagraphLineSpacing::ProportionalTwips(0x7BC0)),
            }
        );

        for (operand, expected) in [
            (
                [0xF0, 0x00, 0x00, 0x00],
                ParagraphLineSpacing::AtLeastTwips(240),
            ),
            (
                [0x40, 0x84, 0x00, 0x00],
                ParagraphLineSpacing::ExactTwips(31_680),
            ),
            (
                [0x00, 0x00, 0x01, 0x00],
                ParagraphLineSpacing::Unrepresentable,
            ),
        ] {
            let mut grpprl = vec![0x12, 0x64];
            grpprl.extend_from_slice(&operand);
            let (parsed, _) = scan_grpprl(&grpprl, 0);
            assert_eq!(parsed.spacing.line, Some(expected));
        }

        let (valid_prefix, _) = scan_grpprl(
            &[
                0x13, 0xA4, 0x2C, 0x01, // before = 300
                0x14, 0xA4, // truncated after operand
            ],
            0,
        );
        assert_eq!(
            valid_prefix.spacing,
            ParagraphSpacingOverrides {
                before_twips: Some(300),
                ..ParagraphSpacingOverrides::default()
            }
        );

        let (style_reset, _) = scan_grpprl(
            &[
                0x13, 0xA4, 0xF0, 0x00, // direct before = 240
                0x12, 0x64, 0xE0, 0x01, 0x01, 0x00, // direct 2 lines
                0x00, 0x46, 0x01, 0x00, // apply paragraph style 1
                0x14, 0xA4, 0x3C, 0x00, // direct after = 60
            ],
            0,
        );
        assert_eq!(
            style_reset.spacing,
            ParagraphSpacingOverrides {
                before_twips: None,
                after_twips: Some(60),
                line: None,
            }
        );
    }

    #[test]
    fn resolves_bounded_flat_paragraph_shading_in_strict_source_order() {
        let yellow = Color::rgb(0xFF, 0xFF, 0);
        let red = Color::rgb(0xFF, 0, 0);
        let clear = Color::rgb(0x11, 0x22, 0x33);
        let solid = Color::rgb(0x44, 0x55, 0x66);
        let equal = Color::rgb(0x24, 0x68, 0xAC);

        let (default, _) = scan_grpprl(&[], 0);
        assert_eq!(default.shading, None);

        for (grpprl, expected) in [
            (
                paragraph_shd80_sprm(0, 7, 0),
                ParagraphShading::Flat(yellow),
            ),
            (paragraph_shd80_sprm(6, 0, 1), ParagraphShading::Flat(red)),
            (
                paragraph_shd_sprm([0, 0, 0, 0xFF], [0x11, 0x22, 0x33, 0], 0),
                ParagraphShading::Flat(clear),
            ),
            (
                paragraph_shd_sprm([0x44, 0x55, 0x66, 0], [0, 0, 0, 0xFF], 1),
                ParagraphShading::Flat(solid),
            ),
            (
                paragraph_shd_sprm(
                    [equal.r, equal.g, equal.b, 0],
                    [equal.r, equal.g, equal.b, 0],
                    8,
                ),
                ParagraphShading::Flat(equal),
            ),
        ] {
            let (pap, _) = scan_grpprl(&grpprl, 0);
            assert_eq!(pap.shading, Some(expected));
        }

        let mut source_ordered = paragraph_shd80_sprm(0, 7, 0);
        source_ordered.extend_from_slice(&paragraph_shd_sprm(
            [0x10, 0x20, 0x30, 0],
            [0xA0, 0xB0, 0xC0, 0],
            8,
        ));
        source_ordered.extend_from_slice(&paragraph_shd_sprm(
            [0, 0, 0, 0xFF],
            [clear.r, clear.g, clear.b, 0],
            0,
        ));
        let (pap, _) = scan_grpprl(&source_ordered, 0);
        assert_eq!(pap.shading, Some(ParagraphShading::Flat(clear)));

        let mut style_reset = paragraph_shd80_sprm(0, 7, 0);
        style_reset.extend_from_slice(&[0x00, 0x46, 0x0A, 0x00]);
        let (pap, _) = scan_grpprl(&style_reset, 0);
        assert_eq!(pap.shading, None);
    }

    #[test]
    fn unsupported_or_malformed_paragraph_shading_suppresses_prior_color() {
        let positive = paragraph_shd80_sprm(0, 7, 0);
        let unsupported = [
            paragraph_shd80_sprm(31, 31, 63),
            paragraph_shd80_sprm(17, 7, 0),
            paragraph_shd80_sprm(6, 7, 0x1A),
            paragraph_shd_sprm([0, 0, 0, 0xFF], [0, 0, 0, 0xFF], 0),
            paragraph_shd_sprm([0, 0, 0, 1], [0x11, 0x22, 0x33, 0], 0),
            {
                let mut wrong_size = Vec::from(SPRM_P_SHD.to_le_bytes());
                wrong_size.extend_from_slice(&[9, 0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0]);
                wrong_size
            },
            vec![0x4D, 0xC6, 10, 0, 0],
        ];

        for later in unsupported {
            let mut grpprl = positive.clone();
            grpprl.extend_from_slice(&later);
            let (pap, _) = scan_grpprl(&grpprl, 0);
            assert_eq!(
                pap.shading,
                Some(ParagraphShading::Unrepresentable),
                "later operand: {later:02X?}"
            );
        }
    }

    #[test]
    fn paragraph_style_shading_is_source_ordered_and_structurally_strict() {
        let yellow = Color::rgb(0xFF, 0xFF, 0);
        let clear = Color::rgb(0x11, 0x22, 0x33);

        assert_eq!(
            scan_paragraph_style_overrides(&paragraph_shd80_sprm(0, 7, 0))
                .unwrap()
                .shading,
            Some(ParagraphShading::Flat(yellow))
        );
        assert_eq!(
            scan_paragraph_style_overrides(&paragraph_shd_sprm(
                [0, 0, 0, 0xFF],
                [clear.r, clear.g, clear.b, 0],
                0,
            ))
            .unwrap()
            .shading,
            Some(ParagraphShading::Flat(clear))
        );

        let mut source_ordered = paragraph_shd80_sprm(0, 7, 0);
        source_ordered.extend_from_slice(&paragraph_shd_sprm(
            [0x10, 0x20, 0x30, 0],
            [0xA0, 0xB0, 0xC0, 0],
            8,
        ));
        assert_eq!(
            scan_paragraph_style_overrides(&source_ordered)
                .unwrap()
                .shading,
            Some(ParagraphShading::Unrepresentable)
        );
        source_ordered.extend_from_slice(&paragraph_shd_sprm(
            [0, 0, 0, 0xFF],
            [clear.r, clear.g, clear.b, 0],
            0,
        ));
        assert_eq!(
            scan_paragraph_style_overrides(&source_ordered)
                .unwrap()
                .shading,
            Some(ParagraphShading::Flat(clear))
        );

        let shd80_nil = paragraph_shd80_sprm(31, 31, 63);
        let mut shd_nil = Vec::from(SPRM_P_SHD.to_le_bytes());
        shd_nil.extend_from_slice(&[10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0]);
        for nil in [shd80_nil, shd_nil] {
            assert_eq!(
                scan_paragraph_style_overrides(&nil).unwrap().shading,
                Some(ParagraphShading::Unrepresentable)
            );
        }

        assert!(
            scan_paragraph_style_overrides(&[0x4D, 0xC6, 10, 0, 0]).is_none(),
            "a truncated style UPX must fail closed"
        );
    }

    #[test]
    fn paragraph_style_spacing_is_source_ordered_and_structurally_strict() {
        assert_eq!(
            scan_paragraph_style_overrides(&[
                0x13, 0xA4, 0xC8, 0x00, // before = 200
                0x13, 0xA4, 0x00, 0x7D, // invalid before preserves 200
                0x14, 0xA4, 0x64, 0x00, // after = 100
                0x12, 0x64, 0x68, 0x01, 0x01, 0x00, // 1.5 lines
                0x12, 0x64, 0x40, 0x84, 0x01, 0x00, // exact spacing
                0x12, 0x64, 0xE0, 0x01, 0x02, 0x00, // invalid multiplier
            ]),
            Some(ParagraphStyleOverrides {
                spacing: ParagraphSpacingOverrides {
                    before_twips: Some(200),
                    after_twips: Some(100),
                    line: Some(ParagraphLineSpacing::ExactTwips(31_680)),
                },
                ..ParagraphStyleOverrides::default()
            })
        );
        assert!(scan_paragraph_style_overrides(&[
            0x13, 0xA4, 0xC8, 0x00, // valid prefix
            0x12, 0x64, 0x68, // truncated LSPD
        ])
        .is_none());
    }

    #[test]
    fn paragraph_style_indents_are_strict_source_ordered_and_exclude_nest() {
        assert_eq!(
            scan_paragraph_style_overrides(&[
                0x5E, 0x84, 0x90, 0x01, // logical left = 400
                0x5E, 0x84, 0x00, 0x7D, // invalid XAS = 32000
                0x5E, 0x84, 0xF4, 0x01, // logical left = 500
                0x5D, 0x84, 0x40, 0x84, // logical right = -31680
                0x5D, 0x84, 0x3F, 0x84, // invalid XAS = -31681
                0x60, 0x84, 0xC0, 0x7B, // first line = 31680
                0x5F, 0x46, 0x78, 0x00, // prohibited style nest is ignored
            ]),
            Some(ParagraphStyleOverrides {
                indent: ParagraphIndentOverrides {
                    logical_left_twips: Some(500),
                    logical_right_twips: Some(-31_680),
                    nest_twips: None,
                    first_line_twips: Some(31_680),
                },
                ..ParagraphStyleOverrides::default()
            })
        );
    }

    #[test]
    fn paragraph_pagination_lookup_uses_fc_ranges_and_safe_defaults() {
        let table = PapxTable {
            entries: vec![
                PapEntry {
                    fc_lim: 100,
                    pagination: ParagraphPaginationOverrides {
                        keep_lines: Some(true),
                        ..ParagraphPaginationOverrides::default()
                    },
                    ..PapEntry::default()
                },
                PapEntry {
                    fc_lim: 200,
                    pagination: ParagraphPaginationOverrides {
                        keep_next: Some(true),
                        page_break_before: Some(true),
                        widow_control: Some(false),
                        ..ParagraphPaginationOverrides::default()
                    },
                    ..PapEntry::default()
                },
            ],
        };

        assert!(table.paragraph_pagination_at(50).keep_lines);
        let second = table.paragraph_pagination_at(150);
        assert!(second.keep_next);
        assert!(second.page_break_before);
        assert!(!second.widow_control);
        assert_eq!(
            table.paragraph_pagination_at(999),
            ParagraphPagination::default()
        );
    }

    #[test]
    fn scans_list_props() {
        // sprmPIlvl (0x260A, 1-byte) = 2, then sprmPIlfo (0x460B, 2-byte) = 5.
        let (p, _) = scan_grpprl(&[0x0A, 0x26, 0x02, 0x0B, 0x46, 0x05, 0x00], 0);
        assert_eq!((p.ilfo, p.ilvl), (5, 2));
    }

    #[test]
    fn scans_style_outline_align() {
        // leading istd = 7; sprmPJc80(0x2403)=1 center; sprmPOutLvl(0x2640)=2.
        let (p, _) = scan_grpprl(&[0x03, 0x24, 0x01, 0x40, 0x26, 0x02], 7);
        assert_eq!(p.istd, 7);
        assert_eq!(p.layout.justification, Some(ParagraphJustification::Center));
        assert_eq!(p.outlvl, Some(2));
        // sprmPIstd (0x4600, 2-byte) overrides the leading istd.
        let (p2, _) = scan_grpprl(&[0x00, 0x46, 0x05, 0x00], 7);
        assert_eq!(p2.istd, 5);
    }

    #[test]
    fn direct_paragraph_bidi_is_strict_bool8_and_last_valid_wins() {
        let (default, _) = scan_grpprl(&[], 0);
        assert_eq!(default.layout.bidi, None);

        let (last_valid, _) = scan_grpprl(
            &[
                0x41, 0x24, 0x01, // on
                0x41, 0x24, 0x00, // off
                0x41, 0x24, 0x01, // on again
            ],
            0,
        );
        assert_eq!(last_valid.layout.bidi, Some(true));

        let (invalid, _) = scan_grpprl(
            &[
                0x41, 0x24, 0x01, // valid prefix
                0x41, 0x24, 0x02, // invalid Bool8 preserves it
                0x41, 0x24, 0xFF, // invalid Bool8 preserves it
            ],
            0,
        );
        assert_eq!(invalid.layout.bidi, Some(true));

        let (only_invalid, _) = scan_grpprl(&[0x41, 0x24, 0x02], 0);
        assert_eq!(only_invalid.layout.bidi, None);

        let (truncated, _) = scan_grpprl(&[0x41, 0x24, 0x01, 0x41, 0x24], 0);
        assert_eq!(truncated.layout.bidi, Some(true));
    }

    #[test]
    fn direct_paragraph_justification_preserves_bounded_classes() {
        for (sprm, value, expected) in [
            (SPRM_P_JC_80, 0, ParagraphJustification::PhysicalLeft),
            (SPRM_P_JC_80, 1, ParagraphJustification::Center),
            (SPRM_P_JC_80, 2, ParagraphJustification::PhysicalRight),
            (SPRM_P_JC_80, 3, ParagraphJustification::Justify),
            (SPRM_P_JC_80, 4, ParagraphJustification::Justify),
            (SPRM_P_JC_80, 5, ParagraphJustification::Justify),
            (SPRM_P_JC, 0, ParagraphJustification::LogicalStart),
            (SPRM_P_JC, 1, ParagraphJustification::Center),
            (SPRM_P_JC, 2, ParagraphJustification::LogicalEnd),
            (SPRM_P_JC, 3, ParagraphJustification::Justify),
            (SPRM_P_JC, 4, ParagraphJustification::Justify),
            (SPRM_P_JC, 5, ParagraphJustification::Justify),
            (SPRM_P_JC, 6, ParagraphJustification::UnsupportedIndented),
            (SPRM_P_JC, 7, ParagraphJustification::Justify),
            (SPRM_P_JC, 8, ParagraphJustification::Justify),
            (SPRM_P_JC, 9, ParagraphJustification::Justify),
        ] {
            let mut grpprl = Vec::from(sprm.to_le_bytes());
            grpprl.push(value);
            let (pap, _) = scan_grpprl(&grpprl, 0);
            assert_eq!(pap.layout.justification, Some(expected));
        }
    }

    #[test]
    fn direct_paragraph_justification_uses_source_order_and_safe_fallbacks() {
        let (logical_last, _) = scan_grpprl(
            &[
                0x03, 0x24, 0x00, // physical left
                0x61, 0x24, 0x02, // logical end
            ],
            0,
        );
        assert_eq!(
            logical_last.layout.justification,
            Some(ParagraphJustification::LogicalEnd)
        );

        let (physical_last, _) = scan_grpprl(
            &[
                0x61, 0x24, 0x02, // logical end
                0x03, 0x24, 0x00, // physical left
            ],
            0,
        );
        assert_eq!(
            physical_last.layout.justification,
            Some(ParagraphJustification::PhysicalLeft)
        );

        let (invalid, _) = scan_grpprl(
            &[
                0x61, 0x24, 0x02, // valid logical end
                0x03, 0x24, 0x06, // invalid physical value
                0x61, 0x24, 0x0A, // invalid logical value
            ],
            0,
        );
        assert_eq!(
            invalid.layout.justification,
            Some(ParagraphJustification::LogicalEnd)
        );

        let (truncated, _) = scan_grpprl(&[0x03, 0x24, 0x02, 0x61, 0x24], 0);
        assert_eq!(
            truncated.layout.justification,
            Some(ParagraphJustification::PhysicalRight)
        );
    }

    #[test]
    fn direct_paragraph_indents_are_strict_source_ordered_and_prefix_safe() {
        let (default, _) = scan_grpprl(&[], 0);
        assert_eq!(default.indent, ParagraphIndentOverrides::default());

        let (values, _) = scan_grpprl(
            &[
                0x5D, 0x84, 0x60, 0x09, // logical right = 2400
                0x5E, 0x84, 0xD0, 0x07, // logical left = 2000
                0x5F, 0x46, 0x88, 0xFF, // nest = -120
                0x60, 0x84, 0x98, 0xFE, // first line = -360
            ],
            0,
        );
        assert_eq!(
            values.indent,
            ParagraphIndentOverrides {
                logical_left_twips: Some(2000),
                logical_right_twips: Some(2400),
                nest_twips: Some(-120),
                first_line_twips: Some(-360),
            }
        );

        let (last_valid, _) = scan_grpprl(
            &[
                0x5E, 0x84, 0x90, 0x01, // logical left = 400
                0x5E, 0x84, 0x00, 0x7D, // invalid XAS = 32000
                0x5E, 0x84, 0xF4, 0x01, // logical left = 500
                0x5D, 0x84, 0x40, 0x84, // valid lower XAS bound = -31680
                0x5D, 0x84, 0x3F, 0x84, // invalid XAS = -31681
                0x60, 0x84, 0xC0, 0x7B, // valid upper XAS bound = 31680
            ],
            0,
        );
        assert_eq!(last_valid.indent.logical_left_twips, Some(500));
        assert_eq!(last_valid.indent.logical_right_twips, Some(-31_680));
        assert_eq!(last_valid.indent.first_line_twips, Some(31_680));

        let (truncated, _) = scan_grpprl(
            &[
                0x5E, 0x84, 0xD0, 0x07, // valid logical-left prefix
                0x5D, 0x84, 0x20, // truncated logical-right operand
            ],
            0,
        );
        assert_eq!(truncated.indent.logical_left_twips, Some(2000));
        assert_eq!(truncated.indent.logical_right_twips, None);
    }

    #[test]
    fn paragraph_style_changes_discard_earlier_non_preserved_properties() {
        let (style_last, _) = scan_grpprl(
            &[
                0x41, 0x24, 0x01, // direct RTL on
                0x61, 0x24, 0x02, // direct logical end
                0x05, 0x24, 0x01, // direct keep lines
                0x06, 0x24, 0x01, // direct keep next
                0x07, 0x24, 0x01, // direct page break before
                0x31, 0x24, 0x00, // direct widow control off
                0x40, 0x26, 0x02, // direct outline level
                0x0A, 0x26, 0x03, // direct list level
                0x0B, 0x46, 0x05, 0x00, // direct list format override
                0x5E, 0x84, 0xD0, 0x07, // direct logical-left indent
                0x60, 0x84, 0x98, 0xFE, // direct hanging indent
                0x16, 0x24, 0x01, // paragraph is in a table
                0x17, 0x24, 0x01, // paragraph terminates a table row
                0x04, 0x34, 0x01, // table header row
                0x66, 0x34, 0x01, // table row cannot split
                0x00, 0x46, 0x05, 0x00, // sprmPIstd = 5
            ],
            2,
        );
        assert_eq!(style_last.istd, 5);
        assert_eq!(style_last.layout, ParagraphLayoutOverrides::default());
        assert_eq!(
            style_last.pagination,
            ParagraphPaginationOverrides::default()
        );
        assert_eq!(style_last.outlvl, Some(4));
        assert_eq!((style_last.ilfo, style_last.ilvl), (0, 0));
        assert_eq!(style_last.indent, ParagraphIndentOverrides::default());
        assert!(style_last.in_table);
        assert!(style_last.ttp);
        assert!(style_last.table_header);
        assert!(style_last.resolved_cant_split());

        let (direct_last, _) = scan_grpprl(
            &[
                0x00, 0x46, 0x05, 0x00, // sprmPIstd = 5
                0x41, 0x24, 0x01, // later direct RTL on
                0x61, 0x24, 0x02, // later direct logical end
                0x05, 0x24, 0x01, // later direct keep lines
                0x40, 0x26, 0x02, // later direct outline level
                0x0A, 0x26, 0x03, // later direct list level
                0x0B, 0x46, 0x05, 0x00, // later direct list format override
                0x5E, 0x84, 0xD0, 0x07, // later direct logical-left indent
                0x60, 0x84, 0x98, 0xFE, // later direct hanging indent
            ],
            2,
        );
        assert_eq!(direct_last.istd, 5);
        assert_eq!(
            direct_last.layout,
            ParagraphLayoutOverrides {
                bidi: Some(true),
                justification: Some(ParagraphJustification::LogicalEnd),
            }
        );
        assert_eq!(direct_last.pagination.keep_lines, Some(true));
        assert_eq!(direct_last.outlvl, Some(2));
        assert_eq!((direct_last.ilfo, direct_last.ilvl), (5, 3));
        assert_eq!(
            direct_last.indent,
            ParagraphIndentOverrides {
                logical_left_twips: Some(2000),
                first_line_twips: Some(-360),
                ..ParagraphIndentOverrides::default()
            }
        );
    }

    #[test]
    fn paragraph_style_permutation_resets_non_preserved_properties_when_affected() {
        // SPPOperand maps styles 4..=6 to 7, 8, and 9 respectively.
        let permutation = [
            0x01, 0xC6, // sprmPIstdPermute
            0x0B, // cb
            0x00, // fLong
            0x04, 0x00, // istdFirst
            0x06, 0x00, // istdLast
            0x07, 0x00, 0x08, 0x00, 0x09, 0x00, // rgIstdPermute
        ];

        let mut affected_grpprl = vec![
            0x41, 0x24, 0x01, // direct RTL on
            0x03, 0x24, 0x02, // direct physical right
            0x05, 0x24, 0x01, // direct keep lines
            0x40, 0x26, 0x02, // direct outline level
            0x0A, 0x26, 0x03, // direct list level
            0x0B, 0x46, 0x05, 0x00, // direct list format override
            0x16, 0x24, 0x01, // paragraph is in a table
            0x04, 0x34, 0x01, // table header row
        ];
        affected_grpprl.extend_from_slice(&permutation);
        let (affected, _) = scan_grpprl(&affected_grpprl, 5);
        assert_eq!(affected.istd, 8);
        assert_eq!(affected.layout, ParagraphLayoutOverrides::default());
        assert_eq!(affected.pagination, ParagraphPaginationOverrides::default());
        assert_eq!(affected.outlvl, Some(7));
        assert_eq!((affected.ilfo, affected.ilvl), (0, 0));
        assert!(affected.in_table);
        assert!(affected.table_header);

        let mut unaffected_grpprl = vec![
            0x41, 0x24, 0x01, // direct RTL on
            0x03, 0x24, 0x02, // direct physical right
            0x05, 0x24, 0x01, // direct keep lines
            0x40, 0x26, 0x02, // direct outline level
            0x0A, 0x26, 0x03, // direct list level
            0x0B, 0x46, 0x05, 0x00, // direct list format override
        ];
        unaffected_grpprl.extend_from_slice(&permutation);
        let (unaffected, _) = scan_grpprl(&unaffected_grpprl, 3);
        assert_eq!(unaffected.istd, 3);
        assert_eq!(
            unaffected.layout,
            ParagraphLayoutOverrides {
                bidi: Some(true),
                justification: Some(ParagraphJustification::PhysicalRight),
            }
        );
        assert_eq!(unaffected.pagination.keep_lines, Some(true));
        assert_eq!(unaffected.outlvl, Some(2));
        assert_eq!((unaffected.ilfo, unaffected.ilvl), (5, 3));
    }

    #[test]
    fn malformed_or_truncated_style_permutation_preserves_direct_layout() {
        let direct = [
            0x41, 0x24, 0x01, // direct RTL on
            0x03, 0x24, 0x02, // direct physical right
        ];
        let expected = ParagraphLayoutOverrides {
            bidi: Some(true),
            justification: Some(ParagraphJustification::PhysicalRight),
        };

        let mut reversed_bounds = direct.to_vec();
        reversed_bounds
            .extend_from_slice(&[0x01, 0xC6, 0x07, 0x00, 0x05, 0x00, 0x04, 0x00, 0x09, 0x00]);
        let (reversed, _) = scan_grpprl(&reversed_bounds, 5);
        assert_eq!(reversed.istd, 5);
        assert_eq!(reversed.layout, expected);

        let mut count_mismatch = direct.to_vec();
        count_mismatch
            .extend_from_slice(&[0x01, 0xC6, 0x07, 0x00, 0x05, 0x00, 0x06, 0x00, 0x09, 0x00]);
        let (mismatched, _) = scan_grpprl(&count_mismatch, 5);
        assert_eq!(mismatched.istd, 5);
        assert_eq!(mismatched.layout, expected);

        let mut truncated = direct.to_vec();
        truncated.extend_from_slice(&[0x01, 0xC6, 0x0B, 0x00, 0x04, 0x00, 0x06, 0x00]);
        let (truncated, _) = scan_grpprl(&truncated, 5);
        assert_eq!(truncated.istd, 5);
        assert_eq!(truncated.layout, expected);
    }

    #[test]
    fn parses_tdeftable_then_reads_flags() {
        // sprmTDefTable (0xD608) with cb=26 (remainder 25 bytes), then the table
        // flags. The walker must skip exactly cb+1 = 27 operand bytes and parse
        // the row definition.
        let mut gp = vec![0x08, 0xD6, 0x1A, 0x00]; // sprm + cb=26
        gp.push(1); // itcMac = 1
        gp.extend_from_slice(&0i16.to_le_bytes()); // rgdxa[0]
        gp.extend_from_slice(&100i16.to_le_bytes()); // rgdxa[1]
        gp.extend_from_slice(&[0u8; 20]); // one TC80 (tcgrf=0 + padding)
        gp.extend_from_slice(&[0x16, 0x24, 0x01]); // sprmPFInTable = 1
        gp.extend_from_slice(&[0x17, 0x24, 0x01]); // sprmPFTtp = 1
        let (p, def) = scan_grpprl(&gp, 0);
        assert!(p.in_table && p.ttp);
        assert_eq!(def.unwrap().rgdxa, vec![0, 100]);
    }

    #[test]
    fn lookup_by_fc() {
        let mk = |fc_lim, in_table, ttp| PapEntry {
            fc_lim,
            in_table,
            ttp,
            ilfo: 0,
            ilvl: 0,
            istd: 0,
            outlvl: None,
            layout: ParagraphLayoutOverrides::default(),
            indent: ParagraphIndentOverrides::default(),
            spacing: ParagraphSpacingOverrides::default(),
            shading: None,
            pagination: ParagraphPaginationOverrides::default(),
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stop_changes: ParagraphTabStopChanges::default(),
            table_header: false,
            table_cant_split: false,
            table_bidi_visual: false,
            table_def: None,
        };
        let t = PapxTable {
            entries: vec![
                mk(100, false, false),
                mk(200, true, false),
                mk(300, true, true),
            ],
        };
        assert_eq!(t.at(50), (false, false)); // first paragraph
        assert_eq!(t.at(150), (true, false)); // cell paragraph
        assert_eq!(t.at(250), (true, true)); // row-terminating
        assert_eq!(t.at(999), (false, false)); // past the end
    }

    #[test]
    fn row_no_split_lookup_and_truncated_papx_default_safely() {
        let table =
            PapxTable::from_test_entries(&[(100, false, false, false), (200, true, true, true)]);
        assert!(!table.table_cant_split_at(50));
        assert!(table.table_cant_split_at(150));
        assert!(!table.table_cant_split_at(250));

        let mut word = vec![0u8; FKP_SIZE];
        word[4..8].copy_from_slice(&100u32.to_le_bytes());
        word[8] = u8::MAX; // PAPX offset 510: the declared payload is truncated.
        word[FKP_SIZE - 1] = 1;
        let mut plc = Vec::new();
        plc.extend_from_slice(&0u32.to_le_bytes());
        plc.extend_from_slice(&100u32.to_le_bytes());
        plc.extend_from_slice(&0u32.to_le_bytes());

        let parsed = parse(&word, &plc, 0, plc.len());
        assert!(!parsed.is_empty());
        assert_eq!(parsed.at(50), (false, false));
        assert!(!parsed.table_cant_split_at(50));
    }

    #[test]
    fn bin_table_entry_count_is_capped() {
        // One valid FKP page (page 0): crun = 29 default entries.
        let mut word = vec![0u8; FKP_SIZE];
        word[FKP_SIZE - 1] = 29;
        // A PlcBtePapx of 10 pages whose page numbers are all 0 (every entry points at the
        // same valid page) — the repeated-page amplification the fix bounds.
        let n_pages = 10usize;
        let plc = vec![0u8; 4 * (n_pages + 1) + 4 * n_pages];
        set_test_max_fkp(64);
        let t = parse(&word, &plc, 0, plc.len());
        set_test_max_fkp(MAX_FKP_ENTRIES);
        // Uncapped this would be 10 * 29 = 290; the cap stops it well before that, without
        // panicking or hanging.
        assert!(
            t.entries.len() >= 64 && t.entries.len() < n_pages * 29,
            "entry cap did not bound the repeated-page bin table: {}",
            t.entries.len()
        );
    }
}
