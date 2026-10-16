//! `DocModel` → `.docx` (OOXML WordprocessingML) — the inverse of the `docx`
//! reader. Each mapping is the exact dual of [`crate::docx`] so the round-trip
//! `read → write → read` preserves the model:
//!
//! * heading level `h` → `<w:outlineLvl w:val="h-1"/>` (the reader recovers
//!   `outline+1`); a heading suppresses list rendering, as in the reader.
//! * list item → `<w:numPr>` referencing a synthetic `numbering.xml` (numId 1 =
//!   ordered/decimal, numId 2 = bullet, all nine levels declared).
//! * alignment → `<w:jc>`; char toggles → `<w:b/> <w:i/> <w:strike/> <w:vanish/>
//!   <w:u w:val="single"/>`.
//! * table merges → `<w:gridSpan>` + `<w:vMerge>` with reconstructed continuation
//!   cells (the reader dropped them on read; we re-insert them).
//! * image → a `media/` part + `<a:blip r:embed>`; external hyperlink → a
//!   relationship + `<w:hyperlink r:id>`; internal hyperlink →
//!   `<w:hyperlink w:anchor>`.

use std::collections::HashMap;

use super::opc::{Package, Rel};
use super::{esc_attr, esc_text};
use crate::annotation::{filename_field_syntax, merge_field_syntax};
use crate::docx::{
    computed_preserved_listnum_start_result, computed_preserved_note_local_ref_result,
    computed_preserved_note_local_style_ref_result, computed_preserved_sequence_reset_result,
    computed_span_free_table_formula_result, preserved_note_local_ref_target,
    preserved_note_local_style_ref_target, supports_computed_symbol_field_syntax,
    supports_context_free_display_field_syntax, supports_context_free_fill_in_field_syntax,
    supports_context_free_formula_field_syntax, supports_context_free_if_compare_field_syntax,
    supports_formula_field_syntax, supports_preserved_document_info_field_syntax,
    supports_quote_field_syntax, supports_reference_index_marker_syntax,
    supports_revision_number_field_syntax, supports_toc_entry_field_syntax,
};
use crate::model::{
    normalize_field_instruction, referenceable_bookmark_name, Align, AuthoredComment,
    AuthoredContentControl, AuthoredNote, AuthoredRevision, Block, CellMargins, CharProps, Chart,
    ChartKind, ChartSeries, ChartShape, Color, DocSetup, FieldRole, FieldUnsupportedReason, Image,
    Indent, LineSpacingHint, NoteWritePayload, PaginationHint, ParaProps, Paragraph,
    ParagraphStyle, RunningBlockPaginationHints, RunningSurfaceColumnBreakHints,
    RunningSurfaceDistanceHints, RunningSurfaceLineSpacingHints, RunningSurfacePaginationHints,
    RunningSurfaceTabStopHints, RunningSurfaceTableCellTabStopHints,
    RunningSurfaceTableLayoutHints, RunningTableLayoutHints, SectionBreakKind,
    SectionColumnLayoutHints, SectionSetup, Spacing, TabAlignment, TabLeader, TabStop, Table,
    TableBorderSide, TableBorderStyle, TableCellColumnBreakHints, TableCellLineSpacingHints,
    TableCellNestedPaginationHints, TableCellPaginationHints, TableCellTabStopHints,
    TablePaginationHints, TableRowPaginationHint, VertAlign, WebExtensionTaskPane, MAX_TAB_STOPS,
};
use crate::{FieldKind, NoteKind, RevisionKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HyperlinkWriteTarget<'a> {
    External(&'a str),
    Anchor(&'a str),
    Invalid,
}

fn hyperlink_write_target(url: &str) -> HyperlinkWriteTarget<'_> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return HyperlinkWriteTarget::Invalid;
    }
    let Some(anchor) = trimmed.strip_prefix('#') else {
        return HyperlinkWriteTarget::External(url);
    };
    if anchor.starts_with('#') || !referenceable_bookmark_name(anchor) {
        HyperlinkWriteTarget::Invalid
    } else {
        HyperlinkWriteTarget::Anchor(anchor)
    }
}

/// `Color` → 6-hex `RRGGBB` for OOXML `w:val`.
fn hex(c: Color) -> String {
    format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

fn table_border_color(table: &Table, side: TableBorderSide) -> String {
    table
        .border_colors
        .get(side)
        .or(table.border_color)
        .map(hex)
        .unwrap_or_else(|| "auto".to_string())
}

fn table_border_size(table: &Table, side: TableBorderSide) -> u16 {
    table
        .border_sizes
        .get(side)
        .or(table.border_size_eighths)
        .unwrap_or(4)
        .max(1)
}

fn table_border_style(table: &Table, side: TableBorderSide) -> TableBorderStyle {
    table
        .border_styles
        .get(side)
        .or(table.border_style)
        .unwrap_or(TableBorderStyle::Single)
}

/// Points → twips (1/20 pt), the OOXML measurement unit.
fn pt_twips(pt: f32) -> i64 {
    (pt * 20.0).round() as i64
}

#[derive(Clone, Copy, Default)]
struct SectionColumnWriteHint<'a> {
    gap_pt: Option<f32>,
    layout: Option<&'a SectionColumnLayoutHints>,
    separator: bool,
    rtl: bool,
}

#[derive(Clone, Copy, Default)]
struct SectionWriteHint<'a> {
    columns: SectionColumnWriteHint<'a>,
    running_surface_distances: RunningSurfaceDistanceHints,
    running_line_spacing: Option<&'a RunningSurfaceLineSpacingHints>,
    running_pagination: Option<&'a RunningSurfacePaginationHints>,
    running_tab_stops: Option<&'a RunningSurfaceTabStopHints>,
    running_table_cell_tab_stops: Option<&'a RunningSurfaceTableCellTabStopHints>,
    running_table_layout: Option<&'a RunningSurfaceTableLayoutHints>,
    running_column_breaks: Option<&'a RunningSurfaceColumnBreakHints>,
}

#[derive(Clone, Copy, Default)]
struct AlignedRunningSectionHints<'a> {
    distances: Option<&'a [RunningSurfaceDistanceHints]>,
    line_spacing: Option<&'a [RunningSurfaceLineSpacingHints]>,
    pagination: Option<&'a [RunningSurfacePaginationHints]>,
    tab_stops: Option<&'a [RunningSurfaceTabStopHints]>,
    table_cell_tab_stops: Option<&'a [RunningSurfaceTableCellTabStopHints]>,
    table_layout: Option<&'a [RunningSurfaceTableLayoutHints]>,
    column_breaks: Option<&'a [RunningSurfaceColumnBreakHints]>,
}

#[derive(Clone, Copy, Default)]
struct RunningBlockWriteHints<'a> {
    line_spacing: Option<&'a [Option<LineSpacingHint>]>,
    pagination: Option<&'a RunningBlockPaginationHints>,
    tab_stops: Option<&'a [Vec<TabStop>]>,
    table_cell_line_spacing: Option<&'a [TableCellLineSpacingHints]>,
    table_cell_tab_stops: Option<&'a [TableCellTabStopHints]>,
    table_layout: Option<&'a RunningTableLayoutHints>,
    column_break_offsets: Option<&'a [Vec<usize>]>,
}

#[derive(Clone, Copy, Default)]
struct RunningBlockSlotWriteHints<'a> {
    line_spacing: Option<LineSpacingHint>,
    pagination: Option<PaginationHint>,
    tab_stops: Option<&'a [TabStop]>,
    table_row_pagination: Option<&'a [TableRowPaginationHint]>,
    table_cell_pagination: Option<&'a TableCellPaginationHints>,
    table_cell_line_spacing: Option<&'a TableCellLineSpacingHints>,
    table_cell_tab_stops: Option<&'a TableCellTabStopHints>,
    table_cell_column_breaks: Option<&'a TableCellColumnBreakHints>,
    table_nested: Option<&'a TableCellNestedPaginationHints>,
    column_break_offsets: Option<&'a [usize]>,
}

#[derive(Clone, Copy, Default)]
struct ParagraphWriteHints<'a> {
    line_spacing: Option<LineSpacingHint>,
    pagination: Option<PaginationHint>,
    tab_stops: Option<&'a [TabStop]>,
    column_break_offsets: Option<&'a [usize]>,
    note_payloads: Option<&'a [Option<NoteWritePayload>]>,
}

#[derive(Clone, Copy, Default)]
struct TableWriteHints<'a> {
    row_pagination: Option<&'a [TableRowPaginationHint]>,
    cell_pagination: Option<&'a TableCellPaginationHints>,
    cell_line_spacing: Option<&'a TableCellLineSpacingHints>,
    cell_column_breaks: Option<&'a TableCellColumnBreakHints>,
    nested_tables: Option<&'a TableCellNestedPaginationHints>,
    cell_tab_stops: Option<&'a TableCellTabStopHints>,
}

#[derive(Clone, Copy, Default)]
struct CellWriteHints<'a> {
    pagination: Option<&'a [Option<PaginationHint>]>,
    line_spacing: Option<&'a [Option<LineSpacingHint>]>,
    column_break_offsets: Option<&'a [Vec<usize>]>,
    nested_tables: Option<&'a [Option<TablePaginationHints>]>,
    tab_stops: Option<&'a [Vec<TabStop>]>,
}

#[derive(Clone, Copy)]
pub(crate) struct SourceWriteHints<'a> {
    pub(crate) gaps: &'a [Option<f32>],
    pub(crate) layouts: &'a [Option<SectionColumnLayoutHints>],
    pub(crate) separators: &'a [bool],
    pub(crate) rtl: &'a [bool],
    pub(crate) final_gap: Option<f32>,
    pub(crate) final_layout: Option<&'a SectionColumnLayoutHints>,
    pub(crate) final_separator: bool,
    pub(crate) final_rtl: bool,
    pub(crate) running_surface_distances: &'a [RunningSurfaceDistanceHints],
    pub(crate) running_line_spacing: &'a [RunningSurfaceLineSpacingHints],
    pub(crate) running_pagination: &'a [RunningSurfacePaginationHints],
    pub(crate) running_tab_stops: &'a [RunningSurfaceTabStopHints],
    pub(crate) running_table_cell_tab_stops: &'a [RunningSurfaceTableCellTabStopHints],
    pub(crate) running_table_layout: &'a [RunningSurfaceTableLayoutHints],
    pub(crate) running_column_break_offsets: &'a [RunningSurfaceColumnBreakHints],
    pub(crate) note_payloads: &'a [Vec<Option<NoteWritePayload>>],
    pub(crate) paragraph_line_spacing: &'a [Option<LineSpacingHint>],
    pub(crate) paragraph_pagination: &'a [PaginationHint],
    pub(crate) paragraph_tab_stops: &'a [Vec<TabStop>],
    pub(crate) column_break_offsets: &'a [Vec<usize>],
    pub(crate) table_cell_column_break_offsets: &'a [TableCellColumnBreakHints],
    pub(crate) table_row_pagination: &'a [Vec<TableRowPaginationHint>],
    pub(crate) table_cell_pagination: &'a [TableCellPaginationHints],
    pub(crate) table_cell_line_spacing: &'a [TableCellLineSpacingHints],
    pub(crate) table_nested_pagination: &'a [TableCellNestedPaginationHints],
    pub(crate) table_cell_tab_stops: &'a [TableCellTabStopHints],
}

impl<'a> SourceWriteHints<'a> {
    fn aligned_distances(self, section_count: usize) -> Option<&'a [RunningSurfaceDistanceHints]> {
        (self.running_surface_distances.len() == section_count)
            .then_some(self.running_surface_distances)
    }

    fn aligned_running_line_spacing(
        self,
        section_count: usize,
    ) -> Option<&'a [RunningSurfaceLineSpacingHints]> {
        (self.running_line_spacing.len() == section_count).then_some(self.running_line_spacing)
    }

    fn aligned_running_pagination(
        self,
        section_count: usize,
    ) -> Option<&'a [RunningSurfacePaginationHints]> {
        (self.running_pagination.len() == section_count).then_some(self.running_pagination)
    }

    fn aligned_running_tab_stops(
        self,
        section_count: usize,
    ) -> Option<&'a [RunningSurfaceTabStopHints]> {
        (self.running_tab_stops.len() == section_count).then_some(self.running_tab_stops)
    }

    fn aligned_running_table_cell_tab_stops(
        self,
        section_count: usize,
    ) -> Option<&'a [RunningSurfaceTableCellTabStopHints]> {
        (self.running_table_cell_tab_stops.len() == section_count)
            .then_some(self.running_table_cell_tab_stops)
    }

    fn aligned_running_table_layout(
        self,
        section_count: usize,
    ) -> Option<&'a [RunningSurfaceTableLayoutHints]> {
        (self.running_table_layout.len() == section_count).then_some(self.running_table_layout)
    }

    fn aligned_running_column_breaks(
        self,
        section_count: usize,
    ) -> Option<&'a [RunningSurfaceColumnBreakHints]> {
        (self.running_column_break_offsets.len() == section_count)
            .then_some(self.running_column_break_offsets)
    }

    fn aligned_paragraph_line_spacing(
        self,
        block_count: usize,
    ) -> Option<&'a [Option<LineSpacingHint>]> {
        (self.paragraph_line_spacing.len() == block_count).then_some(self.paragraph_line_spacing)
    }

    fn aligned_paragraph_pagination(self, block_count: usize) -> Option<&'a [PaginationHint]> {
        (self.paragraph_pagination.len() == block_count).then_some(self.paragraph_pagination)
    }

    fn aligned_paragraph_tab_stops(self, block_count: usize) -> Option<&'a [Vec<TabStop>]> {
        (self.paragraph_tab_stops.len() == block_count).then_some(self.paragraph_tab_stops)
    }

    fn aligned_column_break_offsets(self, block_count: usize) -> Option<&'a [Vec<usize>]> {
        (self.column_break_offsets.len() == block_count).then_some(self.column_break_offsets)
    }

    fn aligned_note_payloads(
        self,
        blocks: &[Block],
    ) -> Option<&'a [Vec<Option<NoteWritePayload>>]> {
        if self.note_payloads.len() != blocks.len() {
            return None;
        }
        blocks
            .iter()
            .zip(self.note_payloads)
            .all(|(block, payloads)| match block {
                Block::Paragraph(paragraph) => {
                    payloads.len() == paragraph.runs.len()
                        && paragraph
                            .runs
                            .iter()
                            .zip(payloads)
                            .all(|(run, payload)| payload.is_none() || run.note.is_some())
                }
                _ => payloads.is_empty(),
            })
            .then_some(self.note_payloads)
    }

    fn aligned_table_row_pagination(
        self,
        block_count: usize,
    ) -> Option<&'a [Vec<TableRowPaginationHint>]> {
        (self.table_row_pagination.len() == block_count).then_some(self.table_row_pagination)
    }

    fn aligned_table_cell_line_spacing(
        self,
        block_count: usize,
    ) -> Option<&'a [TableCellLineSpacingHints]> {
        (self.table_cell_line_spacing.len() == block_count).then_some(self.table_cell_line_spacing)
    }

    fn aligned_table_cell_column_break_offsets(
        self,
        block_count: usize,
    ) -> Option<&'a [TableCellColumnBreakHints]> {
        (self.table_cell_column_break_offsets.len() == block_count)
            .then_some(self.table_cell_column_break_offsets)
    }

    fn aligned_table_nested_pagination(
        self,
        block_count: usize,
    ) -> Option<&'a [TableCellNestedPaginationHints]> {
        (self.table_nested_pagination.len() == block_count).then_some(self.table_nested_pagination)
    }

    fn aligned_table_cell_pagination(
        self,
        block_count: usize,
    ) -> Option<&'a [TableCellPaginationHints]> {
        (self.table_cell_pagination.len() == block_count).then_some(self.table_cell_pagination)
    }

    fn aligned_table_cell_tab_stops(
        self,
        block_count: usize,
    ) -> Option<&'a [TableCellTabStopHints]> {
        (self.table_cell_tab_stops.len() == block_count).then_some(self.table_cell_tab_stops)
    }

    fn for_block(
        &self,
        block_index: usize,
        section_index: usize,
        running: AlignedRunningSectionHints<'a>,
    ) -> SectionWriteHint<'a> {
        SectionWriteHint {
            columns: SectionColumnWriteHint {
                gap_pt: self.gaps.get(block_index).copied().flatten(),
                layout: self.layouts.get(block_index).and_then(Option::as_ref),
                separator: self.separators.get(block_index).copied().unwrap_or(false),
                rtl: self.rtl.get(block_index).copied().unwrap_or(false),
            },
            running_surface_distances: running
                .distances
                .and_then(|values| values.get(section_index))
                .copied()
                .unwrap_or_default(),
            running_line_spacing: running
                .line_spacing
                .and_then(|values| values.get(section_index)),
            running_pagination: running
                .pagination
                .and_then(|values| values.get(section_index)),
            running_tab_stops: running
                .tab_stops
                .and_then(|values| values.get(section_index)),
            running_table_cell_tab_stops: running
                .table_cell_tab_stops
                .and_then(|values| values.get(section_index)),
            running_table_layout: running
                .table_layout
                .and_then(|values| values.get(section_index)),
            running_column_breaks: running
                .column_breaks
                .and_then(|values| values.get(section_index)),
        }
    }

    fn final_section(self, running: AlignedRunningSectionHints<'a>) -> SectionWriteHint<'a> {
        SectionWriteHint {
            columns: SectionColumnWriteHint {
                gap_pt: self.final_gap,
                layout: self.final_layout,
                separator: self.final_separator,
                rtl: self.final_rtl,
            },
            running_surface_distances: running
                .distances
                .and_then(|values| values.last())
                .copied()
                .unwrap_or_default(),
            running_line_spacing: running.line_spacing.and_then(|values| values.last()),
            running_pagination: running.pagination.and_then(|values| values.last()),
            running_tab_stops: running.tab_stops.and_then(|values| values.last()),
            running_table_cell_tab_stops: running
                .table_cell_tab_stops
                .and_then(|values| values.last()),
            running_table_layout: running.table_layout.and_then(|values| values.last()),
            running_column_breaks: running.column_breaks.and_then(|values| values.last()),
        }
    }
}

fn source_column_twips(points: f32, allow_zero: bool) -> Option<i64> {
    if !points.is_finite() || points < 0.0 || (!allow_zero && points == 0.0) {
        return None;
    }
    let twips = pt_twips(points);
    let minimum = if allow_zero { 0 } else { 1 };
    (minimum..=31_680).contains(&twips).then_some(twips)
}

fn source_running_surface_twips(points: Option<f32>) -> i64 {
    points
        .and_then(|value| source_column_twips(value, true))
        .unwrap_or(708)
}

fn source_line_spacing(spacing: Option<LineSpacingHint>) -> Option<(i64, &'static str)> {
    let (points, rule) = match spacing? {
        LineSpacingHint::Exact(points) => (points, "exact"),
        LineSpacingHint::AtLeast(points) => (points, "atLeast"),
    };
    if !points.is_finite() || points <= 0.0 {
        return None;
    }
    let twips = pt_twips(points);
    (1..=31_680).contains(&twips).then_some((twips, rule))
}

fn source_tab_stops_xml(tab_stops: Option<&[TabStop]>) -> Option<String> {
    let tab_stops = tab_stops.filter(|stops| !stops.is_empty())?;
    if tab_stops.len() > MAX_TAB_STOPS {
        return None;
    }

    let mut previous_position = None;
    let mut out = String::from("<w:tabs>");
    for stop in tab_stops {
        let position = source_column_twips(stop.position_pt, true)?;
        if previous_position.is_some_and(|previous| position <= previous) {
            return None;
        }
        previous_position = Some(position);

        let alignment = match stop.alignment {
            TabAlignment::Left => "left",
            TabAlignment::Center => "center",
            TabAlignment::Right => "right",
            TabAlignment::Decimal => "decimal",
            TabAlignment::Bar => "bar",
            TabAlignment::Clear => return None,
        };
        let leader = match stop.leader {
            TabLeader::None => None,
            TabLeader::Dot => Some("dot"),
            TabLeader::Hyphen => Some("hyphen"),
            TabLeader::Underscore => Some("underscore"),
            TabLeader::Heavy => Some("heavy"),
            TabLeader::MiddleDot => Some("middleDot"),
            #[cfg(feature = "render")]
            TabLeader::Bar => return None,
        };
        out.push_str(&format!(r#"<w:tab w:val="{alignment}" w:pos="{position}""#));
        if let Some(leader) = leader {
            out.push_str(&format!(r#" w:leader="{leader}""#));
        }
        out.push_str("/>");
    }
    out.push_str("</w:tabs>");
    Some(out)
}

fn source_column_break_offsets<'a>(
    paragraph: &Paragraph,
    offsets: Option<&'a [usize]>,
) -> Option<&'a [usize]> {
    let offsets = offsets?;
    if offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }

    let mut next = 0usize;
    let mut char_offset = 0usize;
    for run in &paragraph.runs {
        let text_is_emitted = run.image.as_ref().is_none_or(|image| image.bytes.is_none());
        for ch in run.text.chars() {
            if offsets
                .get(next)
                .is_some_and(|offset| *offset == char_offset)
            {
                if ch != '\n' || !text_is_emitted {
                    return None;
                }
                next += 1;
            }
            char_offset = char_offset.saturating_add(1);
        }
    }
    (next == offsets.len()).then_some(offsets)
}

struct ColumnBreakCursor<'a> {
    offsets: &'a [usize],
    next: usize,
    char_offset: usize,
}

impl<'a> ColumnBreakCursor<'a> {
    fn new(offsets: &'a [usize]) -> Self {
        Self {
            offsets,
            next: 0,
            char_offset: 0,
        }
    }

    fn advance(&mut self) -> bool {
        let is_column_break = self
            .offsets
            .get(self.next)
            .is_some_and(|offset| *offset == self.char_offset);
        if is_column_break {
            self.next += 1;
        }
        self.char_offset = self.char_offset.saturating_add(1);
        is_column_break
    }

    fn skip_text(&mut self, text: &str) {
        for _ in text.chars() {
            self.advance();
        }
    }
}

fn section_columns_xml(column_count: Option<u16>, hint: SectionColumnWriteHint<'_>) -> String {
    let count = column_count.map(|columns| usize::from(columns.max(1)));
    let equal_space = hint
        .gap_pt
        .and_then(|points| source_column_twips(points, true));
    let custom_columns = hint.layout.and_then(|layout| {
        if count != Some(layout.columns.len()) || !(1..=64).contains(&layout.columns.len()) {
            return None;
        }
        layout
            .columns
            .iter()
            .map(|column| {
                Some((
                    source_column_twips(column.width_pt, false)?,
                    source_column_twips(column.space_after_pt, true)?,
                ))
            })
            .collect::<Option<Vec<_>>>()
    });

    let has_columns = column_count.is_some()
        || custom_columns.is_some()
        || equal_space.is_some()
        || hint.separator;
    if !has_columns {
        return String::new();
    }

    let mut out = String::from("<w:cols");
    if let Some(count) = count {
        out.push_str(&format!(r#" w:num="{count}""#));
    }
    if custom_columns.is_some() {
        out.push_str(r#" w:equalWidth="0""#);
    } else if let Some(space) = equal_space {
        out.push_str(&format!(r#" w:space="{space}""#));
    }
    if hint.separator {
        out.push_str(r#" w:sep="1""#);
    }

    let Some(custom_columns) = custom_columns else {
        out.push_str("/>");
        return out;
    };
    out.push('>');
    let last = custom_columns.len().saturating_sub(1);
    for (index, (width, space_after)) in custom_columns.into_iter().enumerate() {
        out.push_str(&format!(r#"<w:col w:w="{width}""#));
        if index < last && space_after > 0 {
            out.push_str(&format!(r#" w:space="{space_after}""#));
        }
        out.push_str("/>");
    }
    out.push_str("</w:cols>");
    out
}

/// Image extent in EMU from intrinsic pixels (96 dpi → 9525 EMU/px), clamped to
/// the ~6in content width with aspect preserved. Falls back to 2in² when the
/// dimensions are unknown.
fn image_extent_emu(w: Option<u32>, h: Option<u32>) -> (u32, u32) {
    const EMU_PER_PX: u32 = 9525;
    const MAX_W_EMU: u32 = 5_486_400; // 6 inches
    const FALLBACK: u32 = 1_828_800; // 2 inches
    let (Some(w), Some(h)) = (w, h) else {
        return (FALLBACK, FALLBACK);
    };
    if w == 0 || h == 0 {
        return (FALLBACK, FALLBACK);
    }
    let mut cx = w.saturating_mul(EMU_PER_PX);
    let mut cy = h.saturating_mul(EMU_PER_PX);
    if cx > MAX_W_EMU {
        cy = ((cy as u64 * MAX_W_EMU as u64) / cx as u64).max(1) as u32;
        cx = MAX_W_EMU;
    }
    (cx.max(1), cy.max(1))
}

const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const CT_COMMENTS_EXT: &str = "application/vnd.ms-word.commentsExt+xml";
const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
const REL_COMMENTS_EXT: &str =
    "http://schemas.microsoft.com/office/2011/relationships/commentsExtended";
const CT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const CT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
const CT_CUSTOM_PROPERTIES: &str =
    "application/vnd.openxmlformats-officedocument.custom-properties+xml";
const REL_CUSTOM_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties";
const CT_XML: &str = "application/xml";
const CT_CUSTOM_XML_PROPERTIES: &str =
    "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";
const REL_CUSTOM_XML_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
const CT_WEB_EXTENSION_TASKPANES: &str = "application/vnd.ms-office.webextensiontaskpanes+xml";
const CT_WEB_EXTENSION: &str = "application/vnd.ms-office.webextension+xml";
const REL_WEB_EXTENSION_TASKPANES: &str =
    "http://schemas.microsoft.com/office/2011/relationships/webextensiontaskpanes";
const REL_WEB_EXTENSION: &str =
    "http://schemas.microsoft.com/office/2011/relationships/webextension";
const WEB_EXTENSION_NS: &str =
    "http://schemas.microsoft.com/office/webextensions/webextension/2010/11";
const WEB_EXTENSION_TASKPANES_NS: &str =
    "http://schemas.microsoft.com/office/webextensions/taskpanes/2010/11";

/// Build a `word/header1.xml` / `footer1.xml` part body from running blocks.
/// Tables reuse the body table geometry while their cell content keeps
/// header/footer-local relationships. Appends a centered `PAGE` field when
/// `page_numbers`. A header/footer must contain at least one paragraph.
fn render_hf_body(
    ctx: &mut Ctx,
    blocks: &[crate::model::Block],
    hints: RunningBlockWriteHints<'_>,
    page_numbers: bool,
    rels: &mut Vec<Rel>,
) -> String {
    let mut out = String::new();
    let line_spacing = hints
        .line_spacing
        .filter(|hints| hints.len() == blocks.len());
    let pagination = hints
        .pagination
        .filter(|hints| running_pagination_hints_align(blocks, hints));
    let tab_stops = hints.tab_stops.filter(|hints| hints.len() == blocks.len());
    let table_cell_line_spacing = hints
        .table_cell_line_spacing
        .filter(|hints| hints.len() == blocks.len());
    let table_cell_tab_stops = hints
        .table_cell_tab_stops
        .filter(|hints| hints.len() == blocks.len());
    let table_cell_column_breaks = hints
        .table_layout
        .map(|hints| hints.cell_column_breaks.as_slice())
        .filter(|hints| hints.len() == blocks.len());
    let table_nested = hints
        .table_layout
        .map(|hints| hints.nested_tables.as_slice())
        .filter(|hints| hints.len() == blocks.len());
    let column_break_offsets = hints
        .column_break_offsets
        .filter(|hints| hints.len() == blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        ctx.write_hf_block(
            &mut out,
            block,
            rels,
            RunningBlockSlotWriteHints {
                line_spacing: line_spacing
                    .and_then(|hints| hints.get(index))
                    .copied()
                    .flatten(),
                pagination: pagination
                    .and_then(|hints| hints.paragraphs.get(index))
                    .copied(),
                tab_stops: tab_stops
                    .and_then(|hints| hints.get(index))
                    .map(Vec::as_slice),
                table_row_pagination: pagination
                    .and_then(|hints| hints.table_rows.get(index))
                    .map(Vec::as_slice),
                table_cell_pagination: pagination.and_then(|hints| hints.table_cells.get(index)),
                table_cell_line_spacing: table_cell_line_spacing.and_then(|hints| hints.get(index)),
                table_cell_tab_stops: table_cell_tab_stops.and_then(|hints| hints.get(index)),
                table_cell_column_breaks: table_cell_column_breaks
                    .and_then(|hints| hints.get(index)),
                table_nested: table_nested.and_then(|hints| hints.get(index)),
                column_break_offsets: column_break_offsets
                    .and_then(|hints| hints.get(index))
                    .map(Vec::as_slice),
            },
        );
    }
    if page_numbers {
        out.push_str(
            r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p>"#,
        );
    }
    if out.is_empty() {
        out.push_str("<w:p/>");
    }
    out
}

fn running_pagination_hints_align(blocks: &[Block], hints: &RunningBlockPaginationHints) -> bool {
    if hints.paragraphs.len() != blocks.len()
        || hints.table_rows.len() != blocks.len()
        || hints.table_cells.len() != blocks.len()
    {
        return false;
    }
    blocks.iter().enumerate().all(|(index, block)| {
        let rows = &hints.table_rows[index];
        let cells = &hints.table_cells[index];
        match block {
            Block::Table(table) => {
                rows.len() == table.rows.len()
                    && Ctx::table_cell_paragraph_hints_align(table, cells)
            }
            _ => rows.is_empty() && cells.is_empty(),
        }
    })
}

fn write_hf_run(
    ctx: &mut Ctx,
    rels: &mut Vec<Rel>,
    out: &mut String,
    r: &crate::model::Run,
    column_breaks: &mut ColumnBreakCursor<'_>,
) {
    let comment_id = ctx.begin_comment(out, r.comment.as_ref());
    let deleted = matches!(
        r.revision.as_ref().map(|revision| revision.kind),
        Some(RevisionKind::Deletion)
    );
    let hyperlink_target = match &r.field {
        FieldRole::Hyperlink { url } => Some(hyperlink_write_target(url)),
        _ => None,
    };
    let hyperlink_rid = match hyperlink_target {
        Some(HyperlinkWriteTarget::External(url)) => {
            Some(add_part_rel(rels, REL_HYPERLINK, url, true))
        }
        _ => None,
    };
    let mut run_xml = String::new();
    if let Some(img) = r.image.as_ref().filter(|img| img.bytes.is_some()) {
        ctx.write_image_inner(&mut run_xml, img, Some(rels));
        column_breaks.skip_text(&r.text);
    } else {
        run_xml.push_str("<w:r>");
        write_rpr(&mut run_xml, &r.props);
        if let Some(img) = &r.image {
            let text = image_placeholder_text(img, "image unavailable");
            if deleted {
                write_run_deleted_text(&mut run_xml, &text);
            } else {
                write_run_text(&mut run_xml, &text);
            }
        }
        if deleted {
            write_run_deleted_text_with_column_breaks(&mut run_xml, &r.text, column_breaks);
        } else {
            write_run_text_with_column_breaks(&mut run_xml, &r.text, column_breaks);
        }
        run_xml.push_str("</w:r>");
    }

    match &r.field {
        FieldRole::Hyperlink { .. } => match hyperlink_target {
            Some(HyperlinkWriteTarget::External(_)) => {
                if let Some(rid) = hyperlink_rid {
                    run_xml = format!(r#"<w:hyperlink r:id="{rid}">{run_xml}</w:hyperlink>"#);
                }
            }
            Some(HyperlinkWriteTarget::Anchor(anchor)) => {
                run_xml = format!(
                    r#"<w:hyperlink w:anchor="{}">{run_xml}</w:hyperlink>"#,
                    esc_attr(anchor)
                );
            }
            Some(HyperlinkWriteTarget::Invalid) | None => {}
        },
        FieldRole::Simple { instruction } => {
            let instruction = normalize_field_instruction(instruction);
            if !instruction.is_empty() {
                let dirty = if r.field_dirty {
                    r#" w:dirty="true""#
                } else {
                    ""
                };
                run_xml = format!(
                    r#"<w:fldSimple w:instr=" {} "{dirty}>{run_xml}</w:fldSimple>"#,
                    esc_attr(&instruction)
                );
            }
        }
        _ => {}
    }

    let run_xml = content_control_wrapper(r.content_control.as_ref(), &run_xml);
    let run_xml = ctx.bookmark_wrapper(r.bookmark.as_deref(), &run_xml);
    ctx.write_revision_wrapper(out, r.revision.as_ref(), &run_xml);
    ctx.end_comment(out, comment_id);
}

fn add_part_rel(rels: &mut Vec<Rel>, rel_type: &str, target: &str, external: bool) -> String {
    let id = format!("rId{}", rels.len() + 1);
    rels.push(Rel {
        id: id.clone(),
        rel_type: rel_type.to_string(),
        target: target.to_string(),
        external,
    });
    id
}

fn content_control_wrapper(control: Option<&AuthoredContentControl>, run_xml: &str) -> String {
    let Some(control) = control else {
        return run_xml.to_string();
    };
    let alias = non_empty_trimmed(control.alias.as_deref());
    let tag = non_empty_trimmed(control.tag.as_deref());
    let xpath = non_empty_trimmed(control.data_binding_xpath.as_deref());
    let store_item_id = non_empty_trimmed(control.data_binding_store_item_id.as_deref());
    if alias.is_none() && tag.is_none() && (xpath.is_none() || store_item_id.is_none()) {
        return run_xml.to_string();
    }
    let mut xml = String::new();
    xml.push_str("<w:sdt><w:sdtPr>");
    if let Some(alias) = alias {
        xml.push_str(&format!(r#"<w:alias w:val="{}"/>"#, esc_attr(alias)));
    }
    if let Some(tag) = tag {
        xml.push_str(&format!(r#"<w:tag w:val="{}"/>"#, esc_attr(tag)));
    }
    if let (Some(xpath), Some(store_item_id)) = (xpath, store_item_id) {
        xml.push_str(&format!(
            r#"<w:dataBinding w:xpath="{}" w:storeItemID="{}"/>"#,
            esc_attr(xpath),
            esc_attr(store_item_id)
        ));
    }
    xml.push_str("</w:sdtPr><w:sdtContent>");
    xml.push_str(run_xml);
    xml.push_str("</w:sdtContent></w:sdt>");
    xml
}

/// Wrap a header/footer body in its root element + namespaces.
fn hf_part(tag: &str, body: &str) -> Vec<u8> {
    if body.contains("<w:drawing>") {
        let chart_ns = if body.contains("<c:chart") {
            format!(r#" xmlns:c="{C_NS}""#)
        } else {
            String::new()
        };
        let chart_ex_ns = if body.contains("<cx:chart") {
            format!(r#" xmlns:cx="{CX_NS}""#)
        } else {
            String::new()
        };
        format!(
            r#"{XML_DECL}<w:{tag} xmlns:w="{W_NS}" xmlns:r="{R_NS}" xmlns:wp="{WP_NS}" xmlns:a="{A_NS}" xmlns:pic="{PIC_NS}"{chart_ns}{chart_ex_ns}>{body}</w:{tag}>"#
        )
        .into_bytes()
    } else {
        format!(r#"{XML_DECL}<w:{tag} xmlns:w="{W_NS}" xmlns:r="{R_NS}">{body}</w:{tag}>"#)
            .into_bytes()
    }
}

fn rels_path_for_part(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{path}.rels"),
    }
}

fn settings_xml(even_and_odd_headers: bool, document_id: Option<&str>) -> String {
    let even_odd = if even_and_odd_headers {
        "<w:evenAndOddHeaders/>"
    } else {
        ""
    };
    let w14 = if document_id.is_some() {
        format!(r#" xmlns:w14="{W14_NS}""#)
    } else {
        String::new()
    };
    let doc_id = document_id
        .map(|id| format!(r#"<w14:docId w14:val="{}"/>"#, esc_attr(id)))
        .unwrap_or_default();
    format!(r#"{XML_DECL}<w:settings xmlns:w="{W_NS}"{w14}>{even_odd}{doc_id}</w:settings>"#)
}

/// A `word/styles.xml` defining `Normal`, optional `Heading1..6`, caller-supplied
/// paragraph styles, and otherwise undefined source style id/name pairs.
fn styles_xml(
    styles: &[ParagraphStyle],
    include_headings: bool,
    source_styles: &[SourceParagraphStyle],
) -> String {
    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(&format!(r#"<w:styles xmlns:w="{W_NS}">"#));
    s.push_str(
        r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#,
    );
    if include_headings {
        for (lvl, sz) in [(1u8, 32), (2, 28), (3, 26), (4, 24), (5, 22), (6, 22)] {
            let style_id = format!("Heading{lvl}");
            let default_name = format!("heading {lvl}");
            let name = source_styles
                .iter()
                .find(|style| style.id == style_id)
                .map(|style| style.name.as_str())
                .unwrap_or(default_name.as_str());
            s.push_str(&format!(
                concat!(
                    r#"<w:style w:type="paragraph" w:styleId="{style_id}">"#,
                    r#"<w:name w:val="{name}"/><w:basedOn w:val="Normal"/>"#,
                    r#"<w:next w:val="Normal"/><w:qFormat/>"#,
                    r#"<w:pPr><w:outlineLvl w:val="{ol}"/></w:pPr>"#,
                    r#"<w:rPr><w:b/><w:sz w:val="{sz}"/><w:szCs w:val="{sz}"/></w:rPr></w:style>"#,
                ),
                style_id = style_id,
                name = esc_attr(name),
                ol = lvl - 1,
                sz = sz
            ));
        }
    }
    for style in styles {
        write_paragraph_style(&mut s, style);
    }
    for style in source_styles {
        if source_style_is_defined(style, styles, include_headings) {
            continue;
        }
        s.push_str(&format!(
            r#"<w:style w:type="paragraph" w:styleId="{}"><w:name w:val="{}"/></w:style>"#,
            esc_attr(&style.id),
            esc_attr(&style.name)
        ));
    }
    s.push_str("</w:styles>");
    s
}

fn source_style_is_defined(
    source: &SourceParagraphStyle,
    styles: &[ParagraphStyle],
    include_headings: bool,
) -> bool {
    source.id == "Normal"
        || styles.iter().any(|style| {
            non_empty_trimmed(Some(&style.id)) == Some(source.id.as_str())
                && non_empty_trimmed(Some(&style.name)).is_some()
        })
        || (include_headings
            && matches!(
                source.id.as_str(),
                "Heading1" | "Heading2" | "Heading3" | "Heading4" | "Heading5" | "Heading6"
            ))
}

fn write_paragraph_style(out: &mut String, style: &ParagraphStyle) {
    let Some(id) = non_empty_trimmed(Some(&style.id)) else {
        return;
    };
    let Some(name) = non_empty_trimmed(Some(&style.name)) else {
        return;
    };
    let id = esc_attr(id);
    let name = esc_attr(name);
    out.push_str(&format!(
        r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/>"#
    ));
    if let Some(based_on) = non_empty_trimmed(style.based_on.as_deref()) {
        out.push_str(&format!(r#"<w:basedOn w:val="{}"/>"#, esc_attr(based_on)));
    }
    if let Some(next) = non_empty_trimmed(style.next.as_deref()) {
        out.push_str(&format!(r#"<w:next w:val="{}"/>"#, esc_attr(next)));
    }
    if style.q_format {
        out.push_str("<w:qFormat/>");
    }
    write_style_ppr(out, style);
    write_rpr(out, &style.run);
    out.push_str("</w:style>");
}

fn write_style_ppr(out: &mut String, style: &ParagraphStyle) {
    let jc = match style.align {
        Align::Left => None,
        Align::Center => Some("center"),
        Align::Right => Some("right"),
        Align::Justify => Some("both"),
    };
    let sp = style.spacing;
    let ind = style.indent;
    let has_spacing = sp.before_pt.is_some() || sp.after_pt.is_some() || sp.line_pct.is_some();
    let has_indent = ind.left_pt.is_some()
        || ind.right_pt.is_some()
        || ind.first_line_pt.is_some()
        || ind.hanging_pt.is_some();
    let outline = style.heading_level.map(|level| level.clamp(1, 9) - 1);
    if jc.is_none() && !has_spacing && !has_indent && style.shading.is_none() && outline.is_none() {
        return;
    }
    out.push_str("<w:pPr>");
    if let Some(c) = style.shading {
        out.push_str(&format!(
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
            hex(c)
        ));
    }
    write_spacing(out, sp, None);
    write_indent(out, ind);
    if let Some(j) = jc {
        out.push_str(&format!(r#"<w:jc w:val="{j}"/>"#));
    }
    if let Some(o) = outline {
        out.push_str(&format!(r#"<w:outlineLvl w:val="{o}"/>"#));
    }
    out.push_str("</w:pPr>");
}

fn write_spacing(out: &mut String, sp: Spacing, absolute: Option<LineSpacingHint>) {
    let absolute = source_line_spacing(absolute);
    if sp.before_pt.is_none()
        && sp.after_pt.is_none()
        && sp.line_pct.is_none()
        && absolute.is_none()
    {
        return;
    }
    let mut a = String::new();
    if let Some(b) = sp.before_pt {
        a += &format!(r#" w:before="{}""#, pt_twips(b));
    }
    if let Some(af) = sp.after_pt {
        a += &format!(r#" w:after="{}""#, pt_twips(af));
    }
    if let Some((line, rule)) = absolute {
        a += &format!(r#" w:line="{line}" w:lineRule="{rule}""#);
    } else if let Some(l) = sp.line_pct {
        a += &format!(
            r#" w:line="{}" w:lineRule="auto""#,
            (l * 240.0).round() as i64
        );
    }
    out.push_str(&format!("<w:spacing{a}/>"));
}

fn write_indent(out: &mut String, ind: Indent) {
    if ind.left_pt.is_none()
        && ind.right_pt.is_none()
        && ind.first_line_pt.is_none()
        && ind.hanging_pt.is_none()
    {
        return;
    }
    let mut a = String::new();
    if let Some(l) = ind.left_pt {
        a += &format!(r#" w:left="{}""#, pt_twips(l));
    }
    if let Some(r) = ind.right_pt {
        a += &format!(r#" w:right="{}""#, pt_twips(r));
    }
    if let Some(f) = ind.first_line_pt {
        a += &format!(r#" w:firstLine="{}""#, pt_twips(f));
    }
    if let Some(h) = ind.hanging_pt {
        a += &format!(r#" w:hanging="{}""#, pt_twips(h));
    }
    out.push_str(&format!("<w:ind{a}/>"));
}

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W14_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const C_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const CX_NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const PIC_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const CT_CORE_PROPERTIES: &str = "application/vnd.openxmlformats-package.core-properties+xml";
const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const CT_CHART_EX: &str = "application/vnd.ms-office.chartex+xml";
const CT_EMBEDDED_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const CT_XLSX_WORKBOOK: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const CT_XLSX_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CT_XLSX_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
const CT_XLSX_SHARED_STRINGS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const REL_CHART_EX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
const REL_PACKAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const REL_XLSX_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const REL_XLSX_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_XLSX_SHARED_STRINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
const CORE_PROPERTIES_NS: &str =
    "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const DCTERMS_NS: &str = "http://purl.org/dc/terms/";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const S_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
/// Hard cap on table columns / a cell's column or row span, so a hostile model
/// (`col_span = u16::MAX`) cannot amplify into millions of `<w:gridCol>`/cells.
const MAX_TABLE_COLS: usize = 1024;
const TABLE_GRID_TWIPS: u32 = 9000;

fn page_number_type_xml(setup: &SectionSetup) -> String {
    if setup.page_number_start.is_none() && setup.page_number_format.is_none() {
        return String::new();
    }
    let mut out = String::from("<w:pgNumType");
    if let Some(start) = setup.page_number_start {
        out.push_str(&format!(r#" w:start="{}""#, start.max(1)));
    }
    if let Some(format) = setup.page_number_format {
        out.push_str(&format!(r#" w:fmt="{}""#, format.wml_value()));
    }
    out.push_str("/>");
    out
}

fn doc_grid_xml(setup: &SectionSetup) -> String {
    let Some(grid) = setup.doc_grid else {
        return String::new();
    };
    let mut out = format!(r#"<w:docGrid w:type="{}""#, grid.grid_type.wml_value());
    if let Some(line_pitch) = grid.line_pitch {
        out.push_str(&format!(r#" w:linePitch="{line_pitch}""#));
    }
    if let Some(character_space) = grid.character_space {
        out.push_str(&format!(r#" w:charSpace="{character_space}""#));
    }
    out.push_str("/>");
    out
}

/// Side-state accumulated while folding the model into `document.xml`: the body
/// XML is built in `out` strings passed to each method, while these tables grow.
struct Ctx {
    /// `word/_rels/document.xml.rels` entries (hyperlinks + images).
    doc_rels: Vec<Rel>,
    /// Media parts to emit: `(part path, bytes, extension, content-type)`.
    media: Vec<(String, Vec<u8>, &'static str, &'static str)>,
    /// Chart parts to emit: `(part path, content-type, bytes)`.
    chart_parts: Vec<(String, &'static str, Vec<u8>)>,
    /// Chart relationship files to emit: `(rels path, relationships)`.
    chart_rels: Vec<(String, Vec<Rel>)>,
    /// Embedded XLSX workbooks backing authored chart data.
    embedded_workbooks: Vec<(String, Vec<u8>)>,
    /// Header/footer parts to emit: `(part path, content-type, bytes)`.
    hf_parts: Vec<(String, &'static str, Vec<u8>)>,
    /// Header/footer relationship files to emit: `(rels path, relationships)`.
    hf_rels: Vec<(String, Vec<Rel>)>,
    /// Next relationship id ordinal.
    next_rid: u32,
    /// Whether any list item was emitted (⇒ write `numbering.xml`).
    has_list: bool,
    /// Whether a generated heading style is needed in `styles.xml`.
    has_heading: bool,
    /// Whether any paragraph style reference was emitted (⇒ write `styles.xml`).
    has_styles: bool,
    /// Resolved source style names absent from explicit authored definitions.
    source_styles: Vec<SourceParagraphStyle>,
    /// Whether authored even-page header/footer variants require settings.xml.
    has_even_header_footer: bool,
    /// Image counter for unique `media/imageN` names + drawing ids.
    img_id: u32,
    /// Chart counter for unique `charts/chartN.xml` names.
    chart_id: u32,
    /// Drawing counter for unique `wp:docPr` ids.
    drawing_id: u32,
    /// Next authored comment id.
    comment_id: u32,
    /// Next authored revision id.
    revision_id: u32,
    /// Authored comments emitted while writing body runs.
    comments: Vec<WrittenComment>,
    /// Next authored bookmark id.
    bookmark_id: u32,
    /// Authored footnotes emitted while writing body runs.
    footnotes: Vec<WrittenNote>,
    /// Authored endnotes emitted while writing body runs.
    endnotes: Vec<WrittenNote>,
    /// Relationships owned by `word/footnotes.xml`.
    footnote_rels: Vec<Rel>,
    /// Relationships owned by `word/endnotes.xml`.
    endnote_rels: Vec<Rel>,
    /// Next generated header part number.
    header_id: u32,
    /// Next generated footer part number.
    footer_id: u32,
}

#[derive(Debug, Clone)]
struct WrittenComment {
    id: String,
    comment: AuthoredComment,
}

#[derive(Debug, Clone)]
struct WrittenNote {
    id: String,
    text: String,
    body_xml: Option<String>,
}

fn non_empty_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

impl Ctx {
    fn new() -> Self {
        Ctx {
            doc_rels: Vec::new(),
            media: Vec::new(),
            chart_parts: Vec::new(),
            chart_rels: Vec::new(),
            embedded_workbooks: Vec::new(),
            hf_parts: Vec::new(),
            hf_rels: Vec::new(),
            next_rid: 1,
            has_list: false,
            has_heading: false,
            has_styles: false,
            source_styles: Vec::new(),
            has_even_header_footer: false,
            img_id: 0,
            chart_id: 0,
            drawing_id: 0,
            comment_id: 0,
            revision_id: 0,
            comments: Vec::new(),
            bookmark_id: 0,
            footnotes: Vec::new(),
            endnotes: Vec::new(),
            footnote_rels: Vec::new(),
            endnote_rels: Vec::new(),
            header_id: 0,
            footer_id: 0,
        }
    }

    fn add_rel(&mut self, rel_type: &str, target: &str, external: bool) -> String {
        let id = format!("rId{}", self.next_rid);
        self.next_rid += 1;
        self.doc_rels.push(Rel {
            id: id.clone(),
            rel_type: rel_type.to_string(),
            target: target.to_string(),
            external,
        });
        id
    }

    fn write_block(&mut self, out: &mut String, b: &Block) {
        match b {
            Block::Paragraph(p) => self.write_paragraph(out, p),
            Block::Table(t) => self.write_table(out, t),
            Block::Image(img) => {
                out.push_str("<w:p>");
                self.write_image_or_placeholder(out, img);
                out.push_str("</w:p>");
            }
            Block::Chart(chart) => self.write_chart(out, chart),
            Block::PageBreak => out.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#),
            Block::SectionBreak(setup) => {
                self.write_section_break(out, setup, SectionWriteHint::default())
            }
        }
    }

    fn write_top_level_block(
        &mut self,
        out: &mut String,
        block: &Block,
        section_hints: SectionWriteHint<'_>,
        paragraph_hints: ParagraphWriteHints<'_>,
        table_hints: TableWriteHints<'_>,
    ) {
        match block {
            Block::Paragraph(paragraph) => {
                self.write_paragraph_with_source_hints(out, paragraph, paragraph_hints)
            }
            Block::Table(table) => self.write_table_with_source_hints(out, table, table_hints),
            Block::SectionBreak(setup) => self.write_section_break(out, setup, section_hints),
            _ => self.write_block(out, block),
        }
    }

    fn write_hf_block(
        &mut self,
        out: &mut String,
        block: &Block,
        rels: &mut Vec<Rel>,
        hints: RunningBlockSlotWriteHints<'_>,
    ) {
        match block {
            Block::Paragraph(p) => {
                out.push_str("<w:p>");
                self.write_ppr(
                    out,
                    &p.props,
                    hints.line_spacing,
                    hints.pagination,
                    hints.tab_stops,
                );
                let column_break_offsets =
                    source_column_break_offsets(p, hints.column_break_offsets).unwrap_or(&[]);
                let mut column_breaks = ColumnBreakCursor::new(column_break_offsets);
                for r in &p.runs {
                    write_hf_run(self, rels, out, r, &mut column_breaks);
                }
                out.push_str("</w:p>");
            }
            Block::Table(t) => {
                let row_pagination = hints
                    .table_row_pagination
                    .filter(|hints| hints.len() == t.rows.len());
                let cell_pagination = hints
                    .table_cell_pagination
                    .filter(|hints| Self::table_cell_paragraph_hints_align(t, hints));
                let cell_line_spacing = hints
                    .table_cell_line_spacing
                    .filter(|hints| Self::table_cell_paragraph_hints_align(t, hints));
                let cell_tab_stops = hints
                    .table_cell_tab_stops
                    .filter(|hints| Self::table_cell_tab_stops_align(t, hints));
                let cell_column_breaks = hints
                    .table_cell_column_breaks
                    .filter(|hints| Self::table_cell_column_break_hints_align(t, hints));
                let nested_tables = hints
                    .table_nested
                    .filter(|hints| Self::table_cell_nested_hints_align(t, hints));
                self.write_table_inner(
                    out,
                    t,
                    Some(rels),
                    TableWriteHints {
                        row_pagination,
                        cell_pagination,
                        cell_line_spacing,
                        cell_column_breaks,
                        nested_tables,
                        cell_tab_stops,
                    },
                )
            }
            Block::Image(img) => {
                out.push_str("<w:p>");
                self.write_hf_image_or_placeholder(out, img, rels);
                out.push_str("</w:p>");
            }
            Block::Chart(chart) => {
                if chart.series.is_empty() {
                    out.push_str("<w:p><w:r>");
                    write_run_text(out, &chart_placeholder_text(chart));
                    out.push_str("</w:r></w:p>");
                } else {
                    self.write_chart_inner(out, chart, Some(rels));
                }
            }
            Block::PageBreak => out.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#),
            Block::SectionBreak(_) => {}
        }
    }

    fn write_section_break(
        &mut self,
        out: &mut String,
        setup: &SectionSetup,
        section_hints: SectionWriteHint<'_>,
    ) {
        out.push_str("<w:p><w:pPr>");
        self.write_sect_pr(out, setup, Some(SectionBreakKind::NextPage), section_hints);
        out.push_str("</w:pPr></w:p>");
    }

    fn write_header_ref(
        &mut self,
        refs: &mut String,
        type_name: &str,
        blocks: &[Block],
        hints: RunningBlockWriteHints<'_>,
    ) {
        if blocks.is_empty() {
            return;
        }
        self.header_id += 1;
        let path = format!("word/header{}.xml", self.header_id);
        let target = format!("header{}.xml", self.header_id);
        let rid = self.add_rel(REL_HEADER, &target, false);
        let mut rels = Vec::new();
        let body = render_hf_body(self, blocks, hints, false, &mut rels);
        if !rels.is_empty() {
            self.hf_rels.push((rels_path_for_part(&path), rels));
        }
        self.hf_parts.push((path, CT_HEADER, hf_part("hdr", &body)));
        refs.push_str(&format!(
            r#"<w:headerReference w:type="{type_name}" r:id="{rid}"/>"#
        ));
    }

    fn write_footer_ref(
        &mut self,
        refs: &mut String,
        type_name: &str,
        blocks: &[Block],
        hints: RunningBlockWriteHints<'_>,
        page_numbers: bool,
    ) {
        if blocks.is_empty() && !page_numbers {
            return;
        }
        self.footer_id += 1;
        let path = format!("word/footer{}.xml", self.footer_id);
        let target = format!("footer{}.xml", self.footer_id);
        let rid = self.add_rel(REL_FOOTER, &target, false);
        let mut rels = Vec::new();
        let body = render_hf_body(self, blocks, hints, page_numbers, &mut rels);
        if !rels.is_empty() {
            self.hf_rels.push((rels_path_for_part(&path), rels));
        }
        self.hf_parts.push((path, CT_FOOTER, hf_part("ftr", &body)));
        refs.push_str(&format!(
            r#"<w:footerReference w:type="{type_name}" r:id="{rid}"/>"#
        ));
    }

    fn write_sect_pr(
        &mut self,
        out: &mut String,
        setup: &SectionSetup,
        fallback_break: Option<SectionBreakKind>,
        section_hints: SectionWriteHint<'_>,
    ) {
        let mut refs = String::new();
        let running_spacing = section_hints.running_line_spacing;
        let running_pagination = section_hints.running_pagination;
        let running_tabs = section_hints.running_tab_stops;
        let running_table_tabs = section_hints.running_table_cell_tab_stops;
        let running_table_layout = section_hints.running_table_layout;
        let running_column_breaks = section_hints.running_column_breaks;
        self.write_header_ref(
            &mut refs,
            "default",
            &setup.header,
            RunningBlockWriteHints {
                line_spacing: running_spacing.map(|hints| hints.header.as_slice()),
                pagination: running_pagination.map(|hints| &hints.header),
                tab_stops: running_tabs.map(|hints| hints.header.as_slice()),
                table_cell_line_spacing: running_spacing
                    .map(|hints| hints.header_table_cells.as_slice()),
                table_cell_tab_stops: running_table_tabs.map(|hints| hints.header.as_slice()),
                table_layout: running_table_layout.map(|hints| &hints.header),
                column_break_offsets: running_column_breaks.map(|hints| hints.header.as_slice()),
            },
        );
        self.write_header_ref(
            &mut refs,
            "first",
            &setup.first_header,
            RunningBlockWriteHints {
                line_spacing: running_spacing.map(|hints| hints.first_header.as_slice()),
                pagination: running_pagination.map(|hints| &hints.first_header),
                tab_stops: running_tabs.map(|hints| hints.first_header.as_slice()),
                table_cell_line_spacing: running_spacing
                    .map(|hints| hints.first_header_table_cells.as_slice()),
                table_cell_tab_stops: running_table_tabs.map(|hints| hints.first_header.as_slice()),
                table_layout: running_table_layout.map(|hints| &hints.first_header),
                column_break_offsets: running_column_breaks
                    .map(|hints| hints.first_header.as_slice()),
            },
        );
        self.write_header_ref(
            &mut refs,
            "even",
            &setup.even_header,
            RunningBlockWriteHints {
                line_spacing: running_spacing.map(|hints| hints.even_header.as_slice()),
                pagination: running_pagination.map(|hints| &hints.even_header),
                tab_stops: running_tabs.map(|hints| hints.even_header.as_slice()),
                table_cell_line_spacing: running_spacing
                    .map(|hints| hints.even_header_table_cells.as_slice()),
                table_cell_tab_stops: running_table_tabs.map(|hints| hints.even_header.as_slice()),
                table_layout: running_table_layout.map(|hints| &hints.even_header),
                column_break_offsets: running_column_breaks
                    .map(|hints| hints.even_header.as_slice()),
            },
        );
        self.write_footer_ref(
            &mut refs,
            "default",
            &setup.footer,
            RunningBlockWriteHints {
                line_spacing: running_spacing.map(|hints| hints.footer.as_slice()),
                pagination: running_pagination.map(|hints| &hints.footer),
                tab_stops: running_tabs.map(|hints| hints.footer.as_slice()),
                table_cell_line_spacing: running_spacing
                    .map(|hints| hints.footer_table_cells.as_slice()),
                table_cell_tab_stops: running_table_tabs.map(|hints| hints.footer.as_slice()),
                table_layout: running_table_layout.map(|hints| &hints.footer),
                column_break_offsets: running_column_breaks.map(|hints| hints.footer.as_slice()),
            },
            setup.page_numbers,
        );
        self.write_footer_ref(
            &mut refs,
            "first",
            &setup.first_footer,
            RunningBlockWriteHints {
                line_spacing: running_spacing.map(|hints| hints.first_footer.as_slice()),
                pagination: running_pagination.map(|hints| &hints.first_footer),
                tab_stops: running_tabs.map(|hints| hints.first_footer.as_slice()),
                table_cell_line_spacing: running_spacing
                    .map(|hints| hints.first_footer_table_cells.as_slice()),
                table_cell_tab_stops: running_table_tabs.map(|hints| hints.first_footer.as_slice()),
                table_layout: running_table_layout.map(|hints| &hints.first_footer),
                column_break_offsets: running_column_breaks
                    .map(|hints| hints.first_footer.as_slice()),
            },
            false,
        );
        self.write_footer_ref(
            &mut refs,
            "even",
            &setup.even_footer,
            RunningBlockWriteHints {
                line_spacing: running_spacing.map(|hints| hints.even_footer.as_slice()),
                pagination: running_pagination.map(|hints| &hints.even_footer),
                tab_stops: running_tabs.map(|hints| hints.even_footer.as_slice()),
                table_cell_line_spacing: running_spacing
                    .map(|hints| hints.even_footer_table_cells.as_slice()),
                table_cell_tab_stops: running_table_tabs.map(|hints| hints.even_footer.as_slice()),
                table_layout: running_table_layout.map(|hints| &hints.even_footer),
                column_break_offsets: running_column_breaks
                    .map(|hints| hints.even_footer.as_slice()),
            },
            false,
        );

        let has_first_variant = !setup.first_header.is_empty() || !setup.first_footer.is_empty();
        let has_even_variant = !setup.even_header.is_empty() || !setup.even_footer.is_empty();
        if has_even_variant {
            self.has_even_header_footer = true;
        }

        let page = &setup.page;
        let (w, h) = (pt_twips(page.width_pt), pt_twips(page.height_pt));
        let orient = if page.landscape {
            " w:orient=\"landscape\""
        } else {
            ""
        };
        let (mt, mr, mb, ml) = (
            pt_twips(page.top()),
            pt_twips(page.right()),
            pt_twips(page.bottom()),
            pt_twips(page.left()),
        );
        let columns = section_columns_xml(setup.columns, section_hints.columns);
        let bidi = if section_hints.columns.rtl {
            "<w:bidi/>"
        } else {
            ""
        };
        let header_distance =
            source_running_surface_twips(section_hints.running_surface_distances.header_pt);
        let footer_distance =
            source_running_surface_twips(section_hints.running_surface_distances.footer_pt);
        let text_direction = setup
            .text_direction
            .map(|direction| format!(r#"<w:textDirection w:val="{}"/>"#, direction.wml_value()))
            .unwrap_or_default();
        let doc_grid = doc_grid_xml(setup);
        let page_number_type = page_number_type_xml(setup);
        let start = setup
            .section_break
            .or(fallback_break)
            .map(|kind| format!(r#"<w:type w:val="{}"/>"#, kind.wml_value()))
            .unwrap_or_default();
        let title_pg = if setup.title_page || has_first_variant {
            "<w:titlePg/>"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<w:sectPr>{start}{refs}{title_pg}<w:pgSz w:w="{w}" w:h="{h}"{orient}/><w:pgMar w:top="{mt}" w:right="{mr}" w:bottom="{mb}" w:left="{ml}" w:header="{header_distance}" w:footer="{footer_distance}" w:gutter="0"/>{text_direction}{bidi}{page_number_type}{columns}{doc_grid}</w:sectPr>"#
        ));
    }

    fn write_paragraph(&mut self, out: &mut String, p: &Paragraph) {
        self.write_paragraph_with_source_hints(out, p, ParagraphWriteHints::default());
    }

    fn write_paragraph_with_source_hints(
        &mut self,
        out: &mut String,
        p: &Paragraph,
        hints: ParagraphWriteHints<'_>,
    ) {
        out.push_str("<w:p>");
        self.write_ppr(
            out,
            &p.props,
            hints.line_spacing,
            hints.pagination,
            hints.tab_stops,
        );
        let column_break_offsets =
            source_column_break_offsets(p, hints.column_break_offsets).unwrap_or(&[]);
        let mut column_breaks = ColumnBreakCursor::new(column_break_offsets);
        let note_payloads = hints
            .note_payloads
            .filter(|payloads| payloads.len() == p.runs.len());
        for (index, r) in p.runs.iter().enumerate() {
            self.write_run_with_column_breaks(
                out,
                r,
                &mut column_breaks,
                note_payloads
                    .and_then(|payloads| payloads.get(index))
                    .and_then(Option::as_ref),
            );
        }
        out.push_str("</w:p>");
    }

    fn write_ppr(
        &mut self,
        out: &mut String,
        pr: &ParaProps,
        line_spacing: Option<LineSpacingHint>,
        pagination: Option<PaginationHint>,
        tab_stops: Option<&[TabStop]>,
    ) {
        let heading = pr.heading_level;
        // A heading suppresses list rendering — mirror the reader's precedence.
        let list = pr.list.as_ref().filter(|_| heading.is_none());
        let jc = match pr.align {
            Align::Left if pr.bidi => Some("left"),
            Align::Left => None,
            Align::Center => Some("center"),
            Align::Right => Some("right"),
            Align::Justify => Some("both"),
        };
        // A heading implies its outline level; otherwise the paragraph's own
        // `outline_level` is written so it survives a round trip.
        let outline = heading
            .map(|h| h.saturating_sub(1))
            .or_else(|| pr.outline_level.map(|level| level.min(9)));
        let generated_heading_style = pr.style_id.is_none() && heading.is_some();
        let style_id = pr
            .style_id
            .as_deref()
            .and_then(|style_id| non_empty_trimmed(Some(style_id)))
            .map(str::to_string)
            .or_else(|| heading.map(|h| format!("Heading{}", h.clamp(1, 6))));
        let sp = pr.spacing;
        let ind = pr.indent;
        let line_spacing = line_spacing.filter(|hint| source_line_spacing(Some(*hint)).is_some());
        let has_spacing = sp.before_pt.is_some()
            || sp.after_pt.is_some()
            || sp.line_pct.is_some()
            || line_spacing.is_some();
        let has_indent = ind.left_pt.is_some()
            || ind.right_pt.is_some()
            || ind.first_line_pt.is_some()
            || ind.hanging_pt.is_some();
        let keep_next = pagination.is_some_and(|hint| hint.keep_next);
        let keep_lines = pagination.is_some_and(|hint| hint.keep_lines);
        let widow_control_off = pagination.is_some_and(|hint| !hint.widow_control);
        let tab_stops = source_tab_stops_xml(tab_stops);
        if style_id.is_none()
            && list.is_none()
            && jc.is_none()
            && outline.is_none()
            && !has_spacing
            && !has_indent
            && pr.shading.is_none()
            && !pr.page_break_before
            && !pr.bidi
            && !keep_next
            && !keep_lines
            && !widow_control_off
            && tab_stops.is_none()
        {
            return;
        }
        out.push_str("<w:pPr>");
        // Schema order: pStyle, keepNext, keepLines, pageBreakBefore,
        // widowControl, numPr, shd, tabs, bidi, spacing, ind, jc, outlineLvl.
        if let Some(s) = &style_id {
            self.has_styles = true;
            if let Some(name) = non_empty_trimmed(pr.style_name.as_deref()) {
                self.register_source_style(s, name);
            }
            if generated_heading_style {
                self.has_heading = true;
            }
            out.push_str(&format!(r#"<w:pStyle w:val="{}"/>"#, esc_attr(s)));
        }
        if keep_next {
            out.push_str("<w:keepNext/>");
        }
        if keep_lines {
            out.push_str("<w:keepLines/>");
        }
        if pr.page_break_before {
            out.push_str("<w:pageBreakBefore/>");
        }
        if widow_control_off {
            out.push_str(r#"<w:widowControl w:val="0"/>"#);
        }
        if let Some(li) = list {
            self.has_list = true;
            let num_id = if li.ordered { 1 } else { 2 };
            out.push_str(&format!(
                r#"<w:numPr><w:ilvl w:val="{}"/><w:numId w:val="{num_id}"/></w:numPr>"#,
                li.level
            ));
        }
        if let Some(c) = pr.shading {
            out.push_str(&format!(
                r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
                hex(c)
            ));
        }
        if let Some(tab_stops) = tab_stops {
            out.push_str(&tab_stops);
        }
        if pr.bidi {
            out.push_str("<w:bidi/>");
        }
        write_spacing(out, sp, line_spacing);
        write_indent(out, ind);
        if let Some(j) = jc {
            out.push_str(&format!(r#"<w:jc w:val="{j}"/>"#));
        }
        if let Some(o) = outline {
            out.push_str(&format!(r#"<w:outlineLvl w:val="{o}"/>"#));
        }
        out.push_str("</w:pPr>");
    }

    fn register_source_style(&mut self, id: &str, name: &str) {
        if self.source_styles.iter().any(|style| style.id == id) {
            return;
        }
        self.source_styles.push(SourceParagraphStyle {
            id: id.to_string(),
            name: name.to_string(),
        });
    }

    fn write_run_with_column_breaks(
        &mut self,
        out: &mut String,
        r: &crate::model::Run,
        column_breaks: &mut ColumnBreakCursor<'_>,
        note_payload: Option<&NoteWritePayload>,
    ) {
        let comment_id = self.begin_comment(out, r.comment.as_ref());
        let deleted = matches!(
            r.revision.as_ref().map(|revision| revision.kind),
            Some(RevisionKind::Deletion)
        );
        let mut run_xml = String::new();
        match &r.field {
            FieldRole::Hyperlink { url } => match hyperlink_write_target(url) {
                HyperlinkWriteTarget::External(url) => {
                    let rid = self.add_rel(REL_HYPERLINK, url, true);
                    run_xml.push_str(&format!(r#"<w:hyperlink r:id="{rid}">"#));
                    self.write_run_inner(&mut run_xml, r, deleted, column_breaks);
                    run_xml.push_str("</w:hyperlink>");
                }
                HyperlinkWriteTarget::Anchor(anchor) => {
                    run_xml.push_str(&format!(r#"<w:hyperlink w:anchor="{}">"#, esc_attr(anchor)));
                    self.write_run_inner(&mut run_xml, r, deleted, column_breaks);
                    run_xml.push_str("</w:hyperlink>");
                }
                HyperlinkWriteTarget::Invalid => {
                    self.write_run_inner(&mut run_xml, r, deleted, column_breaks);
                }
            },
            FieldRole::Simple { instruction } => {
                let instruction = normalize_field_instruction(instruction);
                if instruction.is_empty() {
                    self.write_run_inner(&mut run_xml, r, deleted, column_breaks);
                } else {
                    let dirty = if r.field_dirty {
                        r#" w:dirty="true""#
                    } else {
                        ""
                    };
                    run_xml.push_str(&format!(
                        r#"<w:fldSimple w:instr=" {} "{dirty}>"#,
                        esc_attr(&instruction)
                    ));
                    self.write_run_inner(&mut run_xml, r, deleted, column_breaks);
                    run_xml.push_str("</w:fldSimple>");
                }
            }
            _ => self.write_run_inner(&mut run_xml, r, deleted, column_breaks),
        }
        let run_xml = content_control_wrapper(r.content_control.as_ref(), &run_xml);
        let run_xml = self.bookmark_wrapper(r.bookmark.as_deref(), &run_xml);
        self.write_revision_wrapper(out, r.revision.as_ref(), &run_xml);
        self.end_comment(out, comment_id);
        self.write_note_reference(out, r.note.as_ref(), note_payload);
    }

    fn begin_comment(
        &mut self,
        out: &mut String,
        comment: Option<&AuthoredComment>,
    ) -> Option<String> {
        let comment = comment.filter(|comment| !comment.text.is_empty())?;
        let id = self.comment_id.to_string();
        self.comment_id += 1;
        self.comments.push(WrittenComment {
            id: id.clone(),
            comment: comment.clone(),
        });
        out.push_str(&format!(r#"<w:commentRangeStart w:id="{id}"/>"#));
        Some(id)
    }

    fn end_comment(&mut self, out: &mut String, id: Option<String>) {
        if let Some(id) = id {
            out.push_str(&format!(
                r#"<w:commentRangeEnd w:id="{id}"/><w:r><w:commentReference w:id="{id}"/></w:r>"#
            ));
        }
    }

    fn render_note_payload(
        &mut self,
        note: &AuthoredNote,
        payload: &NoteWritePayload,
        rels: &mut Vec<Rel>,
    ) -> Option<String> {
        if !source_note_payload_is_supported(note, payload) {
            return None;
        }
        let block_count = payload.blocks.len();
        let pagination =
            (payload.pagination.len() == block_count).then_some(payload.pagination.as_slice());
        let line_spacing =
            (payload.line_spacing.len() == block_count).then_some(payload.line_spacing.as_slice());
        let tab_stops =
            (payload.tab_stops.len() == block_count).then_some(payload.tab_stops.as_slice());
        let column_break_offsets = (payload.column_break_offsets.len() == block_count)
            .then_some(payload.column_break_offsets.as_slice());
        let table_pagination = (payload.table_pagination.len() == block_count)
            .then_some(payload.table_pagination.as_slice());
        let mut body = String::new();
        for (index, block) in payload.blocks.iter().enumerate() {
            let table_hints = table_pagination
                .and_then(|hints| hints.get(index))
                .and_then(Option::as_ref);
            self.write_hf_block(
                &mut body,
                block,
                rels,
                RunningBlockSlotWriteHints {
                    line_spacing: line_spacing
                        .and_then(|hints| hints.get(index))
                        .copied()
                        .flatten(),
                    pagination: pagination.and_then(|hints| hints.get(index)).copied(),
                    tab_stops: tab_stops
                        .and_then(|hints| hints.get(index))
                        .map(Vec::as_slice),
                    table_row_pagination: table_hints.map(|hints| hints.rows.as_slice()),
                    table_cell_pagination: table_hints.map(|hints| &hints.cells),
                    table_cell_line_spacing: table_hints.map(|hints| &hints.cell_line_spacing),
                    table_cell_tab_stops: table_hints.map(|hints| &hints.cell_tabs),
                    table_cell_column_breaks: table_hints.map(|hints| &hints.cell_column_breaks),
                    table_nested: table_hints.map(|hints| &hints.nested),
                    column_break_offsets: column_break_offsets
                        .and_then(|hints| hints.get(index))
                        .map(Vec::as_slice),
                },
            );
        }
        Some(body)
    }

    fn write_note_reference(
        &mut self,
        out: &mut String,
        note: Option<&AuthoredNote>,
        payload: Option<&NoteWritePayload>,
    ) {
        let Some(note) = note else {
            return;
        };
        let body_xml = match note.kind {
            NoteKind::Footnote => {
                let mut rels = std::mem::take(&mut self.footnote_rels);
                let body_xml =
                    payload.and_then(|payload| self.render_note_payload(note, payload, &mut rels));
                self.footnote_rels = rels;
                body_xml
            }
            NoteKind::Endnote => {
                let mut rels = std::mem::take(&mut self.endnote_rels);
                let body_xml =
                    payload.and_then(|payload| self.render_note_payload(note, payload, &mut rels));
                self.endnote_rels = rels;
                body_xml
            }
        };
        let (tag, notes) = match note.kind {
            NoteKind::Footnote => ("footnoteReference", &mut self.footnotes),
            NoteKind::Endnote => ("endnoteReference", &mut self.endnotes),
        };
        let id = (notes.len() + 1).to_string();
        notes.push(WrittenNote {
            id: id.clone(),
            text: note.text.clone(),
            body_xml,
        });
        out.push_str(&format!(r#"<w:r><w:{tag} w:id="{id}"/></w:r>"#));
    }

    fn bookmark_wrapper(&mut self, name: Option<&str>, run_xml: &str) -> String {
        let Some(name) = name.filter(|name| referenceable_bookmark_name(name)) else {
            return run_xml.to_string();
        };
        let id = self.bookmark_id;
        self.bookmark_id += 1;
        format!(
            r#"<w:bookmarkStart w:id="{id}" w:name="{}"/>{run_xml}<w:bookmarkEnd w:id="{id}"/>"#,
            esc_attr(name)
        )
    }

    fn write_revision_wrapper(
        &mut self,
        out: &mut String,
        revision: Option<&AuthoredRevision>,
        run_xml: &str,
    ) {
        let Some(revision) = revision else {
            out.push_str(run_xml);
            return;
        };
        let tag = match revision.kind {
            RevisionKind::Insertion => "ins",
            RevisionKind::Deletion => "del",
            _ => {
                out.push_str(run_xml);
                return;
            }
        };
        let id = self.revision_id;
        self.revision_id += 1;
        let mut attrs = format!(r#" w:id="{id}""#);
        if let Some(author) = non_empty_trimmed(revision.author.as_deref()) {
            attrs.push_str(&format!(r#" w:author="{}""#, esc_attr(author)));
        }
        if let Some(date) = non_empty_trimmed(revision.date.as_deref()) {
            attrs.push_str(&format!(r#" w:date="{}""#, esc_attr(date)));
        }
        out.push_str(&format!("<w:{tag}{attrs}>"));
        out.push_str(run_xml);
        out.push_str(&format!("</w:{tag}>"));
    }

    fn write_run_inner(
        &mut self,
        out: &mut String,
        r: &crate::model::Run,
        deleted: bool,
        column_breaks: &mut ColumnBreakCursor<'_>,
    ) {
        if let Some(img) = &r.image {
            if img.bytes.is_some() {
                self.write_image(out, img);
                column_breaks.skip_text(&r.text);
                return;
            } else if r.text.is_empty() {
                write_missing_image_placeholder(out, img);
                return;
            }
        }
        out.push_str("<w:r>");
        write_rpr(out, &r.props);
        if deleted {
            write_run_deleted_text_with_column_breaks(out, &r.text, column_breaks);
        } else {
            write_run_text_with_column_breaks(out, &r.text, column_breaks);
        }
        out.push_str("</w:r>");
    }

    fn write_image_or_placeholder(&mut self, out: &mut String, img: &Image) {
        if img.bytes.is_some() {
            self.write_image(out, img);
        } else {
            write_missing_image_placeholder(out, img);
        }
    }

    fn write_hf_image_or_placeholder(
        &mut self,
        out: &mut String,
        img: &Image,
        rels: &mut Vec<Rel>,
    ) {
        if img.bytes.is_some() {
            self.write_image_inner(out, img, Some(rels));
        } else {
            write_image_placeholder(out, img, "image unavailable");
        }
    }

    fn write_image(&mut self, out: &mut String, img: &Image) {
        self.write_image_inner(out, img, None);
    }

    fn write_image_inner(
        &mut self,
        out: &mut String,
        img: &Image,
        part_rels: Option<&mut Vec<Rel>>,
    ) {
        let Some(bytes) = img.bytes.clone() else {
            return;
        };
        let (ext, ct) = img_ext_ct(img.mime.as_deref());
        self.img_id += 1;
        let n = self.img_id;
        self.drawing_id += 1;
        let drawing_id = self.drawing_id;
        let target = format!("media/image{n}.{ext}");
        let rid = if let Some(rels) = part_rels {
            add_part_rel(rels, REL_IMAGE, &target, false)
        } else {
            self.add_rel(REL_IMAGE, &target, false)
        };
        self.media.push((format!("word/{target}"), bytes, ext, ct));
        // Extent (EMU) from the image's intrinsic pixels at 96 dpi (1px = 9525
        // EMU), clamped to the ~6in content width; falls back to 2in² if the
        // header had no dimensions.
        let (cx, cy) = image_extent_emu(img.width_px, img.height_px);
        let descr = non_empty_trimmed(img.alt.as_deref())
            .map(|alt| format!(r#" descr="{}""#, esc_attr(alt)))
            .unwrap_or_default();
        let rotation = img
            .rotation_degrees
            .map(|degrees| format!(r#" rot="{}""#, i64::from(degrees.rem_euclid(360)) * 60_000))
            .unwrap_or_default();
        let graphic = format!(
            concat!(
                r#"<a:graphic><a:graphicData uri="{uri}"><pic:pic><pic:nvPicPr>"#,
                r#"<pic:cNvPr id="{n}" name="Image{n}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                r#"<pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                r#"<pic:spPr><a:xfrm{rotation}><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>"#,
                r#"</pic:pic></a:graphicData></a:graphic>"#,
            ),
            cx = cx,
            cy = cy,
            n = n,
            rotation = rotation,
            uri = PIC_URI,
            rid = rid
        );
        if let Some((x_emu, y_emu)) = img.floating_offset_emu {
            out.push_str(&format!(
                concat!(
                    r#"<w:r><w:drawing><wp:anchor simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1" distT="0" distB="0" distL="0" distR="0">"#,
                    r#"<wp:simplePos x="0" y="0"/>"#,
                    r#"<wp:positionH relativeFrom="page"><wp:posOffset>{x_emu}</wp:posOffset></wp:positionH>"#,
                    r#"<wp:positionV relativeFrom="page"><wp:posOffset>{y_emu}</wp:posOffset></wp:positionV>"#,
                    r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>"#,
                    r#"<wp:wrapSquare wrapText="bothSides"/><wp:docPr id="{drawing_id}" name="Image{n}"{descr}/>"#,
                    r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>"#,
                    r#"{graphic}</wp:anchor></w:drawing></w:r>"#,
                ),
                cx = cx,
                cy = cy,
                x_emu = x_emu,
                y_emu = y_emu,
                n = n,
                drawing_id = drawing_id,
                descr = descr,
                graphic = graphic
            ));
        } else {
            out.push_str(&format!(
                concat!(
                    r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                    r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{drawing_id}" name="Image{n}"{descr}/>"#,
                    r#"{graphic}</wp:inline></w:drawing></w:r>"#,
                ),
                cx = cx,
                cy = cy,
                n = n,
                drawing_id = drawing_id,
                descr = descr,
                graphic = graphic
            ));
        }
    }

    fn write_chart(&mut self, out: &mut String, chart: &Chart) {
        self.write_chart_inner(out, chart, None);
    }

    fn write_chart_inner(
        &mut self,
        out: &mut String,
        chart: &Chart,
        part_rels: Option<&mut Vec<Rel>>,
    ) {
        self.chart_id += 1;
        let chart_id = self.chart_id;
        self.drawing_id += 1;
        let drawing_id = self.drawing_id;
        let (target, rel_type, graphic_tag, graphic_uri) = if is_chart_ex_kind(chart.kind) {
            let target = format!("charts/chartEx{chart_id}.xml");
            self.chart_parts.push((
                format!("word/{target}"),
                CT_CHART_EX,
                chart_ex_xml(chart, chart_id).into_bytes(),
            ));
            (target, REL_CHART_EX, "cx:chart", CX_NS)
        } else {
            let target = format!("charts/chart{chart_id}.xml");
            let workbook_name = format!("Microsoft_Excel_Worksheet{chart_id}.xlsx");
            let workbook_rid = "rId1".to_string();
            self.chart_rels.push((
                format!("word/charts/_rels/chart{chart_id}.xml.rels"),
                vec![Rel {
                    id: workbook_rid.clone(),
                    rel_type: REL_PACKAGE.to_string(),
                    target: format!("../embeddings/{workbook_name}"),
                    external: false,
                }],
            ));
            self.embedded_workbooks.push((
                format!("word/embeddings/{workbook_name}"),
                chart_workbook_xlsx(chart),
            ));
            self.chart_parts.push((
                format!("word/{target}"),
                CT_CHART,
                chart_xml(chart, chart_id, Some(&workbook_rid)).into_bytes(),
            ));
            (target, REL_CHART, "c:chart", C_NS)
        };
        let rid = if let Some(rels) = part_rels {
            add_part_rel(rels, rel_type, &target, false)
        } else {
            self.add_rel(rel_type, &target, false)
        };

        let (cx, cy) = image_extent_emu(chart.width_px, chart.height_px);
        let descr = non_empty_trimmed(chart.alt.as_deref())
            .map(|alt| format!(r#" descr="{}""#, esc_attr(alt)))
            .unwrap_or_default();
        out.push_str(&format!(
            concat!(
                r#"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{drawing_id}" name="Chart{chart_id}"{descr}/>"#,
                r#"<a:graphic><a:graphicData uri="{uri}"><{graphic_tag} r:id="{rid}"/></a:graphicData></a:graphic>"#,
                r#"</wp:inline></w:drawing></w:r></w:p>"#,
            ),
            cx = cx,
            cy = cy,
            drawing_id = drawing_id,
            chart_id = chart_id,
            descr = descr,
            uri = graphic_uri,
            graphic_tag = graphic_tag,
            rid = rid
        ));
    }

    /// Cell content: at least one paragraph; a cell ending in a table needs a
    /// trailing empty paragraph (OOXML requires `w:tc` to end with `w:p`).
    fn write_cell_blocks_with_source_hints(
        &mut self,
        out: &mut String,
        blocks: &[Block],
        hints: CellWriteHints<'_>,
    ) {
        if blocks.is_empty() {
            out.push_str("<w:p/>");
            return;
        }
        for (index, block) in blocks.iter().enumerate() {
            match block {
                Block::Paragraph(paragraph) => self.write_paragraph_with_source_hints(
                    out,
                    paragraph,
                    ParagraphWriteHints {
                        line_spacing: hints
                            .line_spacing
                            .and_then(|hints| hints.get(index))
                            .copied()
                            .flatten(),
                        pagination: hints
                            .pagination
                            .and_then(|hints| hints.get(index))
                            .copied()
                            .flatten(),
                        tab_stops: hints
                            .tab_stops
                            .and_then(|hints| hints.get(index))
                            .map(Vec::as_slice),
                        column_break_offsets: hints
                            .column_break_offsets
                            .and_then(|hints| hints.get(index))
                            .map(Vec::as_slice),
                        note_payloads: None,
                    },
                ),
                Block::Table(table) => {
                    let nested = hints
                        .nested_tables
                        .and_then(|hints| hints.get(index))
                        .and_then(Option::as_ref);
                    if let Some(nested) = nested {
                        self.write_table_with_source_hints(
                            out,
                            table,
                            TableWriteHints {
                                row_pagination: Some(&nested.rows),
                                cell_pagination: Some(&nested.cells),
                                cell_line_spacing: Some(&nested.cell_line_spacing),
                                cell_column_breaks: Some(&nested.cell_column_breaks),
                                nested_tables: Some(&nested.nested),
                                cell_tab_stops: Some(&nested.cell_tabs),
                            },
                        );
                    } else {
                        self.write_table(out, table);
                    }
                }
                _ => self.write_block(out, block),
            }
        }
        if matches!(blocks.last(), Some(Block::Table(_))) {
            out.push_str("<w:p/>");
        }
    }

    fn write_hf_cell_blocks(
        &mut self,
        out: &mut String,
        blocks: &[Block],
        rels: &mut Vec<Rel>,
        hints: CellWriteHints<'_>,
    ) {
        if blocks.is_empty() {
            out.push_str("<w:p/>");
            return;
        }
        for (index, block) in blocks.iter().enumerate() {
            let nested = hints
                .nested_tables
                .and_then(|hints| hints.get(index))
                .and_then(Option::as_ref);
            self.write_hf_block(
                out,
                block,
                rels,
                RunningBlockSlotWriteHints {
                    line_spacing: hints
                        .line_spacing
                        .and_then(|hints| hints.get(index))
                        .copied()
                        .flatten(),
                    pagination: hints
                        .pagination
                        .and_then(|hints| hints.get(index))
                        .copied()
                        .flatten(),
                    tab_stops: hints
                        .tab_stops
                        .and_then(|hints| hints.get(index))
                        .map(Vec::as_slice),
                    table_row_pagination: nested.map(|hints| hints.rows.as_slice()),
                    table_cell_pagination: nested.map(|hints| &hints.cells),
                    table_cell_line_spacing: nested.map(|hints| &hints.cell_line_spacing),
                    table_cell_tab_stops: nested.map(|hints| &hints.cell_tabs),
                    table_cell_column_breaks: nested.map(|hints| &hints.cell_column_breaks),
                    table_nested: nested.map(|hints| &hints.nested),
                    column_break_offsets: hints
                        .column_break_offsets
                        .and_then(|hints| hints.get(index))
                        .map(Vec::as_slice),
                },
            );
        }
        if matches!(blocks.last(), Some(Block::Table(_))) {
            out.push_str("<w:p/>");
        }
    }

    fn write_cell_margins(out: &mut String, margins: CellMargins, bidi_visual: bool) {
        let (leading, trailing) = if bidi_visual {
            (margins.right, margins.left)
        } else {
            (margins.left, margins.right)
        };
        out.push_str("<w:tcMar>");
        out.push_str(&format!(r#"<w:top w:w="{}" w:type="dxa"/>"#, margins.top));
        out.push_str(&format!(r#"<w:left w:w="{leading}" w:type="dxa"/>"#));
        out.push_str(&format!(
            r#"<w:bottom w:w="{}" w:type="dxa"/>"#,
            margins.bottom
        ));
        out.push_str(&format!(r#"<w:right w:w="{}" w:type="dxa"/>"#, trailing));
        out.push_str("</w:tcMar>");
    }

    /// Write a table, reconstructing the full grid (re-inserting the `vMerge`
    /// continuation cells the reader dropped) so merges round-trip.
    fn write_table(&mut self, out: &mut String, t: &Table) {
        self.write_table_inner(out, t, None, TableWriteHints::default());
    }

    fn write_table_with_source_hints(
        &mut self,
        out: &mut String,
        table: &Table,
        hints: TableWriteHints<'_>,
    ) {
        let row_pagination = hints
            .row_pagination
            .filter(|hints| hints.len() == table.rows.len());
        let cell_pagination = hints
            .cell_pagination
            .filter(|hints| Self::table_cell_paragraph_hints_align(table, hints));
        let cell_line_spacing = hints
            .cell_line_spacing
            .filter(|hints| Self::table_cell_paragraph_hints_align(table, hints));
        let cell_column_breaks = hints
            .cell_column_breaks
            .filter(|hints| Self::table_cell_column_break_hints_align(table, hints));
        let nested_tables = hints
            .nested_tables
            .filter(|hints| Self::table_cell_nested_hints_align(table, hints));
        let cell_tab_stops = hints
            .cell_tab_stops
            .filter(|hints| Self::table_cell_tab_stops_align(table, hints));
        self.write_table_inner(
            out,
            table,
            None,
            TableWriteHints {
                row_pagination,
                cell_pagination,
                cell_line_spacing,
                cell_column_breaks,
                nested_tables,
                cell_tab_stops,
            },
        );
    }

    fn table_cell_paragraph_hints_align<T>(table: &Table, hints: &[Vec<Vec<Option<T>>>]) -> bool {
        if hints.len() != table.rows.len() {
            return false;
        }
        for (row, row_hints) in table.rows.iter().zip(hints) {
            if row_hints.len() != row.cells.len() {
                return false;
            }
            for (cell, cell_hints) in row.cells.iter().zip(row_hints) {
                if cell_hints.len() != cell.blocks.len()
                    || cell.blocks.iter().zip(cell_hints).any(|(block, hint)| {
                        hint.is_some() && !matches!(block, Block::Paragraph(_))
                    })
                {
                    return false;
                }
            }
        }
        true
    }

    fn table_cell_tab_stops_align(table: &Table, hints: &TableCellTabStopHints) -> bool {
        if hints.len() != table.rows.len() {
            return false;
        }
        for (row, row_hints) in table.rows.iter().zip(hints) {
            if row_hints.len() != row.cells.len() {
                return false;
            }
            for (cell, cell_hints) in row.cells.iter().zip(row_hints) {
                if cell_hints.len() != cell.blocks.len()
                    || cell.blocks.iter().zip(cell_hints).any(|(block, stops)| {
                        !stops.is_empty() && !matches!(block, Block::Paragraph(_))
                    })
                {
                    return false;
                }
            }
        }
        true
    }

    fn table_cell_column_break_hints_align(
        table: &Table,
        hints: &TableCellColumnBreakHints,
    ) -> bool {
        if hints.len() != table.rows.len() {
            return false;
        }
        for (row, row_hints) in table.rows.iter().zip(hints) {
            if row_hints.len() != row.cells.len() {
                return false;
            }
            for (cell, cell_hints) in row.cells.iter().zip(row_hints) {
                if cell_hints.len() != cell.blocks.len()
                    || cell.blocks.iter().zip(cell_hints).any(|(block, offsets)| {
                        !offsets.is_empty() && !matches!(block, Block::Paragraph(_))
                    })
                {
                    return false;
                }
            }
        }
        true
    }

    fn table_cell_nested_hints_align(
        table: &Table,
        hints: &TableCellNestedPaginationHints,
    ) -> bool {
        if hints.len() != table.rows.len() {
            return false;
        }
        for (row, row_hints) in table.rows.iter().zip(hints) {
            if row_hints.len() != row.cells.len() {
                return false;
            }
            for (cell, cell_hints) in row.cells.iter().zip(row_hints) {
                if cell_hints.len() != cell.blocks.len()
                    || cell
                        .blocks
                        .iter()
                        .zip(cell_hints)
                        .any(|(block, hint)| hint.is_some() && !matches!(block, Block::Table(_)))
                {
                    return false;
                }
            }
        }
        true
    }

    fn write_table_inner(
        &mut self,
        out: &mut String,
        t: &Table,
        mut hf_rels: Option<&mut Vec<Rel>>,
        hints: TableWriteHints<'_>,
    ) {
        struct Active {
            col: usize,
            span: usize,
            rows_left: usize,
        }
        let mut active: Vec<Active> = Vec::new();
        let mut rows_xml = String::new();
        let mut ncols = 0usize;

        for (ri, row) in t.rows.iter().enumerate() {
            let is_header = ri < t.header_rows;
            let cant_split = hints
                .row_pagination
                .and_then(|hints| hints.get(ri))
                .is_some_and(|hint| hint.cant_split);
            let mut row_xml = String::new();
            let mut col = 0usize;
            let mut ci = 0usize;
            let mut carried: Vec<Active> = Vec::new();

            loop {
                if col >= MAX_TABLE_COLS {
                    break;
                }
                if let Some(pos) = active.iter().position(|a| a.col == col) {
                    let a = active.remove(pos);
                    row_xml.push_str("<w:tc><w:tcPr>");
                    if a.span > 1 {
                        row_xml.push_str(&format!(r#"<w:gridSpan w:val="{}"/>"#, a.span));
                    }
                    row_xml.push_str("<w:vMerge/></w:tcPr><w:p/></w:tc>");
                    col += a.span;
                    if a.rows_left > 1 {
                        carried.push(Active {
                            col: a.col,
                            span: a.span,
                            rows_left: a.rows_left - 1,
                        });
                    }
                    continue;
                }
                if ci < row.cells.len() {
                    let source_pagination = hints
                        .cell_pagination
                        .and_then(|rows| rows.get(ri))
                        .and_then(|row| row.get(ci))
                        .map(Vec::as_slice);
                    let source_line_spacing = hints
                        .cell_line_spacing
                        .and_then(|rows| rows.get(ri))
                        .and_then(|row| row.get(ci))
                        .map(Vec::as_slice);
                    let source_column_breaks = hints
                        .cell_column_breaks
                        .and_then(|rows| rows.get(ri))
                        .and_then(|row| row.get(ci))
                        .map(Vec::as_slice);
                    let source_nested_tables = hints
                        .nested_tables
                        .and_then(|rows| rows.get(ri))
                        .and_then(|row| row.get(ci))
                        .map(Vec::as_slice);
                    let source_tab_stops = hints
                        .cell_tab_stops
                        .and_then(|rows| rows.get(ri))
                        .and_then(|row| row.get(ci))
                        .map(Vec::as_slice);
                    let c = &row.cells[ci];
                    ci += 1;
                    let span = (c.col_span.max(1) as usize).min(MAX_TABLE_COLS);
                    let rs = (c.row_span.max(1) as usize).min(MAX_TABLE_COLS);
                    row_xml.push_str("<w:tc><w:tcPr>");
                    if let Some(p) = c.width_pct {
                        let w = (p.clamp(0.0, 1.0) * 5000.0).round() as i64;
                        row_xml.push_str(&format!(r#"<w:tcW w:w="{w}" w:type="pct"/>"#));
                    }
                    if span > 1 {
                        row_xml.push_str(&format!(r#"<w:gridSpan w:val="{span}"/>"#));
                    }
                    if rs > 1 {
                        row_xml.push_str(r#"<w:vMerge w:val="restart"/>"#);
                    }
                    if let Some(col) = c.shading {
                        row_xml.push_str(&format!(
                            r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
                            hex(col)
                        ));
                    }
                    if let Some(margins) = c.margins {
                        Self::write_cell_margins(&mut row_xml, margins, t.bidi_visual);
                    }
                    match c.valign {
                        crate::model::VCell::Center => {
                            row_xml.push_str(r#"<w:vAlign w:val="center"/>"#)
                        }
                        crate::model::VCell::Bottom => {
                            row_xml.push_str(r#"<w:vAlign w:val="bottom"/>"#)
                        }
                        crate::model::VCell::Top => {}
                    }
                    row_xml.push_str("</w:tcPr>");
                    if let Some(rels) = hf_rels.as_deref_mut() {
                        self.write_hf_cell_blocks(
                            &mut row_xml,
                            &c.blocks,
                            rels,
                            CellWriteHints {
                                pagination: source_pagination,
                                line_spacing: source_line_spacing,
                                column_break_offsets: source_column_breaks,
                                nested_tables: source_nested_tables,
                                tab_stops: source_tab_stops,
                            },
                        );
                    } else {
                        self.write_cell_blocks_with_source_hints(
                            &mut row_xml,
                            &c.blocks,
                            CellWriteHints {
                                pagination: source_pagination,
                                line_spacing: source_line_spacing,
                                column_break_offsets: source_column_breaks,
                                nested_tables: source_nested_tables,
                                tab_stops: source_tab_stops,
                            },
                        );
                    }
                    row_xml.push_str("</w:tc>");
                    if rs > 1 {
                        carried.push(Active {
                            col,
                            span,
                            rows_left: rs - 1,
                        });
                    }
                    col += span;
                    continue;
                }
                break;
            }
            ncols = ncols.max(col);
            active.extend(carried);
            active.sort_by_key(|a| a.col);

            rows_xml.push_str("<w:tr>");
            if cant_split || is_header {
                rows_xml.push_str("<w:trPr>");
                if cant_split {
                    rows_xml.push_str("<w:cantSplit/>");
                }
                if is_header {
                    rows_xml.push_str("<w:tblHeader/>");
                }
                rows_xml.push_str("</w:trPr>");
            }
            rows_xml.push_str(&row_xml);
            rows_xml.push_str("</w:tr>");
        }

        let ncols = ncols.max(1);
        out.push_str("<w:tbl><w:tblPr>");
        if t.bidi_visual {
            out.push_str("<w:bidiVisual/>");
        }
        if let Some(width_pct) = t.width_pct {
            let w = (width_pct.clamp(0.0, 1.0) * 5000.0).round() as i64;
            out.push_str(&format!(r#"<w:tblW w:w="{w}" w:type="pct"/>"#));
        } else {
            out.push_str(r#"<w:tblW w:w="0" w:type="auto"/>"#);
        }
        if let Some(indent) = t.indent_twips {
            out.push_str(&format!(r#"<w:tblInd w:w="{indent}" w:type="dxa"/>"#));
        }
        if let Some(align) = t.align {
            let val = match align {
                Align::Left => "left",
                Align::Center => "center",
                Align::Right => "right",
                Align::Justify => "both",
            };
            out.push_str(&format!(r#"<w:jc w:val="{val}"/>"#));
        }
        let top_border_color = table_border_color(t, TableBorderSide::Top);
        let left_border_color = table_border_color(t, TableBorderSide::Left);
        let bottom_border_color = table_border_color(t, TableBorderSide::Bottom);
        let right_border_color = table_border_color(t, TableBorderSide::Right);
        let inside_h_border_color = table_border_color(t, TableBorderSide::InsideHorizontal);
        let inside_v_border_color = table_border_color(t, TableBorderSide::InsideVertical);
        let top_border_size = table_border_size(t, TableBorderSide::Top);
        let left_border_size = table_border_size(t, TableBorderSide::Left);
        let bottom_border_size = table_border_size(t, TableBorderSide::Bottom);
        let right_border_size = table_border_size(t, TableBorderSide::Right);
        let inside_h_border_size = table_border_size(t, TableBorderSide::InsideHorizontal);
        let inside_v_border_size = table_border_size(t, TableBorderSide::InsideVertical);
        let top_border_style = table_border_style(t, TableBorderSide::Top).wml_value();
        let left_border_style = table_border_style(t, TableBorderSide::Left).wml_value();
        let bottom_border_style = table_border_style(t, TableBorderSide::Bottom).wml_value();
        let right_border_style = table_border_style(t, TableBorderSide::Right).wml_value();
        let inside_h_border_style =
            table_border_style(t, TableBorderSide::InsideHorizontal).wml_value();
        let inside_v_border_style =
            table_border_style(t, TableBorderSide::InsideVertical).wml_value();
        out.push_str(&format!(
            concat!(
                r#"<w:tblBorders>"#,
                r#"<w:top w:val="{top_border_style}" w:sz="{top_border_size}" w:space="0" w:color="{top_border_color}"/>"#,
                r#"<w:left w:val="{left_border_style}" w:sz="{left_border_size}" w:space="0" w:color="{left_border_color}"/>"#,
                r#"<w:bottom w:val="{bottom_border_style}" w:sz="{bottom_border_size}" w:space="0" w:color="{bottom_border_color}"/>"#,
                r#"<w:right w:val="{right_border_style}" w:sz="{right_border_size}" w:space="0" w:color="{right_border_color}"/>"#,
                r#"<w:insideH w:val="{inside_h_border_style}" w:sz="{inside_h_border_size}" w:space="0" w:color="{inside_h_border_color}"/>"#,
                r#"<w:insideV w:val="{inside_v_border_style}" w:sz="{inside_v_border_size}" w:space="0" w:color="{inside_v_border_color}"/>"#,
                r#"</w:tblBorders>"#,
            ),
            top_border_size = top_border_size,
            left_border_size = left_border_size,
            bottom_border_size = bottom_border_size,
            right_border_size = right_border_size,
            inside_h_border_size = inside_h_border_size,
            inside_v_border_size = inside_v_border_size,
            top_border_style = top_border_style,
            left_border_style = left_border_style,
            bottom_border_style = bottom_border_style,
            right_border_style = right_border_style,
            inside_h_border_style = inside_h_border_style,
            inside_v_border_style = inside_v_border_style,
            top_border_color = top_border_color,
            left_border_color = left_border_color,
            bottom_border_color = bottom_border_color,
            right_border_color = right_border_color,
            inside_h_border_color = inside_h_border_color,
            inside_v_border_color = inside_v_border_color
        ));
        if t.fixed_layout {
            out.push_str(r#"<w:tblLayout w:type="fixed"/>"#);
        }
        out.push_str("</w:tblPr><w:tblGrid>");
        if let Some(widths) = authored_table_grid_widths(&t.col_widths_pct, ncols) {
            for width in widths {
                out.push_str(&format!(r#"<w:gridCol w:w="{width}"/>"#));
            }
        } else {
            let colw = (TABLE_GRID_TWIPS / ncols as u32).max(1);
            for _ in 0..ncols {
                out.push_str(&format!(r#"<w:gridCol w:w="{colw}"/>"#));
            }
        }
        out.push_str("</w:tblGrid>");
        out.push_str(&rows_xml);
        out.push_str("</w:tbl>");
    }
}

fn authored_table_grid_widths(widths: &[f32], ncols: usize) -> Option<Vec<u32>> {
    if widths.len() != ncols
        || widths
            .iter()
            .any(|width| !width.is_finite() || *width <= 0.0)
    {
        return None;
    }
    let sum = widths.iter().map(|width| f64::from(*width)).sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 || ncols as u32 >= TABLE_GRID_TWIPS {
        return None;
    }

    let mut out = Vec::with_capacity(ncols);
    let mut cumulative = 0.0_f64;
    let mut allocated = 0u32;
    for (index, width) in widths.iter().enumerate() {
        cumulative += f64::from(*width);
        let remaining = (ncols - index - 1) as u32;
        let minimum_target = allocated + 1;
        let maximum_target = TABLE_GRID_TWIPS - remaining;
        let target = if index + 1 == ncols {
            TABLE_GRID_TWIPS
        } else {
            (cumulative / sum * f64::from(TABLE_GRID_TWIPS)).round() as u32
        }
        .clamp(minimum_target, maximum_target);
        out.push(target - allocated);
        allocated = target;
    }
    Some(out)
}

/// Write `<w:rPr>` toggles in schema order (b, i, strike, vanish, u). Free
/// function (no `Ctx` state needed).
fn write_rpr(out: &mut String, p: &CharProps) {
    let font = non_empty_trimmed(p.font.as_deref());
    let highlight = non_empty_trimmed(p.highlight.as_deref());
    let has = p.bold
        || p.italic
        || p.underline
        || p.strike
        || p.hidden
        || p.small_caps
        || p.caps
        || p.rtl
        || font.is_some()
        || p.size_half_pt.is_some()
        || p.color.is_some()
        || highlight.is_some()
        || p.vert_align != VertAlign::Baseline;
    if !has {
        return;
    }
    out.push_str("<w:rPr>");
    // Schema order: rFonts, b, i, smallCaps, strike, vanish, color, sz/szCs,
    // highlight, u, vertAlign, rtl.
    if let Some(f) = font {
        let f = esc_attr(f);
        out.push_str(&format!(
            r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:eastAsia="{f}" w:cs="{f}"/>"#
        ));
    }
    if p.bold {
        out.push_str("<w:b/>");
    }
    if p.italic {
        out.push_str("<w:i/>");
    }
    if p.small_caps {
        out.push_str("<w:smallCaps/>");
    }
    if p.caps {
        out.push_str("<w:caps/>");
    }
    if p.strike {
        out.push_str("<w:strike/>");
    }
    if p.hidden {
        out.push_str("<w:vanish/>");
    }
    if let Some(c) = p.color {
        out.push_str(&format!(r#"<w:color w:val="{}"/>"#, hex(c)));
    }
    if let Some(sz) = p.size_half_pt {
        out.push_str(&format!(r#"<w:sz w:val="{sz}"/><w:szCs w:val="{sz}"/>"#));
    }
    if let Some(h) = highlight {
        out.push_str(&format!(r#"<w:highlight w:val="{}"/>"#, esc_attr(h)));
    }
    if p.underline {
        out.push_str(r#"<w:u w:val="single"/>"#);
    }
    match p.vert_align {
        VertAlign::Super => out.push_str(r#"<w:vertAlign w:val="superscript"/>"#),
        VertAlign::Sub => out.push_str(r#"<w:vertAlign w:val="subscript"/>"#),
        VertAlign::Baseline => {}
    }
    if p.rtl {
        out.push_str("<w:rtl/>");
    }
    out.push_str("</w:rPr>");
}

/// Write a run's text, mapping `\t` → `<w:tab/>` and `\n` → `<w:br/>` (the dual
/// of the reader) and dropping XML-invalid control characters.
fn write_run_text(out: &mut String, text: &str) {
    write_run_text_element(out, text, "w:t");
}

fn write_run_text_with_column_breaks(
    out: &mut String,
    text: &str,
    column_breaks: &mut ColumnBreakCursor<'_>,
) {
    write_run_text_element_with_column_breaks(out, text, "w:t", Some(column_breaks));
}

fn write_run_deleted_text(out: &mut String, text: &str) {
    write_run_text_element(out, text, "w:delText");
}

fn write_run_deleted_text_with_column_breaks(
    out: &mut String,
    text: &str,
    column_breaks: &mut ColumnBreakCursor<'_>,
) {
    write_run_text_element_with_column_breaks(out, text, "w:delText", Some(column_breaks));
}

fn write_run_text_element(out: &mut String, text: &str, tag: &str) {
    write_run_text_element_with_column_breaks(out, text, tag, None);
}

fn write_run_text_element_with_column_breaks(
    out: &mut String,
    text: &str,
    tag: &str,
    mut column_breaks: Option<&mut ColumnBreakCursor<'_>>,
) {
    let mut buf = String::new();
    let flush = |out: &mut String, buf: &mut String| {
        if !buf.is_empty() {
            out.push_str(&format!(r#"<{tag} xml:space="preserve">"#));
            out.push_str(&esc_text(buf));
            out.push_str(&format!("</{tag}>"));
            buf.clear();
        }
    };
    for ch in text.chars() {
        let is_column_break = column_breaks
            .as_deref_mut()
            .is_some_and(ColumnBreakCursor::advance);
        match ch {
            '\t' => {
                flush(out, &mut buf);
                out.push_str("<w:tab/>");
            }
            '\n' => {
                flush(out, &mut buf);
                if is_column_break {
                    out.push_str(r#"<w:br w:type="column"/>"#);
                } else {
                    out.push_str("<w:br/>");
                }
            }
            '\r' => {}
            c if (c as u32) < 0x20 => {}
            c => buf.push(c),
        }
    }
    flush(out, &mut buf);
}

fn write_missing_image_placeholder(out: &mut String, img: &Image) {
    write_image_placeholder(out, img, "bytes unavailable");
}

fn write_image_placeholder(out: &mut String, img: &Image, fallback: &str) {
    out.push_str("<w:r>");
    write_run_text(out, &image_placeholder_text(img, fallback));
    out.push_str("</w:r>");
}

fn image_placeholder_text(img: &Image, fallback: &str) -> String {
    let label = non_empty_trimmed(img.alt.as_deref()).unwrap_or(fallback);
    format!("[rwml image placeholder: {label}]")
}

fn chart_placeholder_text(chart: &Chart) -> String {
    let label = non_empty_trimmed(chart.alt.as_deref())
        .or_else(|| non_empty_trimmed(chart.title.as_deref()))
        .unwrap_or("chart unavailable");
    format!("[rwml chart placeholder: {label}]")
}

/// Extension + content type for an image MIME (reverse of the reader's
/// `mime_for`); unknown -> PNG.
fn img_ext_ct(mime: Option<&str>) -> (&'static str, &'static str) {
    match mime {
        Some("image/jpeg") => ("jpg", "image/jpeg"),
        Some("image/gif") => ("gif", "image/gif"),
        Some("image/bmp") => ("bmp", "image/bmp"),
        Some("image/tiff") => ("tif", "image/tiff"),
        Some("image/webp") => ("webp", "image/webp"),
        _ => ("png", "image/png"),
    }
}

/// The synthetic `word/numbering.xml`: numId 1 = ordered (decimal), numId 2 =
/// bullet, every level 0–8 declared so the reader resolves `ordered` exactly.
fn numbering_xml() -> String {
    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(&format!(r#"<w:numbering xmlns:w="{W_NS}">"#));
    for (aid, fmt, txt) in [(0u8, "decimal", "%1."), (1u8, "bullet", "\u{2022}")] {
        s.push_str(&format!(r#"<w:abstractNum w:abstractNumId="{aid}">"#));
        for lvl in 0u8..9 {
            s.push_str(&format!(
                r#"<w:lvl w:ilvl="{lvl}"><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{txt}"/><w:lvlJc w:val="left"/></w:lvl>"#
            ));
        }
        s.push_str("</w:abstractNum>");
    }
    s.push_str(r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#);
    s.push_str(r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#);
    s.push_str("</w:numbering>");
    s
}

fn comments_xml(comments: &[WrittenComment]) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(XML_DECL);
    let threaded = has_comment_replies(comments);
    if threaded {
        s.push_str(&format!(
            r#"<w:comments xmlns:w="{W_NS}" xmlns:w14="{W14_NS}" xmlns:mc="{MC_NS}" mc:Ignorable="w14">"#
        ));
    } else {
        s.push_str(&format!(r#"<w:comments xmlns:w="{W_NS}">"#));
    }
    for (index, comment) in comments.iter().enumerate() {
        let mut attrs = format!(r#" w:id="{}""#, esc_attr(&comment.id));
        if let Some(author) = non_empty_trimmed(comment.comment.author.as_deref()) {
            attrs.push_str(&format!(r#" w:author="{}""#, esc_attr(author)));
        }
        if let Some(initials) = non_empty_trimmed(comment.comment.initials.as_deref()) {
            attrs.push_str(&format!(r#" w:initials="{}""#, esc_attr(initials)));
        }
        if let Some(date) = non_empty_trimmed(comment.comment.date.as_deref()) {
            attrs.push_str(&format!(r#" w:date="{}""#, esc_attr(date)));
        }
        if let Some(parent_id) = non_empty_trimmed(comment.comment.parent_comment_id.as_deref()) {
            attrs.push_str(&format!(r#" w:parentId="{}""#, esc_attr(parent_id)));
        }
        let para_id = if threaded {
            format!(r#" w14:paraId="{}""#, comment_para_id(index))
        } else {
            String::new()
        };
        s.push_str(&format!(r#"<w:comment{attrs}><w:p{para_id}><w:r>"#));
        write_comment_text(&mut s, &comment.comment.text);
        s.push_str("</w:r></w:p></w:comment>");
    }
    s.push_str("</w:comments>");
    s.into_bytes()
}

fn comments_extended_xml(comments: &[WrittenComment]) -> Option<Vec<u8>> {
    if !has_comment_replies(comments) {
        return None;
    }
    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(&format!(r#"<w15:commentsEx xmlns:w15="{W15_NS}">"#));
    for (index, comment) in comments.iter().enumerate() {
        let para_id = comment_para_id(index);
        let mut attrs = format!(r#" w15:paraId="{para_id}""#);
        if let Some(parent_para_id) = comment_parent_para_id(comments, comment) {
            attrs.push_str(&format!(r#" w15:paraIdParent="{parent_para_id}""#));
        }
        s.push_str(&format!(r#"<w15:commentEx{attrs} w15:done="0"/>"#));
    }
    s.push_str("</w15:commentsEx>");
    Some(s.into_bytes())
}

fn has_comment_replies(comments: &[WrittenComment]) -> bool {
    comments
        .iter()
        .any(|comment| comment_parent_para_id(comments, comment).is_some())
}

fn comment_para_id(index: usize) -> String {
    format!("{:08X}", (index + 1).min(0x7FFF_FFFF))
}

fn comment_parent_para_id(comments: &[WrittenComment], comment: &WrittenComment) -> Option<String> {
    let parent_id = non_empty_trimmed(comment.comment.parent_comment_id.as_deref())?;
    comment_para_id_for_id(comments, parent_id)
}

fn comment_para_id_for_id(comments: &[WrittenComment], id: &str) -> Option<String> {
    comments
        .iter()
        .position(|comment| comment.id == id)
        .map(comment_para_id)
}

fn source_note_payload_is_supported(note: &AuthoredNote, payload: &NoteWritePayload) -> bool {
    if payload.kind != note.kind
        || payload.text.is_empty()
        || payload.text != note.text
        || payload.blocks.is_empty()
        || crate::docx::blocks_text(&payload.blocks) != payload.text
    {
        return false;
    }
    payload.blocks.iter().all(|block| match block {
        Block::Paragraph(paragraph) => source_note_paragraph_is_supported(paragraph),
        Block::Table(table) => source_note_table_is_supported(table),
        Block::Chart(chart) => crate::docx::note_write_chart_supported(chart),
        Block::PageBreak => true,
        _ => false,
    }) && source_note_local_ref_fields_are_supported(&payload.blocks)
        && source_note_style_ref_fields_are_supported(&payload.blocks)
        && source_note_table_formula_fields_are_supported(&payload.blocks)
}

fn source_note_local_ref_fields_are_supported(blocks: &[Block]) -> bool {
    let mut bookmarks: HashMap<String, Option<String>> = HashMap::new();
    visit_source_note_runs(blocks, &mut |run| {
        let Some(name) = run.bookmark.as_deref() else {
            return;
        };
        let target =
            (!run.text.is_empty() && run.image.is_none() && matches!(&run.field, FieldRole::None))
                .then(|| run.text.clone());
        bookmarks
            .entry(name.to_string())
            .and_modify(|text| *text = None)
            .or_insert(target);
    });
    let bookmarks = bookmarks
        .into_iter()
        .filter_map(|(name, text)| text.map(|text| (name, text)))
        .collect::<HashMap<_, _>>();

    let mut supported = true;
    visit_source_note_runs(blocks, &mut |run| {
        let FieldRole::Simple { instruction } = &run.field else {
            return;
        };
        if FieldKind::from_instruction(instruction) == FieldKind::Ref
            && computed_preserved_note_local_ref_result(instruction, &bookmarks).as_deref()
                != Some(run.text.as_str())
        {
            supported = false;
        }
    });
    supported
}

fn visit_source_note_runs(blocks: &[Block], visit: &mut impl FnMut(&crate::model::Run)) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => paragraph.runs.iter().for_each(&mut *visit),
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| &row.cells) {
                    visit_source_note_runs(&cell.blocks, visit);
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
struct SourceNoteStyleRefEntry {
    style_id: String,
    style_name: Option<String>,
    stable_text: Option<String>,
    order: usize,
}

#[derive(Debug)]
struct SourceNoteStyleRefField {
    instruction: String,
    result: String,
    order: usize,
    stable: bool,
}

fn source_note_style_ref_fields_are_supported(blocks: &[Block]) -> bool {
    let mut entries = Vec::new();
    let mut fields = Vec::new();
    let mut next_order = 0usize;
    collect_source_note_style_refs(blocks, &mut entries, &mut fields, &mut next_order);

    fields.into_iter().all(|field| {
        if !field.stable {
            return false;
        }
        let Some(target) = preserved_note_local_style_ref_target(&field.instruction) else {
            return false;
        };
        let Some(entry) = entries
            .iter()
            .rev()
            .find(|entry| {
                entry.order < field.order && source_note_style_ref_entry_matches(entry, &target)
            })
            .or_else(|| {
                entries.iter().find(|entry| {
                    entry.order > field.order && source_note_style_ref_entry_matches(entry, &target)
                })
            })
        else {
            return false;
        };
        let Some(source_text) = entry.stable_text.as_deref() else {
            return false;
        };
        computed_preserved_note_local_style_ref_result(
            &field.instruction,
            source_text,
            entry.order < field.order,
        )
        .as_deref()
            == Some(field.result.as_str())
    })
}

fn collect_source_note_style_refs(
    blocks: &[Block],
    entries: &mut Vec<SourceNoteStyleRefEntry>,
    fields: &mut Vec<SourceNoteStyleRefField>,
    next_order: &mut usize,
) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                for run in &paragraph.runs {
                    let FieldRole::Simple { instruction } = &run.field else {
                        continue;
                    };
                    if !matches!(
                        FieldKind::from_instruction(instruction),
                        FieldKind::DocumentStructure(kind) if kind == "STYLEREF"
                    ) {
                        continue;
                    }
                    fields.push(SourceNoteStyleRefField {
                        instruction: instruction.clone(),
                        result: run.text.clone(),
                        order: *next_order,
                        stable: !run.field_dirty && run.field_unsupported_reason.is_none(),
                    });
                    *next_order = next_order.saturating_add(1);
                }

                let Some(style_id) = non_empty_trimmed(paragraph.props.style_id.as_deref()) else {
                    continue;
                };
                let has_source_text = paragraph.runs.iter().any(|run| {
                    !source_note_run_is_style_ref(run)
                        && run.text.split_whitespace().next().is_some()
                });
                if !has_source_text {
                    continue;
                }
                entries.push(SourceNoteStyleRefEntry {
                    style_id: style_id.to_string(),
                    style_name: non_empty_trimmed(paragraph.props.style_name.as_deref())
                        .map(str::to_string),
                    stable_text: source_note_stable_style_ref_text(paragraph),
                    order: *next_order,
                });
                *next_order = next_order.saturating_add(1);
            }
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| &row.cells) {
                    collect_source_note_style_refs(&cell.blocks, entries, fields, next_order);
                }
            }
            _ => {}
        }
    }
}

fn source_note_run_is_style_ref(run: &crate::model::Run) -> bool {
    let FieldRole::Simple { instruction } = &run.field else {
        return false;
    };
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::DocumentStructure(kind) if kind == "STYLEREF"
    )
}

fn source_note_stable_style_ref_text(paragraph: &Paragraph) -> Option<String> {
    let [run] = paragraph.runs.as_slice() else {
        return None;
    };
    if !matches!(run.field, FieldRole::None)
        || run.field_dirty
        || run.field_unsupported_reason.is_some()
        || run.image.is_some()
        || run.comment.is_some()
        || run.revision.is_some()
        || run.content_control.is_some()
        || run.bookmark.is_some()
        || run.note.is_some()
        || run.text.split_whitespace().next().is_none()
    {
        return None;
    }
    Some(run.text.clone())
}

fn source_note_style_ref_entry_matches(
    entry: &SourceNoteStyleRefEntry,
    style_identifier: &str,
) -> bool {
    entry.style_id.eq_ignore_ascii_case(style_identifier)
        || entry
            .style_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(style_identifier))
}

fn source_note_paragraph_is_supported(paragraph: &Paragraph) -> bool {
    paragraph.runs.iter().all(|run| {
        run.image
            .as_ref()
            .map(source_note_image_is_supported)
            .unwrap_or(true)
            && source_note_field_is_supported(run)
            && run.comment.is_none()
            && run.revision.is_none()
            && source_note_content_control_is_supported(run.content_control.as_ref())
            && run
                .bookmark
                .as_deref()
                .map(referenceable_bookmark_name)
                .unwrap_or(true)
            && run.note.is_none()
    })
}

fn source_note_image_is_supported(image: &Image) -> bool {
    image.bytes.as_ref().is_some_and(|bytes| !bytes.is_empty())
        && matches!(
            image.mime.as_deref(),
            Some(
                "image/png"
                    | "image/jpeg"
                    | "image/gif"
                    | "image/bmp"
                    | "image/tiff"
                    | "image/webp"
            )
        )
        && image.floating_offset_emu.is_none()
}

fn source_note_field_is_supported(run: &crate::model::Run) -> bool {
    match &run.field {
        FieldRole::None => true,
        FieldRole::Hyperlink { url } => {
            !matches!(hyperlink_write_target(url), HyperlinkWriteTarget::Invalid)
        }
        FieldRole::Simple { instruction }
            if !run.field_dirty && !normalize_field_instruction(instruction).is_empty() =>
        {
            match FieldKind::from_instruction(instruction) {
                FieldKind::MergeField => {
                    run.field_unsupported_reason.is_none() && merge_field_syntax(instruction)
                }
                FieldKind::Filename => {
                    run.field_unsupported_reason.is_none() && filename_field_syntax(instruction)
                }
                FieldKind::ReferenceIndex(_) => {
                    run.field_unsupported_reason.is_none()
                        && supports_reference_index_marker_syntax(instruction)
                }
                FieldKind::TocEntry => {
                    run.field_unsupported_reason.is_none()
                        && supports_toc_entry_field_syntax(instruction)
                }
                FieldKind::Display(kind) if kind == "SYMBOL" => {
                    run.field_unsupported_reason.is_none()
                        && supports_computed_symbol_field_syntax(instruction)
                }
                FieldKind::Dynamic(kind) if kind == "QUOTE" => {
                    run.field_unsupported_reason.is_none()
                        && supports_quote_field_syntax(instruction)
                }
                FieldKind::Dynamic(kind) if kind == "IF" || kind == "COMPARE" => {
                    run.field_unsupported_reason.is_none()
                        && supports_context_free_if_compare_field_syntax(instruction)
                }
                FieldKind::Dynamic(kind) if kind == "=" => {
                    run.field_unsupported_reason.is_none()
                        && supports_formula_field_syntax(instruction)
                }
                FieldKind::Dynamic(kind) if kind == "FILLIN" => {
                    run.field_unsupported_reason.is_none()
                        && supports_context_free_fill_in_field_syntax(instruction)
                }
                FieldKind::Display(kind) if kind == "EQ" || kind == "ADVANCE" => {
                    run.field_unsupported_reason.is_none()
                        && supports_context_free_display_field_syntax(instruction)
                }
                FieldKind::DocumentInfo(_) => {
                    run.field_unsupported_reason.is_none()
                        && supports_preserved_document_info_field_syntax(instruction)
                }
                FieldKind::DocumentStructure(kind) if kind == "REVNUM" => {
                    run.field_unsupported_reason.is_none()
                        && supports_revision_number_field_syntax(instruction)
                }
                FieldKind::DocumentStructure(kind) if kind == "STYLEREF" => {
                    run.field_unsupported_reason.is_none()
                        && preserved_note_local_style_ref_target(instruction).is_some()
                }
                FieldKind::Ref => {
                    run.field_unsupported_reason.is_none()
                        && preserved_note_local_ref_target(instruction).is_some()
                }
                FieldKind::Sequence => {
                    run.field_unsupported_reason.is_none()
                        && computed_preserved_sequence_reset_result(instruction).as_deref()
                            == Some(run.text.as_str())
                }
                FieldKind::Numbering(kind) if kind == "LISTNUM" => {
                    run.field_unsupported_reason.is_none()
                        && computed_preserved_listnum_start_result(instruction).as_deref()
                            == Some(run.text.as_str())
                }
                FieldKind::Compatibility(_)
                | FieldKind::InsertedContent(_)
                | FieldKind::MailMerge(_)
                | FieldKind::Barcode(_) => {
                    run.field_unsupported_reason == Some(FieldUnsupportedReason::NoComputedResult)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn source_note_content_control_is_supported(control: Option<&AuthoredContentControl>) -> bool {
    let Some(control) = control else {
        return true;
    };
    let alias = non_empty_trimmed(control.alias.as_deref());
    let tag = non_empty_trimmed(control.tag.as_deref());
    let xpath = non_empty_trimmed(control.data_binding_xpath.as_deref());
    let store_item_id = non_empty_trimmed(control.data_binding_store_item_id.as_deref());
    let has_binding_metadata =
        control.data_binding_xpath.is_some() || control.data_binding_store_item_id.is_some();
    let has_complete_binding = xpath.is_some() && store_item_id.is_some();
    if has_binding_metadata && !has_complete_binding {
        return false;
    }
    alias.is_some() || tag.is_some() || has_complete_binding
}

fn source_note_table_is_supported(table: &Table) -> bool {
    let mut pending = vec![table];
    while let Some(table) = pending.pop() {
        let mut has_cell = false;
        for cell in table.rows.iter().flat_map(|row| &row.cells) {
            has_cell = true;
            if cell.blocks.is_empty() {
                return false;
            }
            for block in &cell.blocks {
                match block {
                    Block::Paragraph(paragraph)
                        if source_note_paragraph_is_supported(paragraph) => {}
                    Block::Chart(chart) if crate::docx::note_write_chart_supported(chart) => {}
                    Block::Table(table) => pending.push(table),
                    Block::PageBreak => {}
                    _ => return false,
                }
            }
        }
        if !has_cell {
            return false;
        }
    }
    true
}

fn source_note_table_formula_fields_are_supported(blocks: &[Block]) -> bool {
    blocks.iter().all(|block| match block {
        Block::Paragraph(paragraph) => !paragraph
            .runs
            .iter()
            .any(source_note_run_is_contextual_table_formula),
        Block::Table(table) => source_note_table_formula_is_supported(table),
        _ => true,
    })
}

fn source_note_run_is_contextual_table_formula(run: &crate::model::Run) -> bool {
    let FieldRole::Simple { instruction } = &run.field else {
        return false;
    };
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Dynamic(kind) if kind == "="
    ) && !supports_context_free_formula_field_syntax(instruction)
}

fn source_note_table_formula_is_supported(table: &Table) -> bool {
    let mut formulas = Vec::new();
    let mut nested_tables = Vec::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        for (col_index, cell) in row.cells.iter().enumerate() {
            for block in &cell.blocks {
                match block {
                    Block::Paragraph(paragraph) => {
                        for run in &paragraph.runs {
                            if source_note_run_is_contextual_table_formula(run) {
                                formulas.push((row_index, col_index, run));
                            }
                        }
                    }
                    Block::Table(table) => nested_tables.push(table),
                    _ => {}
                }
            }
        }
    }
    if !nested_tables
        .into_iter()
        .all(source_note_table_formula_is_supported)
    {
        return false;
    }
    let [(formula_row, formula_col, formula_run)] = formulas.as_slice() else {
        return formulas.is_empty();
    };
    if table.bidi_visual || table.header_rows != 0 || table.rows.is_empty() {
        return false;
    }

    let mut rows = Vec::with_capacity(table.rows.len());
    for (row_index, row) in table.rows.iter().enumerate() {
        if row.cells.is_empty() {
            return false;
        }
        let mut values = Vec::with_capacity(row.cells.len());
        for (col_index, cell) in row.cells.iter().enumerate() {
            if cell.row_span != 1 || cell.col_span != 1 || cell.is_header {
                return false;
            }
            let [Block::Paragraph(paragraph)] = cell.blocks.as_slice() else {
                return false;
            };
            let [run] = paragraph.runs.as_slice() else {
                return false;
            };
            if run.text.is_empty()
                || run.text.contains(['\t', '\n'])
                || run.image.is_some()
                || run.comment.is_some()
                || run.revision.is_some()
                || run.content_control.is_some()
                || run.bookmark.is_some()
                || run.note.is_some()
                || run.field_dirty
                || run.field_unsupported_reason.is_some()
            {
                return false;
            }
            if (row_index, col_index) == (*formula_row, *formula_col) {
                if !source_note_run_is_contextual_table_formula(run) {
                    return false;
                }
            } else if !matches!(run.field, FieldRole::None) {
                return false;
            }
            values.push(run.text.clone());
        }
        rows.push(values);
    }

    let FieldRole::Simple { instruction } = &formula_run.field else {
        return false;
    };
    computed_span_free_table_formula_result(instruction, &rows, *formula_row, *formula_col)
        .as_deref()
        == Some(formula_run.text.as_str())
}

fn notes_xml(root: &str, item: &str, notes: &[WrittenNote], has_relationships: bool) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(XML_DECL);
    let has_drawing = notes.iter().any(|note| {
        note.body_xml
            .as_deref()
            .is_some_and(|xml| xml.contains("<w:drawing>"))
    });
    if has_drawing {
        let chart_ns = if notes.iter().any(|note| {
            note.body_xml
                .as_deref()
                .is_some_and(|xml| xml.contains("<c:chart"))
        }) {
            format!(r#" xmlns:c="{C_NS}""#)
        } else {
            String::new()
        };
        let chart_ex_ns = if notes.iter().any(|note| {
            note.body_xml
                .as_deref()
                .is_some_and(|xml| xml.contains("<cx:chart"))
        }) {
            format!(r#" xmlns:cx="{CX_NS}""#)
        } else {
            String::new()
        };
        s.push_str(&format!(
            r#"<w:{root} xmlns:w="{W_NS}" xmlns:r="{R_NS}" xmlns:wp="{WP_NS}" xmlns:a="{A_NS}" xmlns:pic="{PIC_NS}"{chart_ns}{chart_ex_ns}>"#
        ));
    } else if has_relationships {
        s.push_str(&format!(r#"<w:{root} xmlns:w="{W_NS}" xmlns:r="{R_NS}">"#));
    } else {
        s.push_str(&format!(r#"<w:{root} xmlns:w="{W_NS}">"#));
    }
    s.push_str(&format!(
        concat!(
            r#"<w:{item} w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:{item}>"#,
            r#"<w:{item} w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{item}>"#
        ),
        item = item
    ));
    for note in notes {
        s.push_str(&format!(r#"<w:{item} w:id="{}">"#, esc_attr(&note.id)));
        if let Some(body_xml) = &note.body_xml {
            s.push_str(body_xml);
        } else {
            s.push_str("<w:p><w:r>");
            write_run_text(&mut s, &note.text);
            s.push_str("</w:r></w:p>");
        }
        s.push_str(&format!("</w:{item}>"));
    }
    s.push_str(&format!("</w:{root}>"));
    s.into_bytes()
}

fn write_comment_text(out: &mut String, text: &str) {
    let mut buf = String::new();
    let flush = |out: &mut String, buf: &mut String| {
        if !buf.is_empty() {
            let space = if needs_xml_space(buf) {
                r#" xml:space="preserve""#
            } else {
                ""
            };
            out.push_str(&format!(r#"<w:t{space}>{}</w:t>"#, esc_text(buf)));
            buf.clear();
        }
    };
    for ch in text.chars() {
        match ch {
            '\t' => {
                flush(out, &mut buf);
                out.push_str("<w:tab/>");
            }
            '\n' => {
                flush(out, &mut buf);
                out.push_str("<w:br/>");
            }
            '\r' => {}
            c if (c as u32) < 0x20 => {}
            c => buf.push(c),
        }
    }
    flush(out, &mut buf);
}

fn core_properties_xml(setup: &DocSetup) -> Option<Vec<u8>> {
    let title = non_empty_trimmed(setup.title.as_deref());
    let subject = non_empty_trimmed(setup.subject.as_deref());
    let creator = non_empty_trimmed(setup.creator.as_deref());
    let description = non_empty_trimmed(setup.description.as_deref());
    let keywords = non_empty_trimmed(setup.keywords.as_deref());
    let category = non_empty_trimmed(setup.category.as_deref());
    let content_status = non_empty_trimmed(setup.content_status.as_deref());
    let last_modified_by = non_empty_trimmed(setup.last_modified_by.as_deref());
    let created = non_empty_trimmed(setup.created.as_deref());
    let modified = non_empty_trimmed(setup.modified.as_deref());
    let last_printed = non_empty_trimmed(setup.last_printed.as_deref());
    let revision = non_empty_trimmed(setup.revision.as_deref());
    let version = non_empty_trimmed(setup.version.as_deref());
    if title.is_none()
        && subject.is_none()
        && creator.is_none()
        && description.is_none()
        && keywords.is_none()
        && category.is_none()
        && content_status.is_none()
        && last_modified_by.is_none()
        && created.is_none()
        && modified.is_none()
        && last_printed.is_none()
        && revision.is_none()
        && version.is_none()
    {
        return None;
    }

    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(&format!(
        r#"<cp:coreProperties xmlns:cp="{CORE_PROPERTIES_NS}" xmlns:dc="{DC_NS}" xmlns:dcterms="{DCTERMS_NS}" xmlns:xsi="{XSI_NS}">"#
    ));
    if let Some(title) = title {
        s.push_str(&format!("<dc:title>{}</dc:title>", esc_text(title)));
    }
    if let Some(subject) = subject {
        s.push_str(&format!("<dc:subject>{}</dc:subject>", esc_text(subject)));
    }
    if let Some(creator) = creator {
        s.push_str(&format!("<dc:creator>{}</dc:creator>", esc_text(creator)));
    }
    if let Some(description) = description {
        s.push_str(&format!(
            "<dc:description>{}</dc:description>",
            esc_text(description)
        ));
    }
    if let Some(keywords) = keywords {
        s.push_str(&format!(
            "<cp:keywords>{}</cp:keywords>",
            esc_text(keywords)
        ));
    }
    if let Some(category) = category {
        s.push_str(&format!(
            "<cp:category>{}</cp:category>",
            esc_text(category)
        ));
    }
    if let Some(content_status) = content_status {
        s.push_str(&format!(
            "<cp:contentStatus>{}</cp:contentStatus>",
            esc_text(content_status)
        ));
    }
    if let Some(last_modified_by) = last_modified_by {
        s.push_str(&format!(
            "<cp:lastModifiedBy>{}</cp:lastModifiedBy>",
            esc_text(last_modified_by)
        ));
    }
    if let Some(created) = created {
        s.push_str(&format!(
            r#"<dcterms:created xsi:type="dcterms:W3CDTF">{}</dcterms:created>"#,
            esc_text(created)
        ));
    }
    if let Some(modified) = modified {
        s.push_str(&format!(
            r#"<dcterms:modified xsi:type="dcterms:W3CDTF">{}</dcterms:modified>"#,
            esc_text(modified)
        ));
    }
    if let Some(last_printed) = last_printed {
        s.push_str(&format!(
            "<cp:lastPrinted>{}</cp:lastPrinted>",
            esc_text(last_printed)
        ));
    }
    if let Some(revision) = revision {
        s.push_str(&format!(
            "<cp:revision>{}</cp:revision>",
            esc_text(revision)
        ));
    }
    if let Some(version) = version {
        s.push_str(&format!("<cp:version>{}</cp:version>", esc_text(version)));
    }
    s.push_str("</cp:coreProperties>");
    Some(s.into_bytes())
}

fn custom_properties_xml(properties: &std::collections::BTreeMap<String, String>) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(
        r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">"#,
    );
    for (index, (name, value)) in properties.iter().enumerate() {
        s.push_str(&format!(
            r#"<property fmtid="{{D5CDD505-2E9C-101B-9397-08002B2CF9AE}}" pid="{}" name="{}"><vt:lpwstr>{}</vt:lpwstr></property>"#,
            index + 2,
            esc_attr(name),
            esc_text(value)
        ));
    }
    s.push_str("</Properties>");
    s.into_bytes()
}

fn custom_xml_item_props_xml(store_item_id: &str) -> Vec<u8> {
    format!(
        r#"{XML_DECL}<ds:datastoreItem ds:itemID="{}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs/></ds:datastoreItem>"#,
        esc_attr(store_item_id)
    )
    .into_bytes()
}

fn web_extension_xml(pane: &WebExtensionTaskPane) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(&format!(
        r#"<we:webextension xmlns:we="{WEB_EXTENSION_NS}" id="{}">"#,
        esc_attr(pane.extension_id.trim())
    ));
    s.push_str(&format!(
        r#"<we:reference id="{}" version="{}" store="{}" storeType="{}"/>"#,
        esc_attr(pane.reference_id.trim()),
        esc_attr(pane.version.trim()),
        esc_attr(pane.store.trim()),
        esc_attr(pane.store_type.trim())
    ));
    s.push_str("<we:alternateReferences/>");
    if pane.properties.is_empty() {
        s.push_str("<we:properties/>");
    } else {
        s.push_str("<we:properties>");
        for (name, value) in &pane.properties {
            s.push_str(&format!(
                r#"<we:property name="{}" value="{}"/>"#,
                esc_attr(name),
                esc_attr(value)
            ));
        }
        s.push_str("</we:properties>");
    }
    s.push_str(&format!(
        r#"<we:bindings/><we:snapshot xmlns:r="{R_NS}"/></we:webextension>"#
    ));
    s.into_bytes()
}

fn valid_web_extension_task_pane(pane: &WebExtensionTaskPane) -> bool {
    !pane.extension_id.trim().is_empty()
        && !pane.reference_id.trim().is_empty()
        && !pane.version.trim().is_empty()
        && !pane.store.trim().is_empty()
        && !pane.store_type.trim().is_empty()
}

fn web_extension_taskpanes_xml(panes: &[&WebExtensionTaskPane]) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(XML_DECL);
    s.push_str(&format!(
        r#"<wetp:taskpanes xmlns:wetp="{WEB_EXTENSION_TASKPANES_NS}">"#
    ));
    for (index, pane) in panes.iter().enumerate() {
        let visible = if pane.visible { "1" } else { "0" };
        let dock_state = if pane.dock_state.is_empty() {
            "right"
        } else {
            pane.dock_state.as_str()
        };
        let locked = if pane.locked { r#" locked="1""# } else { "" };
        s.push_str(&format!(
            r#"<wetp:taskpane dockstate="{}" visibility="{visible}" width="{}" row="{}"{locked}>"#,
            esc_attr(dock_state),
            pane.width.max(1),
            pane.row
        ));
        s.push_str(&format!(
            r#"<wetp:webextension xmlns:r="{R_NS}" r:id="rId{}"/>"#,
            index + 1
        ));
        s.push_str("</wetp:taskpane>");
    }
    s.push_str("</wetp:taskpanes>");
    s.into_bytes()
}

fn chart_workbook_xlsx(chart: &Chart) -> Vec<u8> {
    let (sheet_xml, shared_strings_xml) = chart_workbook_sheet_xml(chart);
    let mut pkg = Package::new();
    pkg.add_part(
        "xl/workbook.xml",
        Some(CT_XLSX_WORKBOOK),
        xlsx_workbook_xml().into_bytes(),
    );
    pkg.add_part(
        "xl/worksheets/sheet1.xml",
        Some(CT_XLSX_WORKSHEET),
        sheet_xml.into_bytes(),
    );
    pkg.add_part(
        "xl/sharedStrings.xml",
        Some(CT_XLSX_SHARED_STRINGS),
        shared_strings_xml.into_bytes(),
    );
    pkg.add_part(
        "xl/styles.xml",
        Some(CT_XLSX_STYLES),
        xlsx_styles_xml().into_bytes(),
    );
    pkg.add_rels(
        "_rels/.rels",
        vec![Rel {
            id: "rId1".to_string(),
            rel_type: REL_OFFICE_DOCUMENT.to_string(),
            target: "xl/workbook.xml".to_string(),
            external: false,
        }],
    );
    pkg.add_rels(
        "xl/_rels/workbook.xml.rels",
        vec![
            Rel {
                id: "rId1".to_string(),
                rel_type: REL_XLSX_WORKSHEET.to_string(),
                target: "worksheets/sheet1.xml".to_string(),
                external: false,
            },
            Rel {
                id: "rId2".to_string(),
                rel_type: REL_XLSX_STYLES.to_string(),
                target: "styles.xml".to_string(),
                external: false,
            },
            Rel {
                id: "rId3".to_string(),
                rel_type: REL_XLSX_SHARED_STRINGS.to_string(),
                target: "sharedStrings.xml".to_string(),
                external: false,
            },
        ],
    );
    pkg.try_into_zip().unwrap_or_default()
}

fn is_chart_ex_kind(kind: ChartKind) -> bool {
    matches!(
        kind,
        ChartKind::Waterfall
            | ChartKind::Treemap
            | ChartKind::Sunburst
            | ChartKind::Histogram
            | ChartKind::BoxWhisker
            | ChartKind::Funnel
    )
}

fn chart_ex_layout_id(kind: ChartKind) -> &'static str {
    match kind {
        ChartKind::Waterfall => "waterfall",
        ChartKind::Treemap => "treemap",
        ChartKind::Sunburst => "sunburst",
        ChartKind::Histogram => "histogram",
        ChartKind::BoxWhisker => "boxWhisker",
        ChartKind::Funnel => "funnel",
        _ => unreachable!("non-chartEx kind routed to chartEx writer"),
    }
}

fn xlsx_workbook_xml() -> String {
    format!(
        r#"{XML_DECL}<workbook xmlns="{S_NS}" xmlns:r="{R_NS}"><workbookPr/><sheets><sheet name="Chart Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#
    )
}

fn chart_workbook_sheet_xml(chart: &Chart) -> (String, String) {
    let mut shared_strings = Vec::new();
    let mut sheet = String::new();
    sheet.push_str(XML_DECL);
    sheet.push_str(&format!(
        r#"<worksheet xmlns="{S_NS}" xmlns:r="{R_NS}"><sheetData>"#
    ));

    sheet.push_str(r#"<row r="1">"#);
    let category_header = shared_string_index(&mut shared_strings, "Category");
    write_xlsx_shared_cell(&mut sheet, 0, 1, category_header);
    let mut next_col = 1usize;
    for series in &chart.series {
        let name = shared_string_index(&mut shared_strings, &series.name);
        write_xlsx_shared_cell(&mut sheet, next_col, 1, name);
        next_col += 1;
        if matches!(chart.kind, ChartKind::Bubble | ChartKind::Bubble3D) {
            let size_name =
                shared_string_index(&mut shared_strings, &format!("{} size", series.name));
            write_xlsx_shared_cell(&mut sheet, next_col, 1, size_name);
            next_col += 1;
        }
    }
    sheet.push_str("</row>");

    let row_count = chart
        .series
        .iter()
        .map(|series| series.values.len())
        .max()
        .unwrap_or(0)
        .max(chart.categories.len());
    for row_index in 0..row_count {
        let row_number = row_index + 2;
        sheet.push_str(&format!(r#"<row r="{row_number}">"#));
        let category = chart
            .categories
            .get(row_index)
            .map(String::as_str)
            .unwrap_or("");
        let category_index = shared_string_index(&mut shared_strings, category);
        write_xlsx_shared_cell(&mut sheet, 0, row_number, category_index);
        let mut next_col = 1usize;
        for series in &chart.series {
            let value = series.values.get(row_index).copied().unwrap_or(0.0);
            write_xlsx_number_cell(&mut sheet, next_col, row_number, value);
            next_col += 1;
            if matches!(chart.kind, ChartKind::Bubble | ChartKind::Bubble3D) {
                let bubble_size = series.bubble_sizes.get(row_index).copied().unwrap_or(1.0);
                write_xlsx_number_cell(&mut sheet, next_col, row_number, bubble_size);
                next_col += 1;
            }
        }
        sheet.push_str("</row>");
    }

    sheet.push_str("</sheetData></worksheet>");
    (sheet, shared_strings_xml(&shared_strings))
}

fn shared_string_index(shared_strings: &mut Vec<String>, value: &str) -> usize {
    if let Some(index) = shared_strings.iter().position(|existing| existing == value) {
        index
    } else {
        shared_strings.push(value.to_string());
        shared_strings.len() - 1
    }
}

fn write_xlsx_shared_cell(out: &mut String, col_index: usize, row_number: usize, value: usize) {
    let cell_ref = xlsx_cell_ref(col_index, row_number);
    out.push_str(&format!(r#"<c r="{cell_ref}" t="s"><v>{value}</v></c>"#));
}

fn write_xlsx_number_cell(out: &mut String, col_index: usize, row_number: usize, value: f64) {
    let cell_ref = xlsx_cell_ref(col_index, row_number);
    out.push_str(&format!(
        r#"<c r="{cell_ref}"><v>{}</v></c>"#,
        format_chart_number(value)
    ));
}

fn xlsx_cell_ref(col_index: usize, row_number: usize) -> String {
    format!("{}{}", xlsx_col_name(col_index), row_number)
}

fn xlsx_col_name(mut col_index: usize) -> String {
    col_index += 1;
    let mut name = Vec::new();
    while col_index > 0 {
        let rem = (col_index - 1) % 26;
        name.push((b'A' + rem as u8) as char);
        col_index = (col_index - 1) / 26;
    }
    name.iter().rev().collect()
}

fn shared_strings_xml(shared_strings: &[String]) -> String {
    let mut out = String::new();
    out.push_str(XML_DECL);
    out.push_str(&format!(
        r#"<sst xmlns="{S_NS}" count="{count}" uniqueCount="{count}">"#,
        count = shared_strings.len()
    ));
    for value in shared_strings {
        let space = if needs_xml_space(value) {
            r#" xml:space="preserve""#
        } else {
            ""
        };
        out.push_str(&format!(r#"<si><t{space}>{}</t></si>"#, esc_text(value)));
    }
    out.push_str("</sst>");
    out
}

fn xlsx_styles_xml() -> String {
    format!(
        concat!(
            r#"{xml_decl}<styleSheet xmlns="{s_ns}">"#,
            r#"<fonts count="1"><font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/></font></fonts>"#,
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
            r#"<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>"#,
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
            r#"<cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>"#,
            r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
            r#"<dxfs count="0"/><tableStyles count="0" defaultTableStyle="TableStyleMedium2" defaultPivotStyle="PivotStyleLight16"/>"#,
            r#"</styleSheet>"#
        ),
        xml_decl = XML_DECL,
        s_ns = S_NS
    )
}

fn chart_xml(chart: &Chart, chart_id: u32, workbook_rid: Option<&str>) -> String {
    let cat_axis_id = 10_000u32.saturating_add(chart_id.saturating_mul(2));
    let val_axis_id = cat_axis_id.saturating_add(1);
    let ser_axis_id = val_axis_id.saturating_add(1);
    let mut out = String::new();
    out.push_str(XML_DECL);
    out.push_str(&format!(
        r#"<c:chartSpace xmlns:c="{C_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">"#
    ));
    out.push_str(
        r#"<c:date1904 val="0"/><c:lang val="en-US"/><c:roundedCorners val="0"/><c:chart>"#,
    );
    if let Some(title) = non_empty_trimmed(chart.title.as_deref()) {
        write_chart_title(&mut out, title);
    }
    out.push_str("<c:plotArea><c:layout/>");
    match chart.kind {
        ChartKind::Bar => write_bar_or_column_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "bar",
            "clustered",
        ),
        ChartKind::StackedBar => {
            write_bar_or_column_chart(&mut out, chart, cat_axis_id, val_axis_id, "bar", "stacked")
        }
        ChartKind::PercentStackedBar => write_bar_or_column_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "bar",
            "percentStacked",
        ),
        ChartKind::Bar3D => write_bar_or_column_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "bar",
            "clustered",
        ),
        ChartKind::StackedBar3D => write_bar_or_column_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "bar",
            "stacked",
        ),
        ChartKind::PercentStackedBar3D => write_bar_or_column_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "bar",
            "percentStacked",
        ),
        ChartKind::Column => write_bar_or_column_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "col",
            "clustered",
        ),
        ChartKind::StackedColumn => {
            write_bar_or_column_chart(&mut out, chart, cat_axis_id, val_axis_id, "col", "stacked")
        }
        ChartKind::PercentStackedColumn => write_bar_or_column_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "col",
            "percentStacked",
        ),
        ChartKind::Column3D => write_bar_or_column_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "col",
            "clustered",
        ),
        ChartKind::StackedColumn3D => write_bar_or_column_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "col",
            "stacked",
        ),
        ChartKind::PercentStackedColumn3D => write_bar_or_column_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "col",
            "percentStacked",
        ),
        ChartKind::Line => write_line_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "circle",
            "standard",
            false,
        ),
        ChartKind::LineNoMarkers => write_line_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "none",
            "standard",
            false,
        ),
        ChartKind::SmoothLine => write_line_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "circle",
            "standard",
            true,
        ),
        ChartKind::StackedLine => write_line_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "circle",
            "stacked",
            false,
        ),
        ChartKind::PercentStackedLine => write_line_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "circle",
            "percentStacked",
            false,
        ),
        ChartKind::Line3D => {
            write_line_3d_chart(&mut out, chart, cat_axis_id, val_axis_id, ser_axis_id)
        }
        ChartKind::Area => write_area_chart(&mut out, chart, cat_axis_id, val_axis_id, "standard"),
        ChartKind::StackedArea => {
            write_area_chart(&mut out, chart, cat_axis_id, val_axis_id, "stacked")
        }
        ChartKind::PercentStackedArea => {
            write_area_chart(&mut out, chart, cat_axis_id, val_axis_id, "percentStacked")
        }
        ChartKind::Area3D => write_area_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            ser_axis_id,
            "standard",
        ),
        ChartKind::StackedArea3D => write_area_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            ser_axis_id,
            "stacked",
        ),
        ChartKind::PercentStackedArea3D => write_area_3d_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            ser_axis_id,
            "percentStacked",
        ),
        ChartKind::Radar => {
            write_radar_chart(&mut out, chart, cat_axis_id, val_axis_id, "standard")
        }
        ChartKind::RadarWithMarkers => {
            write_radar_chart(&mut out, chart, cat_axis_id, val_axis_id, "marker")
        }
        ChartKind::FilledRadar => {
            write_radar_chart(&mut out, chart, cat_axis_id, val_axis_id, "filled")
        }
        ChartKind::Scatter => write_scatter_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "lineMarker",
            "circle",
        ),
        ChartKind::ScatterMarkers => write_scatter_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "marker",
            "circle",
        ),
        ChartKind::ScatterLines => {
            write_scatter_chart(&mut out, chart, cat_axis_id, val_axis_id, "line", "none")
        }
        ChartKind::ScatterSmooth => write_scatter_chart(
            &mut out,
            chart,
            cat_axis_id,
            val_axis_id,
            "smoothMarker",
            "circle",
        ),
        ChartKind::ScatterSmoothNoMarkers => {
            write_scatter_chart(&mut out, chart, cat_axis_id, val_axis_id, "smooth", "none")
        }
        ChartKind::Bubble | ChartKind::Bubble3D => {
            write_bubble_chart(&mut out, chart, cat_axis_id, val_axis_id)
        }
        ChartKind::Pie => write_pie_chart(&mut out, chart, false),
        ChartKind::ExplodedPie => write_pie_chart(&mut out, chart, true),
        ChartKind::Pie3D => write_pie_3d_chart(&mut out, chart, false),
        ChartKind::ExplodedPie3D => write_pie_3d_chart(&mut out, chart, true),
        ChartKind::PieOfPie => write_of_pie_chart(&mut out, chart, "pie"),
        ChartKind::BarOfPie => write_of_pie_chart(&mut out, chart, "bar"),
        ChartKind::Doughnut => write_doughnut_chart(&mut out, chart, false),
        ChartKind::ExplodedDoughnut => write_doughnut_chart(&mut out, chart, true),
        ChartKind::Surface => {
            write_surface_chart(&mut out, chart, cat_axis_id, val_axis_id, ser_axis_id)
        }
        ChartKind::Surface3D => {
            write_surface_3d_chart(&mut out, chart, cat_axis_id, val_axis_id, ser_axis_id)
        }
        ChartKind::StockHighLowClose => {
            write_stock_chart(&mut out, chart, cat_axis_id, val_axis_id, false)
        }
        ChartKind::Stock => write_stock_chart(&mut out, chart, cat_axis_id, val_axis_id, true),
        ChartKind::Waterfall
        | ChartKind::Treemap
        | ChartKind::Sunburst
        | ChartKind::Histogram
        | ChartKind::BoxWhisker
        | ChartKind::Funnel => unreachable!("chartEx kind routed to chartEx writer"),
    }
    match chart.kind {
        ChartKind::Pie
        | ChartKind::ExplodedPie
        | ChartKind::Pie3D
        | ChartKind::ExplodedPie3D
        | ChartKind::PieOfPie
        | ChartKind::BarOfPie
        | ChartKind::Doughnut
        | ChartKind::ExplodedDoughnut => {}
        ChartKind::Scatter
        | ChartKind::ScatterMarkers
        | ChartKind::ScatterLines
        | ChartKind::ScatterSmooth
        | ChartKind::ScatterSmoothNoMarkers
        | ChartKind::Bubble
        | ChartKind::Bubble3D => write_scatter_axes(&mut out, cat_axis_id, val_axis_id),
        ChartKind::Line3D
        | ChartKind::Area3D
        | ChartKind::StackedArea3D
        | ChartKind::PercentStackedArea3D
        | ChartKind::Surface
        | ChartKind::Surface3D => {
            write_surface_axes(&mut out, cat_axis_id, val_axis_id, ser_axis_id)
        }
        ChartKind::Waterfall
        | ChartKind::Treemap
        | ChartKind::Sunburst
        | ChartKind::Histogram
        | ChartKind::BoxWhisker
        | ChartKind::Funnel => unreachable!("chartEx kind routed to chartEx writer"),
        _ => write_chart_axes(&mut out, chart.kind, cat_axis_id, val_axis_id),
    }
    out.push_str("</c:plotArea>");
    if chart.series.len() > 1 {
        out.push_str(
            r#"<c:legend><c:legendPos val="r"/><c:layout/><c:overlay val="0"/></c:legend>"#,
        );
    }
    out.push_str(r#"<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart>"#);
    if let Some(rid) = workbook_rid {
        out.push_str(&format!(
            r#"<c:externalData r:id="{}"><c:autoUpdate val="0"/></c:externalData>"#,
            esc_attr(rid)
        ));
    }
    out.push_str(r#"<c:printSettings><c:headerFooter/><c:pageMargins b="0.75" l="0.7" r="0.7" t="0.75" header="0.3" footer="0.3"/><c:pageSetup/></c:printSettings>"#);
    out.push_str("</c:chartSpace>");
    out
}

fn chart_ex_xml(chart: &Chart, chart_id: u32) -> String {
    let mut out = String::new();
    out.push_str(XML_DECL);
    out.push_str(&format!(
        r#"<cx:chartSpace xmlns:cx="{CX_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">"#
    ));
    out.push_str("<cx:chartData>");
    for (index, series) in chart.series.iter().enumerate() {
        write_chart_ex_data(&mut out, chart, series, chart_id, index);
    }
    out.push_str("</cx:chartData><cx:chart>");
    if let Some(title) = non_empty_trimmed(chart.title.as_deref()) {
        write_chart_ex_title(&mut out, title);
    }
    out.push_str("<cx:plotArea><cx:plotAreaRegion>");
    let layout_id = chart_ex_layout_id(chart.kind);
    for (index, series) in chart.series.iter().enumerate() {
        let data_id = chart_ex_data_id(chart_id, index);
        out.push_str(&format!(r#"<cx:series layoutId="{layout_id}">"#));
        write_chart_ex_series_title(&mut out, &series.name);
        out.push_str(&format!(r#"<cx:dataId val="{data_id}"/></cx:series>"#));
    }
    out.push_str(
        r#"</cx:plotAreaRegion><cx:axis id="0"><cx:catScaling/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea>"#,
    );
    if chart.series.len() > 1 {
        out.push_str(r#"<cx:legend pos="r" align="ctr" overlay="0"/>"#);
    }
    out.push_str("</cx:chart></cx:chartSpace>");
    out
}

fn chart_ex_data_id(chart_id: u32, series_index: usize) -> String {
    format!("rwml-chart-ex-{chart_id}-{series_index}")
}

fn write_chart_ex_data(
    out: &mut String,
    chart: &Chart,
    series: &ChartSeries,
    chart_id: u32,
    series_index: usize,
) {
    let data_id = chart_ex_data_id(chart_id, series_index);
    out.push_str(&format!(r#"<cx:data id="{data_id}">"#));
    out.push_str(r#"<cx:strDim type="cat"><cx:lvl>"#);
    for (index, category) in chart.categories.iter().enumerate() {
        out.push_str(&format!(
            r#"<cx:pt idx="{index}"><cx:v>{}</cx:v></cx:pt>"#,
            esc_text(category)
        ));
    }
    out.push_str(r#"</cx:lvl></cx:strDim><cx:numDim type="val"><cx:lvl>"#);
    for (index, value) in series.values.iter().enumerate() {
        out.push_str(&format!(
            r#"<cx:pt idx="{index}"><cx:v>{}</cx:v></cx:pt>"#,
            format_chart_number(*value)
        ));
    }
    out.push_str("</cx:lvl></cx:numDim></cx:data>");
}

fn write_chart_ex_title(out: &mut String, title: &str) {
    out.push_str(r#"<cx:title pos="t" align="ctr" overlay="0"><cx:tx><cx:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>"#);
    out.push_str(&esc_text(title));
    out.push_str("</a:t></a:r></a:p></cx:rich></cx:tx></cx:title>");
}

fn write_chart_ex_series_title(out: &mut String, title: &str) {
    let title = title.trim();
    if title.is_empty() {
        return;
    }
    out.push_str(r#"<cx:tx><cx:txData><cx:strDim type="tx"><cx:lvl><cx:pt idx="0"><cx:v>"#);
    out.push_str(&esc_text(title));
    out.push_str("</cx:v></cx:pt></cx:lvl></cx:strDim></cx:txData></cx:tx>");
}

fn write_chart_title(out: &mut String, title: &str) {
    out.push_str("<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>");
    out.push_str(&esc_text(title));
    out.push_str("</a:t></a:r></a:p></c:rich></c:tx><c:layout/><c:overlay val=\"0\"/></c:title>");
}

fn write_bar_or_column_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    bar_dir: &str,
    grouping: &str,
) {
    out.push_str(&format!(
        r#"<c:barChart><c:barDir val="{bar_dir}"/><c:grouping val="{grouping}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    if matches!(grouping, "stacked" | "percentStacked") {
        out.push_str(r#"<c:overlap val="100"/>"#);
    }
    out.push_str(&format!(
        r#"<c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/></c:barChart>"#
    ));
}

fn write_bar_or_column_3d_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    bar_dir: &str,
    grouping: &str,
) {
    out.push_str(&format!(
        r#"<c:bar3DChart><c:barDir val="{bar_dir}"/><c:grouping val="{grouping}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    let shape = chart_shape_value(chart.shape);
    if matches!(grouping, "stacked" | "percentStacked") {
        out.push_str(r#"<c:overlap val="100"/>"#);
    }
    out.push_str(&format!(
        r#"<c:gapWidth val="150"/><c:gapDepth val="150"/><c:shape val="{shape}"/><c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/></c:bar3DChart>"#
    ));
}

fn chart_shape_value(shape: ChartShape) -> &'static str {
    match shape {
        ChartShape::Box => "box",
        ChartShape::Cylinder => "cylinder",
        ChartShape::Cone => "cone",
        ChartShape::ConeToMax => "coneToMax",
        ChartShape::Pyramid => "pyramid",
        ChartShape::PyramidToMax => "pyramidToMax",
    }
}

fn write_line_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    marker_symbol: &str,
    grouping: &str,
    smooth: bool,
) {
    out.push_str(&format!(
        r#"<c:lineChart><c:grouping val="{grouping}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx><c:marker><c:symbol val="{marker_symbol}"/></c:marker>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    if smooth {
        out.push_str(r#"<c:smooth val="1"/>"#);
    }
    out.push_str(&format!(
        r#"<c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/></c:lineChart>"#
    ));
}

fn write_line_3d_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    ser_axis_id: u32,
) {
    out.push_str(r#"<c:line3DChart><c:grouping val="standard"/><c:varyColors val="0"/>"#);
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx><c:marker><c:symbol val="circle"/></c:marker>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:gapDepth val="150"/><c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/><c:axId val="{ser_axis_id}"/></c:line3DChart>"#
    ));
}

fn write_area_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    grouping: &str,
) {
    out.push_str(&format!(
        r#"<c:areaChart><c:grouping val="{grouping}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/></c:areaChart>"#
    ));
}

fn write_area_3d_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    ser_axis_id: u32,
    grouping: &str,
) {
    out.push_str(&format!(
        r#"<c:area3DChart><c:grouping val="{grouping}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:gapDepth val="150"/><c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/><c:axId val="{ser_axis_id}"/></c:area3DChart>"#
    ));
}

fn write_radar_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    style: &str,
) {
    out.push_str(&format!(
        r#"<c:radarChart><c:radarStyle val="{style}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx><c:marker><c:symbol val="circle"/></c:marker>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/></c:radarChart>"#
    ));
}

fn write_scatter_chart(
    out: &mut String,
    chart: &Chart,
    x_axis_id: u32,
    y_axis_id: u32,
    style: &str,
    marker_symbol: &str,
) {
    out.push_str(&format!(
        r#"<c:scatterChart><c:scatterStyle val="{style}"/><c:varyColors val="0"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx><c:marker><c:symbol val="{marker_symbol}"/></c:marker>"#,
            esc_text(&series.name)
        ));
        write_chart_x_values(out, series.values.len());
        write_chart_y_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:axId val="{x_axis_id}"/><c:axId val="{y_axis_id}"/></c:scatterChart>"#
    ));
}

fn write_bubble_chart(out: &mut String, chart: &Chart, x_axis_id: u32, y_axis_id: u32) {
    out.push_str(r#"<c:bubbleChart><c:varyColors val="0"/>"#);
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_x_values(out, series.values.len());
        write_chart_y_values(out, &series.values);
        write_chart_bubble_sizes(out, series, series.values.len());
        out.push_str("</c:ser>");
    }
    if chart.kind == ChartKind::Bubble3D {
        out.push_str(r#"<c:bubble3D val="1"/>"#);
    }
    out.push_str(&format!(
        r#"<c:bubbleScale val="100"/><c:showNegBubbles val="0"/><c:axId val="{x_axis_id}"/><c:axId val="{y_axis_id}"/></c:bubbleChart>"#
    ));
}

fn write_pie_chart(out: &mut String, chart: &Chart, exploded: bool) {
    out.push_str(r#"<c:pieChart><c:varyColors val="1"/>"#);
    for (index, series) in chart.series.iter().take(1).enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        if exploded {
            out.push_str(r#"<c:explosion val="25"/>"#);
        }
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(r#"<c:firstSliceAng val="0"/></c:pieChart>"#);
}

fn write_pie_3d_chart(out: &mut String, chart: &Chart, exploded: bool) {
    out.push_str(r#"<c:pie3DChart><c:varyColors val="1"/>"#);
    for (index, series) in chart.series.iter().take(1).enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        if exploded {
            out.push_str(r#"<c:explosion val="25"/>"#);
        }
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(r#"<c:firstSliceAng val="0"/></c:pie3DChart>"#);
}

fn write_of_pie_chart(out: &mut String, chart: &Chart, of_pie_type: &str) {
    out.push_str(&format!(
        r#"<c:ofPieChart><c:ofPieType val="{of_pie_type}"/><c:varyColors val="1"/>"#
    ));
    for (index, series) in chart.series.iter().take(1).enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(
        r#"<c:gapWidth val="150"/><c:splitType val="auto"/><c:secondPieSize val="75"/><c:serLines/></c:ofPieChart>"#,
    );
}

fn write_doughnut_chart(out: &mut String, chart: &Chart, exploded: bool) {
    out.push_str(r#"<c:doughnutChart><c:varyColors val="1"/>"#);
    for (index, series) in chart.series.iter().take(1).enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        if exploded {
            out.push_str(r#"<c:explosion val="25"/>"#);
        }
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(r#"<c:firstSliceAng val="0"/><c:holeSize val="50"/></c:doughnutChart>"#);
}

fn write_surface_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    ser_axis_id: u32,
) {
    let wireframe = u8::from(chart.wireframe);
    out.push_str(&format!(
        r#"<c:surfaceChart><c:wireframe val="{wireframe}"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:bandFmts/><c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/><c:axId val="{ser_axis_id}"/></c:surfaceChart>"#
    ));
}

fn write_surface_3d_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    ser_axis_id: u32,
) {
    let wireframe = u8::from(chart.wireframe);
    out.push_str(&format!(
        r#"<c:surface3DChart><c:wireframe val="{wireframe}"/>"#
    ));
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    out.push_str(&format!(
        r#"<c:bandFmts/><c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/><c:axId val="{ser_axis_id}"/></c:surface3DChart>"#
    ));
}

fn write_stock_chart(
    out: &mut String,
    chart: &Chart,
    cat_axis_id: u32,
    val_axis_id: u32,
    up_down_bars: bool,
) {
    out.push_str(r#"<c:stockChart>"#);
    for (index, series) in chart.series.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/><c:tx><c:v>{}</c:v></c:tx>"#,
            esc_text(&series.name)
        ));
        write_chart_categories(out, &chart.categories);
        write_chart_values(out, &series.values);
        out.push_str("</c:ser>");
    }
    let up_down_bars_xml = if up_down_bars {
        r#"<c:upDownBars><c:gapWidth val="150"/></c:upDownBars>"#
    } else {
        ""
    };
    out.push_str(&format!(
        r#"<c:hiLowLines/>{up_down_bars_xml}<c:axId val="{cat_axis_id}"/><c:axId val="{val_axis_id}"/></c:stockChart>"#
    ));
}

fn write_chart_categories(out: &mut String, categories: &[String]) {
    out.push_str(&format!(
        r#"<c:cat><c:strLit><c:ptCount val="{}"/>"#,
        categories.len()
    ));
    for (index, category) in categories.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:pt idx="{index}"><c:v>{}</c:v></c:pt>"#,
            esc_text(category)
        ));
    }
    out.push_str("</c:strLit></c:cat>");
}

fn write_chart_values(out: &mut String, values: &[f64]) {
    out.push_str(&format!(
        r#"<c:val><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="{}"/>"#,
        values.len()
    ));
    for (index, value) in values.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:pt idx="{index}"><c:v>{}</c:v></c:pt>"#,
            format_chart_number(*value)
        ));
    }
    out.push_str("</c:numLit></c:val>");
}

fn write_chart_x_values(out: &mut String, count: usize) {
    out.push_str(&format!(
        r#"<c:xVal><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="{count}"/>"#
    ));
    for index in 0..count {
        out.push_str(&format!(
            r#"<c:pt idx="{index}"><c:v>{}</c:v></c:pt>"#,
            format_chart_number((index + 1) as f64)
        ));
    }
    out.push_str("</c:numLit></c:xVal>");
}

fn write_chart_y_values(out: &mut String, values: &[f64]) {
    out.push_str(&format!(
        r#"<c:yVal><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="{}"/>"#,
        values.len()
    ));
    for (index, value) in values.iter().enumerate() {
        out.push_str(&format!(
            r#"<c:pt idx="{index}"><c:v>{}</c:v></c:pt>"#,
            format_chart_number(*value)
        ));
    }
    out.push_str("</c:numLit></c:yVal>");
}

fn write_chart_bubble_sizes(out: &mut String, series: &ChartSeries, count: usize) {
    out.push_str(&format!(
        r#"<c:bubbleSize><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="{count}"/>"#
    ));
    for index in 0..count {
        let size = series.bubble_sizes.get(index).copied().unwrap_or(1.0);
        out.push_str(&format!(
            r#"<c:pt idx="{index}"><c:v>{}</c:v></c:pt>"#,
            format_chart_number(size)
        ));
    }
    out.push_str("</c:numLit></c:bubbleSize>");
}

fn format_chart_number(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "0".to_string()
    }
}

fn write_chart_axes(out: &mut String, kind: ChartKind, cat_axis_id: u32, val_axis_id: u32) {
    let (cat_pos, val_pos) = match kind {
        ChartKind::Bar
        | ChartKind::StackedBar
        | ChartKind::PercentStackedBar
        | ChartKind::Bar3D
        | ChartKind::StackedBar3D
        | ChartKind::PercentStackedBar3D => ("l", "b"),
        ChartKind::Column
        | ChartKind::StackedColumn
        | ChartKind::PercentStackedColumn
        | ChartKind::Column3D
        | ChartKind::StackedColumn3D
        | ChartKind::PercentStackedColumn3D
        | ChartKind::Line
        | ChartKind::LineNoMarkers
        | ChartKind::SmoothLine
        | ChartKind::StackedLine
        | ChartKind::PercentStackedLine
        | ChartKind::Line3D
        | ChartKind::Area
        | ChartKind::StackedArea
        | ChartKind::PercentStackedArea
        | ChartKind::Area3D
        | ChartKind::StackedArea3D
        | ChartKind::PercentStackedArea3D
        | ChartKind::Radar
        | ChartKind::RadarWithMarkers
        | ChartKind::FilledRadar => ("b", "l"),
        ChartKind::Scatter
        | ChartKind::ScatterMarkers
        | ChartKind::ScatterLines
        | ChartKind::ScatterSmooth
        | ChartKind::ScatterSmoothNoMarkers
        | ChartKind::Bubble
        | ChartKind::Bubble3D
        | ChartKind::Pie
        | ChartKind::ExplodedPie
        | ChartKind::Pie3D
        | ChartKind::ExplodedPie3D
        | ChartKind::PieOfPie
        | ChartKind::BarOfPie
        | ChartKind::Doughnut
        | ChartKind::ExplodedDoughnut
        | ChartKind::Surface
        | ChartKind::Surface3D
        | ChartKind::StockHighLowClose
        | ChartKind::Stock => ("b", "l"),
        ChartKind::Waterfall
        | ChartKind::Treemap
        | ChartKind::Sunburst
        | ChartKind::Histogram
        | ChartKind::BoxWhisker
        | ChartKind::Funnel => unreachable!("chartEx kind routed to chartEx writer"),
    };
    out.push_str(&format!(
        concat!(
            r#"<c:catAx><c:axId val="{cat_axis_id}"/>"#,
            r#"<c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#,
            r#"<c:axPos val="{cat_pos}"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/>"#,
            r#"<c:tickLblPos val="nextTo"/><c:crossAx val="{val_axis_id}"/>"#,
            r#"<c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/>"#,
            r#"<c:lblOffset val="100"/></c:catAx>"#
        ),
        cat_axis_id = cat_axis_id,
        val_axis_id = val_axis_id,
        cat_pos = cat_pos
    ));
    out.push_str(&format!(
        concat!(
            r#"<c:valAx><c:axId val="{val_axis_id}"/>"#,
            r#"<c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#,
            r#"<c:axPos val="{val_pos}"/><c:majorGridlines/><c:numFmt formatCode="General" sourceLinked="1"/>"#,
            r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
            r#"<c:tickLblPos val="nextTo"/><c:crossAx val="{cat_axis_id}"/>"#,
            r#"<c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"#
        ),
        cat_axis_id = cat_axis_id,
        val_axis_id = val_axis_id,
        val_pos = val_pos
    ));
}

fn write_surface_axes(out: &mut String, cat_axis_id: u32, val_axis_id: u32, ser_axis_id: u32) {
    write_chart_axes(out, ChartKind::Surface, cat_axis_id, val_axis_id);
    out.push_str(&format!(
        concat!(
            r#"<c:serAx><c:axId val="{ser_axis_id}"/>"#,
            r#"<c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#,
            r#"<c:axPos val="r"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/>"#,
            r#"<c:tickLblPos val="nextTo"/><c:crossAx val="{val_axis_id}"/>"#,
            r#"<c:crosses val="autoZero"/></c:serAx>"#
        ),
        ser_axis_id = ser_axis_id,
        val_axis_id = val_axis_id
    ));
}

fn write_scatter_axes(out: &mut String, x_axis_id: u32, y_axis_id: u32) {
    out.push_str(&format!(
        concat!(
            r#"<c:valAx><c:axId val="{x_axis_id}"/>"#,
            r#"<c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#,
            r#"<c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/>"#,
            r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
            r#"<c:tickLblPos val="nextTo"/><c:crossAx val="{y_axis_id}"/>"#,
            r#"<c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"#
        ),
        x_axis_id = x_axis_id,
        y_axis_id = y_axis_id
    ));
    out.push_str(&format!(
        concat!(
            r#"<c:valAx><c:axId val="{y_axis_id}"/>"#,
            r#"<c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#,
            r#"<c:axPos val="l"/><c:majorGridlines/><c:numFmt formatCode="General" sourceLinked="1"/>"#,
            r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
            r#"<c:tickLblPos val="nextTo"/><c:crossAx val="{x_axis_id}"/>"#,
            r#"<c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"#
        ),
        x_axis_id = x_axis_id,
        y_axis_id = y_axis_id
    ));
}

fn needs_xml_space(text: &str) -> bool {
    text != text.trim_matches([' ', '\t', '\n', '\r'])
}

/// Serialize a [`crate::DocModel`] to `.docx` bytes.
/// Infallible generator (the original contract): yields an empty buffer on the
/// unreachable in-memory ZIP error rather than panicking.
pub(crate) fn to_docx(model: &crate::DocModel) -> Vec<u8> {
    try_to_docx(model).unwrap_or_default()
}

pub(crate) fn to_docx_with_source_hints(
    model: &crate::DocModel,
    source_hints: SourceWriteHints<'_>,
) -> Vec<u8> {
    try_to_docx_with_source_hints(model, Some(source_hints)).unwrap_or_default()
}

/// Fallible generator — used by the public `try_write_docx` so a serialization
/// failure surfaces instead of becoming silent empty bytes.
pub(crate) fn try_to_docx(model: &crate::DocModel) -> crate::Result<Vec<u8>> {
    try_to_docx_with_source_hints(model, None)
}

fn try_to_docx_with_source_hints(
    model: &crate::DocModel,
    source_hints: Option<SourceWriteHints<'_>>,
) -> crate::Result<Vec<u8>> {
    let br = render_body(model, source_hints);

    let mut pkg = Package::new();
    pkg.add_part("word/document.xml", Some(CT_DOCUMENT), br.document_xml);
    for (path, ct, bytes) in br.hf_parts {
        pkg.add_part(&path, Some(ct), bytes);
    }
    for (path, rels) in br.hf_rels {
        pkg.add_rels(&path, rels);
    }
    if let Some(comments) = br.comments_xml {
        pkg.add_part("word/comments.xml", Some(CT_COMMENTS), comments);
    }
    if let Some(comments_ext) = br.comments_ext_xml {
        pkg.add_part(
            "word/commentsExtended.xml",
            Some(CT_COMMENTS_EXT),
            comments_ext,
        );
    }
    if let Some(footnotes) = br.footnotes_xml {
        pkg.add_part("word/footnotes.xml", Some(CT_FOOTNOTES), footnotes);
    }
    if !br.footnote_rels.is_empty() {
        pkg.add_rels("word/_rels/footnotes.xml.rels", br.footnote_rels);
    }
    if let Some(endnotes) = br.endnotes_xml {
        pkg.add_part("word/endnotes.xml", Some(CT_ENDNOTES), endnotes);
    }
    if !br.endnote_rels.is_empty() {
        pkg.add_rels("word/_rels/endnotes.xml.rels", br.endnote_rels);
    }
    let core_properties_xml = core_properties_xml(&model.setup);
    let has_core_properties = core_properties_xml.is_some();
    if let Some(core_properties_xml) = core_properties_xml {
        pkg.add_part(
            "docProps/core.xml",
            Some(CT_CORE_PROPERTIES),
            core_properties_xml,
        );
    }
    if !model.custom_properties.is_empty() {
        pkg.add_part(
            "docProps/custom.xml",
            Some(CT_CUSTOM_PROPERTIES),
            custom_properties_xml(&model.custom_properties),
        );
    }
    let web_extension_task_panes: Vec<&WebExtensionTaskPane> = model
        .setup
        .web_extension_task_panes
        .iter()
        .filter(|pane| valid_web_extension_task_pane(pane))
        .collect();
    for (index, item) in model.custom_xml_items.iter().enumerate() {
        let n = index + 1;
        pkg.add_part(
            &format!("customXml/item{n}.xml"),
            Some(CT_XML),
            item.xml.as_bytes().to_vec(),
        );
        pkg.add_part(
            &format!("customXml/itemProps{n}.xml"),
            Some(CT_CUSTOM_XML_PROPERTIES),
            custom_xml_item_props_xml(&item.store_item_id),
        );
        pkg.add_rels(
            &format!("customXml/_rels/item{n}.xml.rels"),
            vec![Rel {
                id: "rId1".to_string(),
                rel_type: REL_CUSTOM_XML_PROPERTIES.to_string(),
                target: format!("itemProps{n}.xml"),
                external: false,
            }],
        );
    }
    if !web_extension_task_panes.is_empty() {
        pkg.add_part(
            "word/webextensions/taskpanes.xml",
            Some(CT_WEB_EXTENSION_TASKPANES),
            web_extension_taskpanes_xml(&web_extension_task_panes),
        );
        let mut taskpane_rels = Vec::new();
        for (index, pane) in web_extension_task_panes.iter().enumerate() {
            let n = index + 1;
            pkg.add_part(
                &format!("word/webextensions/webextension{n}.xml"),
                Some(CT_WEB_EXTENSION),
                web_extension_xml(pane),
            );
            taskpane_rels.push(Rel {
                id: format!("rId{n}"),
                rel_type: REL_WEB_EXTENSION.to_string(),
                target: format!("webextension{n}.xml"),
                external: false,
            });
        }
        pkg.add_rels("word/webextensions/_rels/taskpanes.xml.rels", taskpane_rels);
    }
    if br.has_list {
        pkg.add_part(
            "word/numbering.xml",
            Some(CT_NUMBERING),
            numbering_xml().into_bytes(),
        );
    }
    if br.has_styles {
        pkg.add_part(
            "word/styles.xml",
            Some(CT_STYLES),
            styles_xml(&model.setup.styles, br.has_heading, &br.source_styles).into_bytes(),
        );
    }
    if let Some(settings_xml) = br.settings_xml {
        pkg.add_part("word/settings.xml", Some(CT_SETTINGS), settings_xml);
    }
    for (path, bytes, ext, ct) in br.media {
        pkg.add_default(ext, ct);
        pkg.add_part(&path, None, bytes);
    }
    for (path, content_type, bytes) in br.chart_parts {
        pkg.add_part(&path, Some(content_type), bytes);
    }
    for (path, bytes) in br.embedded_workbooks {
        pkg.add_part(&path, Some(CT_EMBEDDED_XLSX), bytes);
    }
    for (path, rels) in br.chart_rels {
        pkg.add_rels(&path, rels);
    }

    if !br.doc_rels.is_empty() {
        pkg.add_rels("word/_rels/document.xml.rels", br.doc_rels);
    }
    let mut root_rels = vec![Rel {
        id: "rId1".to_string(),
        rel_type: REL_OFFICE_DOCUMENT.to_string(),
        target: "word/document.xml".to_string(),
        external: false,
    }];
    if !model.custom_properties.is_empty() {
        root_rels.push(Rel {
            id: format!("rId{}", root_rels.len() + 1),
            rel_type: REL_CUSTOM_PROPERTIES.to_string(),
            target: "docProps/custom.xml".to_string(),
            external: false,
        });
    }
    if has_core_properties {
        root_rels.push(Rel {
            id: format!("rId{}", root_rels.len() + 1),
            rel_type: REL_CORE_PROPERTIES.to_string(),
            target: "docProps/core.xml".to_string(),
            external: false,
        });
    }
    if !web_extension_task_panes.is_empty() {
        root_rels.push(Rel {
            id: format!("rId{}", root_rels.len() + 1),
            rel_type: REL_WEB_EXTENSION_TASKPANES.to_string(),
            target: "word/webextensions/taskpanes.xml".to_string(),
            external: false,
        });
    }
    pkg.add_rels("_rels/.rels", root_rels);

    pkg.try_into_zip()
        .map_err(|e| crate::Error::Docx(format!("docx serialize: {e}")))
}

/// The body half of a `.docx`: `word/document.xml` plus everything it references —
/// produced from the model by the from-scratch generator ([`try_to_docx`]).
pub(crate) struct BodyRender {
    /// Serialized `word/document.xml`.
    pub document_xml: Vec<u8>,
    /// Relationships the body references (hyperlinks, images, header/footer) plus
    /// the `styles.xml`/`numbering.xml` type-links, with `rId`s minted from 1.
    pub doc_rels: Vec<Rel>,
    /// `(part path, content-type, bytes)` for any header/footer parts.
    pub hf_parts: Vec<(String, &'static str, Vec<u8>)>,
    /// `(rels path, relationships)` for header/footer part-local relationships.
    pub hf_rels: Vec<(String, Vec<Rel>)>,
    /// Serialized comments part, if authored comments were emitted.
    pub comments_xml: Option<Vec<u8>>,
    /// Serialized commentsExtended part, if authored comment replies were emitted.
    pub comments_ext_xml: Option<Vec<u8>>,
    /// Serialized footnotes part, if authored footnotes were emitted.
    pub footnotes_xml: Option<Vec<u8>>,
    /// Relationships owned by the serialized footnotes part.
    pub footnote_rels: Vec<Rel>,
    /// Serialized endnotes part, if authored endnotes were emitted.
    pub endnotes_xml: Option<Vec<u8>>,
    /// Relationships owned by the serialized endnotes part.
    pub endnote_rels: Vec<Rel>,
    /// Serialized settings part, if document settings were emitted.
    pub settings_xml: Option<Vec<u8>>,
    /// `(part path, bytes, extension, content-type)` for inline/block images.
    pub media: Vec<(String, Vec<u8>, &'static str, &'static str)>,
    /// `(part path, content-type, bytes)` for authored chart parts.
    pub chart_parts: Vec<(String, &'static str, Vec<u8>)>,
    /// `(rels path, relationships)` for authored chart package relationships.
    pub chart_rels: Vec<(String, Vec<Rel>)>,
    /// `(part path, bytes)` for embedded XLSX chart data workbooks.
    pub embedded_workbooks: Vec<(String, Vec<u8>)>,
    pub has_list: bool,
    pub has_styles: bool,
    pub has_heading: bool,
    source_styles: Vec<SourceParagraphStyle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceParagraphStyle {
    id: String,
    name: String,
}

/// Render the body parts from the model. List items reference the synthetic
/// `numbering.xml` (numId 1 = ordered, 2 = bullet). Self-contained: the returned
/// `doc_rels` already include the `numbering`/`styles` type-links.
fn render_body(model: &crate::DocModel, source_hints: Option<SourceWriteHints<'_>>) -> BodyRender {
    let mut ctx = Ctx::new();
    let mut body = String::new();
    let section_count = model
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count()
        + 1;
    let running_surface_distances =
        source_hints.and_then(|hints| hints.aligned_distances(section_count));
    let running_line_spacing =
        source_hints.and_then(|hints| hints.aligned_running_line_spacing(section_count));
    let running_pagination =
        source_hints.and_then(|hints| hints.aligned_running_pagination(section_count));
    let running_tab_stops =
        source_hints.and_then(|hints| hints.aligned_running_tab_stops(section_count));
    let running_table_cell_tab_stops =
        source_hints.and_then(|hints| hints.aligned_running_table_cell_tab_stops(section_count));
    let running_table_layout =
        source_hints.and_then(|hints| hints.aligned_running_table_layout(section_count));
    let running_column_breaks =
        source_hints.and_then(|hints| hints.aligned_running_column_breaks(section_count));
    let running_section_hints = AlignedRunningSectionHints {
        distances: running_surface_distances,
        line_spacing: running_line_spacing,
        pagination: running_pagination,
        tab_stops: running_tab_stops,
        table_cell_tab_stops: running_table_cell_tab_stops,
        table_layout: running_table_layout,
        column_breaks: running_column_breaks,
    };
    let paragraph_line_spacing =
        source_hints.and_then(|hints| hints.aligned_paragraph_line_spacing(model.blocks.len()));
    let paragraph_pagination =
        source_hints.and_then(|hints| hints.aligned_paragraph_pagination(model.blocks.len()));
    let paragraph_tab_stops =
        source_hints.and_then(|hints| hints.aligned_paragraph_tab_stops(model.blocks.len()));
    let column_break_offsets =
        source_hints.and_then(|hints| hints.aligned_column_break_offsets(model.blocks.len()));
    let note_payloads = source_hints.and_then(|hints| hints.aligned_note_payloads(&model.blocks));
    let table_row_pagination =
        source_hints.and_then(|hints| hints.aligned_table_row_pagination(model.blocks.len()));
    let table_cell_pagination =
        source_hints.and_then(|hints| hints.aligned_table_cell_pagination(model.blocks.len()));
    let table_cell_line_spacing =
        source_hints.and_then(|hints| hints.aligned_table_cell_line_spacing(model.blocks.len()));
    let table_cell_column_break_offsets = source_hints
        .and_then(|hints| hints.aligned_table_cell_column_break_offsets(model.blocks.len()));
    let table_nested_pagination =
        source_hints.and_then(|hints| hints.aligned_table_nested_pagination(model.blocks.len()));
    let table_cell_tab_stops =
        source_hints.and_then(|hints| hints.aligned_table_cell_tab_stops(model.blocks.len()));
    let mut section_index = 0;
    for (index, block) in model.blocks.iter().enumerate() {
        ctx.write_top_level_block(
            &mut body,
            block,
            source_hints
                .map(|hints| hints.for_block(index, section_index, running_section_hints))
                .unwrap_or_default(),
            ParagraphWriteHints {
                line_spacing: paragraph_line_spacing
                    .and_then(|hints| hints.get(index))
                    .copied()
                    .flatten(),
                pagination: paragraph_pagination
                    .and_then(|hints| hints.get(index))
                    .copied(),
                tab_stops: paragraph_tab_stops
                    .and_then(|hints| hints.get(index))
                    .map(Vec::as_slice),
                column_break_offsets: column_break_offsets
                    .and_then(|hints| hints.get(index))
                    .map(Vec::as_slice),
                note_payloads: note_payloads
                    .and_then(|hints| hints.get(index))
                    .map(Vec::as_slice),
            },
            TableWriteHints {
                row_pagination: table_row_pagination
                    .and_then(|hints| hints.get(index))
                    .map(Vec::as_slice),
                cell_pagination: table_cell_pagination.and_then(|hints| hints.get(index)),
                cell_line_spacing: table_cell_line_spacing.and_then(|hints| hints.get(index)),
                cell_column_breaks: table_cell_column_break_offsets
                    .and_then(|hints| hints.get(index)),
                nested_tables: table_nested_pagination.and_then(|hints| hints.get(index)),
                cell_tab_stops: table_cell_tab_stops.and_then(|hints| hints.get(index)),
            },
        );
        if matches!(block, Block::SectionBreak(_)) {
            section_index += 1;
        }
    }

    // word/document.xml
    let mut doc = String::new();
    doc.push_str(XML_DECL);
    doc.push_str(&format!(
        r#"<w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}" xmlns:wp="{WP_NS}" xmlns:a="{A_NS}" xmlns:c="{C_NS}" xmlns:cx="{CX_NS}" xmlns:pic="{PIC_NS}"><w:body>"#
    ));
    doc.push_str(&body);

    // Final section properties describe the last section in the body. Earlier
    // section breaks were emitted while folding blocks.
    ctx.write_sect_pr(
        &mut doc,
        &SectionSetup::from(&model.setup),
        None,
        source_hints
            .map(|hints| hints.final_section(running_section_hints))
            .unwrap_or_default(),
    );
    let comments_xml = if ctx.comments.is_empty() {
        None
    } else {
        let rid = format!("rId{}", ctx.next_rid);
        ctx.next_rid += 1;
        ctx.doc_rels.push(Rel {
            id: rid,
            rel_type: REL_COMMENTS.to_string(),
            target: "comments.xml".to_string(),
            external: false,
        });
        Some(comments_xml(&ctx.comments))
    };
    let comments_ext_xml = comments_extended_xml(&ctx.comments);
    if comments_ext_xml.is_some() {
        ctx.doc_rels.push(Rel {
            id: format!("rId{}", ctx.next_rid),
            rel_type: REL_COMMENTS_EXT.to_string(),
            target: "commentsExtended.xml".to_string(),
            external: false,
        });
        ctx.next_rid += 1;
    }
    let footnotes_xml = if ctx.footnotes.is_empty() {
        None
    } else {
        ctx.doc_rels.push(Rel {
            id: format!("rId{}", ctx.next_rid),
            rel_type: REL_FOOTNOTES.to_string(),
            target: "footnotes.xml".to_string(),
            external: false,
        });
        ctx.next_rid += 1;
        Some(notes_xml(
            "footnotes",
            "footnote",
            &ctx.footnotes,
            !ctx.footnote_rels.is_empty(),
        ))
    };
    let endnotes_xml = if ctx.endnotes.is_empty() {
        None
    } else {
        ctx.doc_rels.push(Rel {
            id: format!("rId{}", ctx.next_rid),
            rel_type: REL_ENDNOTES.to_string(),
            target: "endnotes.xml".to_string(),
            external: false,
        });
        ctx.next_rid += 1;
        Some(notes_xml(
            "endnotes",
            "endnote",
            &ctx.endnotes,
            !ctx.endnote_rels.is_empty(),
        ))
    };
    let settings_xml = if ctx.has_even_header_footer || model.setup.document_id.is_some() {
        Some(
            settings_xml(
                ctx.has_even_header_footer,
                model.setup.document_id.as_deref(),
            )
            .into_bytes(),
        )
    } else {
        None
    };
    doc.push_str("</w:body></w:document>");

    // Type-link rels for the styles/numbering parts (read-by-path doesn't need
    // them, but strict consumers like Word expect the relationship to exist). Minted
    // after the hf rels so ids match the previous single-pass writer exactly.
    if ctx.has_list {
        ctx.doc_rels.push(Rel {
            id: format!("rId{}", ctx.next_rid),
            rel_type: REL_NUMBERING.to_string(),
            target: "numbering.xml".to_string(),
            external: false,
        });
        ctx.next_rid += 1;
    }
    let has_styles = ctx.has_styles || !model.setup.styles.is_empty();
    if has_styles {
        ctx.doc_rels.push(Rel {
            id: format!("rId{}", ctx.next_rid),
            rel_type: REL_STYLES.to_string(),
            target: "styles.xml".to_string(),
            external: false,
        });
        ctx.next_rid += 1;
    }
    if settings_xml.is_some() {
        ctx.doc_rels.push(Rel {
            id: format!("rId{}", ctx.next_rid),
            rel_type: REL_SETTINGS.to_string(),
            target: "settings.xml".to_string(),
            external: false,
        });
        ctx.next_rid += 1;
    }

    BodyRender {
        document_xml: doc.into_bytes(),
        doc_rels: ctx.doc_rels,
        hf_parts: ctx.hf_parts,
        hf_rels: ctx.hf_rels,
        comments_xml,
        comments_ext_xml,
        footnotes_xml,
        footnote_rels: ctx.footnote_rels,
        endnotes_xml,
        endnote_rels: ctx.endnote_rels,
        settings_xml,
        media: ctx.media,
        chart_parts: ctx.chart_parts,
        chart_rels: ctx.chart_rels,
        embedded_workbooks: ctx.embedded_workbooks,
        has_list: ctx.has_list,
        has_styles,
        has_heading: ctx.has_heading,
        source_styles: ctx.source_styles,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_body, section_columns_xml, source_column_break_offsets, source_line_spacing,
        source_note_content_control_is_supported, source_note_field_is_supported,
        source_note_paragraph_is_supported, source_note_payload_is_supported,
        source_note_style_ref_fields_are_supported, source_note_table_formula_fields_are_supported,
        source_tab_stops_xml, styles_xml, SectionColumnWriteHint, SourceParagraphStyle,
        SourceWriteHints, REL_HYPERLINK, REL_IMAGE,
    };
    use crate::model::{
        Align, AuthoredComment, AuthoredContentControl, AuthoredNote, AuthoredRevision, Block,
        Cell, CharProps, Chart, ChartKind, ChartSeries, DocModel, DocSetup, FieldRole,
        FieldUnsupportedReason, Image, LineSpacingHint, ListInfo, NoteWritePayload, PaginationHint,
        ParaProps, Paragraph, Row, Run, RunningBlockPaginationHints,
        RunningSurfaceColumnBreakHints, RunningSurfaceDistanceHints,
        RunningSurfaceLineSpacingHints, RunningSurfacePaginationHints, RunningSurfaceTabStopHints,
        RunningSurfaceTableCellTabStopHints, RunningSurfaceTableLayoutHints,
        RunningTableLayoutHints, SectionColumnHint, SectionColumnLayoutHints, SectionSetup,
        TabAlignment, TabLeader, TabStop, Table, TableCellColumnBreakHints,
        TableCellLineSpacingHints, TableCellNestedPaginationHints, TableCellPaginationHints,
        TableCellTabStopHints, TablePaginationHints, TableRowPaginationHint, MAX_TAB_STOPS,
    };
    use crate::{Document, NoteKind};

    fn para(text: &str) -> Paragraph {
        Paragraph {
            props: ParaProps::default(),
            runs: vec![Run {
                text: text.to_string(),
                ..Run::default()
            }],
        }
    }

    fn cell(text: &str) -> Cell {
        Cell {
            blocks: vec![Block::Paragraph(para(text))],
            ..Cell::default()
        }
    }

    #[test]
    fn source_style_names_fill_only_undefined_paragraph_styles() {
        let explicit = [
            crate::model::ParagraphStyle {
                id: "Custom".to_string(),
                name: "Authored Custom".to_string(),
                ..crate::model::ParagraphStyle::default()
            },
            crate::model::ParagraphStyle {
                id: "Recovered".to_string(),
                name: "   ".to_string(),
                ..crate::model::ParagraphStyle::default()
            },
        ];
        let xml = styles_xml(
            &explicit,
            true,
            &[
                SourceParagraphStyle {
                    id: "Normal".to_string(),
                    name: "Source Normal".to_string(),
                },
                SourceParagraphStyle {
                    id: "Custom".to_string(),
                    name: "Source Custom".to_string(),
                },
                SourceParagraphStyle {
                    id: "Heading1".to_string(),
                    name: "Source Heading".to_string(),
                },
                SourceParagraphStyle {
                    id: "Recovered".to_string(),
                    name: "Recovered Name".to_string(),
                },
                SourceParagraphStyle {
                    id: "Callout&One".to_string(),
                    name: "Callout <One>".to_string(),
                },
            ],
        );

        assert_eq!(xml.matches(r#"w:styleId="Normal""#).count(), 1);
        assert_eq!(xml.matches(r#"w:styleId="Custom""#).count(), 1);
        assert_eq!(xml.matches(r#"w:styleId="Heading1""#).count(), 1);
        assert!(!xml.contains("Source Normal"));
        assert!(!xml.contains("Source Custom"));
        assert!(xml.contains(
            r#"w:styleId="Heading1"><w:name w:val="Source Heading"/><w:basedOn w:val="Normal"/>"#
        ));
        assert!(xml.contains(r#"w:styleId="Recovered"><w:name w:val="Recovered Name"/>"#));
        assert!(
            xml.contains(r#"w:styleId="Callout&amp;One"><w:name w:val="Callout &lt;One&gt;"/>"#)
        );
    }

    fn tab(position_pt: f32, alignment: TabAlignment, leader: TabLeader) -> TabStop {
        TabStop {
            position_pt,
            alignment,
            leader,
        }
    }

    #[test]
    fn source_note_content_control_guard_requires_meaningful_complete_metadata() {
        assert!(source_note_content_control_is_supported(None));
        assert!(source_note_content_control_is_supported(Some(
            &AuthoredContentControl {
                alias: Some(" Alias ".to_string()),
                ..AuthoredContentControl::default()
            }
        )));
        assert!(source_note_content_control_is_supported(Some(
            &AuthoredContentControl {
                tag: Some(" Tag ".to_string()),
                ..AuthoredContentControl::default()
            }
        )));
        assert!(source_note_content_control_is_supported(Some(
            &AuthoredContentControl {
                data_binding_xpath: Some(" /root/value ".to_string()),
                data_binding_store_item_id: Some(" {STORE-ID} ".to_string()),
                ..AuthoredContentControl::default()
            }
        )));

        for rejected in [
            AuthoredContentControl::default(),
            AuthoredContentControl {
                alias: Some("   ".to_string()),
                tag: Some("\t".to_string()),
                ..AuthoredContentControl::default()
            },
            AuthoredContentControl {
                data_binding_xpath: Some("/root/value".to_string()),
                ..AuthoredContentControl::default()
            },
            AuthoredContentControl {
                data_binding_store_item_id: Some("{STORE-ID}".to_string()),
                ..AuthoredContentControl::default()
            },
            AuthoredContentControl {
                tag: Some("tag".to_string()),
                data_binding_xpath: Some("/root/value".to_string()),
                ..AuthoredContentControl::default()
            },
        ] {
            assert!(!source_note_content_control_is_supported(Some(&rejected)));
        }
    }

    #[test]
    fn source_note_field_guard_accepts_only_owned_normalized_results() {
        let cached = |instruction: &str| Run {
            text: "cached".to_string(),
            field: FieldRole::Simple {
                instruction: instruction.to_string(),
            },
            field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
            ..Run::default()
        };
        for instruction in [
            "PRIVATE legacy-data",
            r#"INCLUDETEXT "appendix.docx""#,
            "ADDRESSBLOCK",
            r#"DISPLAYBARCODE "12345" QR \q H"#,
        ] {
            assert!(source_note_field_is_supported(&cached(instruction)));
        }

        let merge = Run {
            text: "cached".to_string(),
            field: FieldRole::Simple {
                instruction: r#"MERGEFIELD "Client Name" \* Upper"#.to_string(),
            },
            ..Run::default()
        };
        assert!(source_note_field_is_supported(&merge));
        let filename = Run {
            text: "cached".to_string(),
            field: FieldRole::Simple {
                instruction: r#"FILENAME \p \* Upper"#.to_string(),
            },
            ..Run::default()
        };
        assert!(source_note_field_is_supported(&filename));
        let mut dirty_filename = filename.clone();
        dirty_filename.field_dirty = true;
        let marker = |instruction: &str| Run {
            field: FieldRole::Simple {
                instruction: instruction.to_string(),
            },
            ..Run::default()
        };
        for instruction in [
            r#"XE "Mercury" \t "See planets" \* FirstCap"#,
            r#"TA \l "Case v. Example" \c 1 \* CHARFORMAT"#,
            r#"rd "appendix.docx" \* MERGEFORMAT"#,
            r#"TC "Manual entry" \f m \l 2 \* Caps"#,
            r#"SYMBOL 183 \f Symbol \s 12"#,
            r#"QUOTE "literal note" \* Upper"#,
            r#"IF 2 >= 1 "ready note" "held note" \* Caps"#,
            r#"COMPARE "Alpha-42" = "Alpha-*""#,
            r#"= 10 / 4 \# "0.00""#,
            r#"= ROUND(AVERAGE(2; 4; 7); 1) \# "0.0""#,
            "= Amount + 1",
            "= SUM(LEFT)",
            "= DEFINED(Known)",
            "= 1e309 + 1",
            r#"FILLIN "Client?" \d "Acme" \o \* Caps"#,
            r#"FILLIN Project display prompt \d Client 42 \* Upper"#,
            r#"EQ \f( "Alpha, One" , "Beta Two" ) \* Upper"#,
            r#"ADVANCE \r"2" \d4 \* MERGEFORMAT"#,
            r#"DOCPROPERTY Subject \* Upper"#,
            r#"DOCPROPERTY "Client Name" \* Caps"#,
            r#"TITLE \* Upper"#,
            r#"REVNUM \* Caps"#,
            r#"REF LocalTarget \h \! \* Caps"#,
            r#"STYLEREF Heading1 \* Upper"#,
        ] {
            assert!(source_note_field_is_supported(&marker(instruction)));
        }
        let mut dirty_marker = marker(r#"XE "Dirty""#);
        dirty_marker.field_dirty = true;
        let mut dirty_symbol = marker(r#"SYMBOL 0x03BB \u"#);
        dirty_symbol.field_dirty = true;
        let mut dirty_quote = marker(r#"QUOTE "dirty note""#);
        dirty_quote.field_dirty = true;
        let mut dirty_if = marker(r#"IF 1 = 1 "dirty" "clean""#);
        dirty_if.field_dirty = true;
        let mut dirty_formula = marker("= 2 + 3");
        dirty_formula.field_dirty = true;
        let mut dirty_fill_in = marker(r#"FILLIN "Client?" \d "Acme""#);
        dirty_fill_in.field_dirty = true;
        let mut dirty_display = marker(r#"EQ \f(1,2)"#);
        dirty_display.field_dirty = true;
        let mut dirty_document_info = marker("DOCPROPERTY Subject");
        dirty_document_info.field_dirty = true;
        let mut dirty_revision_number = marker("REVNUM");
        dirty_revision_number.field_dirty = true;
        let mut dirty_local_ref = marker("REF LocalTarget");
        dirty_local_ref.field_dirty = true;
        let mut dirty_style_ref = marker("STYLEREF Heading1");
        dirty_style_ref.field_dirty = true;
        let sequence_reset = Run {
            text: "1F".to_string(),
            field: FieldRole::Simple {
                instruction: r#"SEQ Appendix \r31 \* Hex"#.to_string(),
            },
            ..Run::default()
        };
        assert!(source_note_field_is_supported(&sequence_reset));
        let mut dirty_sequence_reset = sequence_reset.clone();
        dirty_sequence_reset.field_dirty = true;
        let mut reasoned_sequence_reset = sequence_reset.clone();
        reasoned_sequence_reset.field_unsupported_reason =
            Some(FieldUnsupportedReason::NoComputedResult);
        let mut mismatched_sequence_reset = sequence_reset.clone();
        mismatched_sequence_reset.text = "1E".to_string();
        let listnum_start = Run {
            text: "1F".to_string(),
            field: FieldRole::Simple {
                instruction: r#"LISTNUM LegalDefault \l1 \s31 \* Hex"#.to_string(),
            },
            ..Run::default()
        };
        assert!(source_note_field_is_supported(&listnum_start));
        let mut dirty_listnum_start = listnum_start.clone();
        dirty_listnum_start.field_dirty = true;
        let mut reasoned_listnum_start = listnum_start.clone();
        reasoned_listnum_start.field_unsupported_reason =
            Some(FieldUnsupportedReason::NoComputedResult);
        let mut mismatched_listnum_start = listnum_start.clone();
        mismatched_listnum_start.text = "1E".to_string();

        let mut dirty = cached("PRIVATE legacy-data");
        dirty.field_dirty = true;
        let mut malformed = cached(r#"ADDIN "bad"#);
        malformed.field_unsupported_reason = Some(FieldUnsupportedReason::UnsupportedSwitch);
        let malformed_merge = Run {
            field: FieldRole::Simple {
                instruction: "MERGEFIELD \\* MERGEFORMAT".to_string(),
            },
            ..Run::default()
        };
        let malformed_filename = Run {
            field: FieldRole::Simple {
                instruction: "FILENAME \\x".to_string(),
            },
            ..Run::default()
        };
        for rejected in [
            dirty,
            dirty_filename,
            dirty_marker,
            dirty_symbol,
            dirty_quote,
            dirty_if,
            dirty_formula,
            dirty_fill_in,
            dirty_display,
            dirty_document_info,
            dirty_revision_number,
            dirty_local_ref,
            dirty_style_ref,
            dirty_sequence_reset,
            reasoned_sequence_reset,
            mismatched_sequence_reset,
            dirty_listnum_start,
            reasoned_listnum_start,
            mismatched_listnum_start,
            malformed,
            malformed_merge,
            malformed_filename,
            marker(r#"TA \l "Broken Case" \c"#),
            marker(r#"TC \f m \l 2"#),
            marker(r#"SYMBOL 65 \f "Wingdings"#),
            marker(r#"SYMBOL 66 \f Wingdings"#),
            marker(r#"QUOTE "broken note"#),
            marker(r#"IF Gate = "Ready" "yes" "no""#),
            marker(r#"IF 2 > 1 "yes" "no"#),
            marker(r#"COMPARE 1e309 > 0"#),
            marker("= 1 +"),
            marker(r#"FILLIN "Client?""#),
            marker(r#"FILLIN "Client?" \d \o"#),
            marker(r#"FILLIN "broken prompt \d Acme"#),
            marker(r#"ASK ClientCode "Client code?" \d "ac-42""#),
            marker(r#"EQ \f(1,"#),
            marker(r#"ADVANCE \z 2"#),
            marker(r#"NUMPAGES \* ROMAN"#),
            marker("DOCPROPERTY Pages"),
            marker(r#"DOCVARIABLE ClientCode \* Upper"#),
            marker("FILESIZE"),
            marker(r#"DATE \@ "yyyy-MM-dd""#),
            marker("USERNAME"),
            marker(r#"REVNUM \x"#),
            marker(r#"REF LocalTarget \p"#),
            marker(r#"REF LocalTarget \n"#),
            marker(r#"REF LocalTarget \f"#),
            marker(r#"REF LocalTarget \d ".""#),
            marker(r#"REF LocalTarget \x"#),
            marker(r#"STYLEREF Heading1 \n"#),
            marker(r#"STYLEREF "Broken Heading"#),
            marker("SEQ Figure"),
            marker(r#"SEQ Figure \c"#),
            marker(r#"SEQ Figure \s 1"#),
            marker(r#"SEQ Figure \r 7 \h"#),
            marker(r#"SEQ Figure \r -1"#),
            marker("LISTNUM NumberDefault"),
            marker(r#"LISTNUM NumberDefault \l 2 \s 4"#),
            marker(r#"LISTNUM NumberDefault \s -1"#),
            marker(r#"LISTNUM NumberDefault \s 4 \s 5"#),
            marker(r#"AUTONUM \* ROMAN"#),
            marker(r#"DOCPROPERTY "Broken Name"#),
            marker(r#"MACROBUTTON RunReport "Run""#),
            marker(r#"INDEX \e " - ""#),
            marker(r#"TOC \f m"#),
            cached("MERGEFIELD Client"),
            cached("FILENAME \\p"),
            cached(r#"XE "Wrong reason""#),
            cached(r#"TC "Wrong reason""#),
            cached(r#"SYMBOL 0x03BB \u"#),
            cached(r#"QUOTE "wrong reason""#),
            cached(r#"IF 2 > 1 "wrong reason" "no""#),
            cached("= 2 + 3"),
            cached(r#"FILLIN "Client?" \d "wrong reason""#),
            cached(r#"EQ \f(1,2)"#),
            cached(r#"ADVANCE \r2"#),
            cached("DOCPROPERTY Subject"),
            cached("REVNUM"),
            cached("REF LocalTarget"),
            Run {
                field: FieldRole::Simple {
                    instruction: "CUSTOM literal payload".to_string(),
                },
                ..Run::default()
            },
            Run {
                field: FieldRole::Simple {
                    instruction: String::new(),
                },
                field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                ..Run::default()
            },
            Run {
                field: FieldRole::Other,
                ..Run::default()
            },
        ] {
            assert!(!source_note_field_is_supported(&rejected));
        }

        let split_result = Paragraph {
            runs: vec![
                marker(r#"IF 1 = 1 "split" "no""#),
                Run {
                    field: FieldRole::Other,
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        };
        assert!(!source_note_paragraph_is_supported(&split_result));
    }

    #[test]
    fn source_note_style_refs_require_the_selected_same_note_plain_paragraph() {
        fn styled_paragraph(style_id: &str, style_name: &str, runs: Vec<Run>) -> Paragraph {
            Paragraph {
                props: ParaProps {
                    style_id: Some(style_id.to_string()),
                    style_name: Some(style_name.to_string()),
                    ..ParaProps::default()
                },
                runs,
            }
        }

        fn style_ref(instruction: &str, result: &str) -> Paragraph {
            Paragraph {
                runs: vec![Run {
                    text: result.to_string(),
                    field: FieldRole::Simple {
                        instruction: instruction.to_string(),
                    },
                    ..Run::default()
                }],
                ..Paragraph::default()
            }
        }

        let prior = vec![
            Block::Paragraph(styled_paragraph(
                "Heading1",
                "Heading 1",
                vec![Run {
                    text: "  Alpha\t heading  ".to_string(),
                    ..Run::default()
                }],
            )),
            Block::Paragraph(style_ref(
                r#"STYLEREF "Heading 1" \* Upper"#,
                "ALPHA HEADING",
            )),
        ];
        assert!(source_note_style_ref_fields_are_supported(&prior));

        let nested_later = vec![Block::Table(Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![
                        Block::Paragraph(style_ref(
                            r#"STYLEREF \p "Callout Style" \* Caps"#,
                            "Below",
                        )),
                        Block::Paragraph(styled_paragraph(
                            "Callout",
                            "Callout Style",
                            vec![Run {
                                text: "Later target".to_string(),
                                ..Run::default()
                            }],
                        )),
                    ],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        })];
        assert!(source_note_style_ref_fields_are_supported(&nested_later));

        let missing = vec![Block::Paragraph(style_ref("STYLEREF Heading1", "Missing"))];
        assert!(!source_note_style_ref_fields_are_supported(&missing));

        let multi_run = vec![
            Block::Paragraph(styled_paragraph(
                "Heading1",
                "Heading 1",
                vec![
                    Run {
                        text: "Multi ".to_string(),
                        ..Run::default()
                    },
                    Run {
                        text: "source".to_string(),
                        ..Run::default()
                    },
                ],
            )),
            Block::Paragraph(style_ref("STYLEREF Heading1", "Multi source")),
        ];
        assert!(!source_note_style_ref_fields_are_supported(&multi_run));

        let field_backed = vec![
            Block::Paragraph(styled_paragraph(
                "Heading1",
                "Heading 1",
                vec![Run {
                    text: "Computed source".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"QUOTE "Computed source""#.to_string(),
                    },
                    ..Run::default()
                }],
            )),
            Block::Paragraph(style_ref("STYLEREF Heading1", "Computed source")),
        ];
        assert!(!source_note_style_ref_fields_are_supported(&field_backed));

        let shadowed = vec![
            prior[0].clone(),
            Block::Paragraph(styled_paragraph(
                "Heading1",
                "Heading 1",
                vec![
                    Run {
                        text: "Shadowed ".to_string(),
                        ..Run::default()
                    },
                    Run {
                        text: "target".to_string(),
                        ..Run::default()
                    },
                ],
            )),
            Block::Paragraph(style_ref("STYLEREF Heading1", "Shadowed target")),
        ];
        assert!(!source_note_style_ref_fields_are_supported(&shadowed));

        let numbered = vec![
            prior[0].clone(),
            Block::Paragraph(style_ref(r#"STYLEREF Heading1 \n"#, "1")),
        ];
        assert!(!source_note_style_ref_fields_are_supported(&numbered));

        let mismatch = vec![
            prior[0].clone(),
            Block::Paragraph(style_ref("STYLEREF Heading1", "Stale heading")),
        ];
        assert!(!source_note_style_ref_fields_are_supported(&mismatch));

        let mut dirty = prior.clone();
        let Block::Paragraph(field) = &mut dirty[1] else {
            unreachable!()
        };
        field.runs[0].field_dirty = true;
        assert!(!source_note_style_ref_fields_are_supported(&dirty));

        let mut reasoned = prior;
        let Block::Paragraph(field) = &mut reasoned[1] else {
            unreachable!()
        };
        field.runs[0].field_unsupported_reason = Some(FieldUnsupportedReason::NoComputedResult);
        assert!(!source_note_style_ref_fields_are_supported(&reasoned));
    }

    #[test]
    fn source_note_table_formulas_require_one_exact_span_free_owned_matrix() {
        fn formula_run(instruction: &str, text: &str) -> Run {
            Run {
                text: text.to_string(),
                field: FieldRole::Simple {
                    instruction: instruction.to_string(),
                },
                ..Run::default()
            }
        }

        fn formula_table() -> Table {
            Table {
                rows: vec![Row {
                    cells: vec![
                        cell("2"),
                        cell("3"),
                        Cell {
                            blocks: vec![Block::Paragraph(Paragraph {
                                runs: vec![formula_run(r#"= SUM(LEFT) \# "0.00""#, "5.00")],
                                ..Paragraph::default()
                            })],
                            ..Cell::default()
                        },
                    ],
                }],
                ..Table::default()
            }
        }

        fn run_mut(table: &mut Table, row: usize, col: usize) -> &mut Run {
            let Block::Paragraph(paragraph) = &mut table.rows[row].cells[col].blocks[0] else {
                panic!("formula test paragraph")
            };
            &mut paragraph.runs[0]
        }

        let valid = formula_table();
        let valid_blocks = vec![Block::Table(valid.clone())];
        assert!(source_note_table_formula_fields_are_supported(
            &valid_blocks
        ));
        let mut defined = valid.clone();
        *run_mut(&mut defined, 0, 2) = formula_run("= DEFINED(A1)", "1");
        assert!(source_note_table_formula_fields_are_supported(&[
            Block::Table(defined)
        ]));
        let text = crate::docx::blocks_text(&valid_blocks);
        let note = AuthoredNote {
            kind: NoteKind::Footnote,
            text: text.clone(),
        };
        let payload = NoteWritePayload {
            kind: NoteKind::Footnote,
            text,
            blocks: valid_blocks,
            pagination: Vec::new(),
            line_spacing: Vec::new(),
            tab_stops: Vec::new(),
            column_break_offsets: Vec::new(),
            table_pagination: Vec::new(),
        };
        assert!(source_note_payload_is_supported(&note, &payload));

        let nested = Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Table(valid.clone())],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        };
        assert!(source_note_table_formula_fields_are_supported(&[
            Block::Table(nested)
        ]));

        let mut mismatch = valid.clone();
        run_mut(&mut mismatch, 0, 2).text = "5.01".to_string();
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(mismatch)
        ]));

        let mut multiple = valid.clone();
        *run_mut(&mut multiple, 0, 0) = formula_run("= B1 + 1", "4");
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(multiple)
        ]));

        let mut dirty = valid.clone();
        run_mut(&mut dirty, 0, 2).field_dirty = true;
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(dirty)
        ]));

        let mut unsupported = valid.clone();
        run_mut(&mut unsupported, 0, 2).field_unsupported_reason =
            Some(FieldUnsupportedReason::NoComputedResult);
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(unsupported)
        ]));

        let mut malformed = valid.clone();
        run_mut(&mut malformed, 0, 2).field = FieldRole::Simple {
            instruction: "= SUM(LEFT".to_string(),
        };
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(malformed)
        ]));

        let mut bidi = valid.clone();
        bidi.bidi_visual = true;
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(bidi)
        ]));

        let mut header = valid.clone();
        header.header_rows = 1;
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(header)
        ]));

        let mut span = valid.clone();
        span.rows[0].cells[0].col_span = 2;
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(span)
        ]));

        let mut multi_run = valid.clone();
        let Block::Paragraph(paragraph) = &mut multi_run.rows[0].cells[0].blocks[0] else {
            panic!("operand paragraph")
        };
        paragraph.runs.push(Run {
            text: " extra".to_string(),
            ..Run::default()
        });
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(multi_run)
        ]));

        let mut nested_operand = valid.clone();
        nested_operand.rows[0].cells[0].blocks = vec![Block::Table(Table {
            rows: vec![Row {
                cells: vec![cell("2")],
            }],
            ..Table::default()
        })];
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(nested_operand)
        ]));

        let mut field_backed = valid.clone();
        run_mut(&mut field_backed, 0, 0).field = FieldRole::Simple {
            instruction: "= 1 + 1".to_string(),
        };
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(field_backed)
        ]));

        let mut empty = valid.clone();
        run_mut(&mut empty, 0, 0).text.clear();
        assert!(!source_note_table_formula_fields_are_supported(&[
            Block::Table(empty)
        ]));

        let outside_table = Block::Paragraph(Paragraph {
            runs: vec![formula_run("= SUM(LEFT)", "5")],
            ..Paragraph::default()
        });
        assert!(!source_note_table_formula_fields_are_supported(&[
            outside_table
        ]));
    }

    #[test]
    fn source_note_local_ref_fields_require_one_stable_same_payload_target() {
        let target = Run {
            text: "alpha launch".to_string(),
            bookmark: Some("LocalTarget".to_string()),
            ..Run::default()
        };
        let field = Run {
            text: "Alpha Launch".to_string(),
            field: FieldRole::Simple {
                instruction: r#"REF LocalTarget \* Caps"#.to_string(),
            },
            ..Run::default()
        };
        let supported = |runs: Vec<Run>| {
            let paragraph = Paragraph {
                runs,
                ..Paragraph::default()
            };
            let text = paragraph.text();
            let note = AuthoredNote {
                kind: NoteKind::Footnote,
                text: text.clone(),
            };
            let payload = NoteWritePayload {
                kind: NoteKind::Footnote,
                text,
                blocks: vec![Block::Paragraph(paragraph)],
                pagination: vec![PaginationHint::default()],
                line_spacing: vec![None],
                tab_stops: vec![Vec::new()],
                column_break_offsets: vec![Vec::new()],
                table_pagination: vec![None],
            };
            source_note_payload_is_supported(&note, &payload)
        };

        assert!(supported(vec![target.clone(), field.clone()]));
        assert!(!supported(vec![field.clone()]));
        assert!(!supported(vec![
            target.clone(),
            target.clone(),
            field.clone(),
        ]));

        let mut mismatched = field.clone();
        mismatched.text = "Wrong Result".to_string();
        assert!(!supported(vec![target.clone(), mismatched]));

        let mut field_backed_target = target.clone();
        field_backed_target.field = FieldRole::Simple {
            instruction: r#"QUOTE "alpha launch""#.to_string(),
        };
        assert!(!supported(vec![field_backed_target, field.clone()]));

        let mut empty_target = target;
        empty_target.text.clear();
        assert!(!supported(vec![empty_target, field]));
    }

    fn written_paragraph_with_text<'a>(xml: &'a str, text: &str) -> &'a str {
        let marker = format!(">{text}</w:t>");
        let text_offset = xml.find(&marker).expect("paragraph text");
        let start = xml[..text_offset].rfind("<w:p>").expect("paragraph start");
        let end = text_offset
            + xml[text_offset..].find("</w:p>").expect("paragraph end")
            + "</w:p>".len();
        &xml[start..end]
    }

    fn written_note_with_text<'a>(xml: &'a str, item: &str, text: &str) -> &'a str {
        let text_offset = xml.find(text).expect("note text");
        let start_marker = format!("<w:{item} w:id=");
        let start = xml[..text_offset].rfind(&start_marker).expect("note start");
        let end_marker = format!("</w:{item}>");
        let end = text_offset
            + xml[text_offset..].find(&end_marker).expect("note end")
            + end_marker.len();
        &xml[start..end]
    }

    fn generated_running_part<'a>(
        rendered: &'a super::BodyRender,
        text: &str,
    ) -> (&'a str, &'a str) {
        rendered
            .hf_parts
            .iter()
            .find_map(|(path, _, bytes)| {
                let xml = std::str::from_utf8(bytes).expect("running part is UTF-8");
                xml.contains(text).then_some((path.as_str(), xml))
            })
            .unwrap_or_else(|| panic!("missing running part containing {text:?}"))
    }

    #[test]
    fn source_section_column_writer_falls_back_from_invalid_private_geometry() {
        let invalid = SectionColumnLayoutHints {
            columns: vec![
                SectionColumnHint {
                    width_pt: f32::NAN,
                    space_after_pt: 20.0,
                },
                SectionColumnHint {
                    width_pt: 200.0,
                    space_after_pt: 0.0,
                },
            ],
        };
        let mismatched = SectionColumnLayoutHints {
            columns: vec![SectionColumnHint {
                width_pt: 100.0,
                space_after_pt: 0.0,
            }],
        };
        let oversized = SectionColumnLayoutHints {
            columns: vec![
                SectionColumnHint {
                    width_pt: 100.0,
                    space_after_pt: 0.0,
                };
                65
            ],
        };

        for (count, layout) in [(2, &invalid), (2, &mismatched), (65, &oversized)] {
            assert_eq!(
                section_columns_xml(
                    Some(count),
                    SectionColumnWriteHint {
                        gap_pt: Some(18.0),
                        layout: Some(layout),
                        separator: true,
                        rtl: false,
                    },
                ),
                format!(r#"<w:cols w:num="{count}" w:space="360" w:sep="1"/>"#)
            );
        }
    }

    #[test]
    fn source_section_distance_writer_rejects_misaligned_vector() {
        let model = DocModel {
            blocks: vec![Block::SectionBreak(SectionSetup::default())],
            ..DocModel::default()
        };
        let gaps = [None];
        let layouts = [None];
        let separators = [false];
        let rtl = [false];
        let distances = [RunningSurfaceDistanceHints {
            header_pt: Some(0.0),
            footer_pt: Some(20.0),
        }];
        let rendered = render_body(
            &model,
            Some(SourceWriteHints {
                gaps: &gaps,
                layouts: &layouts,
                separators: &separators,
                rtl: &rtl,
                final_gap: None,
                final_layout: None,
                final_separator: false,
                final_rtl: false,
                running_surface_distances: &distances,
                running_line_spacing: &[],
                running_pagination: &[],
                running_tab_stops: &[],
                running_table_cell_tab_stops: &[],
                running_table_layout: &[],
                running_column_break_offsets: &[],
                note_payloads: &[],
                paragraph_line_spacing: &[],
                paragraph_pagination: &[],
                paragraph_tab_stops: &[],
                column_break_offsets: &[],
                table_cell_column_break_offsets: &[],
                table_row_pagination: &[],
                table_cell_pagination: &[],
                table_cell_line_spacing: &[],
                table_nested_pagination: &[],
                table_cell_tab_stops: &[],
            }),
        );
        let document_xml = String::from_utf8(rendered.document_xml).unwrap();

        assert_eq!(
            document_xml
                .matches(r#"w:header="708" w:footer="708""#)
                .count(),
            2,
            "{document_xml}"
        );
    }

    #[test]
    fn source_paragraph_spacing_writer_rejects_misaligned_vector() {
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("body"))],
            ..DocModel::default()
        };
        let gaps = [None];
        let layouts = [None];
        let separators = [false];
        let rtl = [false];
        let distances = [RunningSurfaceDistanceHints::default()];
        let line_spacing = [
            Some(LineSpacingHint::Exact(12.0)),
            Some(LineSpacingHint::AtLeast(24.0)),
        ];
        let rendered = render_body(
            &model,
            Some(SourceWriteHints {
                gaps: &gaps,
                layouts: &layouts,
                separators: &separators,
                rtl: &rtl,
                final_gap: None,
                final_layout: None,
                final_separator: false,
                final_rtl: false,
                running_surface_distances: &distances,
                running_line_spacing: &[],
                running_pagination: &[],
                running_tab_stops: &[],
                running_table_cell_tab_stops: &[],
                running_table_layout: &[],
                running_column_break_offsets: &[],
                note_payloads: &[],
                paragraph_line_spacing: &line_spacing,
                paragraph_pagination: &[],
                paragraph_tab_stops: &[],
                column_break_offsets: &[],
                table_cell_column_break_offsets: &[],
                table_row_pagination: &[],
                table_cell_pagination: &[],
                table_cell_line_spacing: &[],
                table_nested_pagination: &[],
                table_cell_tab_stops: &[],
            }),
        );
        let document_xml = String::from_utf8(rendered.document_xml).unwrap();

        assert!(!document_xml.contains("w:lineRule="), "{document_xml}");
    }

    #[test]
    fn source_paragraph_spacing_writer_bounds_absolute_values() {
        assert_eq!(
            source_line_spacing(Some(LineSpacingHint::Exact(1_584.0))),
            Some((31_680, "exact"))
        );
        assert_eq!(
            source_line_spacing(Some(LineSpacingHint::AtLeast(12.0))),
            Some((240, "atLeast"))
        );
        for hint in [
            LineSpacingHint::Exact(f32::NAN),
            LineSpacingHint::Exact(-1.0),
            LineSpacingHint::AtLeast(0.0),
            LineSpacingHint::AtLeast(1_584.1),
        ] {
            assert_eq!(source_line_spacing(Some(hint)), None);
        }
    }

    #[test]
    fn source_tab_stop_writer_maps_and_bounds_private_values() {
        let valid = [
            tab(1.0, TabAlignment::Left, TabLeader::Dot),
            tab(2.0, TabAlignment::Center, TabLeader::Hyphen),
            tab(3.0, TabAlignment::Right, TabLeader::Underscore),
            tab(4.0, TabAlignment::Decimal, TabLeader::Heavy),
            tab(5.0, TabAlignment::Bar, TabLeader::MiddleDot),
        ];
        assert_eq!(
            source_tab_stops_xml(Some(&valid)).as_deref(),
            Some(concat!(
                r#"<w:tabs><w:tab w:val="left" w:pos="20" w:leader="dot"/>"#,
                r#"<w:tab w:val="center" w:pos="40" w:leader="hyphen"/>"#,
                r#"<w:tab w:val="right" w:pos="60" w:leader="underscore"/>"#,
                r#"<w:tab w:val="decimal" w:pos="80" w:leader="heavy"/>"#,
                r#"<w:tab w:val="bar" w:pos="100" w:leader="middleDot"/></w:tabs>"#,
            ))
        );
        assert_eq!(source_tab_stops_xml(None), None);
        assert_eq!(source_tab_stops_xml(Some(&[])), None);

        for invalid in [
            vec![tab(f32::NAN, TabAlignment::Left, TabLeader::None)],
            vec![tab(-1.0, TabAlignment::Left, TabLeader::None)],
            vec![tab(1_584.1, TabAlignment::Left, TabLeader::None)],
            vec![tab(1.0, TabAlignment::Clear, TabLeader::None)],
            vec![
                tab(2.0, TabAlignment::Left, TabLeader::None),
                tab(1.0, TabAlignment::Right, TabLeader::None),
            ],
            vec![
                tab(1.0, TabAlignment::Left, TabLeader::None),
                tab(1.0, TabAlignment::Right, TabLeader::None),
            ],
        ] {
            assert_eq!(source_tab_stops_xml(Some(&invalid)), None, "{invalid:?}");
        }
        let too_many = vec![tab(1.0, TabAlignment::Left, TabLeader::None); MAX_TAB_STOPS + 1];
        assert_eq!(source_tab_stops_xml(Some(&too_many)), None);

        #[cfg(feature = "render")]
        assert_eq!(
            source_tab_stops_xml(Some(&[tab(1.0, TabAlignment::Left, TabLeader::Bar,)])),
            None
        );
    }

    #[test]
    fn source_paragraph_tab_writer_rejects_misalignment_independently() {
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("body"))],
            ..DocModel::default()
        };
        let line_spacing = [Some(LineSpacingHint::Exact(12.0))];
        let pagination = [PaginationHint {
            keep_next: true,
            widow_control: true,
            ..PaginationHint::default()
        }];
        let render = |paragraph_tab_stops: &[Vec<TabStop>]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &[None],
                        layouts: &[None],
                        separators: &[false],
                        rtl: &[false],
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &line_spacing,
                        paragraph_pagination: &pagination,
                        paragraph_tab_stops,
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &[],
                        table_row_pagination: &[],
                        table_cell_pagination: &[],
                        table_cell_line_spacing: &[],
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };
        let assert_only_tabs_rejected = |xml: &str| {
            assert!(xml.contains("<w:keepNext/>"), "{xml}");
            assert!(xml.contains(r#"w:line="240" w:lineRule="exact""#), "{xml}");
            assert!(!xml.contains("<w:tabs>"), "{xml}");
        };

        assert_only_tabs_rejected(&render(&[
            vec![tab(36.0, TabAlignment::Left, TabLeader::None)],
            Vec::new(),
        ]));
        assert_only_tabs_rejected(&render(&[vec![tab(
            -1.0,
            TabAlignment::Left,
            TabLeader::None,
        )]]));

        let aligned = render(&[vec![tab(36.0, TabAlignment::Left, TabLeader::None)]]);
        assert!(
            aligned.contains(r#"<w:tabs><w:tab w:val="left" w:pos="720"/></w:tabs>"#),
            "{aligned}"
        );
        assert!(aligned.contains("<w:keepNext/>"), "{aligned}");
        assert!(
            aligned.contains(r#"w:line="240" w:lineRule="exact""#),
            "{aligned}"
        );
    }

    #[test]
    fn source_running_line_spacing_writer_aligns_sections_and_isolates_variants() {
        let running_blocks = |label: &str| vec![Block::PageBreak, Block::Paragraph(para(label))];
        let mut linked_header = para("DEFAULT HEADER");
        linked_header.runs[0].field = FieldRole::Hyperlink {
            url: "https://example.com/running-spacing".to_string(),
        };
        let header_table = Table {
            rows: vec![Row {
                cells: vec![cell("DEFAULT HEADER TABLE")],
            }],
            ..Table::default()
        };
        let mut first_section = SectionSetup {
            header: running_blocks("SECTION ZERO HEADER"),
            ..SectionSetup::default()
        };
        first_section.section_break = Some(crate::model::SectionBreakKind::NextPage);
        let model = DocModel {
            blocks: vec![
                Block::Paragraph(para("body one")),
                Block::SectionBreak(first_section),
                Block::Paragraph(para("body two")),
            ],
            setup: DocSetup {
                header: vec![
                    Block::PageBreak,
                    Block::Table(header_table),
                    Block::Paragraph(linked_header),
                ],
                first_header: running_blocks("FIRST HEADER"),
                even_header: running_blocks("EVEN HEADER"),
                footer: running_blocks("DEFAULT FOOTER"),
                first_footer: running_blocks("FIRST FOOTER"),
                even_footer: running_blocks("EVEN FOOTER"),
                page_numbers: true,
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let running_line_spacing = [
            RunningSurfaceLineSpacingHints {
                header: vec![None, Some(LineSpacingHint::Exact(7.0))],
                ..RunningSurfaceLineSpacingHints::default()
            },
            RunningSurfaceLineSpacingHints {
                header: vec![None, None, Some(LineSpacingHint::Exact(10.0))],
                first_header: vec![None, Some(LineSpacingHint::AtLeast(20.0))],
                even_header: vec![None, Some(LineSpacingHint::Exact(30.0))],
                footer: vec![None, Some(LineSpacingHint::AtLeast(40.0))],
                first_footer: vec![None, Some(LineSpacingHint::Exact(50.0))],
                even_footer: vec![None, Some(LineSpacingHint::AtLeast(60.0))],
                ..RunningSurfaceLineSpacingHints::default()
            },
        ];
        let running_tabs = [
            RunningSurfaceTabStopHints::default(),
            RunningSurfaceTabStopHints {
                header: vec![
                    Vec::new(),
                    Vec::new(),
                    vec![tab(70.0, TabAlignment::Right, TabLeader::Dot)],
                ],
                ..RunningSurfaceTabStopHints::default()
            },
        ];
        let running_table_tabs = [
            RunningSurfaceTableCellTabStopHints::default(),
            RunningSurfaceTableCellTabStopHints {
                header: vec![
                    Vec::new(),
                    vec![vec![vec![vec![tab(
                        80.0,
                        TabAlignment::Decimal,
                        TabLeader::Hyphen,
                    )]]]],
                    Vec::new(),
                ],
                ..RunningSurfaceTableCellTabStopHints::default()
            },
        ];
        let distances = [
            RunningSurfaceDistanceHints {
                header_pt: Some(11.0),
                footer_pt: Some(12.0),
            },
            RunningSurfaceDistanceHints {
                header_pt: Some(21.0),
                footer_pt: Some(22.0),
            },
        ];
        let render = |running_spacing: &[RunningSurfaceLineSpacingHints]| {
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[],
                    layouts: &[],
                    separators: &[],
                    rtl: &[],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &distances,
                    running_line_spacing: running_spacing,
                    running_pagination: &[],
                    running_tab_stops: &running_tabs,
                    running_table_cell_tab_stops: &running_table_tabs,
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
        };

        let rendered = render(&running_line_spacing);
        let document_xml = std::str::from_utf8(&rendered.document_xml).unwrap();
        assert!(document_xml.contains(r#"w:header="220" w:footer="240""#));
        assert!(document_xml.contains(r#"w:header="420" w:footer="440""#));
        for (label, expected) in [
            ("SECTION ZERO HEADER", r#"w:line="140" w:lineRule="exact""#),
            ("DEFAULT HEADER", r#"w:line="200" w:lineRule="exact""#),
            ("FIRST HEADER", r#"w:line="400" w:lineRule="atLeast""#),
            ("EVEN HEADER", r#"w:line="600" w:lineRule="exact""#),
            ("DEFAULT FOOTER", r#"w:line="800" w:lineRule="atLeast""#),
            ("FIRST FOOTER", r#"w:line="1000" w:lineRule="exact""#),
            ("EVEN FOOTER", r#"w:line="1200" w:lineRule="atLeast""#),
        ] {
            let (_, xml) = generated_running_part(&rendered, label);
            assert_eq!(xml.matches("w:lineRule=").count(), 1, "{label}: {xml}");
            assert!(xml.contains(expected), "{label}: {xml}");
        }
        let (default_header_path, default_header) =
            generated_running_part(&rendered, "DEFAULT HEADER");
        assert_eq!(default_header.matches("<w:tabs>").count(), 2);
        assert!(default_header.contains(r#"w:pos="1400" w:leader="dot""#));
        assert!(default_header.contains(r#"w:pos="1600" w:leader="hyphen""#));
        let default_header_rels = format!(
            "word/_rels/{}.rels",
            default_header_path.strip_prefix("word/").unwrap()
        );
        assert!(rendered.hf_rels.iter().any(|(path, rels)| {
            path == &default_header_rels
                && rels
                    .iter()
                    .any(|rel| rel.external && rel.target == "https://example.com/running-spacing")
        }));
        let (_, default_footer) = generated_running_part(&rendered, "DEFAULT FOOTER");
        assert!(default_footer.contains(r#"<w:fldSimple w:instr=" PAGE ">"#));

        let section_misaligned = render(&running_line_spacing[..1]);
        assert!(section_misaligned
            .hf_parts
            .iter()
            .all(|(_, _, bytes)| { !std::str::from_utf8(bytes).unwrap().contains("w:lineRule=") }));
        let (_, surviving_tabs) = generated_running_part(&section_misaligned, "DEFAULT HEADER");
        assert_eq!(surviving_tabs.matches("<w:tabs>").count(), 2);

        let mut variant_misaligned = running_line_spacing.clone();
        variant_misaligned[1].even_footer.pop();
        let isolated = render(&variant_misaligned);
        for label in [
            "SECTION ZERO HEADER",
            "DEFAULT HEADER",
            "FIRST HEADER",
            "EVEN HEADER",
            "DEFAULT FOOTER",
            "FIRST FOOTER",
        ] {
            let (_, xml) = generated_running_part(&isolated, label);
            assert!(xml.contains("w:lineRule="), "{label}: {xml}");
        }
        let (_, even_footer) = generated_running_part(&isolated, "EVEN FOOTER");
        assert!(!even_footer.contains("w:lineRule="), "{even_footer}");

        let mut invalid_value = running_line_spacing.clone();
        invalid_value[1].first_footer[1] = Some(LineSpacingHint::Exact(f32::NAN));
        let bounded = render(&invalid_value);
        let (_, first_footer) = generated_running_part(&bounded, "FIRST FOOTER");
        assert!(!first_footer.contains("w:lineRule="), "{first_footer}");
        let (_, bounded_default) = generated_running_part(&bounded, "DEFAULT HEADER");
        assert!(bounded_default.contains(r#"w:line="200" w:lineRule="exact""#));
    }

    #[test]
    fn source_running_pagination_writer_rejects_misaligned_aggregate() {
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("BODY"))],
            setup: DocSetup {
                header: vec![
                    Block::Paragraph(para("RUNNING DIRECT")),
                    Block::Table(Table {
                        rows: vec![Row {
                            cells: vec![cell("RUNNING CELL")],
                        }],
                        ..Table::default()
                    }),
                ],
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let enabled = PaginationHint {
            keep_next: true,
            keep_lines: true,
            widow_control: false,
        };
        let neutral = PaginationHint {
            widow_control: true,
            ..PaginationHint::default()
        };
        let running_pagination = [RunningSurfacePaginationHints {
            header: RunningBlockPaginationHints {
                paragraphs: vec![enabled, neutral],
                table_rows: vec![
                    Vec::new(),
                    vec![TableRowPaginationHint { cant_split: true }],
                ],
                table_cells: vec![
                    Vec::new(),
                    vec![vec![vec![Some(PaginationHint {
                        keep_lines: true,
                        widow_control: false,
                        ..PaginationHint::default()
                    })]]],
                ],
            },
            ..RunningSurfacePaginationHints::default()
        }];
        let render = |hints: &[RunningSurfacePaginationHints]| {
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[],
                    layouts: &[],
                    separators: &[],
                    rtl: &[],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: hints,
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
        };

        let aligned = render(&running_pagination);
        let (_, xml) = generated_running_part(&aligned, "RUNNING DIRECT");
        let direct = written_paragraph_with_text(xml, "RUNNING DIRECT");
        assert!(direct.contains("<w:keepNext/>"), "{direct}");
        assert!(direct.contains("<w:keepLines/>"), "{direct}");
        assert!(
            direct.contains(r#"<w:widowControl w:val="0"/>"#),
            "{direct}"
        );
        assert!(xml.contains("<w:cantSplit/>"), "{xml}");
        let cell = written_paragraph_with_text(xml, "RUNNING CELL");
        assert!(!cell.contains("<w:keepNext/>"), "{cell}");
        assert!(cell.contains("<w:keepLines/>"), "{cell}");
        assert!(cell.contains(r#"<w:widowControl w:val="0"/>"#), "{cell}");

        let mut block_misaligned = running_pagination.clone();
        block_misaligned[0].header.table_cells.pop();
        let mut table_misaligned = running_pagination.clone();
        table_misaligned[0].header.table_rows[1].clear();
        for rejected in [
            render(&block_misaligned),
            render(&table_misaligned),
            render(&[]),
        ] {
            let (_, xml) = generated_running_part(&rejected, "RUNNING DIRECT");
            assert!(!xml.contains("<w:keepNext"), "{xml}");
            assert!(!xml.contains("<w:keepLines"), "{xml}");
            assert!(!xml.contains("<w:widowControl"), "{xml}");
            assert!(!xml.contains("<w:cantSplit"), "{xml}");
        }
    }

    #[test]
    fn source_running_tab_writer_aligns_sections_and_isolates_variants() {
        let running_blocks = |label: &str, hyperlink: bool| {
            let mut paragraph = para(label);
            if hyperlink {
                paragraph.runs[0].field = FieldRole::Hyperlink {
                    url: "https://example.com/running-tabs".to_string(),
                };
            }
            vec![Block::PageBreak, Block::Paragraph(paragraph)]
        };
        let mut first_section = SectionSetup {
            header: vec![Block::Paragraph(para("FIRST SECTION HEADER"))],
            ..SectionSetup::default()
        };
        first_section.section_break = Some(crate::model::SectionBreakKind::NextPage);
        let model = DocModel {
            blocks: vec![
                Block::Paragraph(para("body one")),
                Block::SectionBreak(first_section),
                Block::Paragraph(para("body two")),
            ],
            setup: DocSetup {
                header: running_blocks("DEFAULT HEADER", true),
                first_header: running_blocks("FIRST HEADER", false),
                even_header: running_blocks("EVEN HEADER", false),
                footer: running_blocks("DEFAULT FOOTER", false),
                first_footer: running_blocks("FIRST FOOTER", false),
                even_footer: running_blocks("EVEN FOOTER", false),
                page_numbers: true,
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let block_tabs = |position_pt, alignment, leader| {
            vec![
                vec![tab(1.0, TabAlignment::Right, TabLeader::Dot)],
                vec![tab(position_pt, alignment, leader)],
            ]
        };
        let running_tab_stops = [
            RunningSurfaceTabStopHints {
                header: vec![vec![tab(70.0, TabAlignment::Right, TabLeader::Underscore)]],
                ..RunningSurfaceTabStopHints::default()
            },
            RunningSurfaceTabStopHints {
                header: block_tabs(10.0, TabAlignment::Left, TabLeader::None),
                first_header: block_tabs(20.0, TabAlignment::Center, TabLeader::Dot),
                even_header: block_tabs(30.0, TabAlignment::Right, TabLeader::Hyphen),
                footer: block_tabs(40.0, TabAlignment::Decimal, TabLeader::Underscore),
                first_footer: block_tabs(50.0, TabAlignment::Bar, TabLeader::Heavy),
                even_footer: block_tabs(60.0, TabAlignment::Left, TabLeader::MiddleDot),
            },
        ];
        let distances = [
            RunningSurfaceDistanceHints {
                header_pt: Some(11.0),
                footer_pt: Some(12.0),
            },
            RunningSurfaceDistanceHints {
                header_pt: Some(21.0),
                footer_pt: Some(22.0),
            },
        ];
        let render = |running_tabs: &[RunningSurfaceTabStopHints]| {
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[],
                    layouts: &[],
                    separators: &[],
                    rtl: &[],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &distances,
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: running_tabs,
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
        };

        let rendered = render(&running_tab_stops);
        let document_xml = std::str::from_utf8(&rendered.document_xml).unwrap();
        assert!(
            document_xml.contains(r#"w:header="220" w:footer="240""#),
            "{document_xml}"
        );
        assert!(
            document_xml.contains(r#"w:header="420" w:footer="440""#),
            "{document_xml}"
        );
        for (label, expected) in [
            (
                "FIRST SECTION HEADER",
                r#"<w:tab w:val="right" w:pos="1400" w:leader="underscore"/>"#,
            ),
            ("DEFAULT HEADER", r#"<w:tab w:val="left" w:pos="200"/>"#),
            (
                "FIRST HEADER",
                r#"<w:tab w:val="center" w:pos="400" w:leader="dot"/>"#,
            ),
            (
                "EVEN HEADER",
                r#"<w:tab w:val="right" w:pos="600" w:leader="hyphen"/>"#,
            ),
            (
                "DEFAULT FOOTER",
                r#"<w:tab w:val="decimal" w:pos="800" w:leader="underscore"/>"#,
            ),
            (
                "FIRST FOOTER",
                r#"<w:tab w:val="bar" w:pos="1000" w:leader="heavy"/>"#,
            ),
            (
                "EVEN FOOTER",
                r#"<w:tab w:val="left" w:pos="1200" w:leader="middleDot"/>"#,
            ),
        ] {
            let (_, xml) = generated_running_part(&rendered, label);
            assert_eq!(xml.matches("<w:tabs>").count(), 1, "{label}: {xml}");
            assert!(xml.contains(expected), "{label}: {xml}");
        }
        let (default_header_path, _) = generated_running_part(&rendered, "DEFAULT HEADER");
        let default_header_rels = format!(
            "word/_rels/{}.rels",
            default_header_path.strip_prefix("word/").unwrap()
        );
        assert!(rendered.hf_rels.iter().any(|(path, rels)| {
            path == &default_header_rels
                && rels
                    .iter()
                    .any(|rel| rel.external && rel.target == "https://example.com/running-tabs")
        }));
        let (_, default_footer) = generated_running_part(&rendered, "DEFAULT FOOTER");
        assert_eq!(default_footer.matches("<w:tabs>").count(), 1);
        assert!(default_footer.contains(r#"<w:fldSimple w:instr=" PAGE ">"#));

        let section_misaligned = render(&running_tab_stops[..1]);
        assert!(section_misaligned
            .hf_parts
            .iter()
            .all(|(_, _, bytes)| { !std::str::from_utf8(bytes).unwrap().contains("<w:tabs>") }));
        let misaligned_document = std::str::from_utf8(&section_misaligned.document_xml).unwrap();
        assert!(misaligned_document.contains(r#"w:header="420" w:footer="440""#));
        let (_, misaligned_footer) = generated_running_part(&section_misaligned, "DEFAULT FOOTER");
        assert!(misaligned_footer.contains(r#"<w:fldSimple w:instr=" PAGE ">"#));

        let mut variant_misaligned = running_tab_stops.clone();
        variant_misaligned[1].even_footer.pop();
        let isolated = render(&variant_misaligned);
        for label in [
            "FIRST SECTION HEADER",
            "DEFAULT HEADER",
            "FIRST HEADER",
            "EVEN HEADER",
            "DEFAULT FOOTER",
            "FIRST FOOTER",
        ] {
            let (_, xml) = generated_running_part(&isolated, label);
            assert!(xml.contains("<w:tabs>"), "{label}: {xml}");
        }
        let (_, even_footer) = generated_running_part(&isolated, "EVEN FOOTER");
        assert!(!even_footer.contains("<w:tabs>"), "{even_footer}");
        let (isolated_header_path, _) = generated_running_part(&isolated, "DEFAULT HEADER");
        let isolated_header_rels = format!(
            "word/_rels/{}.rels",
            isolated_header_path.strip_prefix("word/").unwrap()
        );
        assert!(isolated.hf_rels.iter().any(|(path, rels)| {
            path == &isolated_header_rels && rels.iter().any(|rel| rel.external)
        }));
    }

    #[test]
    fn source_running_table_cell_writer_aligns_sections_and_isolates_tables() {
        let simple_table = |label: &str, hyperlink: bool| {
            let mut paragraph = para(label);
            if hyperlink {
                paragraph.runs[0].field = FieldRole::Hyperlink {
                    url: "https://example.com/running-table-tabs".to_string(),
                };
            }
            Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(paragraph)],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            }
        };
        let vertical_table = Table {
            rows: vec![
                Row {
                    cells: vec![
                        Cell {
                            blocks: vec![Block::Paragraph(para("DEFAULT HEADER OWNER"))],
                            row_span: 2,
                            ..Cell::default()
                        },
                        cell("DEFAULT HEADER PEER"),
                    ],
                },
                Row {
                    cells: vec![cell("DEFAULT HEADER BOTTOM")],
                },
            ],
            ..Table::default()
        };
        let running_blocks = |prefix: &str, cell_label: &str| {
            vec![
                Block::Paragraph(para(prefix)),
                Block::Table(simple_table(cell_label, false)),
            ]
        };
        let mut first_section = SectionSetup {
            header: running_blocks("SECTION ZERO PREFIX", "SECTION ZERO CELL"),
            ..SectionSetup::default()
        };
        first_section.section_break = Some(crate::model::SectionBreakKind::NextPage);
        let model = DocModel {
            blocks: vec![
                Block::Paragraph(para("body one")),
                Block::SectionBreak(first_section),
                Block::Paragraph(para("body two")),
            ],
            setup: DocSetup {
                header: vec![
                    Block::Paragraph(para("DEFAULT HEADER PREFIX")),
                    Block::Table(vertical_table),
                    Block::Table(simple_table("DEFAULT HEADER SECOND", true)),
                ],
                first_header: running_blocks("FIRST HEADER PREFIX", "FIRST HEADER CELL"),
                even_header: running_blocks("EVEN HEADER PREFIX", "EVEN HEADER CELL"),
                footer: running_blocks("DEFAULT FOOTER PREFIX", "DEFAULT FOOTER CELL"),
                first_footer: running_blocks("FIRST FOOTER PREFIX", "FIRST FOOTER CELL"),
                even_footer: running_blocks("EVEN FOOTER PREFIX", "EVEN FOOTER CELL"),
                page_numbers: true,
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let one_cell_tabs = |stop| vec![vec![vec![vec![stop]]]];
        let section_zero_tabs = RunningSurfaceTableCellTabStopHints {
            header: vec![
                Vec::new(),
                one_cell_tabs(tab(10.0, TabAlignment::Left, TabLeader::None)),
            ],
            ..RunningSurfaceTableCellTabStopHints::default()
        };
        let final_tabs = RunningSurfaceTableCellTabStopHints {
            header: vec![
                Vec::new(),
                vec![
                    vec![
                        vec![vec![tab(11.0, TabAlignment::Left, TabLeader::None)]],
                        vec![vec![tab(12.0, TabAlignment::Center, TabLeader::Dot)]],
                    ],
                    vec![vec![vec![tab(
                        13.0,
                        TabAlignment::Right,
                        TabLeader::Hyphen,
                    )]]],
                ],
                one_cell_tabs(tab(14.0, TabAlignment::Decimal, TabLeader::Underscore)),
            ],
            first_header: vec![
                Vec::new(),
                one_cell_tabs(tab(20.0, TabAlignment::Bar, TabLeader::Heavy)),
            ],
            even_header: vec![
                Vec::new(),
                one_cell_tabs(tab(30.0, TabAlignment::Left, TabLeader::MiddleDot)),
            ],
            footer: vec![
                Vec::new(),
                one_cell_tabs(tab(40.0, TabAlignment::Center, TabLeader::Dot)),
            ],
            first_footer: vec![
                Vec::new(),
                one_cell_tabs(tab(50.0, TabAlignment::Right, TabLeader::Hyphen)),
            ],
            even_footer: vec![
                Vec::new(),
                one_cell_tabs(tab(60.0, TabAlignment::Decimal, TabLeader::Underscore)),
            ],
        };
        let running_table_tabs = [section_zero_tabs, final_tabs];
        let one_cell_spacing = |spacing| vec![vec![vec![Some(spacing)]]];
        let prefix_spacing = |block_count: usize| {
            let mut blocks = vec![None; block_count];
            blocks[0] = Some(LineSpacingHint::Exact(7.0));
            blocks
        };
        let running_line_spacing = [
            RunningSurfaceLineSpacingHints {
                header: prefix_spacing(2),
                header_table_cells: vec![
                    Vec::new(),
                    one_cell_spacing(LineSpacingHint::Exact(10.0)),
                ],
                ..RunningSurfaceLineSpacingHints::default()
            },
            RunningSurfaceLineSpacingHints {
                header: prefix_spacing(3),
                header_table_cells: vec![
                    Vec::new(),
                    vec![
                        vec![
                            vec![Some(LineSpacingHint::Exact(11.0))],
                            vec![Some(LineSpacingHint::AtLeast(12.0))],
                        ],
                        vec![vec![Some(LineSpacingHint::Exact(13.0))]],
                    ],
                    one_cell_spacing(LineSpacingHint::AtLeast(14.0)),
                ],
                first_header: prefix_spacing(2),
                first_header_table_cells: vec![
                    Vec::new(),
                    one_cell_spacing(LineSpacingHint::Exact(20.0)),
                ],
                even_header: prefix_spacing(2),
                even_header_table_cells: vec![
                    Vec::new(),
                    one_cell_spacing(LineSpacingHint::AtLeast(30.0)),
                ],
                footer: prefix_spacing(2),
                footer_table_cells: vec![
                    Vec::new(),
                    one_cell_spacing(LineSpacingHint::Exact(40.0)),
                ],
                first_footer: prefix_spacing(2),
                first_footer_table_cells: vec![
                    Vec::new(),
                    one_cell_spacing(LineSpacingHint::AtLeast(50.0)),
                ],
                even_footer: prefix_spacing(2),
                even_footer_table_cells: vec![
                    Vec::new(),
                    one_cell_spacing(LineSpacingHint::Exact(60.0)),
                ],
            },
        ];
        let prefix_tabs = |block_count: usize| {
            let mut blocks = vec![Vec::new(); block_count];
            blocks[0] = vec![tab(72.0, TabAlignment::Right, TabLeader::Dot)];
            blocks
        };
        let running_tabs = [
            RunningSurfaceTabStopHints {
                header: prefix_tabs(2),
                ..RunningSurfaceTabStopHints::default()
            },
            RunningSurfaceTabStopHints {
                header: prefix_tabs(3),
                first_header: prefix_tabs(2),
                even_header: prefix_tabs(2),
                footer: prefix_tabs(2),
                first_footer: prefix_tabs(2),
                even_footer: prefix_tabs(2),
            },
        ];
        let distances = [
            RunningSurfaceDistanceHints {
                header_pt: Some(11.0),
                footer_pt: Some(12.0),
            },
            RunningSurfaceDistanceHints {
                header_pt: Some(21.0),
                footer_pt: Some(22.0),
            },
        ];
        let render = |table_tabs: &[RunningSurfaceTableCellTabStopHints],
                      running_spacing: &[RunningSurfaceLineSpacingHints]| {
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[],
                    layouts: &[],
                    separators: &[],
                    rtl: &[],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &distances,
                    running_line_spacing: running_spacing,
                    running_pagination: &[],
                    running_tab_stops: &running_tabs,
                    running_table_cell_tab_stops: table_tabs,
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
        };

        let rendered = render(&running_table_tabs, &running_line_spacing);
        let document_xml = std::str::from_utf8(&rendered.document_xml).unwrap();
        assert!(document_xml.contains(r#"w:header="220" w:footer="240""#));
        assert!(document_xml.contains(r#"w:header="420" w:footer="440""#));
        for (label, expected) in [
            ("SECTION ZERO CELL", r#"w:val="left" w:pos="200""#),
            ("DEFAULT HEADER OWNER", r#"w:val="left" w:pos="220""#),
            (
                "DEFAULT HEADER PEER",
                r#"w:val="center" w:pos="240" w:leader="dot""#,
            ),
            (
                "DEFAULT HEADER BOTTOM",
                r#"w:val="right" w:pos="260" w:leader="hyphen""#,
            ),
            (
                "DEFAULT HEADER SECOND",
                r#"w:val="decimal" w:pos="280" w:leader="underscore""#,
            ),
            (
                "FIRST HEADER CELL",
                r#"w:val="bar" w:pos="400" w:leader="heavy""#,
            ),
            (
                "EVEN HEADER CELL",
                r#"w:val="left" w:pos="600" w:leader="middleDot""#,
            ),
            (
                "DEFAULT FOOTER CELL",
                r#"w:val="center" w:pos="800" w:leader="dot""#,
            ),
            (
                "FIRST FOOTER CELL",
                r#"w:val="right" w:pos="1000" w:leader="hyphen""#,
            ),
            (
                "EVEN FOOTER CELL",
                r#"w:val="decimal" w:pos="1200" w:leader="underscore""#,
            ),
        ] {
            let (_, xml) = generated_running_part(&rendered, label);
            let paragraph = written_paragraph_with_text(xml, label);
            assert!(paragraph.contains(expected), "{label}: {paragraph}");
        }
        for (label, expected) in [
            ("SECTION ZERO CELL", r#"w:line="200" w:lineRule="exact""#),
            ("DEFAULT HEADER OWNER", r#"w:line="220" w:lineRule="exact""#),
            (
                "DEFAULT HEADER PEER",
                r#"w:line="240" w:lineRule="atLeast""#,
            ),
            (
                "DEFAULT HEADER BOTTOM",
                r#"w:line="260" w:lineRule="exact""#,
            ),
            (
                "DEFAULT HEADER SECOND",
                r#"w:line="280" w:lineRule="atLeast""#,
            ),
            ("FIRST HEADER CELL", r#"w:line="400" w:lineRule="exact""#),
            ("EVEN HEADER CELL", r#"w:line="600" w:lineRule="atLeast""#),
            ("DEFAULT FOOTER CELL", r#"w:line="800" w:lineRule="exact""#),
            ("FIRST FOOTER CELL", r#"w:line="1000" w:lineRule="atLeast""#),
            ("EVEN FOOTER CELL", r#"w:line="1200" w:lineRule="exact""#),
        ] {
            let (_, xml) = generated_running_part(&rendered, label);
            let paragraph = written_paragraph_with_text(xml, label);
            assert!(paragraph.contains(expected), "{label}: {paragraph}");
        }
        for prefix in [
            "SECTION ZERO PREFIX",
            "DEFAULT HEADER PREFIX",
            "FIRST HEADER PREFIX",
            "EVEN HEADER PREFIX",
            "DEFAULT FOOTER PREFIX",
            "FIRST FOOTER PREFIX",
            "EVEN FOOTER PREFIX",
        ] {
            let (_, xml) = generated_running_part(&rendered, prefix);
            let paragraph = written_paragraph_with_text(xml, prefix);
            assert!(
                paragraph.contains(r#"w:val="right" w:pos="1440" w:leader="dot""#),
                "{prefix}: {paragraph}"
            );
            assert!(
                paragraph.contains(r#"w:line="140" w:lineRule="exact""#),
                "{prefix}: {paragraph}"
            );
        }
        let (default_header_path, default_header) =
            generated_running_part(&rendered, "DEFAULT HEADER SECOND");
        assert_eq!(default_header.matches("<w:vMerge/>").count(), 1);
        let default_header_rels = format!(
            "word/_rels/{}.rels",
            default_header_path.strip_prefix("word/").unwrap()
        );
        assert!(rendered.hf_rels.iter().any(|(path, rels)| {
            path == &default_header_rels
                && rels.iter().any(|rel| {
                    rel.external && rel.target == "https://example.com/running-table-tabs"
                })
        }));
        let (_, default_footer) = generated_running_part(&rendered, "DEFAULT FOOTER CELL");
        assert!(default_footer.contains(r#"<w:fldSimple w:instr=" PAGE ">"#));

        let section_misaligned = render(&running_table_tabs[..1], &running_line_spacing);
        for label in [
            "SECTION ZERO CELL",
            "DEFAULT HEADER OWNER",
            "DEFAULT HEADER SECOND",
            "FIRST HEADER CELL",
            "EVEN HEADER CELL",
            "DEFAULT FOOTER CELL",
            "FIRST FOOTER CELL",
            "EVEN FOOTER CELL",
        ] {
            let (_, xml) = generated_running_part(&section_misaligned, label);
            assert!(
                !written_paragraph_with_text(xml, label).contains("<w:tabs>"),
                "{label}: {xml}"
            );
        }
        let (_, misaligned_header) =
            generated_running_part(&section_misaligned, "DEFAULT HEADER PREFIX");
        assert!(
            written_paragraph_with_text(misaligned_header, "DEFAULT HEADER PREFIX")
                .contains(r#"w:pos="1440""#)
        );
        let misaligned_document = std::str::from_utf8(&section_misaligned.document_xml).unwrap();
        assert!(misaligned_document.contains(r#"w:header="420" w:footer="440""#));
        let (_, misaligned_footer) =
            generated_running_part(&section_misaligned, "DEFAULT FOOTER CELL");
        assert!(misaligned_footer.contains(r#"<w:fldSimple w:instr=" PAGE ">"#));
        assert!(
            written_paragraph_with_text(misaligned_footer, "DEFAULT FOOTER CELL")
                .contains(r#"w:line="800" w:lineRule="exact""#)
        );

        let spacing_section_misaligned = render(&running_table_tabs, &running_line_spacing[..1]);
        for label in [
            "SECTION ZERO CELL",
            "DEFAULT HEADER OWNER",
            "DEFAULT HEADER SECOND",
            "FIRST HEADER CELL",
            "EVEN HEADER CELL",
            "DEFAULT FOOTER CELL",
            "FIRST FOOTER CELL",
            "EVEN FOOTER CELL",
        ] {
            let (_, xml) = generated_running_part(&spacing_section_misaligned, label);
            let paragraph = written_paragraph_with_text(xml, label);
            assert!(!paragraph.contains("w:lineRule="), "{label}: {paragraph}");
            assert!(paragraph.contains("<w:tabs>"), "{label}: {paragraph}");
        }
        let (_, spacing_misaligned_prefix) =
            generated_running_part(&spacing_section_misaligned, "DEFAULT HEADER PREFIX");
        assert!(
            !written_paragraph_with_text(spacing_misaligned_prefix, "DEFAULT HEADER PREFIX")
                .contains("w:lineRule=")
        );

        let mut variant_misaligned = running_table_tabs.clone();
        variant_misaligned[1].even_footer.pop();
        let isolated_variant = render(&variant_misaligned, &running_line_spacing);
        let (_, even_footer) = generated_running_part(&isolated_variant, "EVEN FOOTER CELL");
        assert!(!written_paragraph_with_text(even_footer, "EVEN FOOTER CELL").contains("<w:tabs>"));
        let (_, first_footer) = generated_running_part(&isolated_variant, "FIRST FOOTER CELL");
        assert!(
            written_paragraph_with_text(first_footer, "FIRST FOOTER CELL")
                .contains(r#"w:pos="1000""#)
        );
        assert!(written_paragraph_with_text(even_footer, "EVEN FOOTER CELL")
            .contains(r#"w:line="1200" w:lineRule="exact""#));
        let (_, even_prefix) = generated_running_part(&isolated_variant, "EVEN FOOTER PREFIX");
        assert!(
            written_paragraph_with_text(even_prefix, "EVEN FOOTER PREFIX")
                .contains(r#"w:pos="1440""#)
        );

        let mut malformed_table = running_table_tabs.clone();
        malformed_table[1].header[1].pop();
        let isolated_table = render(&malformed_table, &running_line_spacing);
        let (_, header) = generated_running_part(&isolated_table, "DEFAULT HEADER SECOND");
        for label in [
            "DEFAULT HEADER OWNER",
            "DEFAULT HEADER PEER",
            "DEFAULT HEADER BOTTOM",
        ] {
            assert!(
                !written_paragraph_with_text(header, label).contains("<w:tabs>"),
                "{label}: {header}"
            );
        }
        assert!(
            written_paragraph_with_text(header, "DEFAULT HEADER SECOND").contains(r#"w:pos="280""#)
        );
        assert!(written_paragraph_with_text(header, "DEFAULT HEADER PREFIX")
            .contains(r#"w:pos="1440""#));

        let mut spacing_variant_misaligned = running_line_spacing.clone();
        spacing_variant_misaligned[1].even_footer_table_cells.pop();
        let isolated_spacing_variant = render(&running_table_tabs, &spacing_variant_misaligned);
        let (_, even_footer) =
            generated_running_part(&isolated_spacing_variant, "EVEN FOOTER CELL");
        let even_footer_cell = written_paragraph_with_text(even_footer, "EVEN FOOTER CELL");
        assert!(
            !even_footer_cell.contains("w:lineRule="),
            "{even_footer_cell}"
        );
        assert!(even_footer_cell.contains(r#"w:pos="1200""#));
        assert!(
            written_paragraph_with_text(even_footer, "EVEN FOOTER PREFIX")
                .contains(r#"w:line="140" w:lineRule="exact""#)
        );
        let (_, first_footer) =
            generated_running_part(&isolated_spacing_variant, "FIRST FOOTER CELL");
        assert!(
            written_paragraph_with_text(first_footer, "FIRST FOOTER CELL")
                .contains(r#"w:line="1000" w:lineRule="atLeast""#)
        );

        let mut malformed_spacing_table = running_line_spacing.clone();
        malformed_spacing_table[1].header_table_cells[1].pop();
        let isolated_spacing_table = render(&running_table_tabs, &malformed_spacing_table);
        let (_, header) = generated_running_part(&isolated_spacing_table, "DEFAULT HEADER SECOND");
        for label in [
            "DEFAULT HEADER OWNER",
            "DEFAULT HEADER PEER",
            "DEFAULT HEADER BOTTOM",
        ] {
            let paragraph = written_paragraph_with_text(header, label);
            assert!(!paragraph.contains("w:lineRule="), "{label}: {paragraph}");
            assert!(paragraph.contains("<w:tabs>"), "{label}: {paragraph}");
        }
        assert!(written_paragraph_with_text(header, "DEFAULT HEADER SECOND")
            .contains(r#"w:line="280" w:lineRule="atLeast""#));
        assert!(written_paragraph_with_text(header, "DEFAULT HEADER PREFIX")
            .contains(r#"w:line="140" w:lineRule="exact""#));

        let mut invalid_spacing = running_line_spacing.clone();
        invalid_spacing[1].first_header_table_cells[1][0][0][0] =
            Some(LineSpacingHint::Exact(f32::NAN));
        let bounded = render(&running_table_tabs, &invalid_spacing);
        let (_, first_header) = generated_running_part(&bounded, "FIRST HEADER CELL");
        let first_header_cell = written_paragraph_with_text(first_header, "FIRST HEADER CELL");
        assert!(
            !first_header_cell.contains("w:lineRule="),
            "{first_header_cell}"
        );
        assert!(first_header_cell.contains(r#"w:pos="400""#));
        let (_, even_header) = generated_running_part(&bounded, "EVEN HEADER CELL");
        assert!(written_paragraph_with_text(even_header, "EVEN HEADER CELL")
            .contains(r#"w:line="600" w:lineRule="atLeast""#));
    }

    #[test]
    fn source_note_payloads_validate_alignment_components_and_siblings() {
        let footnote_text = "FOOT\nBREAK\nSECOND";
        let endnote_text = "END";
        let model = DocModel {
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    Run {
                        text: "A".to_string(),
                        note: Some(AuthoredNote {
                            kind: NoteKind::Footnote,
                            text: footnote_text.to_string(),
                        }),
                        ..Run::default()
                    },
                    Run {
                        text: "B".to_string(),
                        note: Some(AuthoredNote {
                            kind: NoteKind::Endnote,
                            text: endnote_text.to_string(),
                        }),
                        ..Run::default()
                    },
                ],
                ..Paragraph::default()
            })],
            ..DocModel::default()
        };
        let mut footnote_first = para("FOOT\nBREAK");
        footnote_first.props.align = Align::Center;
        let valid = vec![vec![
            Some(NoteWritePayload {
                kind: NoteKind::Footnote,
                text: footnote_text.to_string(),
                blocks: vec![
                    Block::Paragraph(footnote_first),
                    Block::Paragraph(para("SECOND")),
                ],
                pagination: vec![
                    PaginationHint {
                        keep_next: true,
                        widow_control: false,
                        ..PaginationHint::default()
                    },
                    PaginationHint {
                        widow_control: true,
                        ..PaginationHint::default()
                    },
                ],
                line_spacing: vec![
                    Some(LineSpacingHint::Exact(12.0)),
                    Some(LineSpacingHint::AtLeast(15.0)),
                ],
                tab_stops: vec![
                    vec![tab(36.0, TabAlignment::Center, TabLeader::Dot)],
                    Vec::new(),
                ],
                column_break_offsets: vec![vec![4], Vec::new()],
                table_pagination: vec![None, None],
            }),
            Some(NoteWritePayload {
                kind: NoteKind::Endnote,
                text: endnote_text.to_string(),
                blocks: vec![Block::Paragraph(para(endnote_text))],
                pagination: vec![PaginationHint {
                    widow_control: true,
                    ..PaginationHint::default()
                }],
                line_spacing: vec![Some(LineSpacingHint::Exact(20.0))],
                tab_stops: vec![Vec::new()],
                column_break_offsets: vec![Vec::new()],
                table_pagination: vec![None],
            }),
        ]];
        let render = |note_payloads: &[Vec<Option<NoteWritePayload>>]| {
            let rendered = render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads,
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            );
            (
                String::from_utf8(rendered.footnotes_xml.expect("footnotes part")).unwrap(),
                String::from_utf8(rendered.endnotes_xml.expect("endnotes part")).unwrap(),
                rendered.footnote_rels,
                rendered.endnote_rels,
                rendered.media,
            )
        };
        let counts = |xml: &str, item: &str, text: &str| {
            let note = written_note_with_text(xml, item, text);
            (
                note.matches("<w:p>").count(),
                note.matches(r#"<w:br w:type="column"/>"#).count(),
                note.matches("<w:br/>").count(),
                note.matches("<w:keepNext/>").count(),
                note.matches("w:lineRule=").count(),
                note.matches("<w:tabs>").count(),
            )
        };
        let render_counts = |payloads: &[Vec<Option<NoteWritePayload>>]| {
            let (footnotes, endnotes, _, _, _) = render(payloads);
            (
                counts(&footnotes, "footnote", "FOOT"),
                counts(&endnotes, "endnote", "END"),
            )
        };

        assert_eq!(
            render_counts(&valid),
            ((2, 1, 0, 1, 2, 1), (1, 0, 0, 0, 1, 0))
        );
        assert_eq!(render_counts(&[]), ((1, 0, 2, 0, 0, 0), (1, 0, 0, 0, 0, 0)));

        let bad_outer = [valid[0].clone(), Vec::new()];
        assert_eq!(
            render_counts(&bad_outer),
            ((1, 0, 2, 0, 0, 0), (1, 0, 0, 0, 0, 0))
        );

        let mut bad_run_shape = valid.clone();
        bad_run_shape[0].pop();
        assert_eq!(
            render_counts(&bad_run_shape),
            ((1, 0, 2, 0, 0, 0), (1, 0, 0, 0, 0, 0))
        );

        let mut bad_identity = valid.clone();
        bad_identity[0][0].as_mut().unwrap().text = "WRONG".to_string();
        assert_eq!(
            render_counts(&bad_identity),
            ((1, 0, 2, 0, 0, 0), (1, 0, 0, 0, 1, 0))
        );

        let mut bad_line_component = valid.clone();
        bad_line_component[0][0]
            .as_mut()
            .unwrap()
            .line_spacing
            .pop();
        assert_eq!(
            render_counts(&bad_line_component),
            ((2, 1, 0, 1, 0, 1), (1, 0, 0, 0, 1, 0))
        );

        let mut bad_break_leaf = valid.clone();
        bad_break_leaf[0][0].as_mut().unwrap().column_break_offsets[0] = vec![4, 4];
        assert_eq!(
            render_counts(&bad_break_leaf),
            ((2, 0, 1, 1, 2, 1), (1, 0, 0, 0, 1, 0))
        );

        let mut relationship_bearing = valid.clone();
        let Block::Paragraph(paragraph) =
            &mut relationship_bearing[0][0].as_mut().unwrap().blocks[0]
        else {
            panic!("paragraph payload")
        };
        paragraph.runs[0].field = FieldRole::Hyperlink {
            url: "https://example.com/note".to_string(),
        };
        let (footnotes, endnotes, footnote_rels, endnote_rels, _) = render(&relationship_bearing);
        assert_eq!(
            (
                counts(&footnotes, "footnote", "FOOT"),
                counts(&endnotes, "endnote", "END")
            ),
            ((2, 1, 0, 1, 2, 1), (1, 0, 0, 0, 1, 0))
        );
        assert!(footnotes.contains(r#"<w:hyperlink r:id="rId1">"#));
        assert_eq!(footnote_rels.len(), 1);
        assert_eq!(footnote_rels[0].rel_type, REL_HYPERLINK);
        assert_eq!(footnote_rels[0].target, "https://example.com/note");
        assert!(footnote_rels[0].external);
        assert!(endnote_rels.is_empty());

        let mut internal_anchor = relationship_bearing.clone();
        let Block::Paragraph(paragraph) = &mut internal_anchor[0][0].as_mut().unwrap().blocks[0]
        else {
            panic!("paragraph payload")
        };
        paragraph.runs[0].field = FieldRole::Hyperlink {
            url: "#note-anchor".to_string(),
        };
        let (footnotes, _, footnote_rels, _, _) = render(&internal_anchor);
        assert_eq!(counts(&footnotes, "footnote", "FOOT"), (2, 1, 0, 1, 2, 1));
        assert!(footnotes.contains(r#"<w:hyperlink w:anchor="note-anchor">"#));
        assert!(footnote_rels.is_empty());

        let mut malformed_anchor = relationship_bearing.clone();
        let Block::Paragraph(paragraph) = &mut malformed_anchor[0][0].as_mut().unwrap().blocks[0]
        else {
            panic!("paragraph payload")
        };
        paragraph.runs[0].field = FieldRole::Hyperlink {
            url: "#bad anchor".to_string(),
        };
        let (footnotes, _, footnote_rels, _, _) = render(&malformed_anchor);
        assert_eq!(counts(&footnotes, "footnote", "FOOT"), (1, 0, 2, 0, 0, 0));
        assert!(!footnotes.contains("<w:hyperlink"));
        assert!(footnote_rels.is_empty());

        let mut bookmark = valid.clone();
        let Block::Paragraph(paragraph) = &mut bookmark[0][0].as_mut().unwrap().blocks[0] else {
            panic!("paragraph payload")
        };
        paragraph.runs[0].bookmark = Some("NoteTarget".to_string());
        let (footnotes, _, footnote_rels, _, _) = render(&bookmark);
        assert!(footnotes.contains(r#"<w:bookmarkStart w:id="0" w:name="NoteTarget"/>"#));
        assert!(footnotes.contains(r#"<w:bookmarkEnd w:id="0"/>"#));
        assert!(footnote_rels.is_empty());

        let mut malformed_bookmark = bookmark.clone();
        let Block::Paragraph(paragraph) = &mut malformed_bookmark[0][0].as_mut().unwrap().blocks[0]
        else {
            panic!("paragraph payload")
        };
        paragraph.runs[0].bookmark = Some("Bad Target".to_string());
        let (footnotes, _, footnote_rels, _, _) = render(&malformed_bookmark);
        assert_eq!(counts(&footnotes, "footnote", "FOOT"), (1, 0, 2, 0, 0, 0));
        assert!(!footnotes.contains("<w:bookmark"));
        assert!(footnote_rels.is_empty());

        let mut mixed_semantics = relationship_bearing.clone();
        let Block::Paragraph(paragraph) = &mut mixed_semantics[0][0].as_mut().unwrap().blocks[1]
        else {
            panic!("paragraph payload")
        };
        paragraph.runs[0].field = FieldRole::Simple {
            instruction: "CUSTOM unsupported-note".to_string(),
        };
        let (footnotes, _, footnote_rels, _, _) = render(&mixed_semantics);
        assert_eq!(counts(&footnotes, "footnote", "FOOT"), (1, 0, 2, 0, 0, 0));
        assert!(footnote_rels.is_empty());

        let image_run = |mime: &str| Run {
            image: Some(Image {
                alt: Some("Note raster".to_string()),
                bytes: Some(vec![1, 2, 3]),
                mime: Some(mime.to_string()),
                width_px: Some(2),
                height_px: Some(3),
                ..Image::default()
            }),
            ..Run::default()
        };
        for (mime, ext, content_type) in [
            ("image/png", "png", "image/png"),
            ("image/jpeg", "jpg", "image/jpeg"),
            ("image/gif", "gif", "image/gif"),
            ("image/bmp", "bmp", "image/bmp"),
            ("image/tiff", "tif", "image/tiff"),
            ("image/webp", "webp", "image/webp"),
        ] {
            let mut encoded = valid.clone();
            let Block::Paragraph(paragraph) = &mut encoded[0][0].as_mut().unwrap().blocks[1] else {
                panic!("paragraph payload")
            };
            paragraph.runs.push(image_run(mime));
            let (footnotes, _, footnote_rels, _, media) = render(&encoded);
            assert!(footnotes.contains("<w:drawing>"), "{mime}: {footnotes}");
            assert!(footnotes.contains("xmlns:wp="), "{mime}: {footnotes}");
            assert_eq!(footnote_rels.len(), 1, "{mime}");
            assert_eq!(footnote_rels[0].rel_type, REL_IMAGE, "{mime}");
            assert_eq!(footnote_rels[0].target, format!("media/image1.{ext}"));
            assert!(!footnote_rels[0].external);
            assert_eq!(media.len(), 1, "{mime}");
            assert_eq!(media[0].1, [1, 2, 3]);
            assert_eq!(media[0].2, ext);
            assert_eq!(media[0].3, content_type);
        }

        let mut linked_raster = valid.clone();
        let Block::Paragraph(paragraph) = &mut linked_raster[0][0].as_mut().unwrap().blocks[1]
        else {
            panic!("paragraph payload")
        };
        let mut run = image_run("image/png");
        run.field = FieldRole::Hyperlink {
            url: "https://example.com/note-raster".to_string(),
        };
        paragraph.runs.push(run);
        let (footnotes, _, footnote_rels, _, media) = render(&linked_raster);
        assert!(footnotes.contains(r#"<w:hyperlink r:id="rId1">"#));
        assert!(footnotes.contains(r#"r:embed="rId2""#));
        assert_eq!(footnote_rels.len(), 2);
        assert_eq!(footnote_rels[0].rel_type, REL_HYPERLINK);
        assert_eq!(footnote_rels[1].rel_type, REL_IMAGE);
        assert_eq!(media.len(), 1);

        let reject_image = |image: Image| {
            let mut payloads = valid.clone();
            let Block::Paragraph(paragraph) = &mut payloads[0][0].as_mut().unwrap().blocks[1]
            else {
                panic!("paragraph payload")
            };
            paragraph.runs.push(Run {
                image: Some(image),
                ..Run::default()
            });
            let (footnotes, _, footnote_rels, _, media) = render(&payloads);
            assert_eq!(counts(&footnotes, "footnote", "FOOT"), (1, 0, 2, 0, 0, 0));
            assert!(!footnotes.contains("<w:drawing>"));
            assert!(footnote_rels.is_empty());
            assert!(media.is_empty());
        };
        reject_image(Image {
            mime: Some("image/png".to_string()),
            ..Image::default()
        });
        reject_image(Image {
            bytes: Some(Vec::new()),
            mime: Some("image/png".to_string()),
            ..Image::default()
        });
        reject_image(Image {
            bytes: Some(vec![1, 2, 3]),
            mime: Some(crate::image::MIME_RAW_RGBA.to_string()),
            ..Image::default()
        });
        reject_image(Image {
            bytes: Some(vec![1, 2, 3]),
            mime: Some("image/unknown".to_string()),
            ..Image::default()
        });
        reject_image(Image {
            bytes: Some(vec![1, 2, 3]),
            mime: Some("image/png".to_string()),
            floating_offset_emu: Some((1, 2)),
            ..Image::default()
        });

        let Block::Paragraph(model_paragraph) = &model.blocks[0] else {
            panic!("body paragraph")
        };
        let note = model_paragraph.runs[0].note.as_ref().unwrap();
        let mut chart_payload = valid[0][0].clone().unwrap();
        chart_payload.blocks.push(Block::Chart(Chart {
            kind: ChartKind::Bar,
            categories: vec!["A".to_string()],
            series: vec![ChartSeries {
                name: "Values".to_string(),
                values: vec![1.0],
                bubble_sizes: Vec::new(),
            }],
            ..Chart::default()
        }));
        assert!(source_note_payload_is_supported(note, &chart_payload));

        if let Block::Chart(chart) = chart_payload.blocks.last_mut().unwrap() {
            chart.series[0].values.clear();
        }
        assert!(!source_note_payload_is_supported(note, &chart_payload));
        if let Block::Chart(chart) = chart_payload.blocks.last_mut().unwrap() {
            chart.series[0].values.push(f64::NAN);
        }
        assert!(!source_note_payload_is_supported(note, &chart_payload));
        if let Block::Chart(chart) = chart_payload.blocks.last_mut().unwrap() {
            chart.series.clear();
        }
        assert!(!source_note_payload_is_supported(note, &chart_payload));
    }

    #[test]
    fn source_note_table_payloads_validate_components_leaves_and_siblings() {
        let outer_text = "TABLE\nBREAK";
        let nested_text = "NESTED\nBREAK";
        let footnote_text = "TABLE\nBREAK NESTED\nBREAK";
        let endnote_text = "END";
        let model = DocModel {
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    Run {
                        text: "A".to_string(),
                        note: Some(AuthoredNote {
                            kind: NoteKind::Footnote,
                            text: footnote_text.to_string(),
                        }),
                        ..Run::default()
                    },
                    Run {
                        text: "B".to_string(),
                        note: Some(AuthoredNote {
                            kind: NoteKind::Endnote,
                            text: endnote_text.to_string(),
                        }),
                        ..Run::default()
                    },
                ],
                ..Paragraph::default()
            })],
            ..DocModel::default()
        };
        let mut table_paragraph = para(outer_text);
        table_paragraph.props.align = Align::Center;
        let nested_table = Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Paragraph(para(nested_text)), Block::PageBreak],
                    ..Cell::default()
                }],
            }],
            width_pct: Some(0.6),
            ..Table::default()
        };
        let table = Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![
                        Block::Paragraph(table_paragraph),
                        Block::PageBreak,
                        Block::Table(nested_table),
                    ],
                    ..Cell::default()
                }],
            }],
            width_pct: Some(0.8),
            fixed_layout: true,
            ..Table::default()
        };
        let nested_hints = TablePaginationHints {
            rows: vec![TableRowPaginationHint { cant_split: true }],
            cells: vec![vec![vec![
                Some(PaginationHint {
                    keep_lines: true,
                    widow_control: false,
                    ..PaginationHint::default()
                }),
                None,
            ]]],
            cell_line_spacing: vec![vec![vec![Some(LineSpacingHint::Exact(15.0)), None]]],
            cell_column_breaks: vec![vec![vec![vec![6], Vec::new()]]],
            nested: vec![vec![vec![None, None]]],
            cell_tabs: vec![vec![vec![
                vec![tab(48.0, TabAlignment::Right, TabLeader::Hyphen)],
                Vec::new(),
            ]]],
        };
        let valid = vec![vec![
            Some(NoteWritePayload {
                kind: NoteKind::Footnote,
                text: footnote_text.to_string(),
                blocks: vec![Block::Table(table)],
                pagination: vec![PaginationHint::default()],
                line_spacing: vec![None],
                tab_stops: vec![Vec::new()],
                column_break_offsets: vec![Vec::new()],
                table_pagination: vec![Some(TablePaginationHints {
                    rows: vec![TableRowPaginationHint { cant_split: true }],
                    cells: vec![vec![vec![
                        Some(PaginationHint {
                            keep_lines: true,
                            widow_control: false,
                            ..PaginationHint::default()
                        }),
                        None,
                        None,
                    ]]],
                    cell_line_spacing: vec![vec![vec![
                        Some(LineSpacingHint::Exact(12.0)),
                        None,
                        None,
                    ]]],
                    cell_column_breaks: vec![vec![vec![vec![5], Vec::new(), Vec::new()]]],
                    nested: vec![vec![vec![None, None, Some(nested_hints)]]],
                    cell_tabs: vec![vec![vec![
                        vec![tab(36.0, TabAlignment::Center, TabLeader::Dot)],
                        Vec::new(),
                        Vec::new(),
                    ]]],
                })],
            }),
            Some(NoteWritePayload {
                kind: NoteKind::Endnote,
                text: endnote_text.to_string(),
                blocks: vec![Block::Paragraph(para(endnote_text))],
                pagination: vec![PaginationHint::default()],
                line_spacing: vec![Some(LineSpacingHint::Exact(20.0))],
                tab_stops: vec![Vec::new()],
                column_break_offsets: vec![Vec::new()],
                table_pagination: vec![None],
            }),
        ]];
        let render = |note_payloads: &[Vec<Option<NoteWritePayload>>]| {
            let rendered = render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads,
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            );
            (
                String::from_utf8(rendered.footnotes_xml.expect("footnotes part")).unwrap(),
                String::from_utf8(rendered.endnotes_xml.expect("endnotes part")).unwrap(),
                rendered.footnote_rels,
                rendered.endnote_rels,
            )
        };
        let counts = |xml: &str, item: &str, text: &str| {
            let note = written_note_with_text(xml, item, text);
            (
                note.matches("<w:tbl>").count(),
                note.matches("<w:p>").count(),
                note.matches(r#"<w:br w:type="column"/>"#).count(),
                note.matches("<w:br/>").count(),
                note.matches("<w:cantSplit/>").count(),
                note.matches("<w:keepLines/>").count(),
                note.matches("w:lineRule=").count(),
                note.matches("<w:tabs>").count(),
                note.matches(r#"<w:br w:type="page"/>"#).count(),
            )
        };
        let render_counts = |payloads: &[Vec<Option<NoteWritePayload>>]| {
            let (footnotes, endnotes, _, _) = render(payloads);
            (
                counts(&footnotes, "footnote", "TABLE"),
                counts(&endnotes, "endnote", "END"),
            )
        };

        assert_eq!(
            render_counts(&valid),
            ((2, 4, 2, 0, 2, 2, 2, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut bad_table_outer = valid.clone();
        bad_table_outer[0][0]
            .as_mut()
            .unwrap()
            .table_pagination
            .clear();
        assert_eq!(
            render_counts(&bad_table_outer),
            ((2, 4, 0, 2, 0, 0, 0, 0, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut bad_row_component = valid.clone();
        bad_row_component[0][0].as_mut().unwrap().table_pagination[0]
            .as_mut()
            .unwrap()
            .rows
            .clear();
        assert_eq!(
            render_counts(&bad_row_component),
            ((2, 4, 2, 0, 1, 2, 2, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut bad_line_component = valid.clone();
        bad_line_component[0][0].as_mut().unwrap().table_pagination[0]
            .as_mut()
            .unwrap()
            .cell_line_spacing[0][0]
            .clear();
        assert_eq!(
            render_counts(&bad_line_component),
            ((2, 4, 2, 0, 2, 2, 1, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut bad_break_leaf = valid.clone();
        bad_break_leaf[0][0].as_mut().unwrap().table_pagination[0]
            .as_mut()
            .unwrap()
            .cell_column_breaks[0][0][0] = vec![5, 5];
        assert_eq!(
            render_counts(&bad_break_leaf),
            ((2, 4, 1, 1, 2, 2, 2, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut missing_nested_slot = valid.clone();
        missing_nested_slot[0][0].as_mut().unwrap().table_pagination[0]
            .as_mut()
            .unwrap()
            .nested[0][0][2] = None;
        assert_eq!(
            render_counts(&missing_nested_slot),
            ((2, 4, 1, 1, 1, 1, 1, 1, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut bad_nested_rows = valid.clone();
        bad_nested_rows[0][0].as_mut().unwrap().table_pagination[0]
            .as_mut()
            .unwrap()
            .nested[0][0][2]
            .as_mut()
            .unwrap()
            .rows
            .clear();
        assert_eq!(
            render_counts(&bad_nested_rows),
            ((2, 4, 2, 0, 1, 2, 2, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut bad_nested_break = valid.clone();
        bad_nested_break[0][0].as_mut().unwrap().table_pagination[0]
            .as_mut()
            .unwrap()
            .nested[0][0][2]
            .as_mut()
            .unwrap()
            .cell_column_breaks[0][0][0] = vec![6, 6];
        assert_eq!(
            render_counts(&bad_nested_break),
            ((2, 4, 1, 1, 2, 2, 2, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );

        let mut relationship_bearing = valid.clone();
        let Block::Table(table) = &mut relationship_bearing[0][0].as_mut().unwrap().blocks[0]
        else {
            panic!("table payload")
        };
        let Block::Table(nested) = &mut table.rows[0].cells[0].blocks[2] else {
            panic!("nested table payload")
        };
        let Block::Paragraph(paragraph) = &mut nested.rows[0].cells[0].blocks[0] else {
            panic!("nested table paragraph payload")
        };
        paragraph.runs[0].field = FieldRole::Hyperlink {
            url: "https://example.com/note-table".to_string(),
        };
        let (footnotes, endnotes, footnote_rels, endnote_rels) = render(&relationship_bearing);
        assert_eq!(
            (
                counts(&footnotes, "footnote", "TABLE"),
                counts(&endnotes, "endnote", "END")
            ),
            ((2, 4, 2, 0, 2, 2, 2, 2, 2), (0, 1, 0, 0, 0, 0, 1, 0, 0))
        );
        assert!(footnotes.contains(r#"<w:hyperlink r:id="rId1">"#));
        assert_eq!(footnote_rels.len(), 1);
        assert_eq!(footnote_rels[0].rel_type, REL_HYPERLINK);
        assert_eq!(footnote_rels[0].target, "https://example.com/note-table");
        assert!(footnote_rels[0].external);
        assert!(endnote_rels.is_empty());
    }

    #[test]
    fn source_column_break_writer_validates_offsets_and_preserves_wrappers() {
        let paragraph = Paragraph {
            props: ParaProps::default(),
            runs: vec![
                Run {
                    text: "A\nB".to_string(),
                    props: CharProps {
                        bold: true,
                        ..CharProps::default()
                    },
                    field: FieldRole::Hyperlink {
                        url: "https://example.com/column-break".to_string(),
                    },
                    bookmark: Some("ColumnBreakBookmark".to_string()),
                    comment: Some(AuthoredComment {
                        text: "Column break comment".to_string(),
                        ..AuthoredComment::default()
                    }),
                    ..Run::default()
                },
                Run {
                    text: "\nC\nD".to_string(),
                    field: FieldRole::Simple {
                        instruction: "UNKNOWN ColumnBreak".to_string(),
                    },
                    field_dirty: true,
                    content_control: Some(AuthoredContentControl {
                        tag: Some("column-break-control".to_string()),
                        ..AuthoredContentControl::default()
                    }),
                    revision: Some(AuthoredRevision::default()),
                    ..Run::default()
                },
            ],
        };
        let model = DocModel {
            blocks: vec![Block::Paragraph(paragraph)],
            ..DocModel::default()
        };
        let line_spacing = [Some(LineSpacingHint::Exact(12.0))];
        let pagination = [PaginationHint {
            keep_next: true,
            widow_control: true,
            ..PaginationHint::default()
        }];
        let tabs = [vec![tab(36.0, TabAlignment::Left, TabLeader::Dot)]];
        let render = |column_break_offsets: &[Vec<usize>]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &[None],
                        layouts: &[None],
                        separators: &[false],
                        rtl: &[false],
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &line_spacing,
                        paragraph_pagination: &pagination,
                        paragraph_tab_stops: &tabs,
                        column_break_offsets,
                        table_cell_column_break_offsets: &[],
                        table_row_pagination: &[],
                        table_cell_pagination: &[],
                        table_cell_line_spacing: &[],
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };

        let valid = render(&[vec![1, 3]]);
        assert_eq!(
            valid.matches(r#"<w:br w:type="column"/>"#).count(),
            2,
            "{valid}"
        );
        assert_eq!(valid.matches("<w:br/>").count(), 1, "{valid}");
        for marker in [
            "<w:commentRangeStart",
            "<w:bookmarkStart",
            "<w:hyperlink",
            "<w:b/>",
            "<w:fldSimple",
            "<w:sdt>",
            "<w:ins",
        ] {
            assert!(valid.contains(marker), "missing {marker}: {valid}");
        }
        let hyperlink = &valid[valid.find("<w:hyperlink").unwrap()
            ..valid.find("</w:hyperlink>").unwrap() + "</w:hyperlink>".len()];
        assert!(
            hyperlink.contains(r#"<w:br w:type="column"/>"#),
            "{hyperlink}"
        );
        let field = &valid[valid.find("<w:fldSimple").unwrap()
            ..valid.find("</w:fldSimple>").unwrap() + "</w:fldSimple>".len()];
        assert!(
            field.contains(r#"<w:br w:type="column"/>"#) && field.contains("<w:br/>"),
            "{field}"
        );

        for malformed in [
            vec![vec![1, 3], Vec::new()],
            vec![vec![1, 1]],
            vec![vec![3, 1]],
            vec![vec![0]],
            vec![vec![99]],
        ] {
            let rejected = render(&malformed);
            assert!(
                !rejected.contains(r#"<w:br w:type="column"/>"#),
                "{malformed:?}: {rejected}"
            );
            assert_eq!(rejected.matches("<w:br/>").count(), 3);
            assert!(rejected.contains("<w:keepNext/>"), "{rejected}");
            assert!(
                rejected.contains(r#"w:line="240" w:lineRule="exact""#),
                "{rejected}"
            );
            assert!(rejected.contains("<w:tabs>"), "{rejected}");
        }

        let image_backed = Paragraph {
            runs: vec![Run {
                text: "\n".to_string(),
                image: Some(Image {
                    bytes: Some(vec![0]),
                    ..Image::default()
                }),
                ..Run::default()
            }],
            ..Paragraph::default()
        };
        assert_eq!(source_column_break_offsets(&image_backed, Some(&[0])), None);
    }

    #[test]
    fn source_table_cell_column_break_writer_validates_tree_and_leaves_independently() {
        let wrapped = Paragraph {
            runs: vec![
                Run {
                    text: "A\nB".to_string(),
                    props: CharProps {
                        bold: true,
                        ..CharProps::default()
                    },
                    field: FieldRole::Hyperlink {
                        url: "https://example.com/table-column-break".to_string(),
                    },
                    bookmark: Some("TableColumnBreak".to_string()),
                    ..Run::default()
                },
                Run {
                    text: "\nC".to_string(),
                    content_control: Some(AuthoredContentControl {
                        tag: Some("table-column-break".to_string()),
                        ..AuthoredContentControl::default()
                    }),
                    revision: Some(AuthoredRevision::default()),
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        };
        let trailing = Paragraph {
            runs: vec![Run {
                text: "D\nE\nF".to_string(),
                ..Run::default()
            }],
            ..Paragraph::default()
        };
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![
                            Block::Paragraph(wrapped),
                            Block::PageBreak,
                            Block::Paragraph(trailing),
                        ],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let row_pagination = [vec![TableRowPaginationHint { cant_split: true }]];
        let line_spacing = [vec![vec![vec![
            Some(LineSpacingHint::Exact(12.0)),
            None,
            Some(LineSpacingHint::AtLeast(14.0)),
        ]]]];
        let render = |table_cell_column_break_offsets: &[TableCellColumnBreakHints]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &[None],
                        layouts: &[None],
                        separators: &[false],
                        rtl: &[false],
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination: &[],
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets,
                        table_row_pagination: &row_pagination,
                        table_cell_pagination: &[],
                        table_cell_line_spacing: &line_spacing,
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };
        let valid: TableCellColumnBreakHints = vec![vec![vec![vec![1, 3], Vec::new(), vec![1]]]];

        let written = render(std::slice::from_ref(&valid));
        assert_eq!(written.matches(r#"<w:br w:type="column"/>"#).count(), 3);
        assert_eq!(written.matches(r#"<w:br w:type="page"/>"#).count(), 1);
        assert_eq!(written.matches("<w:br/>").count(), 1);
        for marker in ["<w:bookmarkStart", "<w:hyperlink", "<w:sdt>", "<w:ins"] {
            assert!(written.contains(marker), "missing {marker}: {written}");
        }
        assert!(written.contains("<w:cantSplit/>"), "{written}");
        assert!(written.contains(r#"w:line="240" w:lineRule="exact""#));
        assert!(written.contains(r#"w:line="280" w:lineRule="atLeast""#));

        let mut malformed_trees = vec![
            vec![valid.clone(), valid.clone()],
            vec![vec![valid[0].clone(), valid[0].clone()]],
            vec![vec![vec![valid[0][0].clone(), valid[0][0].clone()]]],
            vec![vec![vec![vec![vec![1, 3]]]]],
        ];
        let mut nonparagraph = valid.clone();
        nonparagraph[0][0][1] = vec![0];
        malformed_trees.push(vec![nonparagraph]);
        for malformed in malformed_trees {
            let rejected = render(&malformed);
            assert!(!rejected.contains(r#"<w:br w:type="column"/>"#));
            assert_eq!(rejected.matches(r#"<w:br w:type="page"/>"#).count(), 1);
            assert_eq!(rejected.matches("<w:br/>").count(), 4);
            assert!(rejected.contains("<w:cantSplit/>"), "{rejected}");
            assert!(rejected.contains(r#"w:line="240" w:lineRule="exact""#));
        }

        for malformed_leaf in [vec![1, 1], vec![3, 1], vec![0], vec![99]] {
            let mut malformed = valid.clone();
            malformed[0][0][0] = malformed_leaf;
            let isolated = render(&[malformed]);
            assert_eq!(isolated.matches(r#"<w:br w:type="column"/>"#).count(), 1);
            assert_eq!(isolated.matches(r#"<w:br w:type="page"/>"#).count(), 1);
            assert_eq!(isolated.matches("<w:br/>").count(), 3);
            assert!(isolated.contains("<w:cantSplit/>"), "{isolated}");
        }
    }

    #[test]
    fn source_paragraph_pagination_writer_rejects_misalignment_and_orders_controls() {
        let mut paragraph = para("body");
        paragraph.props.page_break_before = true;
        paragraph.props.list = Some(ListInfo {
            level: 2,
            ordered: true,
            label: "1.".to_string(),
            marker_font: None,
        });
        let model = DocModel {
            blocks: vec![Block::Paragraph(paragraph)],
            ..DocModel::default()
        };
        let gaps = [None];
        let layouts = [None];
        let separators = [false];
        let rtl = [false];
        let distances = [RunningSurfaceDistanceHints::default()];
        let pagination = PaginationHint {
            keep_next: true,
            keep_lines: true,
            widow_control: false,
        };
        let misaligned = [pagination, pagination];
        let render = |paragraph_pagination: &[PaginationHint]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &gaps,
                        layouts: &layouts,
                        separators: &separators,
                        rtl: &rtl,
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &distances,
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination,
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &[],
                        table_row_pagination: &[],
                        table_cell_pagination: &[],
                        table_cell_line_spacing: &[],
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };

        let rejected = render(&misaligned);
        assert!(!rejected.contains("<w:keepNext"), "{rejected}");
        assert!(!rejected.contains("<w:keepLines"), "{rejected}");
        assert!(!rejected.contains("<w:widowControl"), "{rejected}");

        let aligned = render(&[pagination]);
        assert!(
            aligned.contains(concat!(
                "<w:keepNext/><w:keepLines/><w:pageBreakBefore/>",
                r#"<w:widowControl w:val="0"/><w:numPr>"#,
            )),
            "{aligned}"
        );
    }

    #[test]
    fn source_body_paragraph_hints_exclude_running_surfaces() {
        let mut header = para("header");
        header.props.spacing.line_pct = Some(1.5);
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("body"))],
            setup: DocSetup {
                header: vec![Block::Paragraph(header)],
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let body_tab_stops = [vec![tab(36.0, TabAlignment::Right, TabLeader::Dot)]];
        let rendered = render_body(
            &model,
            Some(SourceWriteHints {
                gaps: &[None],
                layouts: &[None],
                separators: &[false],
                rtl: &[false],
                final_gap: None,
                final_layout: None,
                final_separator: false,
                final_rtl: false,
                running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                running_line_spacing: &[],
                running_pagination: &[],
                running_tab_stops: &[],
                running_table_cell_tab_stops: &[],
                running_table_layout: &[],
                running_column_break_offsets: &[],
                note_payloads: &[],
                paragraph_line_spacing: &[Some(LineSpacingHint::Exact(12.0))],
                paragraph_pagination: &[PaginationHint {
                    keep_next: true,
                    widow_control: true,
                    ..PaginationHint::default()
                }],
                paragraph_tab_stops: &body_tab_stops,
                column_break_offsets: &[],
                table_cell_column_break_offsets: &[],
                table_row_pagination: &[],
                table_cell_pagination: &[],
                table_cell_line_spacing: &[],
                table_nested_pagination: &[],
                table_cell_tab_stops: &[],
            }),
        );
        let document_xml = String::from_utf8(rendered.document_xml).unwrap();
        assert!(document_xml.contains("<w:keepNext/>"), "{document_xml}");
        assert!(
            document_xml
                .contains(r#"<w:tabs><w:tab w:val="right" w:pos="720" w:leader="dot"/></w:tabs>"#),
            "{document_xml}"
        );
        assert!(
            document_xml.contains(r#"w:line="240" w:lineRule="exact""#),
            "{document_xml}"
        );

        let header_xml = rendered
            .hf_parts
            .iter()
            .find(|(path, _, _)| path.starts_with("word/header"))
            .map(|(_, _, bytes)| String::from_utf8_lossy(bytes))
            .expect("generated running header");
        assert!(!header_xml.contains("<w:keepNext"));
        assert!(!header_xml.contains("<w:tabs>"));
        assert!(!header_xml.contains(r#"w:lineRule="exact""#));
        assert!(
            header_xml.contains(r#"w:line="360" w:lineRule="auto""#),
            "{header_xml}"
        );
    }

    #[test]
    fn source_table_row_pagination_writer_rejects_misalignment_and_orders_controls() {
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![cell("header")],
                }],
                header_rows: 1,
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let gaps = [None];
        let layouts = [None];
        let separators = [false];
        let rtl = [false];
        let distances = [RunningSurfaceDistanceHints::default()];
        let hint = TableRowPaginationHint { cant_split: true };
        let render = |table_row_pagination: &[Vec<TableRowPaginationHint>]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &gaps,
                        layouts: &layouts,
                        separators: &separators,
                        rtl: &rtl,
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &distances,
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination: &[],
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &[],
                        table_row_pagination,
                        table_cell_pagination: &[],
                        table_cell_line_spacing: &[],
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };

        let outer_misaligned = render(&[vec![hint], vec![hint]]);
        assert!(
            !outer_misaligned.contains("<w:cantSplit"),
            "{outer_misaligned}"
        );
        let inner_misaligned = render(&[vec![hint, hint]]);
        assert!(
            !inner_misaligned.contains("<w:cantSplit"),
            "{inner_misaligned}"
        );

        let aligned = render(&[vec![hint]]);
        assert!(
            aligned.contains("<w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>"),
            "{aligned}"
        );
    }

    #[test]
    fn source_table_cell_line_writer_rejects_misalignment_and_non_paragraph_hints() {
        let mut paragraph = para("cell");
        paragraph.props.spacing.before_pt = Some(6.0);
        paragraph.props.spacing.after_pt = Some(3.0);
        paragraph.props.spacing.line_pct = Some(1.5);
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(paragraph), Block::PageBreak],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let gaps = [None];
        let layouts = [None];
        let separators = [false];
        let rtl = [false];
        let distances = [RunningSurfaceDistanceHints::default()];
        let exact = Some(LineSpacingHint::Exact(12.0));
        let render = |table_cell_line_spacing: &[TableCellLineSpacingHints]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &gaps,
                        layouts: &layouts,
                        separators: &separators,
                        rtl: &rtl,
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &distances,
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination: &[],
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &[],
                        table_row_pagination: &[],
                        table_cell_pagination: &[],
                        table_cell_line_spacing,
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };
        let assert_rejected = |xml: &str| {
            assert!(!xml.contains(r#"w:lineRule="exact""#), "{xml}");
            assert!(xml.contains(r#"w:line="360" w:lineRule="auto""#), "{xml}");
        };

        assert_rejected(&render(&[
            vec![vec![vec![exact, None]]],
            vec![vec![vec![exact, None]]],
        ]));
        assert_rejected(&render(&[vec![
            vec![vec![exact, None]],
            vec![vec![exact, None]],
        ]]));
        assert_rejected(&render(&[vec![vec![vec![exact, None], vec![exact, None]]]]));
        assert_rejected(&render(&[vec![vec![vec![exact]]]]));
        assert_rejected(&render(&[vec![vec![vec![exact, exact]]]]));

        let aligned = render(&[vec![vec![vec![exact, None]]]]);
        assert!(
            aligned.contains(
                r#"<w:spacing w:before="120" w:after="60" w:line="240" w:lineRule="exact"/>"#
            ),
            "{aligned}"
        );
    }

    #[test]
    fn source_table_cell_pagination_writer_rejects_misalignment_independently() {
        let mut paragraph = para("cell");
        paragraph.props.spacing.before_pt = Some(6.0);
        paragraph.props.spacing.after_pt = Some(3.0);
        paragraph.props.spacing.line_pct = Some(1.5);
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(paragraph), Block::PageBreak],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let row_pagination = [vec![TableRowPaginationHint { cant_split: true }]];
        let pagination = Some(PaginationHint {
            keep_next: true,
            keep_lines: true,
            widow_control: false,
        });
        let valid_pagination = [vec![vec![vec![pagination, None]]]];
        let exact = Some(LineSpacingHint::Exact(12.0));
        let valid_line_spacing = [vec![vec![vec![exact, None]]]];
        let render = |table_cell_pagination: &[TableCellPaginationHints],
                      table_cell_line_spacing: &[TableCellLineSpacingHints]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &[None],
                        layouts: &[None],
                        separators: &[false],
                        rtl: &[false],
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination: &[],
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &[],
                        table_row_pagination: &row_pagination,
                        table_cell_pagination,
                        table_cell_line_spacing,
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &[],
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };
        let assert_pagination_rejected = |xml: &str| {
            assert!(xml.contains("<w:cantSplit/>"), "{xml}");
            assert!(xml.contains(r#"w:line="240" w:lineRule="exact""#), "{xml}");
            assert!(!xml.contains("<w:keepNext"), "{xml}");
            assert!(!xml.contains("<w:keepLines"), "{xml}");
            assert!(!xml.contains("<w:widowControl"), "{xml}");
        };

        assert_pagination_rejected(&render(
            &[
                vec![vec![vec![pagination, None]]],
                vec![vec![vec![pagination, None]]],
            ],
            &valid_line_spacing,
        ));
        assert_pagination_rejected(&render(
            &[vec![
                vec![vec![pagination, None]],
                vec![vec![pagination, None]],
            ]],
            &valid_line_spacing,
        ));
        assert_pagination_rejected(&render(
            &[vec![vec![vec![pagination, None], vec![pagination, None]]]],
            &valid_line_spacing,
        ));
        assert_pagination_rejected(&render(
            &[vec![vec![vec![pagination]]]],
            &valid_line_spacing,
        ));
        assert_pagination_rejected(&render(
            &[vec![vec![vec![pagination, pagination]]]],
            &valid_line_spacing,
        ));

        let aligned = render(&valid_pagination, &valid_line_spacing);
        assert!(aligned.contains("<w:cantSplit/>"), "{aligned}");
        assert!(
            aligned.contains(concat!(
                "<w:pPr><w:keepNext/><w:keepLines/>",
                r#"<w:widowControl w:val="0"/>"#,
                r#"<w:spacing w:before="120" w:after="60" w:line="240" w:lineRule="exact"/>"#,
            )),
            "{aligned}"
        );

        let line_rejected = render(&valid_pagination, &[vec![vec![vec![exact]]]]);
        assert!(line_rejected.contains("<w:cantSplit/>"), "{line_rejected}");
        assert!(line_rejected.contains("<w:keepNext/>"), "{line_rejected}");
        assert!(line_rejected.contains("<w:keepLines/>"), "{line_rejected}");
        assert!(
            line_rejected.contains(r#"<w:widowControl w:val="0"/>"#),
            "{line_rejected}"
        );
        assert!(!line_rejected.contains(r#"w:lineRule="exact""#));
        assert!(
            line_rejected.contains(r#"w:line="360" w:lineRule="auto""#),
            "{line_rejected}"
        );
    }

    #[test]
    fn source_table_cell_tab_writer_rejects_misalignment_independently() {
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(para("cell")), Block::PageBreak],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let row_pagination = [vec![TableRowPaginationHint { cant_split: true }]];
        let cell_pagination = [vec![vec![vec![
            Some(PaginationHint {
                keep_next: true,
                widow_control: true,
                ..PaginationHint::default()
            }),
            None,
        ]]]];
        let cell_line_spacing = [vec![vec![vec![Some(LineSpacingHint::Exact(12.0)), None]]]];
        let valid_table_tabs: TableCellTabStopHints = vec![vec![vec![
            vec![tab(36.0, TabAlignment::Right, TabLeader::Dot)],
            Vec::new(),
        ]]];
        let render = |table_cell_tab_stops: &[TableCellTabStopHints]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &[None],
                        layouts: &[None],
                        separators: &[false],
                        rtl: &[false],
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination: &[],
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &[],
                        table_row_pagination: &row_pagination,
                        table_cell_pagination: &cell_pagination,
                        table_cell_line_spacing: &cell_line_spacing,
                        table_nested_pagination: &[],
                        table_cell_tab_stops,
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };
        let assert_only_tabs_rejected = |xml: &str| {
            assert!(xml.contains("<w:cantSplit/>"), "{xml}");
            assert!(xml.contains("<w:keepNext/>"), "{xml}");
            assert!(xml.contains(r#"w:line="240" w:lineRule="exact""#), "{xml}");
            assert!(!xml.contains("<w:tabs>"), "{xml}");
        };

        assert_only_tabs_rejected(&render(&[
            valid_table_tabs.clone(),
            valid_table_tabs.clone(),
        ]));
        assert_only_tabs_rejected(&render(&[vec![
            valid_table_tabs[0].clone(),
            valid_table_tabs[0].clone(),
        ]]));
        assert_only_tabs_rejected(&render(&[vec![vec![
            valid_table_tabs[0][0].clone(),
            valid_table_tabs[0][0].clone(),
        ]]]));
        assert_only_tabs_rejected(&render(&[vec![vec![vec![vec![tab(
            36.0,
            TabAlignment::Right,
            TabLeader::Dot,
        )]]]]]));
        assert_only_tabs_rejected(&render(&[vec![vec![vec![
            vec![tab(36.0, TabAlignment::Right, TabLeader::Dot)],
            vec![tab(72.0, TabAlignment::Left, TabLeader::None)],
        ]]]]));
        assert_only_tabs_rejected(&render(&[vec![vec![vec![
            vec![tab(-1.0, TabAlignment::Right, TabLeader::Dot)],
            Vec::new(),
        ]]]]));

        let aligned = render(&[valid_table_tabs]);
        assert!(aligned.contains("<w:cantSplit/>"), "{aligned}");
        assert!(aligned.contains("<w:keepNext/>"), "{aligned}");
        assert!(
            aligned.contains(r#"w:line="240" w:lineRule="exact""#),
            "{aligned}"
        );
        assert!(
            aligned.contains(concat!(
                r#"<w:tabs><w:tab w:val="right" w:pos="720" "#,
                r#"w:leader="dot"/></w:tabs>"#,
            )),
            "{aligned}"
        );
    }

    #[test]
    fn source_table_cell_line_writer_tracks_surviving_vertical_merge_cells() {
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![
                    Row {
                        cells: vec![
                            Cell {
                                blocks: vec![Block::Paragraph(para("owner"))],
                                row_span: 2,
                                ..Cell::default()
                            },
                            cell("top"),
                        ],
                    },
                    Row {
                        cells: vec![cell("bottom")],
                    },
                ],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let table_cell_line_spacing = [vec![
            vec![
                vec![Some(LineSpacingHint::Exact(5.0))],
                vec![Some(LineSpacingHint::AtLeast(10.0))],
            ],
            vec![vec![Some(LineSpacingHint::Exact(15.0))]],
        ]];
        let document_xml = String::from_utf8(
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &table_cell_line_spacing,
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
            .document_xml,
        )
        .unwrap();

        for expected in [
            r#"w:line="100" w:lineRule="exact""#,
            r#"w:line="200" w:lineRule="atLeast""#,
            r#"w:line="300" w:lineRule="exact""#,
        ] {
            assert_eq!(document_xml.matches(expected).count(), 1, "{document_xml}");
        }
        assert_eq!(document_xml.matches("w:lineRule=").count(), 3);
    }

    #[test]
    fn source_table_cell_pagination_writer_tracks_surviving_vertical_merge_cells() {
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![
                    Row {
                        cells: vec![
                            Cell {
                                blocks: vec![Block::Paragraph(para("owner"))],
                                row_span: 2,
                                ..Cell::default()
                            },
                            cell("top"),
                        ],
                    },
                    Row {
                        cells: vec![cell("bottom")],
                    },
                ],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let table_cell_pagination = [vec![
            vec![
                vec![Some(PaginationHint {
                    keep_next: true,
                    widow_control: true,
                    ..PaginationHint::default()
                })],
                vec![Some(PaginationHint {
                    keep_lines: true,
                    widow_control: true,
                    ..PaginationHint::default()
                })],
            ],
            vec![vec![Some(PaginationHint::default())]],
        ]];
        let document_xml = String::from_utf8(
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &table_cell_pagination,
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
            .document_xml,
        )
        .unwrap();

        let owner = written_paragraph_with_text(&document_xml, "owner");
        assert!(owner.contains("<w:keepNext/>"), "{owner}");
        assert!(!owner.contains("<w:keepLines"), "{owner}");
        let top = written_paragraph_with_text(&document_xml, "top");
        assert!(top.contains("<w:keepLines/>"), "{top}");
        assert!(!top.contains("<w:keepNext"), "{top}");
        let bottom = written_paragraph_with_text(&document_xml, "bottom");
        assert!(
            bottom.contains(r#"<w:widowControl w:val="0"/>"#),
            "{bottom}"
        );
        assert_eq!(document_xml.matches("<w:keepNext").count(), 1);
        assert_eq!(document_xml.matches("<w:keepLines").count(), 1);
        assert_eq!(document_xml.matches("<w:widowControl").count(), 1);
    }

    #[test]
    fn source_table_cell_tab_writer_tracks_surviving_vertical_merge_cells() {
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![
                    Row {
                        cells: vec![
                            Cell {
                                blocks: vec![Block::Paragraph(para("owner"))],
                                row_span: 2,
                                ..Cell::default()
                            },
                            cell("top"),
                        ],
                    },
                    Row {
                        cells: vec![cell("bottom")],
                    },
                ],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let table_cell_tab_stops = [vec![
            vec![
                vec![vec![tab(5.0, TabAlignment::Left, TabLeader::Dot)]],
                vec![vec![tab(10.0, TabAlignment::Center, TabLeader::Hyphen)]],
            ],
            vec![vec![vec![tab(
                15.0,
                TabAlignment::Right,
                TabLeader::Underscore,
            )]]],
        ]];
        let document_xml = String::from_utf8(
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &[],
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &table_cell_tab_stops,
                }),
            )
            .document_xml,
        )
        .unwrap();

        for (text, expected) in [
            (
                "owner",
                r#"<w:tab w:val="left" w:pos="100" w:leader="dot"/>"#,
            ),
            (
                "top",
                r#"<w:tab w:val="center" w:pos="200" w:leader="hyphen"/>"#,
            ),
            (
                "bottom",
                r#"<w:tab w:val="right" w:pos="300" w:leader="underscore"/>"#,
            ),
        ] {
            let paragraph = written_paragraph_with_text(&document_xml, text);
            assert!(paragraph.contains(expected), "{paragraph}");
        }
        assert_eq!(document_xml.matches("<w:tabs>").count(), 3);
    }

    #[test]
    fn source_table_cell_hints_include_nested_but_exclude_running_surface_tables() {
        let mut nested_paragraph = para("nested\tA\nB");
        nested_paragraph.props.spacing.line_pct = Some(1.5);
        let nested_table = Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Paragraph(nested_paragraph)],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        };
        let mut header_paragraph = para("header");
        header_paragraph.props.spacing.line_pct = Some(1.5);
        let header_table = Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Paragraph(header_paragraph)],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        };
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(para("direct")), Block::Table(nested_table)],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            })],
            setup: DocSetup {
                header: vec![Block::Table(header_table)],
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let table_cell_pagination = [vec![vec![vec![
            Some(PaginationHint {
                keep_next: true,
                widow_control: true,
                ..PaginationHint::default()
            }),
            None,
        ]]]];
        let table_cell_line_spacing = [vec![vec![vec![Some(LineSpacingHint::Exact(5.0)), None]]]];
        let table_cell_tab_stops = [vec![vec![vec![
            vec![tab(36.0, TabAlignment::Decimal, TabLeader::Heavy)],
            Vec::new(),
        ]]]];
        let table_nested_pagination = [vec![vec![vec![
            None,
            Some(TablePaginationHints {
                rows: vec![TableRowPaginationHint { cant_split: true }],
                cells: vec![vec![vec![Some(PaginationHint {
                    keep_lines: true,
                    ..PaginationHint::default()
                })]]],
                cell_line_spacing: vec![vec![vec![Some(LineSpacingHint::Exact(7.0))]]],
                cell_column_breaks: vec![vec![vec![vec![8]]]],
                nested: vec![vec![vec![None]]],
                cell_tabs: vec![vec![vec![vec![tab(
                    18.0,
                    TabAlignment::Center,
                    TabLeader::Hyphen,
                )]]]],
            }),
        ]]]];
        let rendered = render_body(
            &model,
            Some(SourceWriteHints {
                gaps: &[None],
                layouts: &[None],
                separators: &[false],
                rtl: &[false],
                final_gap: None,
                final_layout: None,
                final_separator: false,
                final_rtl: false,
                running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                running_line_spacing: &[],
                running_pagination: &[],
                running_tab_stops: &[],
                running_table_cell_tab_stops: &[],
                running_table_layout: &[],
                running_column_break_offsets: &[],
                note_payloads: &[],
                paragraph_line_spacing: &[],
                paragraph_pagination: &[],
                paragraph_tab_stops: &[],
                column_break_offsets: &[],
                table_cell_column_break_offsets: &[],
                table_row_pagination: &[],
                table_cell_pagination: &table_cell_pagination,
                table_cell_line_spacing: &table_cell_line_spacing,
                table_nested_pagination: &table_nested_pagination,
                table_cell_tab_stops: &table_cell_tab_stops,
            }),
        );
        let document_xml = String::from_utf8(rendered.document_xml).unwrap();
        assert_eq!(document_xml.matches(r#"w:lineRule="exact""#).count(), 2);
        assert_eq!(document_xml.matches("<w:keepNext").count(), 1);
        assert_eq!(document_xml.matches("<w:keepLines").count(), 1);
        assert_eq!(document_xml.matches("<w:cantSplit/>").count(), 1);
        assert_eq!(document_xml.matches("<w:tabs>").count(), 2);
        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            1
        );
        let direct = written_paragraph_with_text(&document_xml, "direct");
        assert!(
            direct.contains(r#"<w:tab w:val="decimal" w:pos="720" w:leader="heavy"/>"#),
            "{direct}"
        );
        let nested = written_paragraph_with_text(&document_xml, "nested");
        assert!(nested.contains("<w:keepLines/>"), "{nested}");
        assert!(
            nested.contains(r#"w:line="140" w:lineRule="exact""#),
            "{nested}"
        );
        assert!(
            nested.contains(r#"<w:tab w:val="center" w:pos="360" w:leader="hyphen"/>"#),
            "{nested}"
        );
        assert_eq!(
            document_xml
                .matches(r#"w:line="360" w:lineRule="auto""#)
                .count(),
            0
        );

        let header_xml = rendered
            .hf_parts
            .iter()
            .find(|(path, _, _)| path.starts_with("word/header"))
            .map(|(_, _, bytes)| String::from_utf8_lossy(bytes))
            .expect("generated header table");
        assert!(!header_xml.contains(r#"w:lineRule="exact""#));
        assert!(!header_xml.contains("<w:keepNext"));
        assert!(!header_xml.contains("<w:tabs>"));
        assert!(
            header_xml.contains(r#"w:line="360" w:lineRule="auto""#),
            "{header_xml}"
        );
    }

    #[test]
    fn source_running_column_break_hints_validate_sections_variants_and_leaves() {
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("BODY"))],
            setup: DocSetup {
                header: vec![
                    Block::Paragraph(para("ALPHA\nBETA")),
                    Block::Paragraph(para("CHARLIE\nDELTA")),
                    Block::Table(Table {
                        rows: vec![Row {
                            cells: vec![cell("TABLE\nTAIL")],
                        }],
                        ..Table::default()
                    }),
                ],
                first_header: vec![Block::Paragraph(para("FIRST ONLY\nSECOND"))],
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let table_layout = [RunningSurfaceTableLayoutHints {
            header: RunningTableLayoutHints {
                cell_column_breaks: vec![Vec::new(), Vec::new(), vec![vec![vec![vec![5]]]]],
                nested_tables: vec![Vec::new(), Vec::new(), vec![vec![vec![None]]]],
            },
            ..RunningSurfaceTableLayoutHints::default()
        }];
        let render = |column_breaks: &[RunningSurfaceColumnBreakHints]| {
            render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: &table_layout,
                    running_column_break_offsets: column_breaks,
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            )
        };
        let running_counts = |rendered: &super::BodyRender| {
            let header = generated_running_part(rendered, "ALPHA").1;
            let first_header = generated_running_part(rendered, "FIRST ONLY").1;
            (
                header.matches(r#"<w:br w:type="column"/>"#).count(),
                header.matches("<w:br/>").count(),
                first_header.matches(r#"<w:br w:type="column"/>"#).count(),
                first_header.matches("<w:br/>").count(),
            )
        };
        let valid = [RunningSurfaceColumnBreakHints {
            header: vec![vec![5], vec![7], Vec::new()],
            first_header: vec![vec![10]],
            ..RunningSurfaceColumnBreakHints::default()
        }];
        assert_eq!(running_counts(&render(&valid)), (3, 0, 1, 0));

        let bad_sections = [valid[0].clone(), RunningSurfaceColumnBreakHints::default()];
        assert_eq!(running_counts(&render(&bad_sections)), (1, 2, 0, 1));

        let mut bad_header_shape = valid.clone();
        bad_header_shape[0].header.pop();
        assert_eq!(running_counts(&render(&bad_header_shape)), (1, 2, 1, 0));

        let mut bad_header_leaf = valid.clone();
        bad_header_leaf[0].header[0] = vec![5, 5];
        assert_eq!(running_counts(&render(&bad_header_leaf)), (2, 1, 1, 0));

        let mut bad_first_leaf = valid.clone();
        bad_first_leaf[0].first_header[0] = vec![99];
        assert_eq!(running_counts(&render(&bad_first_leaf)), (3, 0, 0, 1));
    }

    #[test]
    fn source_running_nested_table_hints_validate_slots_and_components_independently() {
        let nested_table = |label: &str| Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Paragraph(para(&format!("{label}\nX")))],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        };
        let running_table = |label: &str| Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![
                        Block::Paragraph(para(&format!("D{label}\nX"))),
                        Block::Table(nested_table(&format!("N{label}"))),
                    ],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        };
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("BODY"))],
            setup: DocSetup {
                header: vec![
                    Block::Paragraph(para("PREFIX")),
                    Block::Table(running_table("1")),
                    Block::Table(running_table("2")),
                ],
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let nested_hint = || TablePaginationHints {
            rows: vec![TableRowPaginationHint { cant_split: true }],
            cells: vec![vec![vec![Some(PaginationHint {
                keep_lines: true,
                ..PaginationHint::default()
            })]]],
            cell_line_spacing: vec![vec![vec![Some(LineSpacingHint::Exact(7.0))]]],
            cell_column_breaks: vec![vec![vec![vec![2]]]],
            nested: vec![vec![vec![None]]],
            cell_tabs: vec![vec![vec![vec![tab(
                18.0,
                TabAlignment::Right,
                TabLeader::Hyphen,
            )]]]],
        };
        let nested_tree = |hint| vec![vec![vec![None, Some(hint)]]];
        let direct_breaks = || vec![vec![vec![vec![2], Vec::new()]]];
        let valid_header = RunningTableLayoutHints {
            cell_column_breaks: vec![Vec::new(), direct_breaks(), direct_breaks()],
            nested_tables: vec![
                Vec::new(),
                nested_tree(nested_hint()),
                nested_tree(nested_hint()),
            ],
        };
        let render = |table_layout: &[RunningSurfaceTableLayoutHints]| {
            let rendered = render_body(
                &model,
                Some(SourceWriteHints {
                    gaps: &[None],
                    layouts: &[None],
                    separators: &[false],
                    rtl: &[false],
                    final_gap: None,
                    final_layout: None,
                    final_separator: false,
                    final_rtl: false,
                    running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                    running_line_spacing: &[],
                    running_pagination: &[],
                    running_tab_stops: &[],
                    running_table_cell_tab_stops: &[],
                    running_table_layout: table_layout,
                    running_column_break_offsets: &[],
                    note_payloads: &[],
                    paragraph_line_spacing: &[],
                    paragraph_pagination: &[],
                    paragraph_tab_stops: &[],
                    column_break_offsets: &[],
                    table_cell_column_break_offsets: &[],
                    table_row_pagination: &[],
                    table_cell_pagination: &[],
                    table_cell_line_spacing: &[],
                    table_nested_pagination: &[],
                    table_cell_tab_stops: &[],
                }),
            );
            generated_running_part(&rendered, "D1").1.to_string()
        };
        let counts = |xml: &str| {
            (
                xml.matches(r#"<w:br w:type="column"/>"#).count(),
                xml.matches("<w:br/>").count(),
                xml.matches("<w:cantSplit/>").count(),
                xml.matches("<w:keepLines/>").count(),
                xml.matches(r#"w:lineRule="exact""#).count(),
                xml.matches("<w:tabs>").count(),
            )
        };
        let valid = [RunningSurfaceTableLayoutHints {
            header: valid_header.clone(),
            ..RunningSurfaceTableLayoutHints::default()
        }];
        assert_eq!(counts(&render(&valid)), (4, 0, 2, 2, 2, 2));
        assert_eq!(counts(&render(&[])), (0, 4, 0, 0, 0, 0));

        let mut bad_break_outer = valid.clone();
        bad_break_outer[0].header.cell_column_breaks.pop();
        assert_eq!(counts(&render(&bad_break_outer)), (2, 2, 2, 2, 2, 2));

        let mut bad_nested_outer = valid.clone();
        bad_nested_outer[0].header.nested_tables.pop();
        assert_eq!(counts(&render(&bad_nested_outer)), (2, 2, 0, 0, 0, 0));

        let mut bad_first_break_slot = valid.clone();
        bad_first_break_slot[0].header.cell_column_breaks[1].clear();
        assert_eq!(counts(&render(&bad_first_break_slot)), (3, 1, 2, 2, 2, 2));

        let mut bad_first_tree_slot = valid.clone();
        bad_first_tree_slot[0].header.nested_tables[1][0][0][0] = Some(nested_hint());
        assert_eq!(counts(&render(&bad_first_tree_slot)), (3, 1, 1, 1, 1, 1));

        let mut bad_child_rows = valid.clone();
        bad_child_rows[0].header.nested_tables[1][0][0][1]
            .as_mut()
            .unwrap()
            .rows
            .clear();
        assert_eq!(counts(&render(&bad_child_rows)), (4, 0, 1, 2, 2, 2));

        let mut bad_child_break = valid.clone();
        bad_child_break[0].header.nested_tables[1][0][0][1]
            .as_mut()
            .unwrap()
            .cell_column_breaks[0][0][0] = vec![2, 2];
        assert_eq!(counts(&render(&bad_child_break)), (3, 1, 2, 2, 2, 2));
    }

    #[test]
    fn source_nested_table_hints_validate_trees_and_components_independently() {
        let nested_table = Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Paragraph(para("N\nX"))],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        };
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(para("P\nQ")), Block::Table(nested_table)],
                        ..Cell::default()
                    }],
                }],
                ..Table::default()
            })],
            ..DocModel::default()
        };
        let outer_rows = [vec![TableRowPaginationHint { cant_split: true }]];
        let outer_cells = [vec![vec![vec![
            Some(PaginationHint {
                keep_next: true,
                ..PaginationHint::default()
            }),
            None,
        ]]]];
        let outer_lines = [vec![vec![vec![Some(LineSpacingHint::Exact(5.0)), None]]]];
        let outer_breaks = [vec![vec![vec![vec![1], Vec::new()]]]];
        let outer_tabs = [vec![vec![vec![
            vec![tab(12.0, TabAlignment::Left, TabLeader::Dot)],
            Vec::new(),
        ]]]];
        let valid_nested = TablePaginationHints {
            rows: vec![TableRowPaginationHint { cant_split: true }],
            cells: vec![vec![vec![Some(PaginationHint {
                keep_lines: true,
                ..PaginationHint::default()
            })]]],
            cell_line_spacing: vec![vec![vec![Some(LineSpacingHint::Exact(7.0))]]],
            cell_column_breaks: vec![vec![vec![vec![1]]]],
            nested: vec![vec![vec![None]]],
            cell_tabs: vec![vec![vec![vec![tab(
                18.0,
                TabAlignment::Right,
                TabLeader::Hyphen,
            )]]]],
        };
        let tree_for = |nested| vec![vec![vec![None, Some(nested)]]];
        let render = |nested: &[TableCellNestedPaginationHints]| {
            String::from_utf8(
                render_body(
                    &model,
                    Some(SourceWriteHints {
                        gaps: &[None],
                        layouts: &[None],
                        separators: &[false],
                        rtl: &[false],
                        final_gap: None,
                        final_layout: None,
                        final_separator: false,
                        final_rtl: false,
                        running_surface_distances: &[RunningSurfaceDistanceHints::default()],
                        running_line_spacing: &[],
                        running_pagination: &[],
                        running_tab_stops: &[],
                        running_table_cell_tab_stops: &[],
                        running_table_layout: &[],
                        running_column_break_offsets: &[],
                        note_payloads: &[],
                        paragraph_line_spacing: &[],
                        paragraph_pagination: &[],
                        paragraph_tab_stops: &[],
                        column_break_offsets: &[],
                        table_cell_column_break_offsets: &outer_breaks,
                        table_row_pagination: &outer_rows,
                        table_cell_pagination: &outer_cells,
                        table_cell_line_spacing: &outer_lines,
                        table_nested_pagination: nested,
                        table_cell_tab_stops: &outer_tabs,
                    }),
                )
                .document_xml,
            )
            .unwrap()
        };
        let counts = |xml: &str| {
            (
                xml.matches("<w:cantSplit/>").count(),
                xml.matches("<w:keepNext/>").count(),
                xml.matches("<w:keepLines/>").count(),
                xml.matches(r#"w:lineRule="exact""#).count(),
                xml.matches("<w:tabs>").count(),
                xml.matches(r#"<w:br w:type="column"/>"#).count(),
                xml.matches("<w:br/>").count(),
            )
        };

        let valid_tree = tree_for(valid_nested.clone());
        assert_eq!(
            counts(&render(std::slice::from_ref(&valid_tree))),
            (2, 1, 1, 2, 2, 2, 0)
        );
        assert_eq!(counts(&render(&[])), (1, 1, 0, 1, 1, 1, 1));
        assert_eq!(counts(&render(&[Vec::new()])), (1, 1, 0, 1, 1, 1, 1));

        let mut wrong_kind = valid_tree.clone();
        wrong_kind[0][0][0] = Some(valid_nested.clone());
        assert_eq!(counts(&render(&[wrong_kind])), (1, 1, 0, 1, 1, 1, 1));

        let mut bad_rows = valid_nested.clone();
        bad_rows.rows.clear();
        assert_eq!(
            counts(&render(&[tree_for(bad_rows)])),
            (1, 1, 1, 2, 2, 2, 0)
        );

        let mut bad_cells = valid_nested.clone();
        bad_cells.cells.clear();
        assert_eq!(
            counts(&render(&[tree_for(bad_cells)])),
            (2, 1, 0, 2, 2, 2, 0)
        );

        let mut bad_lines = valid_nested.clone();
        bad_lines.cell_line_spacing.clear();
        assert_eq!(
            counts(&render(&[tree_for(bad_lines)])),
            (2, 1, 1, 1, 2, 2, 0)
        );

        let mut bad_tabs = valid_nested.clone();
        bad_tabs.cell_tabs.clear();
        assert_eq!(
            counts(&render(&[tree_for(bad_tabs)])),
            (2, 1, 1, 2, 1, 2, 0)
        );

        let mut bad_break_leaf = valid_nested;
        bad_break_leaf.cell_column_breaks[0][0][0] = vec![1, 1];
        assert_eq!(
            counts(&render(&[tree_for(bad_break_leaf)])),
            (2, 1, 1, 2, 2, 1, 1)
        );
    }

    /// Build a representative model, write it to `.docx`, read it back, and assert
    /// the structure survives the round-trip.
    #[test]
    fn round_trips_structure_through_docx() {
        let heading = Paragraph {
            props: ParaProps {
                heading_level: Some(2),
                outline_level: Some(1),
                align: Align::Center,
                ..ParaProps::default()
            },
            runs: vec![Run {
                text: "제목 둘".to_string(),
                ..Run::default()
            }],
        };
        let emphasized = Paragraph {
            props: ParaProps::default(),
            runs: vec![
                Run {
                    text: "굵게".to_string(),
                    props: CharProps {
                        bold: true,
                        ..CharProps::default()
                    },
                    ..Run::default()
                },
                Run {
                    text: " 보통 ".to_string(),
                    ..Run::default()
                },
                Run {
                    text: "기울임".to_string(),
                    props: CharProps {
                        italic: true,
                        ..CharProps::default()
                    },
                    ..Run::default()
                },
            ],
        };
        let ordered = Paragraph {
            props: ParaProps {
                list: Some(ListInfo {
                    level: 0,
                    ordered: true,
                    label: String::new(),
                    marker_font: None,
                }),
                ..ParaProps::default()
            },
            runs: vec![Run {
                text: "첫째 항목".to_string(),
                ..Run::default()
            }],
        };
        let bullet = Paragraph {
            props: ParaProps {
                list: Some(ListInfo {
                    level: 0,
                    ordered: false,
                    label: String::new(),
                    marker_font: None,
                }),
                ..ParaProps::default()
            },
            runs: vec![Run {
                text: "글머리 항목".to_string(),
                ..Run::default()
            }],
        };
        let link = Paragraph {
            props: ParaProps::default(),
            runs: vec![Run {
                text: "프로젝트 홈".to_string(),
                field: FieldRole::Hyperlink {
                    url: "https://example.com/".to_string(),
                },
                ..Run::default()
            }],
        };
        // 2x2 table: header row, a colspan-2 owner that vertically merges down.
        let table = Table {
            rows: vec![
                Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(para("머리글"))],
                        col_span: 2,
                        row_span: 2,
                        is_header: true,
                        ..Default::default()
                    }],
                },
                Row {
                    cells: vec![cell("a"), cell("b")],
                },
            ],
            header_rows: 1,
            ..Default::default()
        };
        // A genuinely valid 2×3 PNG (sig + IHDR + IDAT + IEND, correct CRCs) so the
        // round-trip proves a real, Office-openable image part — not just self-readable.
        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x36, 0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78,
            0xDA, 0x63, 0x60, 0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let image = Block::Image(Image {
            alt: None,
            bytes: Some(png.clone()),
            mime: Some("image/png".to_string()),
            ..Default::default()
        });

        let model = DocModel {
            blocks: vec![
                Block::Paragraph(heading),
                Block::Paragraph(emphasized),
                Block::Paragraph(ordered),
                Block::Paragraph(bullet),
                Block::Paragraph(link),
                Block::Table(table),
                image,
            ],
            ..DocModel::default()
        };

        let bytes = super::to_docx(&model);
        let doc = Document::open(&bytes).expect("written .docx must reopen");
        let m2 = doc.model();

        // Heading.
        let Block::Paragraph(h) = &m2.blocks[0] else {
            panic!("expected heading paragraph, got {:?}", m2.blocks[0]);
        };
        assert_eq!(h.props.heading_level, Some(2));
        assert_eq!(h.props.align, Align::Center);
        assert_eq!(h.text(), "제목 둘");

        // Emphasis runs.
        let Block::Paragraph(e) = &m2.blocks[1] else {
            panic!("para");
        };
        assert_eq!(e.text(), "굵게 보통 기울임");
        assert!(e.runs.iter().any(|r| r.props.bold && r.text == "굵게"));
        assert!(e.runs.iter().any(|r| r.props.italic && r.text == "기울임"));

        // Lists.
        let Block::Paragraph(o) = &m2.blocks[2] else {
            panic!("para");
        };
        assert_eq!(o.props.list.as_ref().map(|l| l.ordered), Some(true));
        let Block::Paragraph(b) = &m2.blocks[3] else {
            panic!("para");
        };
        assert_eq!(b.props.list.as_ref().map(|l| l.ordered), Some(false));

        // Hyperlink.
        let Block::Paragraph(l) = &m2.blocks[4] else {
            panic!("para");
        };
        assert!(matches!(
            l.runs.iter().find(|r| r.text == "프로젝트 홈").map(|r| &r.field),
            Some(FieldRole::Hyperlink { url }) if url == "https://example.com/"
        ));

        // Table with merges.
        let Block::Table(t) = &m2.blocks[5] else {
            panic!("expected table, got {:?}", m2.blocks[5]);
        };
        assert_eq!(t.header_rows, 1);
        assert_eq!(t.rows[0].cells[0].col_span, 2);
        assert_eq!(t.rows[0].cells[0].row_span, 2);
        assert_eq!(t.rows[0].cells[0].text(), "머리글");
        assert!(t.rows[0].cells[0].is_header);
        // Row 1's continuation cell was dropped, leaving the two body cells.
        assert_eq!(t.rows[1].cells.len(), 2);
        assert_eq!(t.rows[1].cells[1].text(), "b");

        // Image survives.
        let imgs = doc.images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].bytes.as_deref(), Some(&png[..]));
    }

    #[test]
    fn round_trips_rich_char_and_para_formatting() {
        use crate::model::{Color, Indent, Spacing};
        let run = Run {
            text: "빨강굵게".to_string(),
            props: CharProps {
                bold: true,
                color: Some(Color {
                    r: 0xFF,
                    g: 0,
                    b: 0,
                }),
                size_half_pt: Some(28),
                font: Some("맑은 고딕".to_string()),
                ..CharProps::default()
            },
            ..Run::default()
        };
        let para = Paragraph {
            props: ParaProps {
                spacing: Spacing {
                    before_pt: Some(12.0),
                    after_pt: Some(6.0),
                    line_pct: Some(1.5),
                },
                indent: Indent {
                    left_pt: Some(24.0),
                    ..Indent::default()
                },
                shading: Some(Color {
                    r: 0xEE,
                    g: 0xEE,
                    b: 0xEE,
                }),
                ..ParaProps::default()
            },
            runs: vec![run],
        };
        let model = DocModel {
            blocks: vec![Block::Paragraph(para)],
            ..DocModel::default()
        };
        let m2 = Document::open(&super::to_docx(&model)).unwrap().model();
        let Block::Paragraph(p) = &m2.blocks[0] else {
            panic!("para")
        };
        let rp = &p.runs[0].props;
        assert!(rp.bold);
        assert_eq!(
            rp.color,
            Some(Color {
                r: 0xFF,
                g: 0,
                b: 0
            })
        );
        assert_eq!(rp.size_half_pt, Some(28));
        assert_eq!(rp.font.as_deref(), Some("맑은 고딕"));
        assert_eq!(p.props.spacing.before_pt, Some(12.0));
        assert_eq!(p.props.spacing.after_pt, Some(6.0));
        assert_eq!(p.props.spacing.line_pct, Some(1.5));
        assert_eq!(p.props.indent.left_pt, Some(24.0));
        assert_eq!(
            p.props.shading,
            Some(Color {
                r: 0xEE,
                g: 0xEE,
                b: 0xEE
            })
        );
    }

    #[test]
    fn empty_model_writes_openable_docx() {
        let bytes = super::to_docx(&DocModel::default());
        let doc = Document::open(&bytes).expect("empty .docx must still open");
        assert!(doc.model().blocks.is_empty());
    }

    #[test]
    fn giant_span_table_stays_bounded() {
        // A hostile col_span/row_span must be clamped, not amplified into millions
        // of <w:gridCol>/cells.
        let model = DocModel {
            blocks: vec![Block::Table(Table {
                rows: vec![Row {
                    cells: vec![Cell {
                        blocks: vec![Block::Paragraph(para("x"))],
                        col_span: u16::MAX,
                        row_span: u16::MAX,
                        is_header: false,
                        ..Default::default()
                    }],
                }],
                header_rows: 0,
                ..Default::default()
            })],
            ..DocModel::default()
        };
        let bytes = super::to_docx(&model);
        assert!(
            bytes.len() < 1_000_000,
            "giant span amplified output to {} bytes",
            bytes.len()
        );
        assert!(Document::open(&bytes).is_ok());
    }

    /// A header/footer + page numbers emit the `header1.xml`/`footer1.xml` parts
    /// (with a `PAGE` field) and section references, and crucially do **not**
    /// corrupt the body: it still re-opens and reads back. (LibreOffice's headless
    /// converter cannot load *any* docx with a header — even canonical ones — so
    /// the body round-trip + the part bytes are the verifiable oracle.)
    #[test]
    fn emits_header_footer_and_page_numbers() {
        use crate::model::DocSetup;
        let model = DocModel {
            blocks: vec![Block::Paragraph(para("본문"))],
            setup: DocSetup {
                header: vec![Block::Paragraph(para("러닝 헤더"))],
                footer: vec![Block::Paragraph(para("푸터"))],
                page_numbers: true,
                ..DocSetup::default()
            },
            ..DocModel::default()
        };
        let bytes = super::to_docx(&model);
        let blob = String::from_utf8_lossy(&bytes);
        // The OPC zip stores part names uncompressed in the local headers.
        assert!(blob.contains("word/header1.xml"), "missing header part");
        assert!(blob.contains("word/footer1.xml"), "missing footer part");
        // Full round-trip: the reader now extracts the header/footer back, so the
        // body stays in main_text() and the running header/footer reach text().
        let doc = Document::open(&bytes).expect("doc with header/footer must open");
        assert_eq!(doc.main_text().trim(), "본문");
        let full = doc.text();
        assert!(full.contains("본문"), "body lost: {full:?}");
        assert!(full.contains("러닝 헤더"), "header not read back: {full:?}");
        assert!(full.contains("푸터"), "footer not read back: {full:?}");
    }
}
