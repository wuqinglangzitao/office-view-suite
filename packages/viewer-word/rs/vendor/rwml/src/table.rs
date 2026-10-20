//! Table reconstruction: parse `sprmTDefTable` row definitions and fold streamed
//! rows/cells into a merge-aware [`model::Table`] (colspan from `fMerged`,
//! rowspan from `fVertRestart`/`fVertMerge`, matched by column).
//!
//! Reference: [MS-DOC] 2.4.3 (cell boundaries), 2.9.349 (TDefTableOperand),
//! 2.9.330 (TC80).

#[cfg(feature = "docx")]
use crate::model::TableCellColumnBreakHints;
use crate::model::{
    Block, Cell, CellBorder, CellBorders, CellMargins, Color, LineSpacingHint, PaginationHint, Row, Table,
    TableBorderSide, TableBorderStyle, TableCellLineSpacingHints, TableCellPaginationHints, VCell,
};
#[cfg(any(feature = "docx", feature = "render"))]
use crate::model::{TabStop, TableCellTabStopHints};

const F_MERGED: u16 = 0x0002; // cell folds into the one to its left
const F_VERT_MERGE: u16 = 0x0020; // cell continues a vertical merge from above
const F_VERT_RESTART: u16 = 0x0040; // cell starts a vertical-merge group
const TC80_LEN: usize = 20;
const BRC80_LEN: usize = 4;
const BRC_LEN: usize = 8;
const MAX_BORDER_SIZE_EIGHTHS: u8 = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyBorderColor {
    Auto,
    Explicit(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LegacyBorder {
    color: LegacyBorderColor,
    size_eighths: u16,
    style: TableBorderStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum LegacyBorderState {
    #[default]
    Inherit,
    Nil,
    Unsupported,
    Positive(LegacyBorder),
}

#[derive(Debug, Clone, Copy, Default)]
struct LegacyCellBorders {
    top: LegacyBorderState,
    left: LegacyBorderState,
    bottom: LegacyBorderState,
    right: LegacyBorderState,
}

#[derive(Debug, Clone, Copy, Default)]
struct LegacyRowBorders {
    top: LegacyBorderState,
    left: LegacyBorderState,
    bottom: LegacyBorderState,
    right: LegacyBorderState,
    inside_h: LegacyBorderState,
    inside_v: LegacyBorderState,
}

/// MS-DOC 2.6.3 table layout properties. These are independent of the order of
/// sprmTDefTable, which supplies cell definitions rather than layout defaults.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LegacyTableLayout {
    auto_fit: bool,
    width_pct: Option<f32>,
    minimum_height_pt: Option<f32>,
    margins: CellMargins,
}

impl Default for LegacyTableLayout {
    fn default() -> Self {
        Self {
            auto_fit: false,
            width_pct: None,
            minimum_height_pt: None,
            margins: CellMargins { top: 0, right: 108, bottom: 0, left: 108 },
        }
    }
}

impl LegacyTableLayout {
    pub(crate) fn apply_sprm(&mut self, sprm: u16, operand: &[u8]) {
        match (sprm, operand) {
            // sprmTFAutofit: fixed columns are the binary format's default.
            (0x3615, [value @ 0..=1]) => self.auto_fit = *value == 1,
            // FtsWWidth_Table: percentages use fiftieths of one percent.
            (0xF614, [unit, low, high]) => {
                let value = u16::from_le_bytes([*low, *high]);
                match (*unit, value) {
                    (2, 0..=30_000) => self.width_pct = Some(value as f32 / 5000.0),
                    (0 | 1, 0) | (3, 0..=31_680) => self.width_pct = None,
                    _ => {}
                }
            }
            // YAS: positive means at least; zero is content-derived. Exact
            // negative heights are not projected as minimum heights.
            (0x9407, [low, high]) => {
                let height = i16::from_le_bytes([*low, *high]);
                if (-31_680..=31_680).contains(&height) {
                    self.minimum_height_pt = (height > 0).then_some(height as f32 / 20.0);
                }
            }
            // CSSAOperand for row-default padding: cb, first, limit, sides,
            // unit, width. Validate the whole operand before touching a side.
            (0xD634, [6, 0, 1, sides, unit, low, high]) if sides & !0x0F == 0 => {
                let width = u16::from_le_bytes([*low, *high]);
                if !matches!((*unit, width), (0, 0) | (3, 0..=31_680)) { return; }
                for (mask, target) in [
                    (1, &mut self.margins.top), (2, &mut self.margins.left),
                    (4, &mut self.margins.bottom), (8, &mut self.margins.right),
                ] {
                    if sides & mask != 0 { *target = u32::from(width); }
                }
            }
            _ => {}
        }
    }
}

/// A parsed row definition (the `sprmTDefTable` operand carried on the TTP).
#[derive(Debug, Clone, Default)]
pub(crate) struct TableDef {
    pub layout: LegacyTableLayout,
    /// Cell-boundary x-positions in twips (`itcMac + 1` entries).
    pub rgdxa: Vec<i16>,
    /// Per-cell `TC80.tcgrf` (merge flags); `itcMac` entries.
    pub tcgrf: Vec<u16>,
    /// Complete direct TC80 border state for each cell.
    cell_borders: Vec<LegacyCellBorders>,
    /// Direct table/row border defaults, independent of the TC80 definition order.
    row_borders: Option<LegacyRowBorders>,
    /// A later table modifier made exact six-role projection ambiguous.
    border_projection_blocked: bool,
}

impl TableDef {
    /// Parse a `TDefTableOperand`: `cb:u16, itcMac:u8, rgdxaCenter[itcMac+1]:i16,
    /// rgTc[itcMac]:TC80(20B)`.
    pub(crate) fn parse(operand: &[u8]) -> Option<TableDef> {
        let declared = usize::from(u16::from_le_bytes([*operand.first()?, *operand.get(1)?]));
        if declared == 0 || operand.len() != declared.checked_add(1)? {
            return None;
        }
        let itc_mac = *operand.get(2)? as usize;
        if itc_mac == 0 || itc_mac > 63 {
            return None;
        }
        let tc_base = 3usize.checked_add(2usize.checked_mul(itc_mac.checked_add(1)?)?)?;
        let mut rgdxa = Vec::with_capacity(itc_mac + 1);
        for k in 0..=itc_mac {
            let o = 3 + 2 * k;
            let b = operand.get(o..o + 2)?;
            rgdxa.push(i16::from_le_bytes([b[0], b[1]]));
        }
        if rgdxa.windows(2).any(|pair| pair[0] > pair[1]) {
            return None;
        }
        let tc_bytes = operand.get(tc_base..)?;
        let complete_tc80s = tc_bytes.len() / TC80_LEN;
        let mut tcgrf = vec![0; itc_mac];
        let mut cell_borders = vec![LegacyCellBorders::default(); itc_mac];
        for k in 0..complete_tc80s.min(itc_mac) {
            let o = tc_base + k * TC80_LEN;
            let tc = operand.get(o..o + TC80_LEN)?;
            tcgrf[k] = u16::from_le_bytes([tc[0], tc[1]]);
            cell_borders[k] = LegacyCellBorders {
                top: parse_brc80(&tc[4..8])?,
                left: parse_brc80(&tc[8..12])?,
                bottom: parse_brc80(&tc[12..16])?,
                right: parse_brc80(&tc[16..20])?,
            };
        }
        let border_projection_blocked = complete_tc80s != itc_mac
            || tc_bytes.len() % TC80_LEN != 0
            || rgdxa.windows(2).any(|pair| pair[0] == pair[1]);
        Some(TableDef {
            layout: LegacyTableLayout::default(),
            rgdxa,
            tcgrf,
            cell_borders,
            row_borders: None,
            border_projection_blocked,
        })
    }

    pub(crate) fn table_borders80_operand_is_valid(operand: &[u8]) -> bool {
        parse_row_borders(operand, BRC80_LEN, parse_brc80).is_some()
    }

    pub(crate) fn table_borders_operand_is_valid(operand: &[u8]) -> bool {
        parse_row_borders(operand, BRC_LEN, parse_brc).is_some()
    }

    pub(crate) fn apply_table_borders80(&mut self, operand: &[u8]) -> bool {
        let Some(borders) = parse_row_borders(operand, BRC80_LEN, parse_brc80) else {
            return false;
        };
        self.row_borders = Some(borders);
        true
    }

    pub(crate) fn apply_table_borders(&mut self, operand: &[u8]) -> bool {
        let Some(borders) = parse_row_borders(operand, BRC_LEN, parse_brc) else {
            return false;
        };
        self.row_borders = Some(borders);
        true
    }

    pub(crate) fn block_border_projection(&mut self) {
        self.border_projection_blocked = true;
    }

    pub(crate) fn inherit_row_borders(&mut self, defaults: &TableDef) {
        self.row_borders = defaults.row_borders;
    }

    /// Applying a table style replaces existing borders. Its unsupported style
    /// borders stay unspecified until a later direct declaration supplies them.
    pub(crate) fn reset_border_properties(&mut self) {
        self.row_borders = None;
        self.cell_borders.fill(LegacyCellBorders::default());
    }

    /// Automatic TC color arrays do not change a default/automatic TC80 border.
    /// Explicit color overrides remain unsupported rather than rendering stale colors.
    pub(crate) fn automatic_cell_border_colors(&self, operand: &[u8]) -> bool {
        let count = self.tcgrf.len();
        operand.first().copied().map(usize::from) == Some(count * 4)
            && operand.len() == 1 + count * 4
            && operand[1..]
                .chunks_exact(4)
                .all(|color| color == [0, 0, 0, 0xFF])
            && self.cell_borders.iter().all(|cell| {
                [cell.top, cell.left, cell.bottom, cell.right]
                    .into_iter()
                    .all(|border| {
                        !matches!(
                            border,
                            LegacyBorderState::Positive(LegacyBorder {
                                color: LegacyBorderColor::Explicit(_),
                                ..
                            })
                        )
                    })
            })
    }

    fn resolved_cell_borders(&self, cell: usize, row: usize, row_count: usize) -> CellBorders {
        if self.border_projection_blocked {
            return CellBorders::default();
        }
        let Some(direct) = self.cell_borders.get(cell) else {
            return CellBorders::default();
        };
        let defaults = self.row_borders.unwrap_or_default();
        let edge = |direct, inherited| {
            modeled_cell_border(if direct == LegacyBorderState::Inherit {
                inherited
            } else {
                direct
            })
        };
        CellBorders {
            top: edge(
                direct.top,
                if row == 0 {
                    defaults.top
                } else {
                    defaults.inside_h
                },
            ),
            bottom: edge(
                direct.bottom,
                if row + 1 == row_count {
                    defaults.bottom
                } else {
                    defaults.inside_h
                },
            ),
            left: edge(
                direct.left,
                if cell == 0 {
                    defaults.left
                } else {
                    defaults.inside_v
                },
            ),
            right: edge(
                direct.right,
                if cell + 1 == self.tcgrf.len() {
                    defaults.right
                } else {
                    defaults.inside_v
                },
            ),
            ..CellBorders::default()
        }
    }
}

fn modeled_cell_border(state: LegacyBorderState) -> Option<CellBorder> {
    match state {
        LegacyBorderState::Inherit => None,
        LegacyBorderState::Nil | LegacyBorderState::Unsupported => Some(CellBorder {
            style: "none",
            color: None,
            width_eighths: 0,
        }),
        LegacyBorderState::Positive(border) => Some(CellBorder {
            style: match border.style {
                TableBorderStyle::Single => "solid",
                TableBorderStyle::Double => "double",
                TableBorderStyle::Dotted => "dotted",
                TableBorderStyle::Dashed => "dashed",
            },
            color: match border.color {
                LegacyBorderColor::Auto => None,
                LegacyBorderColor::Explicit(color) => Some(color),
            },
            width_eighths: border.size_eighths,
        }),
    }
}

fn supported_border_style(value: u8) -> Option<TableBorderStyle> {
    match value {
        0x01 | 0x05 => Some(TableBorderStyle::Single),
        0x03 => Some(TableBorderStyle::Double),
        0x06 => Some(TableBorderStyle::Dotted),
        0x07 => Some(TableBorderStyle::Dashed),
        _ => None,
    }
}

fn positive_border(
    size: u8,
    style: u8,
    color: LegacyBorderColor,
    effects: u8,
) -> LegacyBorderState {
    if style == 0 {
        return LegacyBorderState::Inherit;
    }
    if effects != 0 || size > MAX_BORDER_SIZE_EIGHTHS || supported_border_style(style).is_none() {
        return LegacyBorderState::Unsupported;
    }
    LegacyBorderState::Positive(LegacyBorder {
        color,
        size_eighths: u16::from(size.max(2)),
        style: supported_border_style(style).expect("checked supported style"),
    })
}

fn parse_brc80(bytes: &[u8]) -> Option<LegacyBorderState> {
    let [size, style, ico, flags]: [u8; BRC80_LEN] = bytes.try_into().ok()?;
    if bytes.iter().all(|byte| *byte == 0xFF) {
        return Some(LegacyBorderState::Nil);
    }
    let color = match ico {
        0 => LegacyBorderColor::Auto,
        1..=16 => LegacyBorderColor::Explicit(crate::chpx::ico_color(ico)?),
        _ => return Some(LegacyBorderState::Unsupported),
    };
    Some(positive_border(size, style, color, flags & 0x7F))
}

fn parse_brc(bytes: &[u8]) -> Option<LegacyBorderState> {
    let [red, green, blue, f_auto, size, style, flags, _reserved]: [u8; BRC_LEN] =
        bytes.try_into().ok()?;
    if bytes[4..].iter().all(|byte| *byte == 0xFF) {
        return Some(LegacyBorderState::Nil);
    }
    let color = match f_auto {
        0 => LegacyBorderColor::Explicit(Color::rgb(red, green, blue)),
        0xFF => LegacyBorderColor::Auto,
        _ => return Some(LegacyBorderState::Unsupported),
    };
    Some(positive_border(size, style, color, flags & 0x7F))
}

fn parse_row_borders(
    operand: &[u8],
    border_len: usize,
    parse: fn(&[u8]) -> Option<LegacyBorderState>,
) -> Option<LegacyRowBorders> {
    let declared = usize::from(*operand.first()?);
    let expected = border_len.checked_mul(6)?;
    if declared != expected || operand.len() != expected.checked_add(1)? {
        return None;
    }
    let mut borders = [LegacyBorderState::Inherit; 6];
    for (index, border) in borders.iter_mut().enumerate() {
        let start = 1 + index * border_len;
        *border = parse(operand.get(start..start + border_len)?)?;
    }
    Some(LegacyRowBorders {
        top: borders[0],
        left: borders[1],
        bottom: borders[2],
        right: borders[3],
        inside_h: borders[4],
        inside_v: borders[5],
    })
}

/// One streamed cell, keeping block content and its source pagination metadata
/// together through merge resolution.
#[derive(Default)]
pub(crate) struct CellBuild {
    pub blocks: Vec<Block>,
    pub pagination: Vec<Option<PaginationHint>>,
    pub line_spacing: Vec<Option<LineSpacingHint>>,
    #[cfg(feature = "docx")]
    pub column_break_offsets: Vec<Vec<usize>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub tab_stops: Vec<Vec<TabStop>>,
}

/// One streamed row: its cells + the row definition + header flag.
pub(crate) struct RowBuild {
    pub cells: Vec<CellBuild>,
    pub def: Option<TableDef>,
    pub header: bool,
}

pub(crate) struct TableBuildOutput {
    pub table: Table,
    pub cell_pagination: TableCellPaginationHints,
    pub cell_line_spacing: TableCellLineSpacingHints,
    #[cfg(feature = "docx")]
    pub cell_column_breaks: TableCellColumnBreakHints,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub cell_tab_stops: TableCellTabStopHints,
}

/// An output cell during merge resolution.
struct Out {
    blocks: Vec<Block>,
    borders: CellBorders,
    margins: Option<CellMargins>,
    pagination: Vec<Option<PaginationHint>>,
    line_spacing: Vec<Option<LineSpacingHint>>,
    #[cfg(feature = "docx")]
    column_break_offsets: Vec<Vec<usize>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: Vec<Vec<TabStop>>,
    /// Starting column over the table's global boundary set.
    col: usize,
    colspan: u16,
    rowspan: u16,
    tcgrf: u16,
    dropped: bool,
}

fn normalized_column_widths(rows: &[RowBuild], bounds: &[i16]) -> Vec<f32> {
    let Some(first) = rows.first().and_then(|row| row.def.as_ref()) else {
        return Vec::new();
    };
    let Some((&left, &right)) = first.rgdxa.first().zip(first.rgdxa.last()) else {
        return Vec::new();
    };
    if first.rgdxa.len() < 2 {
        return Vec::new();
    }

    for row in rows {
        let Some(def) = row.def.as_ref() else {
            return Vec::new();
        };
        if def.rgdxa.len() < 2
            || def.rgdxa.first() != Some(&left)
            || def.rgdxa.last() != Some(&right)
            || def.rgdxa.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Vec::new();
        }
    }

    let total = i32::from(right) - i32::from(left);
    if total <= 0 || bounds.len() < 2 {
        return Vec::new();
    }
    bounds
        .windows(2)
        .map(|pair| (i32::from(pair[1]) - i32::from(pair[0])) as f32 / total as f32)
        .collect()
}

fn positive(state: LegacyBorderState) -> Option<LegacyBorder> {
    match state {
        LegacyBorderState::Positive(border) => Some(border),
        LegacyBorderState::Inherit | LegacyBorderState::Nil | LegacyBorderState::Unsupported => {
            None
        }
    }
}

fn cell_borders_inherit_row(def: &TableDef) -> bool {
    def.cell_borders.iter().all(|cell| {
        [cell.top, cell.left, cell.bottom, cell.right]
            .into_iter()
            .all(|border| border == LegacyBorderState::Inherit)
    })
}

fn coherent_positive(states: impl IntoIterator<Item = LegacyBorderState>) -> Option<LegacyBorder> {
    let mut states = states.into_iter();
    let first = positive(states.next()?)?;
    states
        .all(|state| positive(state) == Some(first))
        .then_some(first)
}

fn coherent_borders(
    borders: impl IntoIterator<Item = Option<LegacyBorder>>,
) -> Option<LegacyBorder> {
    let mut borders = borders.into_iter();
    let first = borders.next()??;
    borders.all(|border| border == Some(first)).then_some(first)
}

fn row_top(def: &TableDef) -> Option<LegacyBorder> {
    match def.row_borders {
        Some(borders) => positive(borders.top),
        None => coherent_positive(def.cell_borders.iter().map(|cell| cell.top)),
    }
}

fn row_bottom(def: &TableDef) -> Option<LegacyBorder> {
    match def.row_borders {
        Some(borders) => positive(borders.bottom),
        None => coherent_positive(def.cell_borders.iter().map(|cell| cell.bottom)),
    }
}

fn row_left(def: &TableDef) -> Option<LegacyBorder> {
    match def.row_borders {
        Some(borders) => positive(borders.left),
        None => positive(def.cell_borders.first()?.left),
    }
}

fn row_right(def: &TableDef) -> Option<LegacyBorder> {
    match def.row_borders {
        Some(borders) => positive(borders.right),
        None => positive(def.cell_borders.last()?.right),
    }
}

fn row_inside_vertical(def: &TableDef) -> Option<LegacyBorder> {
    if let Some(borders) = def.row_borders {
        return positive(borders.inside_v);
    }
    let mut shared = Vec::with_capacity(def.cell_borders.len().saturating_sub(1));
    for pair in def.cell_borders.windows(2) {
        let right = positive(pair[0].right)?;
        let left = positive(pair[1].left)?;
        if right != left {
            return None;
        }
        shared.push(Some(right));
    }
    coherent_borders(shared)
}

fn row_inside_horizontal(def: &TableDef, bottom_edge: bool) -> Option<LegacyBorder> {
    if let Some(borders) = def.row_borders {
        return positive(borders.inside_h);
    }
    if bottom_edge {
        coherent_positive(def.cell_borders.iter().map(|cell| cell.bottom))
    } else {
        coherent_positive(def.cell_borders.iter().map(|cell| cell.top))
    }
}

fn coherent_table_borders(rows: &[RowBuild]) -> Option<[LegacyBorder; 6]> {
    if rows.len() < 2 {
        return None;
    }
    let first = rows.first()?.def.as_ref()?;
    let cell_count = first.rgdxa.len().checked_sub(1)?;
    if cell_count < 2 {
        return None;
    }

    let mut defs = Vec::with_capacity(rows.len());
    for row in rows {
        let def = row.def.as_ref()?;
        if row.cells.len() != cell_count
            || def.rgdxa != first.rgdxa
            || def.tcgrf.len() != cell_count
            || def.cell_borders.len() != cell_count
            || def.border_projection_blocked
            || (def.row_borders.is_some() && !cell_borders_inherit_row(def))
            || def
                .tcgrf
                .iter()
                .any(|flags| flags & (F_MERGED | F_VERT_MERGE | F_VERT_RESTART) != 0)
        {
            return None;
        }
        defs.push(def);
    }

    let top = row_top(defs.first()?)?;
    let left = coherent_borders(defs.iter().map(|def| row_left(def)))?;
    let bottom = row_bottom(defs.last()?)?;
    let right = coherent_borders(defs.iter().map(|def| row_right(def)))?;
    let inside_v = coherent_borders(defs.iter().map(|def| row_inside_vertical(def)))?;
    let mut inside_h_edges = Vec::with_capacity(defs.len() - 1);
    for pair in defs.windows(2) {
        let upper = row_inside_horizontal(pair[0], true)?;
        let lower = row_inside_horizontal(pair[1], false)?;
        if upper != lower {
            return None;
        }
        inside_h_edges.push(Some(upper));
    }
    let inside_h = coherent_borders(inside_h_edges)?;
    let borders = [top, left, bottom, right, inside_h, inside_v];
    let all_auto = borders
        .iter()
        .all(|border| border.color == LegacyBorderColor::Auto);
    let all_explicit = borders
        .iter()
        .all(|border| matches!(border.color, LegacyBorderColor::Explicit(_)));
    (all_auto || all_explicit).then_some(borders)
}

fn apply_table_borders(table: &mut Table, borders: [LegacyBorder; 6], bidi_visual: bool) {
    let mut sides = [
        TableBorderSide::Top,
        TableBorderSide::Left,
        TableBorderSide::Bottom,
        TableBorderSide::Right,
        TableBorderSide::InsideHorizontal,
        TableBorderSide::InsideVertical,
    ];
    if bidi_visual {
        sides.swap(1, 3);
    }
    for (side, border) in sides.into_iter().zip(borders) {
        if let LegacyBorderColor::Explicit(color) = border.color {
            table.border_colors.set(side, color);
        }
        table.border_sizes.set(side, border.size_eighths);
        table.border_styles.set(side, border.style);
    }

    if let LegacyBorderColor::Explicit(color) = borders[0].color {
        if borders
            .iter()
            .all(|border| border.color == LegacyBorderColor::Explicit(color))
        {
            table.border_color = Some(color);
        }
    }
    if borders
        .iter()
        .all(|border| border.size_eighths == borders[0].size_eighths)
    {
        table.border_size_eighths = Some(borders[0].size_eighths);
    }
    if borders
        .iter()
        .all(|border| border.style == borders[0].style)
    {
        table.border_style = Some(borders[0].style);
    }
}

/// Fold streamed rows into a merge-aware table.
///
/// Column geometry uses the **global set of cell-boundary x-positions**
/// (`rgdxaCenter`) across all rows, so a row with fewer cells than the table has
/// columns (e.g. a single wide header cell) gets the right colspan. Within a row,
/// `fMerged` cells fold left; `rgdxa` then yields the final span.
#[cfg(test)]
pub(crate) fn build(rows: Vec<RowBuild>) -> TableBuildOutput {
    build_with_direction(rows, false)
}

pub(crate) fn build_with_direction(rows: Vec<RowBuild>, bidi_visual: bool) -> TableBuildOutput {
    let borders = coherent_table_borders(&rows);
    let header_rows = rows.iter().take_while(|r| r.header).count();
    let fixed_layout = rows.iter().any(|row| row.def.is_some())
        && rows.iter().filter_map(|row| row.def.as_ref()).all(|def| !def.layout.auto_fit);
    let width_pct = rows.iter().filter_map(|row| row.def.as_ref()).find_map(|def| def.layout.width_pct);
    let row_min_heights_pt = rows.iter()
        .map(|row| row.def.as_ref().and_then(|def| def.layout.minimum_height_pt)).collect();

    // Global sorted set of distinct boundary positions across the whole table.
    let mut bounds: Vec<i16> = rows
        .iter()
        .filter_map(|r| r.def.as_ref())
        .flat_map(|d| d.rgdxa.iter().copied())
        .collect();
    bounds.sort_unstable();
    bounds.dedup();
    let col_widths_pct = normalized_column_widths(&rows, &bounds);
    let col_of = |x: i16| bounds.binary_search(&x).unwrap_or_else(|e| e);

    // Phase A: per-row cells, folding `fMerged` left and computing colspan/col
    // from the global boundary set (or sequential columns when no row definition).
    let mut grid: Vec<Vec<Out>> = Vec::with_capacity(rows.len());
    let row_count = rows.len();
    for (row_index, rb) in rows.into_iter().enumerate() {
        let mut out: Vec<Out> = Vec::new();
        match rb.def.filter(|d| d.rgdxa.len() >= 2) {
            Some(def) => {
                let ncell = def.rgdxa.len() - 1;
                let mut cells = rb.cells.into_iter();
                for k in 0..ncell {
                    let cell = cells.next().unwrap_or_default();
                    let borders = def.resolved_cell_borders(k, row_index, row_count);
                    let g = def.tcgrf.get(k).copied().unwrap_or(0);
                    let (left, right) = (def.rgdxa[k], def.rgdxa[k + 1]);
                    if g & F_MERGED != 0 && !out.is_empty() {
                        let last = out.last_mut().expect("non-empty");
                        last.colspan = (col_of(right).saturating_sub(last.col)).max(1) as u16;
                        last.borders.right = borders.right;
                        // A single CSS edge cannot represent differing segments.
                        if last.borders.top != borders.top {
                            last.borders.top = None;
                        }
                        if last.borders.bottom != borders.bottom {
                            last.borders.bottom = None;
                        }
                        last.blocks.extend(cell.blocks);
                        last.pagination.extend(cell.pagination);
                        last.line_spacing.extend(cell.line_spacing);
                        #[cfg(feature = "docx")]
                        last.column_break_offsets.extend(cell.column_break_offsets);
                        #[cfg(any(feature = "docx", feature = "render"))]
                        last.tab_stops.extend(cell.tab_stops);
                    } else {
                        let col = col_of(left);
                        let colspan = (col_of(right).saturating_sub(col)).max(1) as u16;
                        out.push(Out {
                            blocks: cell.blocks,
                            borders,
                            margins: Some(def.layout.margins),
                            pagination: cell.pagination,
                            line_spacing: cell.line_spacing,
                            #[cfg(feature = "docx")]
                            column_break_offsets: cell.column_break_offsets,
                            #[cfg(any(feature = "docx", feature = "render"))]
                            tab_stops: cell.tab_stops,
                            col,
                            colspan,
                            rowspan: 1,
                            tcgrf: g,
                            dropped: false,
                        });
                    }
                }
                // Extra streamed cells beyond the definition fold into the last.
                for cell in cells {
                    if let Some(last) = out.last_mut() {
                        last.blocks.extend(cell.blocks);
                        last.pagination.extend(cell.pagination);
                        last.line_spacing.extend(cell.line_spacing);
                        #[cfg(feature = "docx")]
                        last.column_break_offsets.extend(cell.column_break_offsets);
                        #[cfg(any(feature = "docx", feature = "render"))]
                        last.tab_stops.extend(cell.tab_stops);
                    }
                }
            }
            None => {
                for (k, cell) in rb.cells.into_iter().enumerate() {
                    out.push(Out {
                        blocks: cell.blocks,
                        borders: CellBorders::default(),
                        margins: None,
                        pagination: cell.pagination,
                        line_spacing: cell.line_spacing,
                        #[cfg(feature = "docx")]
                        column_break_offsets: cell.column_break_offsets,
                        #[cfg(any(feature = "docx", feature = "render"))]
                        tab_stops: cell.tab_stops,
                        col: k,
                        colspan: 1,
                        rowspan: 1,
                        tcgrf: 0,
                        dropped: false,
                    });
                }
            }
        }
        grid.push(out);
    }

    // Phase B: vertical merge (fVertRestart/fVertMerge), matched by column index.
    // open[col] = (row, idx) of the cell currently owning the vertical span.
    let mut open: std::collections::HashMap<usize, (usize, usize)> =
        std::collections::HashMap::new();
    for r in 0..grid.len() {
        for o in 0..grid[r].len() {
            let g = grid[r][o].tcgrf;
            let col = grid[r][o].col;
            let vert_merge = g & F_VERT_MERGE != 0;
            let vert_restart = g & F_VERT_RESTART != 0;
            if vert_restart {
                open.insert(col, (r, o));
            } else if vert_merge {
                if let Some(&(rr, oo)) = open.get(&col) {
                    let continued = grid[r][o].borders;
                    let owner = &mut grid[rr][oo];
                    owner.borders.bottom = continued.bottom;
                    if owner.borders.left != continued.left {
                        owner.borders.left = None;
                    }
                    if owner.borders.right != continued.right {
                        owner.borders.right = None;
                    }
                    grid[rr][oo].rowspan = grid[rr][oo].rowspan.saturating_add(1);
                    grid[r][o].dropped = true;
                }
            } else {
                open.remove(&col);
            }
        }
    }

    // Emit, skipping merged-away cells.
    let mut model_rows = Vec::with_capacity(grid.len());
    let mut cell_pagination = Vec::with_capacity(grid.len());
    let mut cell_line_spacing = Vec::with_capacity(grid.len());
    #[cfg(feature = "docx")]
    let mut cell_column_breaks = Vec::with_capacity(grid.len());
    #[cfg(any(feature = "docx", feature = "render"))]
    let mut cell_tab_stops = Vec::with_capacity(grid.len());
    for (r, row) in grid.into_iter().enumerate() {
        let is_header = r < header_rows;
        let mut cells = Vec::with_capacity(row.len());
        let mut row_pagination = Vec::with_capacity(row.len());
        let mut row_line_spacing = Vec::with_capacity(row.len());
        #[cfg(feature = "docx")]
        let mut row_column_breaks = Vec::with_capacity(row.len());
        #[cfg(any(feature = "docx", feature = "render"))]
        let mut row_tab_stops = Vec::with_capacity(row.len());
        for output in row.into_iter().filter(|output| !output.dropped) {
            debug_assert!(
                output.pagination.is_empty() || output.blocks.len() == output.pagination.len()
            );
            debug_assert!(
                output.line_spacing.is_empty() || output.blocks.len() == output.line_spacing.len()
            );
            #[cfg(feature = "docx")]
            debug_assert!(
                output.column_break_offsets.is_empty()
                    || output.blocks.len() == output.column_break_offsets.len()
            );
            #[cfg(any(feature = "docx", feature = "render"))]
            debug_assert!(
                output.tab_stops.is_empty() || output.blocks.len() == output.tab_stops.len()
            );
            row_pagination.push(output.pagination);
            row_line_spacing.push(output.line_spacing);
            #[cfg(feature = "docx")]
            row_column_breaks.push(output.column_break_offsets);
            #[cfg(any(feature = "docx", feature = "render"))]
            row_tab_stops.push(output.tab_stops);
            cells.push(Cell {
                blocks: output.blocks,
                borders: if bidi_visual {
                    CellBorders {
                        left: output.borders.right,
                        right: output.borders.left,
                        ..output.borders
                    }
                } else {
                    output.borders
                },
                col_span: output.colspan,
                row_span: output.rowspan,
                is_header,
                margins: output.margins,
                valign: match (output.tcgrf >> 7) & 3 {
                    1 => VCell::Center,
                    2 => VCell::Bottom,
                    _ => VCell::Top,
                },
                ..Default::default()
            });
        }
        model_rows.push(Row { cells });
        cell_pagination.push(row_pagination);
        cell_line_spacing.push(row_line_spacing);
        #[cfg(feature = "docx")]
        cell_column_breaks.push(row_column_breaks);
        #[cfg(any(feature = "docx", feature = "render"))]
        cell_tab_stops.push(row_tab_stops);
    }
    let mut table = Table {
        rows: model_rows,
        header_rows,
        col_widths_pct,
        fixed_layout,
        width_pct,
        row_min_heights_pt,
        bidi_visual,
        ..Default::default()
    };
    if let Some(borders) = borders {
        apply_table_borders(&mut table, borders, bidi_visual);
    }
    TableBuildOutput {
        table,
        cell_pagination,
        cell_line_spacing,
        #[cfg(feature = "docx")]
        cell_column_breaks,
        #[cfg(any(feature = "docx", feature = "render"))]
        cell_tab_stops,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Block, Color, PaginationHint, ParaProps, Paragraph, Run, TableBorderSide, TableBorderStyle,
    };
    #[cfg(any(feature = "docx", feature = "render"))]
    use crate::model::{TabAlignment, TabLeader, TabStop};

    fn cell(text: &str) -> Vec<Block> {
        vec![Block::Paragraph(Paragraph {
            props: ParaProps::default(),
            runs: vec![Run {
                text: text.to_string(),
                ..Default::default()
            }],
        })]
    }

    fn row_with_bounds(bounds: &[i16]) -> RowBuild {
        let cell_count = bounds.len().saturating_sub(1);
        RowBuild {
            cells: (0..cell_count).map(|_| CellBuild::default()).collect(),
            def: Some(TableDef {
                rgdxa: bounds.to_vec(),
                tcgrf: vec![0; cell_count],
                ..TableDef::default()
            }),
            header: false,
        }
    }

    #[test]
    fn legacy_form_layout_retains_row_minima_cell_margins_and_vertical_alignment() {
        let mut first = row_with_bounds(&[0, 1000, 4000]);
        let definition = first.def.as_mut().unwrap();
        definition.tcgrf = vec![0x0080, 0x0100];
        definition.layout.apply_sprm(0x9407, &600i16.to_le_bytes());
        definition.layout.apply_sprm(0xF614, &[2, 0x88, 0x13]);
        definition.layout.apply_sprm(0xD634, &[6, 0, 1, 0x05, 3, 40, 0]);
        let table = build(vec![first, row_with_bounds(&[0, 4000])]).table;
        assert!(table.fixed_layout);
        assert_eq!(table.width_pct, Some(1.0));
        assert_eq!(table.row_min_heights_pt, vec![Some(30.0), None]);
        assert_eq!(table.col_widths_pct, vec![0.25, 0.75]);
        assert_eq!(table.rows[1].cells[0].col_span, 2);
        assert_eq!(table.rows[0].cells[0].valign, VCell::Center);
        assert_eq!(table.rows[0].cells[1].valign, VCell::Bottom);
        assert_eq!(table.rows[0].cells[0].margins,
            Some(CellMargins { top: 40, bottom: 40, left: 108, right: 108 }));
    }

    #[test]
    fn legacy_layout_ignores_invalid_operands_and_honors_autofit() {
        let mut row = row_with_bounds(&[0, 1000, 2000]);
        let layout = &mut row.def.as_mut().unwrap().layout;
        layout.apply_sprm(0x3615, &[1]);
        layout.apply_sprm(0x3615, &[2]);
        layout.apply_sprm(0xF614, &[2, 0xC4, 0x09]); // 50%
        layout.apply_sprm(0xF614, &[2, 0xFF, 0xFF]);
        layout.apply_sprm(0xD634, &[6, 0, 1, 0xFA, 3, 40, 0]);
        layout.apply_sprm(0xD634, &[6, 0, 1, 0x0A, 3, 0xFF, 0xFF]);
        layout.apply_sprm(0x9407, &600i16.to_le_bytes());
        layout.apply_sprm(0x9407, &i16::MAX.to_le_bytes());
        assert_eq!(layout.minimum_height_pt, Some(30.0));
        layout.apply_sprm(0x9407, &(-600i16).to_le_bytes());
        let table = build(vec![row]).table;
        assert!(!table.fixed_layout);
        assert_eq!(table.width_pct, Some(0.5));
        assert_eq!(table.row_min_heights_pt, vec![None]);
        assert_eq!(table.rows[0].cells[0].margins, Some(LegacyTableLayout::default().margins));
    }

    fn brc80(size: u8, style: u8, ico: u8) -> [u8; 4] {
        [size, style, ico, 0]
    }

    fn tdef_with_tc80(bounds: &[i16], cells: &[[[u8; 4]; 4]]) -> Vec<u8> {
        assert_eq!(bounds.len(), cells.len() + 1);
        tdef_with_tc80_records(bounds, cells)
    }

    fn tdef_with_tc80_records(bounds: &[i16], cells: &[[[u8; 4]; 4]]) -> Vec<u8> {
        assert!(!bounds.is_empty());
        let cb = 2 + 2 * bounds.len() + 20 * cells.len();
        let mut operand = Vec::with_capacity(cb + 1);
        operand.extend_from_slice(&(cb as u16).to_le_bytes());
        operand.push((bounds.len() - 1) as u8);
        for &boundary in bounds {
            operand.extend_from_slice(&boundary.to_le_bytes());
        }
        for borders in cells {
            operand.extend_from_slice(&0u16.to_le_bytes());
            operand.extend_from_slice(&0u16.to_le_bytes());
            for border in borders {
                operand.extend_from_slice(border);
            }
        }
        operand
    }

    fn modeled_border(
        table: &Table,
        side: TableBorderSide,
    ) -> (Option<Color>, Option<u16>, Option<TableBorderStyle>) {
        (
            table.border_colors.get(side).or(table.border_color),
            table.border_sizes.get(side).or(table.border_size_eighths),
            table.border_styles.get(side).or(table.border_style),
        )
    }

    #[test]
    fn mixed_row_grids_preserve_individual_borders_and_row_defaults() {
        let solid = brc80(12, 0x01, 0);
        let inner = brc80(6, 0x01, 0);
        let mut row_defaults = vec![24];
        for border in [solid, solid, solid, solid, inner, inner] {
            row_defaults.extend_from_slice(&border);
        }
        let mut rows = Vec::new();
        for bounds in [vec![0, 400], vec![0, 100, 300, 400], vec![0, 100, 400]] {
            let mut row = row_with_bounds(&bounds);
            let count = bounds.len() - 1;
            let mut cells = vec![[[0; 4]; 4]; count];
            if count == 3 {
                // A direct dashed top and an explicit clear take precedence over defaults.
                cells[1][0] = brc80(8, 0x07, 6);
                cells[1][3] = [0xFF; 4];
            }
            let mut def = TableDef::parse(&tdef_with_tc80(&bounds, &cells)).unwrap();
            assert!(def.apply_table_borders80(&row_defaults));
            row.def = Some(def);
            rows.push(row);
        }
        let table = build(rows).table;
        let first = &table.rows[0].cells[0];
        assert_eq!(first.col_span, 3);
        assert_eq!(first.borders.top.unwrap().width_eighths, 12);
        assert_eq!(first.borders.bottom.unwrap().width_eighths, 6);
        assert_eq!(first.borders.left.unwrap().width_eighths, 12);
        assert_eq!(first.borders.right.unwrap().width_eighths, 12);
        let middle = &table.rows[1].cells[1];
        assert_eq!(middle.borders.top.unwrap().style, "dashed");
        assert_eq!(
            middle.borders.top.unwrap().color,
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(middle.borders.right.unwrap().style, "none");
        assert_eq!(middle.borders.left.unwrap().width_eighths, 6);
        let last = &table.rows[2].cells[1];
        assert_eq!(last.col_span, 2);
        assert_eq!(last.borders.bottom.unwrap().width_eighths, 12);
        assert_eq!(last.borders.left.unwrap().width_eighths, 6);
        assert_eq!(last.borders.right.unwrap().width_eighths, 12);
    }

    #[test]
    fn cell_borders_follow_horizontal_and_vertical_merge_perimeters() {
        let top = brc80(4, 0x01, 0);
        let bottom = brc80(12, 0x01, 0);
        let left = brc80(6, 0x03, 0);
        let right = brc80(8, 0x07, 0);
        let mut first = TableDef::parse(&tdef_with_tc80(
            &[0, 100, 200],
            &[[top, left, top, top], [top, top, top, right]],
        ))
        .unwrap();
        first.tcgrf = vec![F_VERT_RESTART, F_MERGED];
        let mut second =
            TableDef::parse(&tdef_with_tc80(&[0, 200], &[[top, left, bottom, right]])).unwrap();
        second.tcgrf = vec![F_VERT_MERGE];
        let table = build(vec![
            RowBuild {
                cells: vec![CellBuild::default(), CellBuild::default()],
                def: Some(first),
                header: false,
            },
            RowBuild {
                cells: vec![CellBuild::default()],
                def: Some(second),
                header: false,
            },
        ])
        .table;
        let cell = &table.rows[0].cells[0];
        assert_eq!((cell.col_span, cell.row_span), (2, 2));
        assert_eq!(cell.borders.top.unwrap().width_eighths, 4);
        assert_eq!(cell.borders.bottom.unwrap().width_eighths, 12);
        assert_eq!(cell.borders.left.unwrap().style, "double");
        assert_eq!(cell.borders.right.unwrap().style, "dashed");
        assert!(table.rows[1].cells.is_empty());
    }

    #[test]
    fn parse_tdef_two_cells() {
        // cb, itcMac=2, rgdxa[3] = {0, 100, 200}, rgTc[2] TC80 (only tcgrf set).
        let mut op = vec![48u8, 0u8, 2u8];
        for v in [0i16, 100, 200] {
            op.extend_from_slice(&v.to_le_bytes());
        }
        // TC80 #0: tcgrf=0, then 18 padding; TC80 #1: tcgrf=fMerged.
        op.extend_from_slice(&0u16.to_le_bytes());
        op.extend_from_slice(&[0u8; 18]);
        op.extend_from_slice(&F_MERGED.to_le_bytes());
        op.extend_from_slice(&[0u8; 18]);
        let def = TableDef::parse(&op).unwrap();
        assert_eq!(def.rgdxa, vec![0, 100, 200]);
        assert_eq!(def.tcgrf, vec![0, F_MERGED]);
    }

    #[test]
    fn parse_tdef_rejects_incomplete_decreasing_and_trailing_tc80_data() {
        let complete = tdef_with_tc80(
            &[0, 100, 200],
            &[
                [[0; 4], [0; 4], [0; 4], [0; 4]],
                [[0; 4], [0; 4], [0; 4], [0; 4]],
            ],
        );

        assert!(TableDef::parse(&complete[..complete.len() - 1]).is_none());

        let mut decreasing = complete.clone();
        decreasing[5..7].copy_from_slice(&(-1i16).to_le_bytes());
        assert!(TableDef::parse(&decreasing).is_none());

        let mut trailing = complete;
        trailing.push(0);
        assert!(TableDef::parse(&trailing).is_none());
    }

    #[test]
    fn spec_valid_variable_tc80_counts_and_equal_boundaries_preserve_structure_only() {
        let border = brc80(8, 0x01, 6);
        let default_cell = [[0; 4]; 4];
        let cases = [
            tdef_with_tc80_records(&[0, 100, 200], &[default_cell]),
            tdef_with_tc80_records(&[0, 100, 200], &[default_cell, default_cell, default_cell]),
            tdef_with_tc80_records(
                &[0, 0, 200],
                &[
                    [border, border, border, border],
                    [border, border, border, border],
                ],
            ),
        ];

        for operand in cases {
            let definition =
                TableDef::parse(&operand).expect("spec-valid table definition must parse");
            assert_eq!(definition.tcgrf.len(), 2);
            let table = build(vec![
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
            .table;

            assert_eq!(table.rows.len(), 2);
            assert_eq!(table.rows[0].cells.len(), 2);
            for side in [
                TableBorderSide::Top,
                TableBorderSide::Left,
                TableBorderSide::Bottom,
                TableBorderSide::Right,
                TableBorderSide::InsideHorizontal,
                TableBorderSide::InsideVertical,
            ] {
                assert_eq!(modeled_border(&table, side), (None, None, None));
            }
        }
    }

    #[test]
    fn build_recovers_coherent_six_way_tc80_borders() {
        let top = brc80(2, 0x01, 2);
        let left = brc80(4, 0x03, 3);
        let bottom = brc80(6, 0x06, 4);
        let right = brc80(8, 0x07, 5);
        let inside_h = brc80(10, 0x01, 6);
        let inside_v = brc80(12, 0x03, 7);
        let first = TableDef::parse(&tdef_with_tc80(
            &[0, 100, 200],
            &[
                [top, left, inside_h, inside_v],
                [top, inside_v, inside_h, right],
            ],
        ))
        .expect("complete first-row TC80");
        let second = TableDef::parse(&tdef_with_tc80(
            &[0, 100, 200],
            &[
                [inside_h, left, bottom, inside_v],
                [inside_h, inside_v, bottom, right],
            ],
        ))
        .expect("complete second-row TC80");

        let table = build(vec![
            RowBuild {
                cells: vec![CellBuild::default(), CellBuild::default()],
                def: Some(first),
                header: false,
            },
            RowBuild {
                cells: vec![CellBuild::default(), CellBuild::default()],
                def: Some(second),
                header: false,
            },
        ])
        .table;

        let expected = [
            (
                TableBorderSide::Top,
                Color::rgb(0, 0, 0xFF),
                2,
                TableBorderStyle::Single,
            ),
            (
                TableBorderSide::Left,
                Color::rgb(0, 0xFF, 0xFF),
                4,
                TableBorderStyle::Double,
            ),
            (
                TableBorderSide::Bottom,
                Color::rgb(0, 0xFF, 0),
                6,
                TableBorderStyle::Dotted,
            ),
            (
                TableBorderSide::Right,
                Color::rgb(0xFF, 0, 0xFF),
                8,
                TableBorderStyle::Dashed,
            ),
            (
                TableBorderSide::InsideHorizontal,
                Color::rgb(0xFF, 0, 0),
                10,
                TableBorderStyle::Single,
            ),
            (
                TableBorderSide::InsideVertical,
                Color::rgb(0xFF, 0xFF, 0),
                12,
                TableBorderStyle::Double,
            ),
        ];
        for (side, color, size, style) in expected {
            assert_eq!(
                modeled_border(&table, side),
                (Some(color), Some(size), Some(style)),
                "side={side:?}"
            );
        }
    }

    #[test]
    fn tc80_auto_color_and_subminimum_width_project_without_inventing_color() {
        let border = brc80(0, 0x01, 0);
        let definition = TableDef::parse(&tdef_with_tc80(
            &[0, 100, 200],
            &[
                [border, border, border, border],
                [border, border, border, border],
            ],
        ))
        .expect("complete auto-color TC80");
        let table = build(vec![
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
        .table;

        for side in [
            TableBorderSide::Top,
            TableBorderSide::Left,
            TableBorderSide::Bottom,
            TableBorderSide::Right,
            TableBorderSide::InsideHorizontal,
            TableBorderSide::InsideVertical,
        ] {
            assert_eq!(
                modeled_border(&table, side),
                (None, Some(2), Some(TableBorderStyle::Single)),
                "side={side:?}"
            );
        }
    }

    #[test]
    fn tc80_conflicts_and_merges_decline_border_projection_without_losing_structure() {
        let border = brc80(8, 0x01, 6);
        let conflicting = brc80(8, 0x01, 2);
        let conflict = TableDef::parse(&tdef_with_tc80(
            &[0, 100, 200],
            &[
                [border, border, border, border],
                [border, conflicting, border, border],
            ],
        ))
        .expect("complete conflicting TC80");
        let mut merged = TableDef::parse(&tdef_with_tc80(
            &[0, 100, 200],
            &[
                [border, border, border, border],
                [border, border, border, border],
            ],
        ))
        .expect("complete merged TC80");
        merged.tcgrf[1] = F_MERGED;

        for (definition, expected_cells) in [(conflict, 2), (merged, 1)] {
            let table = build(vec![
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
            .table;

            assert_eq!(table.rows[0].cells.len(), expected_cells);
            for side in [
                TableBorderSide::Top,
                TableBorderSide::Left,
                TableBorderSide::Bottom,
                TableBorderSide::Right,
                TableBorderSide::InsideHorizontal,
                TableBorderSide::InsideVertical,
            ] {
                assert_eq!(modeled_border(&table, side), (None, None, None));
            }
        }
    }

    #[test]
    fn row_wide_borders_decline_when_tc80_has_nil_or_positive_cell_overrides() {
        let row_border = brc80(8, 0x01, 6);
        let conflicting_cell = brc80(8, 0x01, 2);
        let mut row_operand = vec![24];
        for _ in 0..6 {
            row_operand.extend_from_slice(&row_border);
        }

        for cell_border in [conflicting_cell, [0xFF; 4]] {
            let mut definition = TableDef::parse(&tdef_with_tc80(
                &[0, 100, 200],
                &[
                    [cell_border, cell_border, cell_border, cell_border],
                    [cell_border, cell_border, cell_border, cell_border],
                ],
            ))
            .expect("complete TC80");
            assert!(definition.apply_table_borders80(&row_operand));
            let table = build(vec![
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
            .table;

            for side in [
                TableBorderSide::Top,
                TableBorderSide::Left,
                TableBorderSide::Bottom,
                TableBorderSide::Right,
                TableBorderSide::InsideHorizontal,
                TableBorderSide::InsideVertical,
            ] {
                assert_eq!(modeled_border(&table, side), (None, None, None));
            }
        }
    }

    #[test]
    fn horizontal_merge_colspan() {
        // Row: cell A, cell B(fMerged → folds into A) → one cell, colspan 2.
        let def = TableDef {
            rgdxa: vec![0, 100, 200],
            tcgrf: vec![0, F_MERGED],
            ..TableDef::default()
        };
        let t = build(vec![RowBuild {
            cells: vec![
                CellBuild {
                    blocks: cell("A"),
                    ..CellBuild::default()
                },
                CellBuild {
                    blocks: cell("B"),
                    ..CellBuild::default()
                },
            ],
            def: Some(def),
            header: false,
        }])
        .table;
        assert_eq!(t.rows[0].cells.len(), 1);
        assert_eq!(t.rows[0].cells[0].col_span, 2);
    }

    #[test]
    fn mixed_row_grids_preserve_global_column_proportions() {
        let table = build(vec![
            row_with_bounds(&[-500, 500, 3500]),
            row_with_bounds(&[-500, 1500, 3500]),
        ])
        .table;

        assert_eq!(table.col_widths_pct, vec![0.25, 0.25, 0.5]);
        assert_eq!(
            table.rows[0]
                .cells
                .iter()
                .map(|cell| cell.col_span)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            table.rows[1]
                .cells
                .iter()
                .map(|cell| cell.col_span)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
    }

    #[test]
    fn unusable_row_geometry_keeps_content_sized_width_fallback() {
        let cases = [
            vec![RowBuild {
                cells: vec![CellBuild::default()],
                def: None,
                header: false,
            }],
            vec![row_with_bounds(&[0, 0, 100])],
            vec![row_with_bounds(&[0, 200, 100])],
            vec![
                row_with_bounds(&[0, 100, 300]),
                row_with_bounds(&[10, 100, 300]),
            ],
        ];

        for rows in cases {
            assert!(build(rows).table.col_widths_pct.is_empty());
        }
    }

    #[test]
    fn vertical_merge_rowspan() {
        // Two rows, column 0: top fVertRestart, bottom fVertMerge → rowspan 2,
        // the continuation cell dropped.
        let top = RowBuild {
            cells: vec![
                CellBuild {
                    blocks: cell("X"),
                    ..CellBuild::default()
                },
                CellBuild {
                    blocks: cell("a"),
                    ..CellBuild::default()
                },
            ],
            def: Some(TableDef {
                rgdxa: vec![0, 100, 200],
                tcgrf: vec![F_VERT_RESTART, 0],
                ..TableDef::default()
            }),
            header: false,
        };
        let bot = RowBuild {
            cells: vec![
                CellBuild {
                    blocks: cell(""),
                    ..CellBuild::default()
                },
                CellBuild {
                    blocks: cell("b"),
                    ..CellBuild::default()
                },
            ],
            def: Some(TableDef {
                rgdxa: vec![0, 100, 200],
                tcgrf: vec![F_VERT_MERGE, 0],
                ..TableDef::default()
            }),
            header: false,
        };
        let t = build(vec![top, bot]).table;
        assert_eq!(t.rows[0].cells[0].row_span, 2);
        assert_eq!(t.rows[1].cells.len(), 1); // continuation dropped
    }

    #[test]
    fn merge_resolution_keeps_cell_pagination_aligned() {
        let a = PaginationHint {
            keep_next: true,
            ..PaginationHint::default()
        };
        let b = PaginationHint {
            keep_lines: true,
            ..PaginationHint::default()
        };
        let c = PaginationHint {
            widow_control: true,
            ..PaginationHint::default()
        };
        let extra = PaginationHint {
            keep_next: true,
            keep_lines: true,
            ..PaginationHint::default()
        };
        let d = PaginationHint {
            keep_next: true,
            widow_control: true,
            ..PaginationHint::default()
        };
        let e = PaginationHint {
            keep_lines: true,
            widow_control: true,
            ..PaginationHint::default()
        };
        let dropped = PaginationHint {
            keep_next: true,
            keep_lines: true,
            widow_control: true,
        };
        let built_cell = |text: &str, pagination, line_spacing| CellBuild {
            blocks: cell(text),
            pagination: vec![Some(pagination)],
            line_spacing: vec![Some(line_spacing)],
            #[cfg(feature = "docx")]
            column_break_offsets: vec![vec![usize::from(text.as_bytes()[0])]],
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops: vec![vec![TabStop {
                position_pt: f32::from(text.as_bytes()[0]),
                alignment: TabAlignment::Left,
                leader: TabLeader::None,
            }]],
        };

        let built = build(vec![
            RowBuild {
                cells: vec![
                    built_cell("A", a, LineSpacingHint::Exact(1.0)),
                    built_cell("B", b, LineSpacingHint::AtLeast(2.0)),
                    built_cell("C", c, LineSpacingHint::Exact(3.0)),
                    built_cell("extra", extra, LineSpacingHint::AtLeast(4.0)),
                ],
                def: Some(TableDef {
                    rgdxa: vec![0, 100, 200, 300],
                    tcgrf: vec![0, F_MERGED, F_VERT_RESTART],
                    ..TableDef::default()
                }),
                header: false,
            },
            RowBuild {
                cells: vec![
                    built_cell("D", d, LineSpacingHint::Exact(5.0)),
                    built_cell("E", e, LineSpacingHint::AtLeast(6.0)),
                    built_cell("dropped", dropped, LineSpacingHint::Exact(7.0)),
                ],
                def: Some(TableDef {
                    rgdxa: vec![0, 100, 200, 300],
                    tcgrf: vec![0, 0, F_VERT_MERGE],
                    ..TableDef::default()
                }),
                header: false,
            },
        ]);

        assert_eq!(built.table.rows[0].cells.len(), 2);
        assert_eq!(built.table.rows[0].cells[0].col_span, 2);
        assert_eq!(built.table.rows[0].cells[1].row_span, 2);
        assert_eq!(built.table.rows[1].cells.len(), 2);
        assert_eq!(
            built.cell_pagination,
            vec![
                vec![vec![Some(a), Some(b)], vec![Some(c), Some(extra)]],
                vec![vec![Some(d)], vec![Some(e)]],
            ]
        );
        assert_eq!(
            built.cell_line_spacing,
            vec![
                vec![
                    vec![
                        Some(LineSpacingHint::Exact(1.0)),
                        Some(LineSpacingHint::AtLeast(2.0)),
                    ],
                    vec![
                        Some(LineSpacingHint::Exact(3.0)),
                        Some(LineSpacingHint::AtLeast(4.0)),
                    ],
                ],
                vec![
                    vec![Some(LineSpacingHint::Exact(5.0))],
                    vec![Some(LineSpacingHint::AtLeast(6.0))],
                ],
            ]
        );
        #[cfg(feature = "docx")]
        assert_eq!(
            built.cell_column_breaks,
            vec![
                vec![
                    vec![vec![usize::from(b'A')], vec![usize::from(b'B')]],
                    vec![vec![usize::from(b'C')], vec![usize::from(b'e')]],
                ],
                vec![vec![vec![usize::from(b'D')]], vec![vec![usize::from(b'E')]],],
            ]
        );
        #[cfg(any(feature = "docx", feature = "render"))]
        {
            let tabs = |label: u8| {
                vec![TabStop {
                    position_pt: f32::from(label),
                    alignment: TabAlignment::Left,
                    leader: TabLeader::None,
                }]
            };
            assert_eq!(
                built.cell_tab_stops,
                vec![
                    vec![vec![tabs(b'A'), tabs(b'B')], vec![tabs(b'C'), tabs(b'e')]],
                    vec![vec![tabs(b'D')], vec![tabs(b'E')]],
                ]
            );
        }
    }

    #[test]
    fn doc_vertical_merge_row_span_saturates_instead_of_overflowing() {
        let mut rows = Vec::with_capacity(u16::MAX as usize + 1);
        rows.push(RowBuild {
            cells: vec![CellBuild::default()],
            def: Some(TableDef {
                rgdxa: vec![0, 100],
                tcgrf: vec![F_VERT_RESTART],
                ..TableDef::default()
            }),
            header: false,
        });
        rows.extend((0..u16::MAX as usize).map(|_| RowBuild {
            cells: vec![CellBuild::default()],
            def: Some(TableDef {
                rgdxa: vec![0, 100],
                tcgrf: vec![F_VERT_MERGE],
                ..TableDef::default()
            }),
            header: false,
        }));

        let table = build(rows).table;

        assert_eq!(table.rows[0].cells[0].row_span, u16::MAX);
    }
}
