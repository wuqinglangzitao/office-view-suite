//! Table reconstruction: parse `sprmTDefTable` row definitions and fold streamed
//! rows/cells into a merge-aware [`model::Table`] (colspan from `fMerged`,
//! rowspan from `fVertRestart`/`fVertMerge`, matched by column).
//!
//! Reference: [MS-DOC] 2.4.3 (cell boundaries), 2.9.349 (TDefTableOperand),
//! 2.9.330 (TC80).

#[cfg(feature = "docx")]
use crate::model::TableCellColumnBreakHints;
use crate::model::{
    Block, Cell, Color, LineSpacingHint, PaginationHint, Row, Table, TableBorderSide,
    TableBorderStyle, TableCellLineSpacingHints, TableCellPaginationHints,
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

/// A parsed row definition (the `sprmTDefTable` operand carried on the TTP).
#[derive(Debug, Clone, Default)]
pub(crate) struct TableDef {
    /// Cell-boundary x-positions in twips (`itcMac + 1` entries).
    pub rgdxa: Vec<i16>,
    /// Per-cell `TC80.tcgrf` (merge flags); `itcMac` entries.
    pub tcgrf: Vec<u16>,
    /// Complete direct TC80 border state for each cell.
    cell_borders: Vec<LegacyCellBorders>,
    /// A later direct row-wide border modifier, if one was applied.
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
    for rb in rows {
        let mut out: Vec<Out> = Vec::new();
        match rb.def.filter(|d| d.rgdxa.len() >= 2) {
            Some(def) => {
                let ncell = def.rgdxa.len() - 1;
                let mut cells = rb.cells.into_iter();
                for k in 0..ncell {
                    let cell = cells.next().unwrap_or_default();
                    let g = def.tcgrf.get(k).copied().unwrap_or(0);
                    let (left, right) = (def.rgdxa[k], def.rgdxa[k + 1]);
                    if g & F_MERGED != 0 && !out.is_empty() {
                        let last = out.last_mut().expect("non-empty");
                        last.colspan = (col_of(right).saturating_sub(last.col)).max(1) as u16;
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
                col_span: output.colspan,
                row_span: output.rowspan,
                is_header,
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
