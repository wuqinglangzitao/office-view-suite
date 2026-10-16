//! `.docx` body (`word/document.xml`) → [`Block`]s, by recursive descent over a
//! streaming [`quick_xml`] reader.
//!
//! Each `read_*` helper is entered just after its element's `Start` event and
//! consumes through the matching `End`. The invariant that keeps the loops simple
//! is: **every child `Start` is consumed by a sub-parser or [`skip_subtree`], and
//! `w:t` text is read by [`read_text`]** — so the only `End` that reaches a
//! parser's own loop is its own, and it can break on the first `End` it sees.
//! (`w:pPr`/`w:rPr`/`w:tcPr`/`w:trPr` flatten their simple children instead and
//! break on their *named* end.)

use std::collections::{HashMap, HashSet};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::fields::{
    computed_contextless_result, computed_run_symbol_char, ContextlessFieldState, TocEntry,
};
use super::numbering::Numbering;
use super::parse_rgb_hex_color;
use super::styles::{
    apply_paragraph_layout_child, ParagraphLayoutProps, RunProps, Styles, TableCellStyleRegions,
    TableRowStyleRegions, TableStyleCellProps,
};
use super::xml_text::{inline_marker_text, read_i64_text, read_text, skip_subtree};
use super::{
    attr_f32, attr_i32, attr_i64, attr_local, attr_local_trimmed, attr_u16, attr_u32, attr_u8,
    field_char_type, is_column_break_type, is_page_break_type, local, toggle_on,
};
use crate::annotation::{
    barcode_field_syntax, direct_ref_field_syntax, instruction_parts, legacy_form_field_syntax,
    merge_field_syntax, normalized_field_instruction, note_ref_field_syntax, opaque_field_syntax,
    page_ref_field_syntax, ref_field_syntax, toc_field_syntax, FieldKind,
};
use crate::model::{
    Align, AuthoredContentControl, Block, Cell, CellMargins, CharProps, Chart, Color, DocGrid,
    DocGridType, FieldRole, FieldUnsupportedReason, Image, Indent, LineSpacingHint, ListInfo,
    PageNumberFormat, PageSetup, PaginationHint, ParaProps, Paragraph, Row, Run, SectionBreakKind,
    SectionColumnHint, SectionColumnLayoutHints, SectionSetup, TabAlignment, TabStop, Table,
    TableBorderColors, TableBorderSide, TableBorderSizes, TableBorderStyle, TableBorderStyles,
    TableCellColumnBreakHints, TableCellLineSpacingHints, TableCellNestedPaginationHints,
    TableCellPaginationHints, TableCellTabStopHints, TablePaginationHints, TableRowPaginationHint,
    TextDirection, VCell, MAX_TAB_STOPS,
};
use crate::text;
use crate::CoreProperties;

/// Twips (1/20 pt) string → points.
fn twips_to_pt(s: &str) -> Option<f32> {
    s.trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| value / 20.0)
}

fn type_defaults_to_dxa(e: &BytesStart<'_>) -> bool {
    attr_local_trimmed(e, b"type")
        .as_deref()
        .is_none_or(|value| value == "dxa")
}

/// The borrowing reader produced by `Reader::from_str`.
type Xml<'a> = Reader<&'a [u8]>;

/// Hard cap on structural nesting depth (nested tables / run wrappers). Real
/// documents nest a handful of levels; pathological/fuzzed files (e.g. POI's
/// `deep-table-cell.docx`) nest thousands deep and would overflow the recursive
/// descent's stack — a process abort that breaks the panic-free contract. Past
/// this depth the subtree is skipped rather than recursed into.
const MAX_DEPTH: u32 = 128;
const MAX_TABLE_GRID_COLS: usize = 1024;
const MAX_UNEQUAL_SECTION_COLUMNS: usize = 64;
const MAX_SECTION_COLUMN_TWIPS: u32 = 31_680;
const PAGE_BREAK_MARKER: char = '\u{000C}';
const COLUMN_BREAK_MARKER: char = '\u{000B}';

#[derive(Default)]
pub(super) struct PaginationCapture {
    hints: Vec<PaginationHint>,
    line_spacing: Vec<Option<LineSpacingHint>>,
    tab_stops: Vec<Vec<TabStop>>,
    column_break_offsets: Vec<Vec<usize>>,
    table_row_pagination: Vec<Vec<TableRowPaginationHint>>,
    table_cell_pagination: Vec<TableCellPaginationHints>,
    table_cell_line_spacing: Vec<TableCellLineSpacingHints>,
    table_cell_column_breaks: Vec<TableCellColumnBreakHints>,
    table_nested_pagination: Vec<TableCellNestedPaginationHints>,
    table_cell_tab_stops: Vec<TableCellTabStopHints>,
    suspended: usize,
}

#[derive(Default)]
pub(super) struct SectionColumnCapture {
    gaps: Vec<Option<f32>>,
    layouts: Vec<Option<SectionColumnLayoutHints>>,
    separators: Vec<bool>,
    rtl: Vec<bool>,
    suspended: usize,
}

#[derive(Default)]
pub(super) struct BodySectionColumnHints {
    pub(super) gaps: Vec<Option<f32>>,
    pub(super) layouts: Vec<Option<SectionColumnLayoutHints>>,
    pub(super) separators: Vec<bool>,
    pub(super) rtl: Vec<bool>,
}

#[derive(Default)]
pub(super) struct BodyLayoutHints {
    pub(super) pagination: Vec<PaginationHint>,
    pub(super) line_spacing: Vec<Option<LineSpacingHint>>,
    pub(super) tab_stops: Vec<Vec<TabStop>>,
    pub(super) column_break_offsets: Vec<Vec<usize>>,
    pub(super) table_rows: Vec<Vec<TableRowPaginationHint>>,
    pub(super) table_cells: Vec<TableCellPaginationHints>,
    pub(super) table_cell_line_spacing: Vec<TableCellLineSpacingHints>,
    pub(super) table_cell_column_breaks: Vec<TableCellColumnBreakHints>,
    pub(super) table_nested: Vec<TableCellNestedPaginationHints>,
    pub(super) table_cell_tabs: Vec<TableCellTabStopHints>,
}

#[derive(Clone, Copy, Default)]
struct BlockSectionColumnHints<'a> {
    gap_pt: Option<f32>,
    layout: Option<&'a SectionColumnLayoutHints>,
    separator: bool,
    rtl: bool,
}

/// Resolved supplementary tables, passed down the descent.
pub(crate) struct Ctx<'a> {
    pub styles: &'a Styles,
    pub numbering: &'a Numbering,
    pub rels: &'a HashMap<String, (String, bool)>,
    pub media: &'a HashMap<String, Image>,
    pub charts: &'a HashMap<String, Chart>,
    pub ref_targets: &'a HashMap<String, String>,
    pub ref_position_context: &'a super::fields::RefPositionContext,
    pub ref_number_context: &'a super::fields::RefNumberContext,
    pub page_ref_context: &'a super::fields::PageRefContext,
    pub note_ref_context: &'a super::fields::NoteRefContext,
    pub section_context: &'a super::fields::SectionContext,
    pub style_ref_context: &'a super::fields::StyleRefContext,
    pub legacy_form_context: &'a super::fields::LegacyFormContext,
    pub table_formula_context: &'a super::fields::TableFormulaContext,
    pub toc_entries: &'a [TocEntry],
    pub bookmark_names: &'a HashSet<String>,
    pub core_properties: &'a CoreProperties,
    pub custom_properties: &'a HashMap<String, String>,
    pub document_variables: &'a HashMap<String, String>,
    pub extended_properties: &'a HashMap<String, String>,
    pub file_size_bytes: Option<usize>,
    pub preserve_note_local_fields: bool,
    pub ref_field_cursor: std::cell::RefCell<usize>,
    pub page_field_cursor: std::cell::RefCell<usize>,
    pub last_page_field_unsupported_display_format: std::cell::RefCell<Option<bool>>,
    pub page_ref_field_cursor: std::cell::RefCell<usize>,
    pub note_ref_field_cursor: std::cell::RefCell<usize>,
    pub section_field_cursor: std::cell::RefCell<usize>,
    pub style_ref_field_cursor: std::cell::RefCell<usize>,
    pub form_field_cursor: std::cell::RefCell<usize>,
    pub formula_field_cursor: std::cell::RefCell<usize>,
    pub last_formula_field_used_table_context: std::cell::Cell<bool>,
    pub sequence_counters: std::cell::RefCell<HashMap<String, i64>>,
    pub sequence_heading_counts: std::cell::RefCell<[u32; 9]>,
    pub sequence_heading_scopes: std::cell::RefCell<HashMap<(String, u8), u32>>,
    pub autonum_counter: std::cell::RefCell<i64>,
    pub listnum_counter: std::cell::RefCell<i64>,
    pub field_bookmarks: std::cell::RefCell<HashMap<String, String>>,
    /// Live per-`numId` level counters for autonumber labels, advanced in document
    /// order as list paragraphs are finalized (interior-mutable: parsing is
    /// single-threaded and `finalize_paragraph` runs in reading order).
    pub counters: std::cell::RefCell<HashMap<String, [u32; 9]>>,
    pub paragraph_charts: std::cell::RefCell<Vec<Vec<Chart>>>,
    pub(super) section_column_capture: std::cell::RefCell<Option<SectionColumnCapture>>,
    pub(super) pagination_capture: std::cell::RefCell<Option<PaginationCapture>>,
}

impl Ctx<'_> {
    fn begin_paragraph_charts(&self) {
        self.paragraph_charts.borrow_mut().push(Vec::new());
    }

    fn push_paragraph_chart(&self, chart: Chart) {
        if let Some(charts) = self.paragraph_charts.borrow_mut().last_mut() {
            charts.push(chart);
        }
    }

    fn end_paragraph_charts(&self) -> Vec<Chart> {
        self.paragraph_charts.borrow_mut().pop().unwrap_or_default()
    }

    pub(crate) fn begin_section_column_capture(&self) {
        *self.section_column_capture.borrow_mut() = Some(SectionColumnCapture::default());
    }

    pub(crate) fn take_section_column_hints(&self) -> BodySectionColumnHints {
        self.section_column_capture
            .borrow_mut()
            .take()
            .map(|capture| BodySectionColumnHints {
                gaps: capture.gaps,
                layouts: capture.layouts,
                separators: capture.separators,
                rtl: capture.rtl,
            })
            .unwrap_or_default()
    }

    pub(crate) fn begin_pagination_capture(&self) {
        *self.pagination_capture.borrow_mut() = Some(PaginationCapture::default());
    }

    pub(crate) fn take_layout_hints(&self) -> BodyLayoutHints {
        self.pagination_capture
            .borrow_mut()
            .take()
            .map(|capture| BodyLayoutHints {
                pagination: capture.hints,
                line_spacing: capture.line_spacing,
                tab_stops: capture.tab_stops,
                column_break_offsets: capture.column_break_offsets,
                table_rows: capture.table_row_pagination,
                table_cells: capture.table_cell_pagination,
                table_cell_line_spacing: capture.table_cell_line_spacing,
                table_cell_column_breaks: capture.table_cell_column_breaks,
                table_nested: capture.table_nested_pagination,
                table_cell_tabs: capture.table_cell_tab_stops,
            })
            .unwrap_or_default()
    }

    fn suspend_block_captures(&self) {
        if let Some(capture) = self.section_column_capture.borrow_mut().as_mut() {
            capture.suspended = capture.suspended.saturating_add(1);
        }
        if let Some(capture) = self.pagination_capture.borrow_mut().as_mut() {
            capture.suspended = capture.suspended.saturating_add(1);
        }
    }

    fn resume_block_captures(&self) {
        if let Some(capture) = self.section_column_capture.borrow_mut().as_mut() {
            capture.suspended = capture.suspended.saturating_sub(1);
        }
        if let Some(capture) = self.pagination_capture.borrow_mut().as_mut() {
            capture.suspended = capture.suspended.saturating_sub(1);
        }
    }

    fn capture_block_hints(
        &self,
        hint: PaginationHint,
        line_spacing: Option<LineSpacingHint>,
        _tab_stops: &[TabStop],
        section_columns: BlockSectionColumnHints<'_>,
        _column_break_offsets: &[usize],
    ) {
        if let Some(capture) = self.section_column_capture.borrow_mut().as_mut() {
            if capture.suspended == 0 {
                capture.gaps.push(section_columns.gap_pt);
                capture.layouts.push(section_columns.layout.cloned());
                capture.separators.push(section_columns.separator);
                capture.rtl.push(section_columns.rtl);
            }
        }
        if let Some(capture) = self.pagination_capture.borrow_mut().as_mut() {
            if capture.suspended == 0 {
                capture.hints.push(hint);
                capture.line_spacing.push(line_spacing);
                capture.table_row_pagination.push(Vec::new());
                capture.table_cell_pagination.push(Vec::new());
                capture.table_cell_line_spacing.push(Vec::new());
                capture.table_cell_column_breaks.push(Vec::new());
                capture.tab_stops.push(_tab_stops.to_vec());
                capture.table_cell_tab_stops.push(Vec::new());
                capture
                    .column_break_offsets
                    .push(_column_break_offsets.to_vec());
                capture.table_nested_pagination.push(Vec::new());
            }
        }
    }

    fn capture_table_block_hints(&self, table: &TablePaginationHints) {
        if let Some(capture) = self.section_column_capture.borrow_mut().as_mut() {
            if capture.suspended == 0 {
                capture.gaps.push(None);
                capture.layouts.push(None);
                capture.separators.push(false);
                capture.rtl.push(false);
            }
        }
        if let Some(capture) = self.pagination_capture.borrow_mut().as_mut() {
            if capture.suspended == 0 {
                capture.hints.push(PaginationHint::default());
                capture.line_spacing.push(None);
                capture.table_row_pagination.push(table.rows.clone());
                capture.table_cell_pagination.push(table.cells.clone());
                capture
                    .table_cell_line_spacing
                    .push(table.cell_line_spacing.clone());
                capture
                    .table_cell_column_breaks
                    .push(table.cell_column_breaks.clone());
                capture.tab_stops.push(Vec::new());
                capture.table_cell_tab_stops.push(table.cell_tabs.clone());
                capture.column_break_offsets.push(Vec::new());
                capture.table_nested_pagination.push(table.nested.clone());
            }
        }
    }

    fn capture_paragraph_blocks(&self, data: &ParagraphBlockData) {
        for (index, block) in data.blocks.iter().enumerate() {
            let break_offsets = data
                .column_break_offsets
                .get(index)
                .map_or(&[][..], Vec::as_slice);
            if matches!(block, Block::Paragraph(_)) {
                self.capture_block_hints(
                    data.pagination,
                    data.line_spacing,
                    &data.tab_stops,
                    BlockSectionColumnHints::default(),
                    break_offsets,
                );
            } else {
                let column_gap = matches!(block, Block::SectionBreak(_))
                    .then_some(data.section_column_gap_pt)
                    .flatten();
                let column_layout = matches!(block, Block::SectionBreak(_))
                    .then_some(data.section_column_layout.as_ref())
                    .flatten();
                let column_separator =
                    matches!(block, Block::SectionBreak(_)) && data.section_column_separator;
                let column_rtl = matches!(block, Block::SectionBreak(_)) && data.section_column_rtl;
                self.capture_block_hints(
                    PaginationHint::default(),
                    None,
                    &[],
                    BlockSectionColumnHints {
                        gap_pt: column_gap,
                        layout: column_layout,
                        separator: column_separator,
                        rtl: column_rtl,
                    },
                    &[],
                );
            }
        }
    }
}

/// Parse `word/document.xml` into block-level nodes.
pub(crate) fn parse_document(xml: &str, ctx: &Ctx<'_>) -> Vec<Block> {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"body" => {
                return read_blocks(&mut r, ctx, 0);
            }
            Ok(Event::Eof) | Err(_) => return Vec::new(),
            _ => {}
        }
    }
}

/// A header/footer reference declared by a body `sectPr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeaderFooterRef {
    /// Relationship id from `r:id`.
    pub rel_id: String,
    /// WordprocessingML reference type: `default`, `first`, or `even`.
    pub type_name: String,
}

/// Header/footer references declared by one body `sectPr`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HeaderFooterRefs {
    pub headers: Vec<HeaderFooterRef>,
    pub footers: Vec<HeaderFooterRef>,
    pub header_distance_twips: Option<u32>,
    pub footer_distance_twips: Option<u32>,
}

/// Scan `word/document.xml` for every `<w:headerReference>` / `<w:footerReference>`
/// relationship id and reference type (in document order, across all `w:sectPr`).
/// Returns `(header refs, footer refs)`; the caller resolves and de-duplicates
/// them.
#[cfg(test)]
pub(crate) fn scan_hf_refs(xml: &str) -> (Vec<HeaderFooterRef>, Vec<HeaderFooterRef>) {
    let sections = scan_hf_ref_sections(xml);
    let mut headers = Vec::new();
    let mut footers = Vec::new();
    for section in sections {
        headers.extend(section.headers);
        footers.extend(section.footers);
    }
    (headers, footers)
}

/// Scan `word/document.xml` for header/footer references grouped by each
/// `sectPr` in document order. Paragraph-level groups correspond to emitted
/// `Block::SectionBreak` nodes; the trailing body-level group describes the
/// final document setup.
pub(crate) fn scan_hf_ref_sections(xml: &str) -> Vec<HeaderFooterRefs> {
    scan_hf_ref_sections_with_view(xml, HeaderFooterRevisionView::Accepted)
}

pub(crate) fn scan_hf_ref_sections_for_revision_reject(xml: &str) -> Vec<HeaderFooterRefs> {
    scan_hf_ref_sections_with_view(xml, HeaderFooterRevisionView::Rejected)
}

#[derive(Clone, Copy)]
enum HeaderFooterRevisionView {
    Accepted,
    Rejected,
}

fn scan_hf_ref_sections_with_view(
    xml: &str,
    view: HeaderFooterRevisionView,
) -> Vec<HeaderFooterRefs> {
    let mut r = Reader::from_str(xml);
    let mut sections = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if skip_header_footer_revision_subtree(&e, view) => {
                skip_subtree(&mut r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                sections.push(read_hf_ref_section(&mut r));
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                sections.push(HeaderFooterRefs::default());
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    sections
}

fn skip_header_footer_revision_subtree(e: &BytesStart<'_>, view: HeaderFooterRevisionView) -> bool {
    let qname = e.name();
    let name = local(qname.as_ref());
    let hidden_content = match view {
        HeaderFooterRevisionView::Accepted => matches!(name, b"del" | b"moveFrom"),
        HeaderFooterRevisionView::Rejected => matches!(name, b"ins" | b"moveTo"),
    };
    hidden_content
        || matches!(
            name,
            b"pPrChange"
                | b"rPrChange"
                | b"tblPrChange"
                | b"trPrChange"
                | b"tcPrChange"
                | b"sectPrChange"
        )
}

fn read_hf_ref_section(r: &mut Xml<'_>) -> HeaderFooterRefs {
    let mut refs = HeaderFooterRefs::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_hf_ref_alternate_content(r, &mut refs);
            }
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"headerReference" | b"footerReference" => {
                    record_header_footer_ref(&mut refs, &e);
                    skip_subtree(r);
                }
                b"pgMar" => {
                    record_header_footer_distances(&mut refs, &e);
                    skip_subtree(r);
                }
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"headerReference" | b"footerReference" => record_header_footer_ref(&mut refs, &e),
                b"pgMar" => record_header_footer_distances(&mut refs, &e),
                _ => {}
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    refs
}

fn read_hf_ref_alternate_content(r: &mut Xml<'_>, refs: &mut HeaderFooterRefs) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_hf_ref_alternate_content_branch(r, refs, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_hf_ref_alternate_content_branch(
    r: &mut Xml<'_>,
    refs: &mut HeaderFooterRefs,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_hf_ref_alternate_content(r, refs);
            }
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"headerReference" | b"footerReference" => {
                    record_header_footer_ref(refs, &e);
                    skip_subtree(r);
                }
                b"pgMar" => {
                    record_header_footer_distances(refs, &e);
                    skip_subtree(r);
                }
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"headerReference" | b"footerReference" => record_header_footer_ref(refs, &e),
                b"pgMar" => record_header_footer_distances(refs, &e),
                _ => {}
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn record_header_footer_ref(refs: &mut HeaderFooterRefs, e: &BytesStart<'_>) {
    let Some(reference) = header_footer_ref(e) else {
        return;
    };
    match local(e.name().as_ref()) {
        b"headerReference" => refs.headers.push(reference),
        b"footerReference" => refs.footers.push(reference),
        _ => {}
    }
}

fn record_header_footer_distances(refs: &mut HeaderFooterRefs, e: &BytesStart<'_>) {
    if let Some(value) = attr_u32(e, b"header") {
        refs.header_distance_twips = Some(value);
    }
    if let Some(value) = attr_u32(e, b"footer") {
        refs.footer_distance_twips = Some(value);
    }
}

fn header_footer_ref(e: &BytesStart<'_>) -> Option<HeaderFooterRef> {
    attr_local_trimmed(e, b"id").map(|rel_id| HeaderFooterRef {
        rel_id,
        type_name: attr_local_trimmed(e, b"type").unwrap_or_else(|| "default".to_string()),
    })
}

/// Scan the body's section properties for page geometry (`<w:pgSz>` size +
/// orientation, `<w:pgMar>` left margin) → [`crate::model::PageSetup`]. Uses the
/// last `sectPr` (the final/primary section). Falls back to the A4 default when
/// absent. Twips (1/20 pt) → points.
pub(crate) fn scan_page_setup(xml: &str) -> PageSetup {
    let mut r = Reader::from_str(xml);
    let mut page = PageSetup::default();
    let mut found = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                scan_page_setup_alternate_content(&mut r, &mut page, &mut found);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if apply_page_setup_child(&mut page, &e) => {
                found = true;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if found {
        page
    } else {
        PageSetup::default()
    }
}

fn scan_page_setup_alternate_content(r: &mut Xml<'_>, page: &mut PageSetup, found: &mut bool) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        scan_page_setup_alternate_content_branch(r, page, found, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn scan_page_setup_alternate_content_branch(
    r: &mut Xml<'_>,
    page: &mut PageSetup,
    found: &mut bool,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                scan_page_setup_alternate_content(r, page, found);
            }
            Ok(Event::Start(e)) => {
                if apply_page_setup_child(page, &e) {
                    *found = true;
                } else {
                    skip_subtree(r);
                }
            }
            Ok(Event::Empty(e)) if apply_page_setup_child(page, &e) => {
                *found = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_page_setup_child(page: &mut PageSetup, e: &BytesStart<'_>) -> bool {
    match local(e.name().as_ref()) {
        b"pgSz" => {
            if let Some(size) = section_page_size(e) {
                apply_section_page_size(page, size);
                true
            } else {
                false
            }
        }
        b"pgMar" => {
            let margins = section_page_margins(e);
            if section_page_margins_present(margins) {
                apply_section_page_margins(page, margins);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

fn section_page_size(e: &BytesStart<'_>) -> Option<(f32, f32, bool)> {
    let width = attr_local(e, b"w").and_then(|value| twips_to_pt(&value))?;
    let height = attr_local(e, b"h").and_then(|value| twips_to_pt(&value))?;
    let landscape = attr_local(e, b"orient").is_some_and(|value| value.trim() == "landscape");
    Some((width, height, landscape))
}

fn apply_section_page_size(page: &mut PageSetup, (width, height, landscape): (f32, f32, bool)) {
    page.width_pt = width;
    page.height_pt = height;
    page.landscape = landscape;
}

fn section_page_margins(
    e: &BytesStart<'_>,
) -> (Option<f32>, Option<f32>, Option<f32>, Option<f32>) {
    (
        attr_local(e, b"left").and_then(|value| twips_to_pt(&value)),
        attr_local(e, b"right").and_then(|value| twips_to_pt(&value)),
        attr_local(e, b"top").and_then(|value| twips_to_pt(&value)),
        attr_local(e, b"bottom").and_then(|value| twips_to_pt(&value)),
    )
}

fn section_page_margins_present(
    (left, right, top, bottom): (Option<f32>, Option<f32>, Option<f32>, Option<f32>),
) -> bool {
    left.or(right).or(top).or(bottom).is_some()
}

fn apply_section_page_margins(
    page: &mut PageSetup,
    (left, right, top, bottom): (Option<f32>, Option<f32>, Option<f32>, Option<f32>),
) {
    if let Some(left) = left {
        page.margin_pt = left;
    }
    page.margin_left_pt = left;
    page.margin_right_pt = right;
    page.margin_top_pt = top;
    page.margin_bottom_pt = bottom;
}

#[derive(Default)]
pub(super) struct FinalSectionColumnHints {
    pub(super) gap_pt: Option<f32>,
    pub(super) layout: Option<SectionColumnLayoutHints>,
    pub(super) separator: bool,
    pub(super) rtl: bool,
}

pub(crate) fn scan_final_section_column_hints(xml: &str) -> FinalSectionColumnHints {
    let mut r = Reader::from_str(xml);
    let mut hints = FinalSectionColumnHints::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                let section = read_sect_pr(&mut r, 0);
                hints = FinalSectionColumnHints {
                    gap_pt: section.column_gap_pt,
                    layout: section.column_layout,
                    separator: section.column_separator,
                    rtl: section.column_rtl,
                };
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                hints = FinalSectionColumnHints::default();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    hints
}

/// Scan the final/body section properties for text column count.
pub(crate) fn scan_section_columns(xml: &str) -> Option<u16> {
    let mut r = Reader::from_str(xml);
    let mut columns = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                columns = read_section_columns(&mut r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                columns = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    columns
}

/// Scan the final/body section properties for text flow direction.
pub(crate) fn scan_section_text_direction(xml: &str) -> Option<TextDirection> {
    let mut r = Reader::from_str(xml);
    let mut text_direction = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                text_direction = read_section_text_direction(&mut r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                text_direction = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text_direction
}

/// Scan the final/body section properties for document grid settings.
pub(crate) fn scan_section_doc_grid(xml: &str) -> Option<DocGrid> {
    let mut r = Reader::from_str(xml);
    let mut doc_grid = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                doc_grid = read_section_doc_grid(&mut r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                doc_grid = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    doc_grid
}

/// Scan the final/body section properties for explicit first-page section behavior.
pub(crate) fn scan_section_title_page(xml: &str) -> bool {
    let mut r = Reader::from_str(xml);
    let mut title_page = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                title_page = read_section_title_page(&mut r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                title_page = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    title_page
}

/// Scan the final/body section properties for a displayed page-number restart.
pub(crate) fn scan_page_number_start(xml: &str) -> Option<u32> {
    let mut r = Reader::from_str(xml);
    let mut page_number_start = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                page_number_start = read_section_page_number_start(&mut r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                page_number_start = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_start
}

/// Scan the final/body section properties for a displayed page-number format.
pub(crate) fn scan_page_number_format(xml: &str) -> Option<PageNumberFormat> {
    let mut r = Reader::from_str(xml);
    let mut page_number_format = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPr" => {
                page_number_format = read_section_page_number_format(&mut r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sectPr" => {
                page_number_format = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_format
}

fn read_section_columns(r: &mut Xml<'_>) -> Option<u16> {
    read_section_column_properties(r).count
}

#[derive(Default)]
struct ParsedSectionColumns {
    count: Option<u16>,
    gap_pt: Option<f32>,
    layout: Option<SectionColumnLayoutHints>,
    separator: bool,
}

fn read_section_column_properties(r: &mut Xml<'_>) -> ParsedSectionColumns {
    let mut columns = ParsedSectionColumns::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                let alternate = read_section_setup_alternate_content(r);
                if let Some(value) = alternate.columns {
                    columns.count = value;
                }
                if let Some(value) = alternate.column_gap_pt {
                    columns.gap_pt = value;
                }
                if let Some(value) = alternate.column_layout {
                    columns.layout = value;
                }
                if let Some(value) = alternate.column_separator {
                    columns.separator = value;
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"cols" => {
                columns = read_section_columns_element(r, &e);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"cols" => {
                columns = section_columns_from_attrs(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    columns
}

fn section_columns_from_attrs(e: &BytesStart<'_>) -> ParsedSectionColumns {
    let separator = section_column_separator(e);
    if explicitly_unequal_columns(e) {
        return ParsedSectionColumns {
            separator,
            ..ParsedSectionColumns::default()
        };
    }
    ParsedSectionColumns {
        count: section_columns(e),
        gap_pt: section_column_gap_pt(e),
        layout: None,
        separator,
    }
}

fn section_column_separator(e: &BytesStart<'_>) -> bool {
    attr_local(e, b"sep").is_some_and(|value| toggle_on(Some(value)))
}

fn explicitly_unequal_columns(e: &BytesStart<'_>) -> bool {
    attr_local_trimmed(e, b"equalWidth").is_some_and(|value| !toggle_on(Some(value)))
}

fn read_section_columns_element(r: &mut Xml<'_>, e: &BytesStart<'_>) -> ParsedSectionColumns {
    if !explicitly_unequal_columns(e) {
        let columns = section_columns_from_attrs(e);
        skip_subtree(r);
        return columns;
    }

    let separator = section_column_separator(e);
    let mut values = Vec::new();
    let mut valid = true;
    loop {
        match r.read_event() {
            Ok(Event::Start(child)) if local(child.name().as_ref()) == b"col" => {
                if let Some(column) = section_column_hint(&child) {
                    if values.len() < MAX_UNEQUAL_SECTION_COLUMNS {
                        values.push(column);
                    } else {
                        valid = false;
                    }
                } else {
                    valid = false;
                }
                skip_subtree(r);
            }
            Ok(Event::Empty(child)) if local(child.name().as_ref()) == b"col" => {
                if let Some(column) = section_column_hint(&child) {
                    if values.len() < MAX_UNEQUAL_SECTION_COLUMNS {
                        values.push(column);
                    } else {
                        valid = false;
                    }
                } else {
                    valid = false;
                }
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(end)) if local(end.name().as_ref()) == b"cols" => break,
            Ok(Event::Eof) | Err(_) => {
                valid = false;
                break;
            }
            _ => {}
        }
    }
    if !valid || values.is_empty() {
        return ParsedSectionColumns {
            separator,
            ..ParsedSectionColumns::default()
        };
    }
    ParsedSectionColumns {
        count: u16::try_from(values.len()).ok(),
        gap_pt: None,
        layout: Some(SectionColumnLayoutHints { columns: values }),
        separator,
    }
}

fn section_column_hint(e: &BytesStart<'_>) -> Option<SectionColumnHint> {
    let width_twips = attr_u32(e, b"w")?;
    if width_twips == 0 || width_twips > MAX_SECTION_COLUMN_TWIPS {
        return None;
    }
    let space_twips = match attr_local_trimmed(e, b"space") {
        Some(_) => attr_u32(e, b"space")?,
        None => 0,
    };
    if space_twips > MAX_SECTION_COLUMN_TWIPS {
        return None;
    }
    Some(SectionColumnHint {
        width_pt: width_twips as f32 / 20.0,
        space_after_pt: space_twips as f32 / 20.0,
    })
}

fn section_columns(e: &BytesStart<'_>) -> Option<u16> {
    attr_u16(e, b"num").map(|value| value.max(1))
}

fn section_column_gap_pt(e: &BytesStart<'_>) -> Option<f32> {
    if !toggle_on(attr_local(e, b"equalWidth")) || section_columns(e)? < 2 {
        return None;
    }
    attr_local(e, b"space")
        .and_then(|value| twips_to_pt(&value))
        .filter(|value| *value >= 0.0)
}

fn read_section_text_direction(r: &mut Xml<'_>) -> Option<TextDirection> {
    let mut text_direction = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_section_setup_alternate_content(r).text_direction {
                    text_direction = value;
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"textDirection" =>
            {
                text_direction = section_text_direction(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text_direction
}

fn section_text_direction(e: &BytesStart<'_>) -> Option<TextDirection> {
    attr_local(e, b"val").and_then(|value| TextDirection::from_wml_value(&value))
}

fn doc_grid_from_attrs(e: &BytesStart<'_>) -> Option<DocGrid> {
    let grid_type = attr_local(e, b"type")
        .and_then(|value| DocGridType::from_wml_value(&value))
        .unwrap_or(DocGridType::Default);
    let line_pitch = attr_u32(e, b"linePitch");
    let character_space = attr_u32(e, b"charSpace");
    Some(DocGrid {
        grid_type,
        line_pitch,
        character_space,
    })
}

fn read_section_doc_grid(r: &mut Xml<'_>) -> Option<DocGrid> {
    let mut doc_grid = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_section_setup_alternate_content(r).doc_grid {
                    doc_grid = value;
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"docGrid" => {
                doc_grid = doc_grid_from_attrs(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    doc_grid
}

// The AlternateContent arm keeps its nested `if` deliberately: collapsing the
// `read_section_setup_alternate_content(r).title_page` check into the arm guard
// would let a non-title-page AlternateContent fall through to the later
// `Ok(Event::Start(_)) => skip_subtree(r)` arm, double-consuming the reader.
fn read_section_title_page(r: &mut Xml<'_>) -> bool {
    let mut title_page = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                title_page |= read_section_setup_alternate_content(r).title_page;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"titlePg" => {
                title_page = true;
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    title_page
}

#[derive(Default)]
struct SectionSetupScan {
    columns: Option<Option<u16>>,
    column_gap_pt: Option<Option<f32>>,
    column_layout: Option<Option<SectionColumnLayoutHints>>,
    column_separator: Option<bool>,
    column_rtl: Option<bool>,
    text_direction: Option<Option<TextDirection>>,
    doc_grid: Option<Option<DocGrid>>,
    title_page: bool,
}

fn read_section_setup_alternate_content(r: &mut Xml<'_>) -> SectionSetupScan {
    let mut setup = SectionSetupScan::default();
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        setup = read_section_setup_alternate_content_branch(r, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if !took && matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    setup
}

fn read_section_setup_alternate_content_branch(r: &mut Xml<'_>, branch: &[u8]) -> SectionSetupScan {
    let mut setup = SectionSetupScan::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                merge_section_setup_scan(&mut setup, read_section_setup_alternate_content(r));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"cols" => {
                record_section_columns_scan(&mut setup, read_section_columns_element(r, &e));
            }
            Ok(Event::Start(e)) if !record_section_setup_child(&mut setup, &e) => {
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => {
                record_section_setup_child(&mut setup, &e);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    setup
}

fn merge_section_setup_scan(target: &mut SectionSetupScan, source: SectionSetupScan) {
    if source.columns.is_some() {
        target.columns = source.columns;
    }
    if source.column_gap_pt.is_some() {
        target.column_gap_pt = source.column_gap_pt;
    }
    if source.column_layout.is_some() {
        target.column_layout = source.column_layout;
    }
    if source.column_separator.is_some() {
        target.column_separator = source.column_separator;
    }
    if source.column_rtl.is_some() {
        target.column_rtl = source.column_rtl;
    }
    if source.text_direction.is_some() {
        target.text_direction = source.text_direction;
    }
    if source.doc_grid.is_some() {
        target.doc_grid = source.doc_grid;
    }
    target.title_page |= source.title_page;
}

fn record_section_setup_child(setup: &mut SectionSetupScan, e: &BytesStart<'_>) -> bool {
    match local(e.name().as_ref()) {
        b"cols" => {
            record_section_columns_scan(setup, section_columns_from_attrs(e));
            true
        }
        b"textDirection" => {
            setup.text_direction = Some(section_text_direction(e));
            true
        }
        b"bidi" => {
            setup.column_rtl = Some(toggle_on(attr_local(e, b"val")));
            true
        }
        b"docGrid" => {
            setup.doc_grid = Some(doc_grid_from_attrs(e));
            true
        }
        b"titlePg" => {
            setup.title_page = true;
            true
        }
        _ => false,
    }
}

fn record_section_columns_scan(setup: &mut SectionSetupScan, columns: ParsedSectionColumns) {
    setup.columns = Some(columns.count);
    setup.column_gap_pt = Some(columns.gap_pt);
    setup.column_layout = Some(columns.layout);
    setup.column_separator = Some(columns.separator);
}

fn read_section_page_number_start(r: &mut Xml<'_>) -> Option<u32> {
    let mut page_number_start = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                page_number_start = read_section_page_number_start_alternate_content(r);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"pgNumType" =>
            {
                page_number_start = section_page_number_start(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_start
}

fn read_section_page_number_start_alternate_content(r: &mut Xml<'_>) -> Option<u32> {
    let mut page_number_start = None;
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        page_number_start =
                            read_section_page_number_start_alternate_content_branch(r, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_start
}

fn read_section_page_number_start_alternate_content_branch(
    r: &mut Xml<'_>,
    branch: &[u8],
) -> Option<u32> {
    let mut page_number_start = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                page_number_start = read_section_page_number_start_alternate_content(r);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"pgNumType" =>
            {
                page_number_start = section_page_number_start(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_start
}

fn read_section_page_number_format(r: &mut Xml<'_>) -> Option<PageNumberFormat> {
    let mut page_number_format = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                page_number_format = read_section_page_number_format_alternate_content(r);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"pgNumType" =>
            {
                page_number_format = section_page_number_format(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_format
}

fn read_section_page_number_format_alternate_content(r: &mut Xml<'_>) -> Option<PageNumberFormat> {
    let mut page_number_format = None;
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        page_number_format =
                            read_section_page_number_format_alternate_content_branch(r, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_format
}

fn read_section_page_number_format_alternate_content_branch(
    r: &mut Xml<'_>,
    branch: &[u8],
) -> Option<PageNumberFormat> {
    let mut page_number_format = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                page_number_format = read_section_page_number_format_alternate_content(r);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"pgNumType" =>
            {
                page_number_format = section_page_number_format(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    page_number_format
}

fn section_page_number_start(e: &BytesStart<'_>) -> Option<u32> {
    attr_u32(e, b"start").map(|value| value.max(1))
}

fn section_page_number_format(e: &BytesStart<'_>) -> Option<PageNumberFormat> {
    attr_local(e, b"fmt").and_then(|value| PageNumberFormat::from_wml_value(&value))
}

/// Parse a `word/headerN.xml` / `footerN.xml` part (root `<w:hdr>` / `<w:ftr>`)
/// into block-level nodes, reusing the same grammar as the body.
pub(crate) fn parse_hdrftr(xml: &str, ctx: &Ctx<'_>) -> Vec<Block> {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if matches!(local(e.name().as_ref()), b"hdr" | b"ftr") => {
                return read_blocks(&mut r, ctx, 0);
            }
            Ok(Event::Eof) | Err(_) => return Vec::new(),
            _ => {}
        }
    }
}

/// Parse `word/footnotes.xml` / `endnotes.xml`: the real notes' block content,
/// skipping the `separator`/`continuationSeparator`/`continuationNotice`
/// boilerplate notes. `tag` is `b"footnote"` or `b"endnote"`.
#[cfg(test)]
pub(crate) fn parse_notes(xml: &str, ctx: &Ctx<'_>, tag: &[u8]) -> Vec<Block> {
    parse_note_entries(xml, ctx, tag)
        .into_iter()
        .flat_map(|(_, blocks)| blocks)
        .collect()
}

/// Parse `word/footnotes.xml` / `endnotes.xml` into individual real note
/// entries. Each entry keeps the OOXML note id plus the block content parsed
/// with the same grammar as the flattened note reader.
pub(crate) fn parse_note_entries(
    xml: &str,
    ctx: &Ctx<'_>,
    tag: &[u8],
) -> Vec<(String, Vec<Block>)> {
    let mut r = Reader::from_str(xml);
    let mut entries = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_note_entries_alternate_content(&mut r, ctx, tag, &mut entries);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == tag => {
                if let Some(entry) = read_note_entry(&mut r, ctx, &e) {
                    entries.push(entry);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    entries
}

fn read_note_entries_alternate_content(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    tag: &[u8],
    entries: &mut Vec<(String, Vec<Block>)>,
) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_note_entries_alternate_content_branch(r, ctx, tag, entries, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_note_entries_alternate_content_branch(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    tag: &[u8],
    entries: &mut Vec<(String, Vec<Block>)>,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_note_entries_alternate_content(r, ctx, tag, entries);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == tag => {
                if let Some(entry) = read_note_entry(r, ctx, &e) {
                    entries.push(entry);
                }
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_note_entry(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    e: &BytesStart<'_>,
) -> Option<(String, Vec<Block>)> {
    let boilerplate = matches!(
        attr_local_trimmed(e, b"type").as_deref(),
        Some("separator") | Some("continuationSeparator") | Some("continuationNotice")
    );
    if boilerplate {
        skip_subtree(r);
        return None;
    }
    let Some(id) = attr_local_trimmed(e, b"id") else {
        skip_subtree(r);
        return None;
    };
    Some((id, read_blocks(r, ctx, 0)))
}

/// Scan `word/document.xml` for note reference ids and the containing top-level
/// body block text. `tag` is `b"footnoteReference"` or `b"endnoteReference"`.
pub(crate) fn scan_note_ref_anchors(
    xml: &str,
    tag: &[u8],
    ctx: &super::fields::FieldResolutionContext<'_>,
) -> HashMap<String, String> {
    let mut anchors = HashMap::new();
    for reference in scan_note_ref_positions(xml, tag, ctx).references {
        anchors
            .entry(reference.id)
            .or_insert_with(|| text::finalize(&reference.block_text));
    }
    anchors
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteRefPosition {
    pub(crate) id: String,
    pub(crate) block_index: usize,
    pub(crate) block_text: String,
    pub(crate) text_offset: usize,
    pub(crate) paragraph: bool,
    pub(crate) custom_mark: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NoteRefPositionScan {
    pub(crate) block_count: usize,
    pub(crate) references: Vec<NoteRefPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingNoteRef {
    id: String,
    text_offset: usize,
    custom_mark: bool,
}

pub(crate) fn scan_note_ref_positions(
    xml: &str,
    tag: &[u8],
    ctx: &super::fields::FieldResolutionContext<'_>,
) -> NoteRefPositionScan {
    let mut r = Reader::from_str(xml);
    let mut scan = NoteRefPositionScan::default();
    let mut in_body = false;
    let mut body_depth = 0usize;
    let mut body_block_candidate_depths = vec![0usize];
    let mut current_block_depth = None;
    let mut current_block_index = None;
    let mut current_block_is_paragraph = false;
    let mut current_block_text = String::new();
    let mut current_block_refs = Vec::new();
    let mut complex_field = NoteAnchorComplexField::default();
    let mut field_state = ContextlessFieldState::with_document_and_note_context(
        ctx.properties,
        ctx.document_bookmarks,
        ctx.note_refs,
    )
    .with_toc_context(ctx.toc_entries, ctx.bookmark_names)
    .with_section_context(ctx.sections)
    .with_style_ref_context_from(ctx.style_refs, 0)
    .with_legacy_form_context_from(ctx.legacy_forms, 0);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"body" {
                    in_body = true;
                    body_depth = 0;
                    body_block_candidate_depths.clear();
                    body_block_candidate_depths.push(0);
                    current_block_depth = None;
                    current_block_index = None;
                    current_block_is_paragraph = false;
                    current_block_text.clear();
                    current_block_refs.clear();
                    scan.block_count = 0;
                    field_state.clear();
                    continue;
                }
                if in_body {
                    if current_block_depth.is_none()
                        && body_block_candidate_depths.contains(&body_depth)
                        && is_note_anchor_transparent_body_container(name)
                    {
                        body_block_candidate_depths.push(body_depth + 1);
                    }
                    if current_block_depth.is_none()
                        && body_block_candidate_depths.contains(&body_depth)
                        && is_note_anchor_body_block(name)
                    {
                        current_block_depth = Some(body_depth + 1);
                        current_block_index = Some(scan.block_count);
                        current_block_is_paragraph = name == b"p";
                        scan.block_count = scan.block_count.saturating_add(1);
                        current_block_text.clear();
                        current_block_refs.clear();
                        complex_field = NoteAnchorComplexField::default();
                        field_state.clear();
                    }
                    body_depth += 1;
                }
                if current_block_depth.is_some() {
                    if matches!(name, b"del" | b"moveFrom") {
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                    } else if name == tag {
                        if let Some(reference) = pending_note_ref(&e, &current_block_text) {
                            current_block_refs.push(reference);
                        }
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                    } else if name == b"fldChar" {
                        if let Some(text) = complex_field.apply_field_char(&e, &mut field_state) {
                            current_block_text.push_str(&text);
                        }
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                    } else if name == b"instrText" {
                        let instruction = read_text(&mut r);
                        complex_field.append_instruction_text(&instruction);
                        body_depth = body_depth.saturating_sub(1);
                    } else if name == b"fldSimple" {
                        if complex_field.suppresses_result() {
                            skip_subtree(&mut r);
                            body_depth = body_depth.saturating_sub(1);
                        } else if let Some(instruction) = attr_local_trimmed(&e, b"instr") {
                            if is_note_anchor_text_form_field_instruction(&instruction) {
                                if let Some(text) = computed_note_anchor_simple_text_form_field_text(
                                    &mut r,
                                    &instruction,
                                    &mut field_state,
                                ) {
                                    current_block_text.push_str(&text);
                                }
                                body_depth = body_depth.saturating_sub(1);
                            } else if let Some(text) =
                                computed_note_anchor_field_text(&instruction, &mut field_state)
                            {
                                current_block_text.push_str(&text);
                                skip_subtree(&mut r);
                                body_depth = body_depth.saturating_sub(1);
                            }
                        }
                    } else if name == b"t" {
                        let text = read_text(&mut r);
                        complex_field.append_result_text(&text);
                        if !complex_field.suppresses_result() {
                            current_block_text.push_str(&text);
                        }
                        body_depth = body_depth.saturating_sub(1);
                    } else if let Some(marker) = inline_marker_text(&e) {
                        complex_field.append_result_text(marker);
                        if !complex_field.suppresses_result() {
                            current_block_text.push_str(marker);
                        }
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                    } else if name == b"sym" {
                        complex_field.append_result_symbol(&e);
                        if !complex_field.suppresses_result() {
                            append_run_symbol(&mut current_block_text, &e);
                        }
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                    } else if name == b"AlternateContent" {
                        append_note_anchor_alternate_content(
                            &mut r,
                            tag,
                            &mut current_block_text,
                            &mut current_block_refs,
                            &mut complex_field,
                            &mut field_state,
                            0,
                        );
                        body_depth = body_depth.saturating_sub(1);
                    } else if is_note_anchor_embedded_body(name) {
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if current_block_depth.is_some() {
                    if name == tag {
                        if let Some(reference) = pending_note_ref(&e, &current_block_text) {
                            current_block_refs.push(reference);
                        }
                    } else if name == b"fldChar" {
                        if let Some(text) = complex_field.apply_field_char(&e, &mut field_state) {
                            current_block_text.push_str(&text);
                        }
                    } else if name == b"fldSimple" {
                        if !complex_field.suppresses_result() {
                            if let Some(text) =
                                computed_note_anchor_simple_field_text(&e, &mut field_state)
                            {
                                current_block_text.push_str(&text);
                            }
                        }
                    } else {
                        complex_field.append_result_empty(&e, name);
                        if !complex_field.suppresses_result() {
                            append_note_anchor_empty(&mut current_block_text, &e, name);
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"body" {
                    in_body = false;
                    body_depth = 0;
                    body_block_candidate_depths.clear();
                    body_block_candidate_depths.push(0);
                    current_block_depth = None;
                    current_block_index = None;
                    current_block_is_paragraph = false;
                    current_block_text.clear();
                    current_block_refs.clear();
                    complex_field = NoteAnchorComplexField::default();
                    field_state.clear();
                    continue;
                }
                if in_body {
                    let ending_current_block = current_block_depth == Some(body_depth);
                    if ending_current_block {
                        insert_note_anchor_block(
                            &mut scan.references,
                            &current_block_refs,
                            &current_block_text,
                            current_block_index.unwrap_or_default(),
                            current_block_is_paragraph,
                        );
                    }
                    if body_block_candidate_depths.last().copied() == Some(body_depth) {
                        body_block_candidate_depths.pop();
                    }
                    body_depth = body_depth.saturating_sub(1);
                    if ending_current_block {
                        current_block_depth = None;
                        current_block_index = None;
                        current_block_is_paragraph = false;
                        current_block_text.clear();
                        current_block_refs.clear();
                        complex_field = NoteAnchorComplexField::default();
                        field_state.clear();
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    scan
}

#[derive(Default)]
struct NoteAnchorComplexField {
    depth: usize,
    instruction: String,
    phase: Option<NoteAnchorComplexFieldPhase>,
    computed_result: Option<String>,
    result_text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoteAnchorComplexFieldPhase {
    Instruction,
    Result,
}

impl NoteAnchorComplexField {
    fn apply_field_char(
        &mut self,
        e: &BytesStart<'_>,
        field_state: &mut ContextlessFieldState<'_>,
    ) -> Option<String> {
        match field_char_type(e).as_deref() {
            Some("begin") => {
                if self.depth == 0 {
                    self.instruction.clear();
                    self.result_text.clear();
                    self.phase = Some(NoteAnchorComplexFieldPhase::Instruction);
                    self.computed_result = None;
                }
                self.depth += 1;
                None
            }
            Some("separate")
                if self.depth == 1
                    && self.phase == Some(NoteAnchorComplexFieldPhase::Instruction) =>
            {
                self.phase = Some(NoteAnchorComplexFieldPhase::Result);
                if !is_note_anchor_text_form_field_instruction(&self.instruction) {
                    self.computed_result =
                        computed_note_anchor_field_text(&self.instruction, field_state);
                }
                self.computed_result.clone()
            }
            Some("end") => {
                let computed_text_form = if self.depth == 1
                    && self.phase == Some(NoteAnchorComplexFieldPhase::Result)
                    && self.computed_result.is_none()
                    && is_note_anchor_text_form_field_instruction(&self.instruction)
                {
                    field_state
                        .computed_legacy_text_form_current_result(
                            &self.instruction,
                            &self.result_text,
                        )
                        .or_else(|| {
                            (!self.result_text.is_empty()).then_some(self.result_text.clone())
                        })
                } else {
                    None
                };
                if self.depth > 0 {
                    self.depth -= 1;
                    if self.depth == 0 {
                        self.instruction.clear();
                        self.result_text.clear();
                        self.phase = None;
                        self.computed_result = None;
                    }
                }
                computed_text_form
            }
            _ => None,
        }
    }

    fn append_instruction_text(&mut self, text: &str) {
        if self.depth == 1 && self.phase == Some(NoteAnchorComplexFieldPhase::Instruction) {
            self.instruction.push_str(text);
        }
    }

    fn suppresses_result(&self) -> bool {
        self.depth > 0
            && self.phase == Some(NoteAnchorComplexFieldPhase::Result)
            && (self.computed_result.is_some()
                || is_note_anchor_text_form_field_instruction(&self.instruction))
    }

    fn append_result_text(&mut self, text: &str) {
        if self.collects_result_text() {
            self.result_text.push_str(text);
        }
    }

    fn append_result_symbol(&mut self, e: &BytesStart<'_>) {
        if self.collects_result_text() {
            append_run_symbol(&mut self.result_text, e);
        }
    }

    fn append_result_empty(&mut self, e: &BytesStart<'_>, name: &[u8]) {
        if self.collects_result_text() {
            append_note_anchor_empty(&mut self.result_text, e, name);
        }
    }

    fn collects_result_text(&self) -> bool {
        self.depth > 0
            && self.phase == Some(NoteAnchorComplexFieldPhase::Result)
            && self.computed_result.is_none()
            && is_note_anchor_text_form_field_instruction(&self.instruction)
    }
}

fn is_note_anchor_body_block(name: &[u8]) -> bool {
    matches!(name, b"p" | b"tbl")
}

fn is_note_anchor_transparent_body_container(name: &[u8]) -> bool {
    matches!(
        name,
        b"sdt" | b"sdtContent" | b"customXml" | b"smartTag" | b"ins" | b"moveTo"
    )
}

fn is_note_anchor_embedded_body(name: &[u8]) -> bool {
    matches!(name, b"drawing" | b"pict" | b"object")
}

fn is_note_anchor_text_form_field_instruction(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::FormField(kind) if kind == "FORMTEXT"
    )
}

fn computed_note_anchor_simple_field_text(
    e: &BytesStart<'_>,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    let instruction = attr_local_trimmed(e, b"instr")?;
    computed_note_anchor_field_text(&instruction, field_state)
}

fn computed_note_anchor_simple_text_form_field_text(
    r: &mut Xml<'_>,
    instruction: &str,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    let current_result = read_note_anchor_simple_field_current_result(r);
    field_state
        .computed_legacy_text_form_current_result(instruction, &current_result)
        .or_else(|| (!current_result.is_empty()).then_some(current_result))
}

fn read_note_anchor_simple_field_current_result(r: &mut Xml<'_>) -> String {
    let mut result = String::new();
    let mut depth = 1usize;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"t" {
                    result.push_str(&read_text(r));
                } else if name == b"sym" {
                    append_run_symbol(&mut result, &e);
                    skip_subtree(r);
                } else if let Some(marker) = inline_marker_text(&e) {
                    result.push_str(marker);
                    skip_subtree(r);
                } else {
                    depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                append_note_anchor_empty(&mut result, &e, name);
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    result
}

fn computed_note_anchor_field_text(
    instruction: &str,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    computed_contextless_result(instruction, field_state)
}

fn append_note_anchor_empty(out: &mut String, e: &BytesStart<'_>, name: &[u8]) {
    if name == b"sym" {
        append_run_symbol(out, e);
    } else if let Some(marker) = inline_marker_text(e) {
        out.push_str(marker);
    }
}

fn append_note_anchor_alternate_content(
    r: &mut Xml<'_>,
    tag: &[u8],
    text: &mut String,
    refs: &mut Vec<PendingNoteRef>,
    complex_field: &mut NoteAnchorComplexField,
    field_state: &mut ContextlessFieldState<'_>,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"Choice" | b"Fallback" if !took => {
                    took = true;
                    append_note_anchor_content(
                        r,
                        tag,
                        text,
                        refs,
                        complex_field,
                        field_state,
                        depth + 1,
                    );
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn append_note_anchor_content(
    r: &mut Xml<'_>,
    tag: &[u8],
    text: &mut String,
    refs: &mut Vec<PendingNoteRef>,
    complex_field: &mut NoteAnchorComplexField,
    field_state: &mut ContextlessFieldState<'_>,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == tag {
                    if let Some(reference) = pending_note_ref(&e, text) {
                        refs.push(reference);
                    }
                    skip_subtree(r);
                } else if name == b"fldChar" {
                    if let Some(computed) = complex_field.apply_field_char(&e, field_state) {
                        text.push_str(&computed);
                    }
                    skip_subtree(r);
                } else if name == b"instrText" {
                    let instruction = read_text(r);
                    complex_field.append_instruction_text(&instruction);
                } else if name == b"fldSimple" {
                    if complex_field.suppresses_result() {
                        skip_subtree(r);
                    } else if let Some(instruction) = attr_local_trimmed(&e, b"instr") {
                        if is_note_anchor_text_form_field_instruction(&instruction) {
                            if let Some(computed) = computed_note_anchor_simple_text_form_field_text(
                                r,
                                &instruction,
                                field_state,
                            ) {
                                text.push_str(&computed);
                            }
                        } else if let Some(computed) =
                            computed_note_anchor_field_text(&instruction, field_state)
                        {
                            text.push_str(&computed);
                            skip_subtree(r);
                        } else {
                            append_note_anchor_content(
                                r,
                                tag,
                                text,
                                refs,
                                complex_field,
                                field_state,
                                depth + 1,
                            );
                        }
                    } else {
                        append_note_anchor_content(
                            r,
                            tag,
                            text,
                            refs,
                            complex_field,
                            field_state,
                            depth + 1,
                        );
                    }
                } else if name == b"t" {
                    let value = read_text(r);
                    complex_field.append_result_text(&value);
                    if !complex_field.suppresses_result() {
                        text.push_str(&value);
                    }
                } else if let Some(marker) = inline_marker_text(&e) {
                    complex_field.append_result_text(marker);
                    if !complex_field.suppresses_result() {
                        text.push_str(marker);
                    }
                    skip_subtree(r);
                } else if name == b"sym" {
                    complex_field.append_result_symbol(&e);
                    if !complex_field.suppresses_result() {
                        append_run_symbol(text, &e);
                    }
                    skip_subtree(r);
                } else if name == b"AlternateContent" {
                    append_note_anchor_alternate_content(
                        r,
                        tag,
                        text,
                        refs,
                        complex_field,
                        field_state,
                        depth + 1,
                    );
                } else if is_note_anchor_embedded_body(name) {
                    skip_subtree(r);
                } else {
                    append_note_anchor_content(
                        r,
                        tag,
                        text,
                        refs,
                        complex_field,
                        field_state,
                        depth + 1,
                    );
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == tag {
                    if let Some(reference) = pending_note_ref(&e, text) {
                        refs.push(reference);
                    }
                } else if name == b"fldChar" {
                    if let Some(computed) = complex_field.apply_field_char(&e, field_state) {
                        text.push_str(&computed);
                    }
                } else if name == b"fldSimple" {
                    if !complex_field.suppresses_result() {
                        if let Some(computed) =
                            computed_note_anchor_simple_field_text(&e, field_state)
                        {
                            text.push_str(&computed);
                        }
                    }
                } else {
                    complex_field.append_result_empty(&e, name);
                    if !complex_field.suppresses_result() {
                        append_note_anchor_empty(text, &e, name);
                    }
                }
            }
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn pending_note_ref(e: &BytesStart<'_>, text: &str) -> Option<PendingNoteRef> {
    Some(PendingNoteRef {
        id: attr_local_trimmed(e, b"id")?,
        text_offset: text.chars().count(),
        custom_mark: attr_local(e, b"customMarkFollows")
            .is_some_and(|value| toggle_on(Some(value))),
    })
}

fn insert_note_anchor_block(
    anchors: &mut Vec<NoteRefPosition>,
    refs: &[PendingNoteRef],
    raw_text: &str,
    block_index: usize,
    paragraph: bool,
) {
    if refs.is_empty() {
        return;
    }
    for reference in refs {
        anchors.push(NoteRefPosition {
            id: reference.id.clone(),
            block_index,
            block_text: raw_text.to_string(),
            text_offset: reference.text_offset,
            paragraph,
            custom_mark: reference.custom_mark,
        });
    }
}

/// Parse visible `w:txbxContent` text boxes from `word/document.xml`, using the
/// same block parser and `mc:AlternateContent` first-branch policy as the flat
/// body reader.
pub(crate) fn parse_text_boxes(xml: &str, ctx: &Ctx<'_>) -> Vec<String> {
    let mut r = Reader::from_str(xml);
    let mut text_boxes = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"drawing" | b"pict" | b"object" => {
                    walk_text_box_drawing(&mut r, ctx, &mut text_boxes, 0)
                }
                b"AlternateContent" => {
                    walk_text_box_alternate_content(&mut r, ctx, &mut text_boxes, 0)
                }
                b"txbxContent" => {
                    let blocks = read_blocks(&mut r, ctx, 1);
                    let text = blocks_text(&blocks);
                    if !text.trim().is_empty() {
                        text_boxes.push(text);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text_boxes
}

fn walk_text_box_drawing(r: &mut Xml<'_>, ctx: &Ctx<'_>, text_boxes: &mut Vec<String>, depth: u32) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"txbxContent" => {
                    if depth < MAX_DEPTH {
                        let blocks = read_blocks(r, ctx, depth + 1);
                        let text = blocks_text(&blocks);
                        if !text.trim().is_empty() {
                            text_boxes.push(text);
                        }
                    } else {
                        skip_subtree(r);
                    }
                }
                b"AlternateContent" => {
                    walk_text_box_alternate_content(r, ctx, text_boxes, depth + 1)
                }
                _ => {
                    if depth < MAX_DEPTH {
                        walk_text_box_drawing(r, ctx, text_boxes, depth + 1);
                    } else {
                        skip_subtree(r);
                    }
                }
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn walk_text_box_alternate_content(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    text_boxes: &mut Vec<String>,
    depth: u32,
) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"Choice" | b"Fallback" if !took => {
                    took = true;
                    if depth < MAX_DEPTH {
                        walk_text_box_drawing(r, ctx, text_boxes, depth + 1);
                    } else {
                        skip_subtree(r);
                    }
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// Read block-level children (`w:p`, `w:tbl`) until the enclosing `End`. Block
/// content controls (`w:sdt`/`w:sdtContent`), `w:customXml`, `w:smartTag`, and
/// accepted-current revision wrappers (`w:ins`/`w:moveTo`) are transparent
/// containers — descended into so their paragraphs/tables aren't lost.
fn read_blocks(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> Vec<Block> {
    read_blocks_with_pagination(r, ctx, depth).blocks
}

#[derive(Default)]
struct BlockBatch {
    blocks: Vec<Block>,
    pagination: Vec<Option<PaginationHint>>,
    line_spacing: Vec<Option<LineSpacingHint>>,
    nested_tables: Vec<Option<TablePaginationHints>>,
    tab_stops: Vec<Vec<TabStop>>,
    column_break_offsets: Vec<Vec<usize>>,
}

impl BlockBatch {
    fn push_table(&mut self, block: Block, pagination: TablePaginationHints) {
        self.blocks.push(block);
        self.pagination.push(None);
        self.line_spacing.push(None);
        self.nested_tables.push(Some(pagination));
        self.tab_stops.push(Vec::new());
        self.column_break_offsets.push(Vec::new());
    }

    fn extend(&mut self, other: Self) {
        self.blocks.extend(other.blocks);
        self.pagination.extend(other.pagination);
        self.line_spacing.extend(other.line_spacing);
        self.nested_tables.extend(other.nested_tables);
        self.tab_stops.extend(other.tab_stops);
        self.column_break_offsets.extend(other.column_break_offsets);
    }

    fn append_to(
        self,
        blocks: &mut Vec<Block>,
        pagination: &mut Vec<Option<PaginationHint>>,
        line_spacing: &mut Vec<Option<LineSpacingHint>>,
        nested_tables: &mut Vec<Option<TablePaginationHints>>,
        tab_stops: &mut Vec<Vec<TabStop>>,
        column_break_offsets: &mut Vec<Vec<usize>>,
    ) {
        blocks.extend(self.blocks);
        pagination.extend(self.pagination);
        line_spacing.extend(self.line_spacing);
        nested_tables.extend(self.nested_tables);
        tab_stops.extend(self.tab_stops);
        column_break_offsets.extend(self.column_break_offsets);
    }
}

fn read_blocks_with_pagination(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> BlockBatch {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return BlockBatch::default();
    }
    let mut batch = BlockBatch::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                match local(e.name().as_ref()) {
                    b"p" => batch.extend(read_paragraph_block_batch(r, ctx, depth + 1)),
                    b"tbl" => {
                        if let Some((table, pagination)) = read_table_block(r, ctx, depth + 1) {
                            batch.push_table(table, pagination);
                        }
                    }
                    b"sdt" => {
                        batch.extend(read_content_control_blocks_with_pagination(
                            r,
                            ctx,
                            depth + 1,
                        ));
                    }
                    b"sdtContent" | b"customXml" | b"smartTag" | b"ins" | b"moveTo" => {
                        batch.extend(read_blocks_with_pagination(r, ctx, depth + 1))
                    }
                    b"AlternateContent" => batch.extend(
                        read_alternate_content_blocks_with_pagination(r, ctx, depth + 1),
                    ),
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    batch
}

fn read_alternate_content_blocks_with_pagination(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
) -> BlockBatch {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return BlockBatch::default();
    }
    let mut batch = BlockBatch::default();
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"Choice" | b"Fallback" if !took => {
                    took = true;
                    batch.extend(read_blocks_with_pagination(r, ctx, depth + 1));
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    batch
}

fn read_content_control_blocks_with_pagination(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
) -> BlockBatch {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return BlockBatch::default();
    }
    let mut control = None;
    let mut batch = BlockBatch::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"sdtPr" => control = read_content_control_pr(r),
                b"sdtContent" => batch.extend(read_blocks_with_pagination(r, ctx, depth + 1)),
                b"p" => batch.extend(read_paragraph_block_batch(r, ctx, depth + 1)),
                b"tbl" => {
                    if let Some((table, pagination)) = read_table_block(r, ctx, depth + 1) {
                        batch.push_table(table, pagination);
                    }
                }
                b"sdt" => {
                    batch.extend(read_content_control_blocks_with_pagination(
                        r,
                        ctx,
                        depth + 1,
                    ));
                }
                b"customXml" | b"smartTag" | b"ins" | b"moveTo" => {
                    batch.extend(read_blocks_with_pagination(r, ctx, depth + 1))
                }
                b"AlternateContent" => {
                    read_content_control_blocks_alternate_content(
                        r,
                        ctx,
                        depth + 1,
                        &mut control,
                        &mut batch,
                    );
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    apply_content_control_to_blocks(&mut batch.blocks, control);
    batch
}

fn read_content_control_blocks_alternate_content(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    control: &mut Option<AuthoredContentControl>,
    batch: &mut BlockBatch,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_content_control_blocks_alternate_content_branch(
                            r,
                            ctx,
                            depth + 1,
                            control,
                            batch,
                            name,
                        );
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_content_control_blocks_alternate_content_branch(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    control: &mut Option<AuthoredContentControl>,
    batch: &mut BlockBatch,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"sdtPr" => *control = read_content_control_pr(r),
                b"sdtContent" => batch.extend(read_blocks_with_pagination(r, ctx, depth + 1)),
                b"p" => batch.extend(read_paragraph_block_batch(r, ctx, depth + 1)),
                b"tbl" => {
                    if let Some((table, pagination)) = read_table_block(r, ctx, depth + 1) {
                        batch.push_table(table, pagination);
                    }
                }
                b"sdt" => {
                    batch.extend(read_content_control_blocks_with_pagination(
                        r,
                        ctx,
                        depth + 1,
                    ));
                }
                b"customXml" | b"smartTag" | b"ins" | b"moveTo" => {
                    batch.extend(read_blocks_with_pagination(r, ctx, depth + 1))
                }
                b"AlternateContent" => {
                    read_content_control_blocks_alternate_content(r, ctx, depth + 1, control, batch)
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_content_control_to_blocks(blocks: &mut [Block], control: Option<AuthoredContentControl>) {
    let Some(control) = control else {
        return;
    };
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                apply_content_control(&mut paragraph.runs, Some(control.clone()));
            }
            Block::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        apply_content_control_to_blocks(&mut cell.blocks, Some(control.clone()));
                    }
                }
            }
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

/// Read a `<w:p>`: its `w:pPr` properties and inline runs.
fn read_paragraph(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
) -> (
    Paragraph,
    Vec<Chart>,
    Option<ParsedSectionSetup>,
    PaginationHint,
    Vec<TabStop>,
    Option<LineSpacingHint>,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return (
            Paragraph::default(),
            Vec::new(),
            None,
            PaginationHint::default(),
            Vec::new(),
            None,
        );
    }
    ctx.begin_paragraph_charts();
    let mut runs: Vec<Run> = Vec::new();
    let mut pp = PPr::default();
    let mut sequence_heading_applied = false;
    let mut complex_field = ComplexFieldTracker::default();
    let mut bookmarks = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"pPr" => {
                    pp = read_ppr(r, depth + 1);
                    apply_sequence_heading_scope(&pp, ctx, &mut sequence_heading_applied);
                }
                b"r" => {
                    let start = runs.len();
                    let next = read_run(
                        r,
                        ctx,
                        pp.style_id.as_deref(),
                        None,
                        depth + 1,
                        Some(&mut complex_field),
                        runs.len(),
                    );
                    runs.extend(next);
                    complex_field.apply_pending(&mut runs);
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                b"hyperlink" => {
                    let start = runs.len();
                    runs.extend(read_hyperlink(r, &e, ctx, pp.style_id.as_deref(), depth));
                    mark_complex_field_result_runs(&mut complex_field, &runs, start);
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                b"fldSimple" => {
                    let start = runs.len();
                    runs.extend(read_fldsimple(r, &e, ctx, pp.style_id.as_deref(), depth));
                    mark_complex_field_result_runs(&mut complex_field, &runs, start);
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                b"sdt" => {
                    let start = runs.len();
                    append_content_control_runs_with_complex(
                        r,
                        ctx,
                        pp.style_id.as_deref(),
                        None,
                        depth + 1,
                        &mut runs,
                        &mut complex_field,
                    );
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                b"customXml" | b"ins" | b"moveTo" | b"smartTag" | b"sdtContent" | b"bdo"
                | b"dir" => {
                    let start = runs.len();
                    append_runs_container_with_complex(
                        r,
                        ctx,
                        pp.style_id.as_deref(),
                        None,
                        depth + 1,
                        &mut runs,
                        &mut complex_field,
                    );
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                b"AlternateContent" => {
                    let start = runs.len();
                    append_alternate_content_runs_with_complex(
                        r,
                        ctx,
                        pp.style_id.as_deref(),
                        None,
                        depth + 1,
                        &mut runs,
                        &mut complex_field,
                    );
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                b"bookmarkStart" => {
                    push_active_bookmark(&mut bookmarks, &e);
                    skip_subtree(r);
                }
                b"bookmarkEnd" => {
                    remove_active_bookmark(&mut bookmarks, &e);
                    skip_subtree(r);
                }
                // `w:del` = tracked deletion (removed text) → drop.
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"bookmarkStart" => push_active_bookmark(&mut bookmarks, &e),
                b"bookmarkEnd" => remove_active_bookmark(&mut bookmarks, &e),
                b"fldSimple" => {
                    let start = runs.len();
                    push_empty_fldsimple_run(&mut runs, &e, ctx);
                    mark_complex_field_result_runs(&mut complex_field, &runs, start);
                    apply_active_bookmark(&mut runs, start, &bookmarks);
                }
                _ => {}
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    apply_sequence_heading_scope(&pp, ctx, &mut sequence_heading_applied);
    let section = pp.section.take();
    let (paragraph, pagination, tab_stops, line_spacing) = finalize_paragraph(runs, pp, ctx);
    let charts = ctx.end_paragraph_charts();
    (
        paragraph,
        charts,
        section,
        pagination,
        tab_stops,
        line_spacing,
    )
}

fn push_active_bookmark(bookmarks: &mut Vec<(String, String)>, e: &BytesStart<'_>) {
    let Some(id) = attr_local_trimmed(e, b"id") else {
        return;
    };
    let Some(name) = attr_local_trimmed(e, b"name") else {
        return;
    };
    bookmarks.push((id, name));
}

fn remove_active_bookmark(bookmarks: &mut Vec<(String, String)>, e: &BytesStart<'_>) {
    let Some(id) = attr_local_trimmed(e, b"id") else {
        return;
    };
    if let Some(index) = bookmarks
        .iter()
        .rposition(|(active_id, _)| active_id == &id)
    {
        bookmarks.remove(index);
    }
}

fn apply_active_bookmark(runs: &mut [Run], start: usize, bookmarks: &[(String, String)]) {
    let Some((_, name)) = bookmarks.last() else {
        return;
    };
    for run in runs.iter_mut().skip(start) {
        if run.bookmark.is_none() {
            run.bookmark = Some(name.clone());
        }
    }
}

fn mark_complex_field_result_runs(
    complex_field: &mut ComplexFieldTracker,
    runs: &[Run],
    start: usize,
) {
    if !complex_field.in_result() {
        return;
    }
    for (index, run) in runs.iter().enumerate().skip(start) {
        if !run.text.is_empty() {
            complex_field.push_result_run(index, &run.text, false);
        }
    }
}

struct ParagraphBlockData {
    blocks: Vec<Block>,
    pagination: PaginationHint,
    line_spacing: Option<LineSpacingHint>,
    tab_stops: Vec<TabStop>,
    section_column_gap_pt: Option<f32>,
    section_column_layout: Option<SectionColumnLayoutHints>,
    section_column_separator: bool,
    section_column_rtl: bool,
    column_break_offsets: Vec<Vec<usize>>,
}

fn read_paragraph_blocks_data(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> ParagraphBlockData {
    let (paragraph, charts, section, pagination, tab_stops, line_spacing) =
        read_paragraph(r, ctx, depth);
    let mut split = if !charts.is_empty() && paragraph.runs.is_empty() {
        let blocks = charts.into_iter().map(Block::Chart).collect::<Vec<_>>();
        let column_break_offsets = vec![Vec::new(); blocks.len()];
        ParagraphBlockSplit {
            blocks,
            column_break_offsets,
        }
    } else {
        split_page_breaks(paragraph)
    };
    let mut section_column_gap_pt = None;
    let mut section_column_layout = None;
    let mut section_column_separator = false;
    let mut section_column_rtl = false;
    if let Some(mut section) = section {
        if section.setup.section_break.is_none() {
            section.setup.section_break = Some(SectionBreakKind::NextPage);
        }
        section_column_gap_pt = section.column_gap_pt;
        section_column_layout = section.column_layout;
        section_column_separator = section.column_separator;
        section_column_rtl = section.column_rtl;
        split.push(Block::SectionBreak(section.setup), Vec::new());
    }
    ParagraphBlockData {
        blocks: split.blocks,
        pagination,
        line_spacing,
        tab_stops,
        section_column_gap_pt,
        section_column_layout,
        section_column_separator,
        section_column_rtl,
        column_break_offsets: split.column_break_offsets,
    }
}

fn read_paragraph_block_batch(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> BlockBatch {
    let data = read_paragraph_blocks_data(r, ctx, depth);
    ctx.capture_paragraph_blocks(&data);
    let pagination = data
        .blocks
        .iter()
        .map(|block| {
            if matches!(block, Block::Paragraph(_)) {
                Some(data.pagination)
            } else {
                None
            }
        })
        .collect();
    let nested_tables = vec![None; data.blocks.len()];
    let line_spacing = data
        .blocks
        .iter()
        .map(|block| {
            if matches!(block, Block::Paragraph(_)) {
                data.line_spacing
            } else {
                None
            }
        })
        .collect();
    let tab_stops = data
        .blocks
        .iter()
        .map(|block| {
            if matches!(block, Block::Paragraph(_)) {
                data.tab_stops.clone()
            } else {
                Vec::new()
            }
        })
        .collect();
    BlockBatch {
        blocks: data.blocks,
        pagination,
        line_spacing,
        nested_tables,
        tab_stops,
        column_break_offsets: data.column_break_offsets,
    }
}

#[derive(Default)]
struct ParagraphBlockSplit {
    blocks: Vec<Block>,
    column_break_offsets: Vec<Vec<usize>>,
}

impl ParagraphBlockSplit {
    fn push(&mut self, block: Block, offsets: Vec<usize>) {
        self.blocks.push(block);
        self.column_break_offsets.push(offsets);
    }
}

fn flush_split_paragraph(
    split: &mut ParagraphBlockSplit,
    current: &mut Paragraph,
    props: &ParaProps,
    column_break_offsets: &mut Vec<usize>,
) {
    if !current.is_blank() || paragraph_has_field_runs(current) {
        let paragraph = std::mem::replace(
            current,
            Paragraph {
                props: props.clone(),
                runs: Vec::new(),
            },
        );
        split.push(
            Block::Paragraph(paragraph),
            std::mem::take(column_break_offsets),
        );
    } else {
        current.runs.clear();
        column_break_offsets.clear();
    }
}

fn normalize_column_breaks(
    text: &str,
    visible: bool,
    source_chars: &mut usize,
    offsets: &mut Vec<usize>,
) -> String {
    let mut normalized = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == COLUMN_BREAK_MARKER {
            if visible {
                offsets.push(*source_chars);
            }
            normalized.push('\n');
        } else {
            normalized.push(ch);
        }
        *source_chars = source_chars.saturating_add(1);
    }
    normalized
}

fn split_page_breaks(paragraph: Paragraph) -> ParagraphBlockSplit {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.text.contains(PAGE_BREAK_MARKER) || run.text.contains(COLUMN_BREAK_MARKER))
    {
        let mut split = ParagraphBlockSplit::default();
        if !paragraph.is_blank() || paragraph_has_field_runs(&paragraph) {
            split.push(Block::Paragraph(paragraph), Vec::new());
        }
        return split;
    }

    let props = paragraph.props;
    let mut split = ParagraphBlockSplit::default();
    let mut current = Paragraph {
        props: props.clone(),
        runs: Vec::new(),
    };
    let mut column_break_offsets = Vec::new();
    let mut source_chars = 0usize;
    for run in paragraph.runs {
        let parts: Vec<_> = run
            .text
            .split(PAGE_BREAK_MARKER)
            .map(str::to_owned)
            .collect();
        for (index, part) in parts.into_iter().enumerate() {
            if index > 0 {
                flush_split_paragraph(&mut split, &mut current, &props, &mut column_break_offsets);
                split.push(Block::PageBreak, Vec::new());
                source_chars = 0;
            }
            let part = normalize_column_breaks(
                &part,
                !run.props.hidden,
                &mut source_chars,
                &mut column_break_offsets,
            );
            if !part.is_empty() {
                let mut split_run = run.clone();
                split_run.text = part;
                current.runs.push(split_run);
            }
        }
    }
    flush_split_paragraph(&mut split, &mut current, &props, &mut column_break_offsets);
    split
}

fn paragraph_has_field_runs(paragraph: &Paragraph) -> bool {
    paragraph
        .runs
        .iter()
        .any(|run| !matches!(run.field, FieldRole::Other))
}

#[derive(Default)]
struct ComplexFieldTracker {
    instruction: String,
    phase: Option<ComplexFieldPhase>,
    result_runs: Vec<ComplexFieldResultRun>,
    result_text: String,
    result_start: Option<usize>,
    pending: Option<PendingComplexField>,
    preserve_empty_runs: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ComplexFieldPhase {
    Instruction,
    Result,
}

struct PendingComplexField {
    instruction: String,
    text: Option<String>,
    preserve_instruction: bool,
    unsupported_reason: Option<FieldUnsupportedReason>,
    result_runs: Vec<ComplexFieldResultRun>,
    insert_at: usize,
}

#[derive(Clone, Copy)]
struct ComplexFieldResultRun {
    index: usize,
    preserve_hyperlink: bool,
}

impl ComplexFieldTracker {
    fn begin(&mut self) {
        self.instruction.clear();
        self.result_runs.clear();
        self.result_text.clear();
        self.result_start = None;
        self.phase = Some(ComplexFieldPhase::Instruction);
        self.pending = None;
    }

    fn separate(&mut self, index: usize) {
        if self.phase.is_some() {
            self.phase = Some(ComplexFieldPhase::Result);
            self.result_start = Some(index);
        }
    }

    fn end(&mut self, ctx: &Ctx<'_>, index: usize) {
        if self.phase.is_some() {
            let instruction = normalized_field_instruction(&self.instruction);
            if !instruction.is_empty() {
                let current_result = self.result_text.as_str();
                let text = computed_simple_field_result(&instruction, ctx, current_result);
                let preserve_instruction =
                    text.is_some() && preserves_computed_field_instruction(&instruction, ctx);
                let unsupported_reason = text
                    .is_none()
                    .then(|| unsupported_simple_field_reason_hint(&instruction, ctx))
                    .flatten();
                let insert_at = self.result_start.unwrap_or(index);
                self.pending = Some(PendingComplexField {
                    text,
                    instruction,
                    preserve_instruction,
                    unsupported_reason,
                    result_runs: std::mem::take(&mut self.result_runs),
                    insert_at,
                });
            }
        }
        self.instruction.clear();
        self.result_runs.clear();
        self.result_text.clear();
        self.result_start = None;
        self.phase = None;
    }

    fn push_instruction(&mut self, text: &str) {
        if self.phase == Some(ComplexFieldPhase::Instruction) {
            self.instruction.push_str(text);
        }
    }

    fn in_result(&self) -> bool {
        self.phase == Some(ComplexFieldPhase::Result)
    }

    fn push_result_run(&mut self, index: usize, text: &str, preserve_hyperlink: bool) {
        if self.in_result() {
            self.result_runs.push(ComplexFieldResultRun {
                index,
                preserve_hyperlink,
            });
            self.result_text.push_str(text);
        }
    }

    fn apply_pending(&mut self, runs: &mut Vec<Run>) {
        let Some(computed) = self.pending.take() else {
            return;
        };
        if computed.result_runs.is_empty() {
            if let Some(text) = computed.text {
                runs.insert(
                    computed.insert_at.min(runs.len()),
                    computed_simple_field_run(
                        computed.instruction,
                        text,
                        computed.preserve_instruction,
                    ),
                );
            } else {
                runs.insert(
                    computed.insert_at.min(runs.len()),
                    empty_simple_field_run(computed.instruction, computed.unsupported_reason),
                );
            }
            return;
        }
        for (offset, result_run) in computed.result_runs.iter().copied().enumerate() {
            let Some(run) = runs.get_mut(result_run.index) else {
                continue;
            };
            if computed.text.is_some() {
                run.field = if result_run.preserve_hyperlink
                    && matches!(run.field, FieldRole::Hyperlink { .. })
                {
                    run.field.clone()
                } else if offset == 0 && computed.preserve_instruction {
                    FieldRole::Simple {
                        instruction: computed.instruction.clone(),
                    }
                } else {
                    FieldRole::Other
                };
                run.field_unsupported_reason = None;
            } else {
                run.field = FieldRole::Simple {
                    instruction: computed.instruction.clone(),
                };
                run.field_unsupported_reason = computed.unsupported_reason;
            }
            if let Some(text) = computed.text.as_deref() {
                run.text = if offset == 0 {
                    text.to_string()
                } else {
                    String::new()
                };
            }
        }
    }
}

fn computed_field_run(text: String) -> Run {
    Run {
        text,
        props: CharProps::default(),
        field: FieldRole::Other,
        field_dirty: false,
        field_unsupported_reason: None,
        image: None,
        comment: None,
        revision: None,
        content_control: None,
        bookmark: None,
        note: None,
    }
}

fn computed_simple_field_run(instruction: String, text: String, preserve_instruction: bool) -> Run {
    if preserve_instruction {
        Run {
            text,
            field: FieldRole::Simple { instruction },
            ..Default::default()
        }
    } else {
        computed_field_run(text)
    }
}

fn preserves_computed_field_instruction(instruction: &str, ctx: &Ctx<'_>) -> bool {
    preserves_context_free_computed_field_instruction(instruction)
        || super::fields::computed_preserved_document_info_result(
            instruction,
            ctx.core_properties,
            ctx.custom_properties,
        )
        .is_some()
        || super::fields::computed_preserved_revision_number_result(
            instruction,
            ctx.core_properties,
        )
        .is_some()
        || (ctx.preserve_note_local_fields
            && super::fields::computed_preserved_note_local_ref_result(
                instruction,
                ctx.ref_targets,
            )
            .is_some())
        || (ctx.preserve_note_local_fields
            && super::fields::computed_preserved_sequence_reset_result(instruction).is_some())
        || (ctx.preserve_note_local_fields
            && super::fields::computed_preserved_listnum_start_result(instruction).is_some())
        || (ctx.preserve_note_local_fields
            && super::fields::preserved_note_local_style_ref_target(instruction).is_some())
        || (ctx.preserve_note_local_fields
            && ctx.last_formula_field_used_table_context.get()
            && matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Dynamic(kind) if kind == "="
            ))
}

fn preserves_context_free_computed_field_instruction(instruction: &str) -> bool {
    preserves_computed_empty_field_instruction(instruction)
        || super::fields::supports_computed_symbol_field_syntax(instruction)
        || super::fields::supports_quote_field_syntax(instruction)
        || super::fields::supports_context_free_if_compare_field_syntax(instruction)
        || super::fields::supports_context_free_formula_field_syntax(instruction)
        || super::fields::supports_context_free_fill_in_field_syntax(instruction)
        || super::fields::supports_context_free_display_field_syntax(instruction)
}

fn preserves_computed_empty_field_instruction(instruction: &str) -> bool {
    match FieldKind::from_instruction(instruction) {
        FieldKind::TocEntry => super::fields::supports_toc_entry_field_syntax(instruction),
        FieldKind::ReferenceIndex(_) => {
            super::fields::supports_reference_index_marker_syntax(instruction)
        }
        _ => false,
    }
}

fn empty_simple_field_run(
    instruction: String,
    unsupported_reason: Option<FieldUnsupportedReason>,
) -> Run {
    Run {
        text: String::new(),
        field: FieldRole::Simple { instruction },
        field_unsupported_reason: unsupported_reason,
        ..Default::default()
    }
}

/// Collected `<w:pPr>` properties.
#[derive(Default)]
struct PPr {
    style_id: Option<String>,
    num: Option<(String, u8)>,
    jc: Option<String>,
    outline: Option<u8>,
    layout: ParagraphLayoutProps,
    indent: Indent,
    indent_start_pt: Option<f32>,
    indent_end_pt: Option<f32>,
    bidi: Option<bool>,
    keep_next: Option<bool>,
    keep_lines: Option<bool>,
    widow_control: Option<bool>,
    tab_stops: Vec<TabStop>,
    section: Option<ParsedSectionSetup>,
}

fn read_runs_container_with_complex(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    preserve_empty_runs: bool,
) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut complex_field = ComplexFieldTracker {
        preserve_empty_runs,
        ..ComplexFieldTracker::default()
    };
    append_runs_container_with_complex(
        r,
        ctx,
        paragraph_style_id,
        link,
        depth,
        &mut runs,
        &mut complex_field,
    );
    runs
}

fn append_runs_container_with_complex(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    runs: &mut Vec<Run>,
    complex_field: &mut ComplexFieldTracker,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"r" => {
                    let next = read_run(
                        r,
                        ctx,
                        paragraph_style_id,
                        link,
                        depth + 1,
                        Some(complex_field),
                        runs.len(),
                    );
                    runs.extend(next);
                    complex_field.apply_pending(runs);
                }
                b"hyperlink" => {
                    let start = runs.len();
                    runs.extend(read_hyperlink(r, &e, ctx, paragraph_style_id, depth));
                    mark_complex_field_result_runs(complex_field, runs, start);
                }
                b"fldSimple" => {
                    let start = runs.len();
                    runs.extend(read_fldsimple(r, &e, ctx, paragraph_style_id, depth));
                    mark_complex_field_result_runs(complex_field, runs, start);
                }
                b"sdt" => append_content_control_runs_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                b"customXml" | b"ins" | b"moveTo" | b"smartTag" | b"sdtContent" | b"bdo"
                | b"dir" => append_runs_container_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                b"AlternateContent" => append_alternate_content_runs_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"fldSimple" => {
                let start = runs.len();
                push_empty_fldsimple_run(runs, &e, ctx);
                mark_complex_field_result_runs(complex_field, runs, start);
            }
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn append_content_control_runs_with_complex(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    runs: &mut Vec<Run>,
    complex_field: &mut ComplexFieldTracker,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let start = runs.len();
    let mut control = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"sdtPr" => control = read_content_control_pr(r),
                b"sdtContent" => append_runs_container_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                b"r" => {
                    let next = read_run(
                        r,
                        ctx,
                        paragraph_style_id,
                        link,
                        depth + 1,
                        Some(complex_field),
                        runs.len(),
                    );
                    runs.extend(next);
                    complex_field.apply_pending(runs);
                }
                b"hyperlink" => {
                    let start = runs.len();
                    runs.extend(read_hyperlink(r, &e, ctx, paragraph_style_id, depth));
                    mark_complex_field_result_runs(complex_field, runs, start);
                }
                b"fldSimple" => {
                    let start = runs.len();
                    runs.extend(read_fldsimple(r, &e, ctx, paragraph_style_id, depth));
                    mark_complex_field_result_runs(complex_field, runs, start);
                }
                b"sdt" => append_content_control_runs_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                b"customXml" | b"ins" | b"moveTo" | b"smartTag" | b"bdo" | b"dir" => {
                    append_runs_container_with_complex(
                        r,
                        ctx,
                        paragraph_style_id,
                        link,
                        depth + 1,
                        runs,
                        complex_field,
                    )
                }
                b"AlternateContent" => append_content_control_runs_alternate_content_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    &mut ContentControlSink {
                        control: &mut control,
                        runs: &mut *runs,
                        complex_field: &mut *complex_field,
                    },
                ),
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"fldSimple" => {
                let start = runs.len();
                push_empty_fldsimple_run(runs, &e, ctx);
                mark_complex_field_result_runs(complex_field, runs, start);
            }
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    apply_content_control(&mut runs[start..], control);
}

/// Content-control accumulation state threaded through the `AlternateContent`
/// readers under an `sdt`: the authored control being built, the runs collected
/// so far, and the complex-field tracker. Bundled so these readers pass one
/// borrow instead of three.
struct ContentControlSink<'a> {
    control: &'a mut Option<AuthoredContentControl>,
    runs: &'a mut Vec<Run>,
    complex_field: &'a mut ComplexFieldTracker,
}

fn append_content_control_runs_alternate_content_with_complex(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    sink: &mut ContentControlSink<'_>,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        append_content_control_runs_alternate_content_branch_with_complex(
                            r,
                            ctx,
                            paragraph_style_id,
                            link,
                            depth + 1,
                            sink,
                            name,
                        );
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn append_content_control_runs_alternate_content_branch_with_complex(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    sink: &mut ContentControlSink<'_>,
    branch: &[u8],
) {
    let control = &mut *sink.control;
    let runs = &mut *sink.runs;
    let complex_field = &mut *sink.complex_field;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"sdtPr" => *control = read_content_control_pr(r),
                b"sdtContent" => append_runs_container_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                b"r" => {
                    let next = read_run(
                        r,
                        ctx,
                        paragraph_style_id,
                        link,
                        depth + 1,
                        Some(complex_field),
                        runs.len(),
                    );
                    runs.extend(next);
                    complex_field.apply_pending(runs);
                }
                b"hyperlink" => {
                    let start = runs.len();
                    runs.extend(read_hyperlink(r, &e, ctx, paragraph_style_id, depth));
                    mark_complex_field_result_runs(complex_field, runs, start);
                }
                b"fldSimple" => {
                    let start = runs.len();
                    runs.extend(read_fldsimple(r, &e, ctx, paragraph_style_id, depth));
                    mark_complex_field_result_runs(complex_field, runs, start);
                }
                b"sdt" => append_content_control_runs_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    runs,
                    complex_field,
                ),
                b"customXml" | b"ins" | b"moveTo" | b"smartTag" | b"bdo" | b"dir" => {
                    append_runs_container_with_complex(
                        r,
                        ctx,
                        paragraph_style_id,
                        link,
                        depth + 1,
                        runs,
                        complex_field,
                    )
                }
                b"AlternateContent" => append_content_control_runs_alternate_content_with_complex(
                    r,
                    ctx,
                    paragraph_style_id,
                    link,
                    depth + 1,
                    &mut ContentControlSink {
                        control: &mut *control,
                        runs: &mut *runs,
                        complex_field: &mut *complex_field,
                    },
                ),
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"fldSimple" => {
                let start = runs.len();
                push_empty_fldsimple_run(runs, &e, ctx);
                mark_complex_field_result_runs(complex_field, runs, start);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn append_alternate_content_runs_with_complex(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    runs: &mut Vec<Run>,
    complex_field: &mut ComplexFieldTracker,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"Choice" | b"Fallback" if !took => {
                    took = true;
                    append_runs_container_with_complex(
                        r,
                        ctx,
                        paragraph_style_id,
                        link,
                        depth + 1,
                        runs,
                        complex_field,
                    );
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_content_control_pr(r: &mut Xml<'_>) -> Option<AuthoredContentControl> {
    let mut control = AuthoredContentControl::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_content_control_pr_alternate_content(r, &mut control);
            }
            Ok(Event::Start(e)) => {
                read_content_control_pr_item(&mut control, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => read_content_control_pr_item(&mut control, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sdtPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    content_control_if_present(control)
}

fn read_content_control_pr_alternate_content(
    r: &mut Xml<'_>,
    control: &mut AuthoredContentControl,
) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_content_control_pr_alternate_content_branch(r, control, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_content_control_pr_alternate_content_branch(
    r: &mut Xml<'_>,
    control: &mut AuthoredContentControl,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_content_control_pr_alternate_content(r, control);
            }
            Ok(Event::Start(e)) => {
                read_content_control_pr_item(control, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => read_content_control_pr_item(control, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_content_control_pr_item(control: &mut AuthoredContentControl, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"alias" => control.alias = attr_local_trimmed(e, b"val"),
        b"tag" => control.tag = attr_local_trimmed(e, b"val"),
        b"dataBinding" => {
            control.data_binding_xpath = attr_local_trimmed(e, b"xpath");
            control.data_binding_store_item_id = attr_local_trimmed(e, b"storeItemID");
        }
        _ => {}
    }
}

fn content_control_if_present(control: AuthoredContentControl) -> Option<AuthoredContentControl> {
    (control.alias.is_some()
        || control.tag.is_some()
        || control.data_binding_xpath.is_some()
        || control.data_binding_store_item_id.is_some())
    .then_some(control)
}

fn apply_content_control(runs: &mut [Run], control: Option<AuthoredContentControl>) {
    let Some(control) = control else {
        return;
    };
    for run in runs {
        if run.content_control.is_none() {
            run.content_control = Some(control.clone());
        }
    }
}

/// Read `<w:pPr>` properties (flattening `w:numPr`'s `w:ilvl`/`w:numId`).
fn read_ppr(r: &mut Xml<'_>, depth: u32) -> PPr {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return PPr::default();
    }
    let mut pp = PPr::default();
    let mut num_id: Option<String> = None;
    let mut ilvl: u8 = 0;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                read_ppr_child(r, &mut pp, &e, &mut num_id, &mut ilvl, depth, true)
            }
            Ok(Event::Empty(e)) => {
                read_ppr_child(r, &mut pp, &e, &mut num_id, &mut ilvl, depth, false)
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"pPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if let Some(id) = num_id {
        pp.num = Some((id, ilvl));
    }
    pp
}

fn is_ppr_leaf(name: &[u8]) -> bool {
    matches!(
        name,
        b"pStyle"
            | b"jc"
            | b"outlineLvl"
            | b"pageBreakBefore"
            | b"bidi"
            | b"keepNext"
            | b"keepLines"
            | b"widowControl"
            | b"spacing"
            | b"ind"
            | b"shd"
    )
}

fn read_ppr_child(
    r: &mut Xml<'_>,
    pp: &mut PPr,
    e: &BytesStart<'_>,
    num_id: &mut Option<String>,
    ilvl: &mut u8,
    depth: u32,
    is_start: bool,
) {
    match local(e.name().as_ref()) {
        b"pPrChange" if is_start => skip_subtree(r),
        b"AlternateContent" if is_start => {
            read_ppr_alternate_content(r, pp, num_id, ilvl, depth + 1);
        }
        b"sectPr" if is_start => pp.section = Some(read_sect_pr(r, depth + 1)),
        b"sectPr" => pp.section = Some(ParsedSectionSetup::default()),
        b"numPr" if is_start => read_num_pr(r, num_id, ilvl, depth + 1),
        b"tabs" if is_start => read_ppr_tabs(r, pp, depth + 1),
        name if is_ppr_leaf(name) => {
            read_ppr_item(pp, e, num_id, ilvl);
            if is_start {
                skip_subtree(r);
            }
        }
        _ if is_start => skip_subtree(r),
        _ => {}
    }
}

fn read_num_pr(r: &mut Xml<'_>, num_id: &mut Option<String>, ilvl: &mut u8, depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    read_num_pr_content(r, num_id, ilvl, b"numPr", depth);
}

pub(crate) fn read_num_pr_content(
    r: &mut Xml<'_>,
    num_id: &mut Option<String>,
    ilvl: &mut u8,
    end: &[u8],
    depth: u32,
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"AlternateContent" => {
                    read_num_pr_alternate_content(r, num_id, ilvl, depth + 1);
                }
                b"ilvl" | b"numId" => {
                    apply_num_pr_child(&e, num_id, ilvl);
                    skip_subtree(r);
                }
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) if matches!(local(e.name().as_ref()), b"ilvl" | b"numId") => {
                apply_num_pr_child(&e, num_id, ilvl);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_num_pr_child(e: &BytesStart<'_>, num_id: &mut Option<String>, ilvl: &mut u8) {
    match local(e.name().as_ref()) {
        b"ilvl" => {
            if let Some(value) = attr_u8(e, b"val") {
                *ilvl = value;
            }
        }
        b"numId" => *num_id = attr_local_trimmed(e, b"val"),
        _ => {}
    }
}

fn read_num_pr_alternate_content(
    r: &mut Xml<'_>,
    num_id: &mut Option<String>,
    ilvl: &mut u8,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_num_pr_content(r, num_id, ilvl, name, depth);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if !took && matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_ppr_tabs(r: &mut Xml<'_>, pp: &mut PPr, depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    read_ppr_tabs_content(r, pp, b"tabs", depth);
}

fn read_ppr_tabs_content(r: &mut Xml<'_>, pp: &mut PPr, end: &[u8], depth: u32) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"AlternateContent" => read_ppr_tabs_alternate_content(r, pp, depth + 1),
                b"tab" => {
                    push_ppr_tab_stop(pp, &e);
                    skip_subtree(r);
                }
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"tab" => {
                push_ppr_tab_stop(pp, &e);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn push_ppr_tab_stop(pp: &mut PPr, e: &BytesStart<'_>) {
    if pp.tab_stops.len() < MAX_TAB_STOPS {
        if let Some(tab) = super::styles::tab_stop(e) {
            pp.tab_stops.push(tab);
        }
    }
}

fn read_ppr_tabs_alternate_content(r: &mut Xml<'_>, pp: &mut PPr, depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_ppr_tabs_content(r, pp, name, depth);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if !took && matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_ppr_alternate_content(
    r: &mut Xml<'_>,
    pp: &mut PPr,
    num_id: &mut Option<String>,
    ilvl: &mut u8,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_ppr_alternate_content_branch(r, pp, num_id, ilvl, name, depth);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if !took && matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_ppr_alternate_content_branch(
    r: &mut Xml<'_>,
    pp: &mut PPr,
    num_id: &mut Option<String>,
    ilvl: &mut u8,
    branch: &[u8],
    depth: u32,
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => read_ppr_child(r, pp, &e, num_id, ilvl, depth, true),
            Ok(Event::Empty(e)) => read_ppr_child(r, pp, &e, num_id, ilvl, depth, false),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_ppr_item(pp: &mut PPr, e: &BytesStart<'_>, num_id: &mut Option<String>, ilvl: &mut u8) {
    match local(e.name().as_ref()) {
        b"pStyle" => {
            pp.style_id = attr_local(e, b"val").map(|value| value.trim().to_owned());
        }
        b"ilvl" => {
            if let Some(v) = attr_u8(e, b"val") {
                *ilvl = v;
            }
        }
        b"numId" => *num_id = attr_local_trimmed(e, b"val"),
        b"jc" => pp.jc = attr_local_trimmed(e, b"val"),
        b"outlineLvl" => pp.outline = attr_u8(e, b"val"),
        b"bidi" => pp.bidi = Some(toggle_on(attr_local(e, b"val"))),
        b"keepNext" => pp.keep_next = Some(toggle_on(attr_local(e, b"val"))),
        b"keepLines" => pp.keep_lines = Some(toggle_on(attr_local(e, b"val"))),
        b"widowControl" => pp.widow_control = Some(toggle_on(attr_local(e, b"val"))),
        b"pageBreakBefore" | b"spacing" | b"shd" => {
            apply_paragraph_layout_child(&mut pp.layout, e);
        }
        b"ind" => {
            pp.indent.left_pt = attr_local(e, b"left").and_then(|v| twips_to_pt(&v));
            pp.indent.right_pt = attr_local(e, b"right").and_then(|v| twips_to_pt(&v));
            pp.indent_start_pt = attr_local(e, b"start").and_then(|v| twips_to_pt(&v));
            pp.indent_end_pt = attr_local(e, b"end").and_then(|v| twips_to_pt(&v));
            apply_paragraph_layout_child(&mut pp.layout, e);
        }
        b"tab" if pp.tab_stops.len() < MAX_TAB_STOPS => {
            if let Some(tab) = super::styles::tab_stop(e) {
                pp.tab_stops.push(tab);
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct ParsedSectionSetup {
    setup: SectionSetup,
    column_gap_pt: Option<f32>,
    column_layout: Option<SectionColumnLayoutHints>,
    column_separator: bool,
    column_rtl: bool,
}

fn read_sect_pr(r: &mut Xml<'_>, depth: u32) -> ParsedSectionSetup {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return ParsedSectionSetup::default();
    }
    let mut section = ParsedSectionSetup::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_sect_pr_alternate_content(r, &mut section, depth + 1);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"cols" => {
                apply_parsed_section_columns(&mut section, read_section_columns_element(r, &e));
            }
            Ok(Event::Start(e)) if is_sect_pr_leaf(local(e.name().as_ref())) => {
                apply_sect_pr_child(&mut section, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) if is_sect_pr_leaf(local(e.name().as_ref())) => {
                apply_sect_pr_child(&mut section, &e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"sectPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    section
}

fn read_sect_pr_alternate_content(r: &mut Xml<'_>, section: &mut ParsedSectionSetup, depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_sect_pr_alternate_content_branch(r, section, name, depth);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if !took && matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_sect_pr_alternate_content_branch(
    r: &mut Xml<'_>,
    section: &mut ParsedSectionSetup,
    branch: &[u8],
    depth: u32,
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sectPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_sect_pr_alternate_content(r, section, depth + 1);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"cols" => {
                apply_parsed_section_columns(section, read_section_columns_element(r, &e));
            }
            Ok(Event::Start(e)) if is_sect_pr_leaf(local(e.name().as_ref())) => {
                apply_sect_pr_child(section, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) if is_sect_pr_leaf(local(e.name().as_ref())) => {
                apply_sect_pr_child(section, &e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn is_sect_pr_leaf(name: &[u8]) -> bool {
    matches!(
        name,
        b"pgSz"
            | b"type"
            | b"pgMar"
            | b"pgNumType"
            | b"cols"
            | b"textDirection"
            | b"docGrid"
            | b"titlePg"
            | b"bidi"
    )
}

fn apply_sect_pr_child(section: &mut ParsedSectionSetup, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"pgSz" => {
            if let Some(size) = section_page_size(e) {
                apply_section_page_size(&mut section.setup.page, size);
            }
        }
        b"type" => {
            section.setup.section_break =
                attr_local(e, b"val").and_then(|value| SectionBreakKind::from_wml_value(&value));
        }
        b"pgMar" => {
            apply_section_page_margins(&mut section.setup.page, section_page_margins(e));
        }
        b"pgNumType" => {
            section.setup.page_number_start = section_page_number_start(e);
            section.setup.page_number_format = section_page_number_format(e);
        }
        b"cols" => {
            apply_parsed_section_columns(section, section_columns_from_attrs(e));
        }
        b"textDirection" => {
            section.setup.text_direction = section_text_direction(e);
        }
        b"docGrid" => {
            section.setup.doc_grid = doc_grid_from_attrs(e);
        }
        b"titlePg" => {
            section.setup.title_page = true;
        }
        b"bidi" => {
            section.column_rtl = toggle_on(attr_local(e, b"val"));
        }
        _ => {}
    }
}

fn apply_parsed_section_columns(section: &mut ParsedSectionSetup, columns: ParsedSectionColumns) {
    section.setup.columns = columns.count;
    section.column_gap_pt = columns.gap_pt;
    section.column_layout = columns.layout;
    section.column_separator = columns.separator;
}

/// Read a `<w:r>`: its `w:rPr` formatting plus text / breaks / drawings. Returns
/// a (possibly empty) text run followed by any inline image runs.
/// Push an extracted drawing's image and/or text-box text as plain runs so they
/// surface in the body / exporters / renderer.
fn push_drawing_runs(images: &mut Vec<Run>, img: Option<Image>, txbx: String) {
    if let Some(img) = img {
        images.push(Run {
            text: String::new(),
            props: CharProps::default(),
            field: FieldRole::None,
            field_dirty: false,
            field_unsupported_reason: None,
            image: Some(img),
            comment: None,
            revision: None,
            content_control: None,
            bookmark: None,
            note: None,
        });
    }
    if !txbx.trim().is_empty() {
        images.push(Run {
            text: txbx,
            props: CharProps::default(),
            field: FieldRole::None,
            field_dirty: false,
            field_unsupported_reason: None,
            image: None,
            comment: None,
            revision: None,
            content_control: None,
            bookmark: None,
            note: None,
        });
    }
}

fn read_run(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    link: Option<&str>,
    depth: u32,
    mut complex_field: Option<&mut ComplexFieldTracker>,
    base_index: usize,
) -> Vec<Run> {
    // A run can recurse back into block content through a drawing's text box
    // (drawing → txbxContent → paragraph → run → drawing …); `depth` threads the
    // structural recursion budget across that boundary so MAX_DEPTH bounds it.
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return Vec::new();
    }
    let mut direct_props = DirectRunProps::default();
    let mut text = String::new();
    let mut text_is_field_result = false;
    let mut has_field_markup = false;
    let mut images: Vec<Run> = Vec::new();
    let mut image_result_runs = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"rPr" => direct_props = read_rpr(r),
                b"fldChar" => {
                    has_field_markup = true;
                    apply_complex_field_char(&e, ctx, complex_field.as_deref_mut(), base_index);
                    skip_subtree(r);
                }
                b"instrText" => {
                    has_field_markup = true;
                    let instruction = read_text(r);
                    if let Some(tracker) = complex_field.as_deref_mut() {
                        tracker.push_instruction(&instruction);
                    }
                }
                b"t" => {
                    let in_result = complex_field
                        .as_deref()
                        .map(ComplexFieldTracker::in_result)
                        .unwrap_or(false);
                    if in_result {
                        text_is_field_result = true;
                    }
                    text.push_str(&read_text(r));
                }
                b"sym" => {
                    if append_run_symbol(&mut text, &e) {
                        let in_result = complex_field
                            .as_deref()
                            .map(ComplexFieldTracker::in_result)
                            .unwrap_or(false);
                        if in_result {
                            text_is_field_result = true;
                        }
                    }
                    skip_subtree(r);
                }
                b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                    append_run_inline_marker(
                        &mut text,
                        &e,
                        complex_field.as_deref(),
                        &mut text_is_field_result,
                    );
                    skip_subtree(r);
                }
                b"drawing" | b"pict" | b"object" => {
                    let start = images.len();
                    let (img, txbx) = read_drawing(r, ctx, depth);
                    push_drawing_runs(&mut images, img, txbx);
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                // AlternateContent can wrap either ordinary run children or the
                // DrawingML/VML forms of the same shape; materialize one branch.
                b"AlternateContent" => {
                    append_run_alternate_content(
                        r,
                        ctx,
                        paragraph_style_id,
                        depth + 1,
                        complex_field.as_deref_mut(),
                        base_index,
                        &mut RunSink {
                            text: &mut text,
                            text_is_field_result: &mut text_is_field_result,
                            images: &mut images,
                            image_result_runs: &mut image_result_runs,
                        },
                    );
                }
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"fldChar" => {
                    has_field_markup = true;
                    apply_complex_field_char(&e, ctx, complex_field.as_deref_mut(), base_index)
                }
                b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                    append_run_inline_marker(
                        &mut text,
                        &e,
                        complex_field.as_deref(),
                        &mut text_is_field_result,
                    );
                }
                b"sym" if append_run_symbol(&mut text, &e) => {
                    let in_result = complex_field
                        .as_deref()
                        .map(ComplexFieldTracker::in_result)
                        .unwrap_or(false);
                    if in_result {
                        text_is_field_result = true;
                    }
                }
                _ => {}
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut runs = Vec::new();
    let preserve_empty_run = text.is_empty()
        && images.is_empty()
        && !has_field_markup
        && complex_field
            .as_deref()
            .is_some_and(|tracker| tracker.preserve_empty_runs && tracker.phase.is_none());
    if !text.is_empty() || preserve_empty_run {
        let props = effective_run_props(ctx, paragraph_style_id, &direct_props);
        if text_is_field_result {
            if let Some(tracker) = complex_field.as_deref_mut() {
                tracker.push_result_run(base_index + runs.len(), &text, link.is_some());
            }
        }
        runs.push(Run {
            text,
            props,
            field: link
                .map(|u| FieldRole::Hyperlink { url: u.to_string() })
                .unwrap_or(FieldRole::None),
            field_dirty: false,
            field_unsupported_reason: None,
            image: None,
            comment: None,
            revision: None,
            content_control: None,
            bookmark: None,
            note: None,
        });
    }
    if let Some(tracker) = complex_field {
        let image_start = runs.len();
        for image_index in image_result_runs {
            let Some(run) = images.get(image_index) else {
                continue;
            };
            if !run.text.is_empty() {
                tracker.push_result_run(base_index + image_start + image_index, &run.text, false);
            }
        }
    }
    if let Some(url) = link {
        for run in &mut images {
            run.field = FieldRole::Hyperlink {
                url: url.to_string(),
            };
        }
    }
    runs.extend(images);
    runs
}

fn append_run_inline_marker(
    text: &mut String,
    e: &BytesStart<'_>,
    complex_field: Option<&ComplexFieldTracker>,
    text_is_field_result: &mut bool,
) -> bool {
    let marker = match local(e.name().as_ref()) {
        b"tab" => Some('\t'),
        b"br" => Some(if is_page_break_type(e) {
            PAGE_BREAK_MARKER
        } else if is_column_break_type(e) {
            COLUMN_BREAK_MARKER
        } else {
            '\n'
        }),
        b"cr" => Some('\n'),
        b"noBreakHyphen" => Some('-'),
        b"softHyphen" => Some('\u{00ad}'),
        _ => None,
    };
    if let Some(marker) = marker {
        mark_complex_field_result_text(complex_field, text_is_field_result);
        text.push(marker);
        true
    } else {
        false
    }
}

fn mark_complex_field_result_text(
    complex_field: Option<&ComplexFieldTracker>,
    text_is_field_result: &mut bool,
) {
    if complex_field.is_some_and(ComplexFieldTracker::in_result) {
        *text_is_field_result = true;
    }
}

fn append_run_symbol(text: &mut String, e: &BytesStart<'_>) -> bool {
    let Some(value) = attr_local_trimmed(e, b"char") else {
        return false;
    };
    let font = attr_local_trimmed(e, b"font");
    let Some(ch) = computed_run_symbol_char(font.as_deref(), &value) else {
        return false;
    };
    text.push(ch);
    true
}

/// Run-accumulation sink threaded through the run readers: the flattened text
/// plus the image runs materialized alongside it, and which of those are field
/// results. Bundled so the recursive `AlternateContent` readers pass one borrow
/// instead of four.
struct RunSink<'a> {
    text: &'a mut String,
    text_is_field_result: &'a mut bool,
    images: &'a mut Vec<Run>,
    image_result_runs: &'a mut Vec<usize>,
}

fn append_run_alternate_content(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    depth: u32,
    mut complex_field: Option<&mut ComplexFieldTracker>,
    base_index: usize,
    sink: &mut RunSink<'_>,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"Choice" | b"Fallback" if !took => {
                    took = true;
                    append_run_alternate_content_branch(
                        r,
                        ctx,
                        paragraph_style_id,
                        depth + 1,
                        complex_field.as_deref_mut(),
                        base_index,
                        sink,
                    );
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn append_run_alternate_content_branch(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    depth: u32,
    mut complex_field: Option<&mut ComplexFieldTracker>,
    base_index: usize,
    sink: &mut RunSink<'_>,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let text = &mut *sink.text;
    let text_is_field_result = &mut *sink.text_is_field_result;
    let images = &mut *sink.images;
    let image_result_runs = &mut *sink.image_result_runs;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"r" => {
                    let start = images.len();
                    let mut nested_complex_field = ComplexFieldTracker::default();
                    let next = read_run(
                        r,
                        ctx,
                        paragraph_style_id,
                        None,
                        depth + 1,
                        Some(&mut nested_complex_field),
                        images.len(),
                    );
                    images.extend(next);
                    nested_complex_field.apply_pending(images);
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                b"fldChar" => {
                    apply_complex_field_char(&e, ctx, complex_field.as_deref_mut(), base_index);
                    skip_subtree(r);
                }
                b"instrText" => {
                    let instruction = read_text(r);
                    if let Some(tracker) = complex_field.as_deref_mut() {
                        tracker.push_instruction(&instruction);
                    }
                }
                b"t" => {
                    let in_result = complex_field
                        .as_deref()
                        .map(ComplexFieldTracker::in_result)
                        .unwrap_or(false);
                    if in_result {
                        *text_is_field_result = true;
                    }
                    text.push_str(&read_text(r));
                }
                b"sym" => {
                    if append_run_symbol(text, &e) {
                        let in_result = complex_field
                            .as_deref()
                            .map(ComplexFieldTracker::in_result)
                            .unwrap_or(false);
                        if in_result {
                            *text_is_field_result = true;
                        }
                    }
                    skip_subtree(r);
                }
                b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                    append_run_inline_marker(
                        text,
                        &e,
                        complex_field.as_deref(),
                        text_is_field_result,
                    );
                    skip_subtree(r);
                }
                b"drawing" | b"pict" | b"object" => {
                    let start = images.len();
                    let (img, txbx) = read_drawing(r, ctx, depth);
                    push_drawing_runs(images, img, txbx);
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                b"fldSimple" => {
                    let start = images.len();
                    images.extend(read_fldsimple(r, &e, ctx, paragraph_style_id, depth));
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                b"hyperlink" => {
                    let start = images.len();
                    images.extend(read_hyperlink(r, &e, ctx, paragraph_style_id, depth));
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                b"sdt" => {
                    let start = images.len();
                    let mut nested_complex_field = ComplexFieldTracker::default();
                    append_content_control_runs_with_complex(
                        r,
                        ctx,
                        paragraph_style_id,
                        None,
                        depth + 1,
                        images,
                        &mut nested_complex_field,
                    );
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                b"customXml" | b"ins" | b"moveTo" | b"smartTag" | b"sdtContent" | b"bdo"
                | b"dir" => {
                    let start = images.len();
                    let mut nested_complex_field = ComplexFieldTracker::default();
                    append_runs_container_with_complex(
                        r,
                        ctx,
                        paragraph_style_id,
                        None,
                        depth + 1,
                        images,
                        &mut nested_complex_field,
                    );
                    if complex_field
                        .as_deref()
                        .is_some_and(ComplexFieldTracker::in_result)
                    {
                        image_result_runs.extend(start..images.len());
                    }
                }
                b"AlternateContent" => append_run_alternate_content(
                    r,
                    ctx,
                    paragraph_style_id,
                    depth + 1,
                    complex_field.as_deref_mut(),
                    base_index,
                    &mut RunSink {
                        text: &mut *text,
                        text_is_field_result: &mut *text_is_field_result,
                        images: &mut *images,
                        image_result_runs: &mut *image_result_runs,
                    },
                ),
                _ => skip_subtree(r),
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"fldChar" => {
                    apply_complex_field_char(&e, ctx, complex_field.as_deref_mut(), base_index)
                }
                b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                    append_run_inline_marker(
                        text,
                        &e,
                        complex_field.as_deref(),
                        text_is_field_result,
                    );
                }
                b"sym" if append_run_symbol(text, &e) => {
                    let in_result = complex_field
                        .as_deref()
                        .map(ComplexFieldTracker::in_result)
                        .unwrap_or(false);
                    if in_result {
                        *text_is_field_result = true;
                    }
                }
                _ => {}
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_complex_field_char(
    e: &BytesStart<'_>,
    ctx: &Ctx<'_>,
    tracker: Option<&mut ComplexFieldTracker>,
    index: usize,
) {
    let Some(tracker) = tracker else {
        return;
    };
    match field_char_type(e).as_deref() {
        Some("begin") => tracker.begin(),
        Some("separate") => tracker.separate(index),
        Some("end") => tracker.end(ctx, index),
        _ => {}
    }
}

#[derive(Default)]
struct DirectRunProps {
    style_id: Option<String>,
    props: RunProps,
}

/// Read `<w:rPr>` formatting overrides and an optional character style.
fn read_rpr(r: &mut Xml<'_>) -> DirectRunProps {
    let mut p = DirectRunProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"rPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_rpr_alternate_content(r, &mut p);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => apply_rpr_child(&mut p, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"rPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    p
}

fn read_rpr_alternate_content(r: &mut Xml<'_>, props: &mut DirectRunProps) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_rpr_alternate_content_branch(r, props, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_rpr_alternate_content_branch(r: &mut Xml<'_>, props: &mut DirectRunProps, branch: &[u8]) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"rPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_rpr_alternate_content(r, props);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => apply_rpr_child(props, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_rpr_child(props: &mut DirectRunProps, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"rStyle" => props.style_id = attr_local_trimmed(e, b"val"),
        _ => super::styles::apply_run_props_child(&mut props.props, e),
    }
}

fn effective_run_props(
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    direct_props: &DirectRunProps,
) -> CharProps {
    let mut resolved = ctx
        .styles
        .resolved_run_props(paragraph_style_id, direct_props.style_id.as_deref());
    // Intentional ceiling: this uses layered property override. Full ECMA-376
    // toggle XOR semantics and table-style conditional `tblStylePr` are the
    // upgrade path.
    resolved.overlay(&direct_props.props);
    let mut props = CharProps::default();
    resolved.apply_to(&mut props);
    props
}

/// Scan a `<w:drawing>`/`<w:pict>` subtree for the first image or modeled chart
/// relationship plus any text-box (`w:txbxContent`) text. Honors
/// `mc:AlternateContent` (descends a single branch) so duplicate DrawingML/VML
/// representations are not counted twice.
#[derive(Default)]
struct DrawingReadState {
    image: Option<Image>,
    chart: Option<Chart>,
    text: String,
    anchor: DrawingAnchorOffset,
    extent: Option<DrawingExtent>,
    alt: Option<String>,
}

fn read_drawing(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> (Option<Image>, String) {
    let mut state = DrawingReadState::default();
    // Start from the caller's structural depth (not 0) so the recursion budget is
    // continuous across the drawing/text-box boundary.
    walk_drawing(r, ctx, &mut state, depth);
    if let Some(mut chart) = state.chart.take() {
        chart.alt = state.alt.take();
        if let Some(extent) = state.extent {
            chart.width_px = emu_to_px(extent.cx);
            chart.height_px = emu_to_px(extent.cy);
        }
        ctx.push_paragraph_chart(chart);
    }
    (state.image, state.text)
}

/// Recursively consume a drawing subtree through its `End`, collecting the first
/// blip image or modeled chart and all text-box text. `txbxContent` children hold
/// body-level content, parsed with [`read_blocks`] and flattened to text.
fn walk_drawing(r: &mut Xml<'_>, ctx: &Ctx<'_>, state: &mut DrawingReadState, depth: u32) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"anchor" => {
                    let previous_anchor = state.anchor;
                    let had_image = state.image.is_some();
                    state.anchor = DrawingAnchorOffset {
                        active: true,
                        ..DrawingAnchorOffset::default()
                    };
                    if depth < MAX_DEPTH {
                        walk_drawing(r, ctx, state, depth + 1);
                    } else {
                        skip_subtree(r);
                    }
                    if !had_image {
                        apply_floating_anchor_offset(&mut state.image, &state.anchor);
                    }
                    state.anchor = previous_anchor;
                }
                b"positionH" => {
                    state.anchor.horizontal_page_offset_emu = read_page_position_offset(r, &e);
                }
                b"positionV" => {
                    state.anchor.vertical_page_offset_emu = read_page_position_offset(r, &e);
                }
                b"txbxContent" => {
                    if depth < MAX_DEPTH {
                        let blocks = read_blocks(r, ctx, depth + 1);
                        append_blocks_text(&mut state.text, &blocks);
                    } else {
                        skip_subtree(r);
                    }
                }
                b"AlternateContent" => walk_alternate_content(r, ctx, state, depth + 1),
                _ => {
                    capture_drawing_alt(&e, &mut state.image, &mut state.alt);
                    capture_drawing_extent(&e, &mut state.extent);
                    if local(e.name().as_ref()) == b"xfrm" {
                        apply_image_rotation(&mut state.image, &e);
                    }
                    if state.image.is_none() {
                        state.image = blip_image(&e, ctx);
                        apply_drawing_alt(&mut state.image, &state.alt);
                        apply_floating_anchor_offset(&mut state.image, &state.anchor);
                    }
                    if state.chart.is_none() {
                        state.chart = drawing_chart(&e, ctx);
                    }
                    if depth < MAX_DEPTH {
                        walk_drawing(r, ctx, state, depth + 1);
                    } else {
                        skip_subtree(r);
                    }
                }
            },
            Ok(Event::Empty(e)) => {
                capture_drawing_alt(&e, &mut state.image, &mut state.alt);
                capture_drawing_extent(&e, &mut state.extent);
                if local(e.name().as_ref()) == b"xfrm" {
                    apply_image_rotation(&mut state.image, &e);
                }
                if state.image.is_none() {
                    state.image = blip_image(&e, ctx);
                    apply_drawing_alt(&mut state.image, &state.alt);
                    apply_floating_anchor_offset(&mut state.image, &state.anchor);
                }
                if state.chart.is_none() {
                    state.chart = drawing_chart(&e, ctx);
                }
            }
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn capture_drawing_alt(e: &BytesStart<'_>, img: &mut Option<Image>, alt: &mut Option<String>) {
    if alt.is_some() || local(e.name().as_ref()) != b"docPr" {
        return;
    }
    if let Some(description) = attr_local_trimmed(e, b"descr") {
        *alt = Some(description);
        apply_drawing_alt(img, alt);
    }
}

fn apply_drawing_alt(img: &mut Option<Image>, alt: &Option<String>) {
    if let (Some(image), Some(alt)) = (img.as_mut(), alt.as_ref()) {
        image.alt = Some(alt.clone());
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DrawingAnchorOffset {
    active: bool,
    horizontal_page_offset_emu: Option<i64>,
    vertical_page_offset_emu: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct DrawingExtent {
    cx: i64,
    cy: i64,
}

fn capture_drawing_extent(e: &BytesStart<'_>, extent: &mut Option<DrawingExtent>) {
    if extent.is_some() || local(e.name().as_ref()) != b"extent" {
        return;
    }
    let Some(cx) = attr_i64(e, b"cx").filter(|value| *value > 0) else {
        return;
    };
    let Some(cy) = attr_i64(e, b"cy").filter(|value| *value > 0) else {
        return;
    };
    *extent = Some(DrawingExtent { cx, cy });
}

fn emu_to_px(value: i64) -> Option<u32> {
    u32::try_from(value.saturating_add(4_762) / 9_525).ok()
}

fn drawing_chart(e: &BytesStart<'_>, ctx: &Ctx<'_>) -> Option<Chart> {
    if local(e.name().as_ref()) != b"chart" {
        return None;
    }
    let id = attr_local_trimmed(e, b"id")?;
    ctx.charts.get(&id).cloned()
}

fn read_page_position_offset(r: &mut Xml<'_>, start: &BytesStart<'_>) -> Option<i64> {
    let page_relative =
        attr_local_trimmed(start, b"relativeFrom").is_some_and(|value| value == "page");
    let mut offset = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"posOffset" => {
                if page_relative {
                    offset = read_i64_text(r);
                } else {
                    skip_subtree(r);
                }
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e))
                if matches!(local(e.name().as_ref()), b"positionH" | b"positionV") =>
            {
                break;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    offset
}

fn apply_floating_anchor_offset(img: &mut Option<Image>, anchor: &DrawingAnchorOffset) {
    if !anchor.active {
        return;
    }
    if let (Some(image), Some(x), Some(y)) = (
        img.as_mut(),
        anchor.horizontal_page_offset_emu,
        anchor.vertical_page_offset_emu,
    ) {
        image.floating_offset_emu = Some((x, y));
    }
}

fn apply_image_rotation(img: &mut Option<Image>, e: &BytesStart<'_>) {
    let Some(image) = img.as_mut() else {
        return;
    };
    let Some(rot) = attr_i64(e, b"rot") else {
        return;
    };
    let units = rot.rem_euclid(21_600_000);
    image.rotation_degrees = Some(((units + 30_000) / 60_000) as i32 % 360);
}

/// `mc:AlternateContent` wraps the SAME box as a `Choice` (DrawingML) and a
/// `Fallback` (VML); descend the first branch only so its text isn't doubled.
fn walk_alternate_content(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    state: &mut DrawingReadState,
    depth: u32,
) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"Choice" | b"Fallback" if !took => {
                    took = true;
                    if depth < MAX_DEPTH {
                        walk_drawing(r, ctx, state, depth + 1);
                    } else {
                        skip_subtree(r);
                    }
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// Append the flattened text of block-level nodes (a text box's paragraphs/tables)
/// to `out`, newline-separated.
fn blocks_text(blocks: &[Block]) -> String {
    let mut text = String::new();
    append_blocks_text(&mut text, blocks);
    text
}

fn append_blocks_text(out: &mut String, blocks: &[Block]) {
    for b in blocks {
        let chunk = match b {
            Block::Paragraph(p) => p.text(),
            Block::Table(t) => t
                .rows
                .iter()
                .flat_map(|row| row.cells.iter().map(|c| c.text()))
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {
                String::new()
            }
        };
        if !chunk.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&chunk);
        }
    }
}

/// `<a:blip r:embed>` (DrawingML) or `<v:imagedata r:id>` (VML) → the extracted
/// image for that relationship id, if it is one we extracted.
fn blip_image(e: &BytesStart<'_>, ctx: &Ctx<'_>) -> Option<Image> {
    let id = match local(e.name().as_ref()) {
        b"blip" => attr_local(e, b"embed")?,
        b"imagedata" => attr_local(e, b"id")?,
        _ => return None,
    };
    ctx.media.get(id.trim()).cloned()
}

/// Read `<w:hyperlink>`: resolve its target (external `r:id` rel, or `#anchor`)
/// and tag its runs with the link.
fn read_hyperlink(
    r: &mut Xml<'_>,
    start: &BytesStart<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    depth: u32,
) -> Vec<Run> {
    let url = hyperlink_url(start, ctx);
    read_runs_container_with_complex(r, ctx, paragraph_style_id, url.as_deref(), depth + 1, false)
}

fn hyperlink_url(start: &BytesStart<'_>, ctx: &Ctx<'_>) -> Option<String> {
    if let Some(id) = attr_local_trimmed(start, b"id") {
        if let Some((target, _external)) = ctx.rels.get(&id) {
            return Some(target.clone());
        }
    }
    attr_local_trimmed(start, b"anchor").map(|a| format!("#{a}"))
}

/// Read `<w:fldSimple>`: hyperlinks keep link semantics; other simple fields
/// keep normalized instructions, including formatted empty marker results.
fn read_fldsimple(
    r: &mut Xml<'_>,
    start: &BytesStart<'_>,
    ctx: &Ctx<'_>,
    paragraph_style_id: Option<&str>,
    depth: u32,
) -> Vec<Run> {
    ctx.last_formula_field_used_table_context.set(false);
    let source_instruction = attr_local(start, b"instr").unwrap_or_default();
    let url = hyperlink_instr_url(&source_instruction);
    let instruction = normalized_field_instruction(&source_instruction);
    let preserve_instruction = url.is_none()
        && !instruction.is_empty()
        && preserves_computed_field_instruction(&instruction, ctx);
    let mut runs = read_runs_container_with_complex(
        r,
        ctx,
        paragraph_style_id,
        url.as_deref(),
        depth + 1,
        preserve_instruction,
    );
    if url.is_none() && !instruction.is_empty() {
        let current_result = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        let computed = computed_simple_field_result(&instruction, ctx, &current_result);
        let preserve_result_instruction =
            computed.is_some() && preserves_computed_field_instruction(&instruction, ctx);
        if runs.is_empty() {
            if let Some(text) = computed {
                runs.push(computed_simple_field_run(
                    instruction.clone(),
                    text,
                    preserve_result_instruction,
                ));
            } else {
                runs.push(empty_simple_field_run(
                    instruction.clone(),
                    unsupported_simple_field_reason_hint(&instruction, ctx),
                ));
            }
            return runs;
        }
        for (index, run) in runs.iter_mut().enumerate() {
            if let Some(text) = computed.as_deref() {
                run.field = if index == 0 && preserve_result_instruction {
                    FieldRole::Simple {
                        instruction: instruction.clone(),
                    }
                } else {
                    FieldRole::Other
                };
                run.field_unsupported_reason = None;
                run.text = if index == 0 {
                    text.to_string()
                } else {
                    String::new()
                };
            } else {
                run.field = FieldRole::Simple {
                    instruction: instruction.clone(),
                };
                run.field_unsupported_reason =
                    unsupported_simple_field_reason_hint(&instruction, ctx);
            }
        }
    }
    runs
}

fn read_empty_fldsimple(start: &BytesStart<'_>, ctx: &Ctx<'_>) -> Option<Run> {
    let instruction =
        normalized_field_instruction(&attr_local(start, b"instr").unwrap_or_default());
    if instruction.is_empty() {
        return None;
    }
    let computed = computed_simple_field_result(&instruction, ctx, "");
    let preserve_instruction =
        computed.is_some() && preserves_computed_field_instruction(&instruction, ctx);
    computed
        .map(|text| computed_simple_field_run(instruction.clone(), text, preserve_instruction))
        .or_else(|| {
            Some(empty_simple_field_run(
                instruction.clone(),
                unsupported_simple_field_reason_hint(&instruction, ctx),
            ))
        })
}

fn push_empty_fldsimple_run(runs: &mut Vec<Run>, start: &BytesStart<'_>, ctx: &Ctx<'_>) {
    if let Some(run) = read_empty_fldsimple(start, ctx) {
        runs.push(run);
    }
}

fn unsupported_simple_field_reason_hint(
    instruction: &str,
    ctx: &Ctx<'_>,
) -> Option<FieldUnsupportedReason> {
    if let Some(reason) = unsupported_ref_reason_hint(instruction, ctx) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_page_ref_reason_hint(instruction, ctx) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_note_ref_reason_hint(instruction, ctx) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_toc_reason_hint(instruction, ctx) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_toc_entry_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_page_reason_hint(instruction, ctx) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_reference_index_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_document_structure_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_dynamic_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_compatibility_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_inserted_content_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_mail_merge_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_barcode_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_form_field_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_display_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_action_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_sequence_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_numbering_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_document_info_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_filename_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_hyperlink_reason_hint(instruction) {
        return Some(reason);
    }
    if let Some(reason) = unsupported_merge_field_reason_hint(instruction) {
        return Some(reason);
    }
    None
}

fn unsupported_ref_reason_hint(instruction: &str, ctx: &Ctx<'_>) -> Option<FieldUnsupportedReason> {
    let field_bookmarks = ctx.field_bookmarks.borrow();
    let syntax = if FieldKind::from_instruction(instruction) == FieldKind::Ref {
        let Some(syntax) = ref_field_syntax(instruction) else {
            return Some(FieldUnsupportedReason::UnsupportedSwitch);
        };
        syntax
    } else {
        let syntax = direct_ref_field_syntax(instruction)?;
        if !ctx.bookmark_names.contains(&syntax.target)
            && !field_bookmarks.contains_key(&syntax.target)
        {
            return None;
        }
        syntax
    };
    if ctx.bookmark_names.contains(&syntax.target) || field_bookmarks.contains_key(&syntax.target) {
        Some(FieldUnsupportedReason::NoComputedResult)
    } else {
        Some(FieldUnsupportedReason::UnresolvedBookmark)
    }
}

fn unsupported_page_ref_reason_hint(
    instruction: &str,
    ctx: &Ctx<'_>,
) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::PageRef {
        return None;
    }
    let Some(syntax) = page_ref_field_syntax(instruction) else {
        return Some(FieldUnsupportedReason::UnsupportedSwitch);
    };
    if ctx
        .page_ref_context
        .target_uses_unsupported_display_format(&syntax.target)
    {
        return Some(FieldUnsupportedReason::UnsupportedSwitch);
    }
    if ctx.bookmark_names.contains(&syntax.target) {
        Some(FieldUnsupportedReason::NoComputedResult)
    } else {
        Some(FieldUnsupportedReason::UnresolvedBookmark)
    }
}

fn unsupported_note_ref_reason_hint(
    instruction: &str,
    ctx: &Ctx<'_>,
) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::NoteRef {
        return None;
    }
    let Some(syntax) = note_ref_field_syntax(instruction) else {
        return Some(FieldUnsupportedReason::UnsupportedSwitch);
    };
    if ctx.note_ref_context.target_is_note_marker(&syntax.target)
        || ctx.bookmark_names.contains(&syntax.target)
    {
        Some(FieldUnsupportedReason::NoComputedResult)
    } else {
        Some(FieldUnsupportedReason::UnresolvedBookmark)
    }
}

fn unsupported_toc_reason_hint(instruction: &str, ctx: &Ctx<'_>) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::Toc {
        return None;
    }
    let Some(syntax) = toc_field_syntax(instruction) else {
        return Some(FieldUnsupportedReason::UnsupportedSwitch);
    };
    match syntax.bookmark {
        Some(target) if ctx.bookmark_names.contains(&target) => {
            Some(FieldUnsupportedReason::NoComputedResult)
        }
        Some(_) => Some(FieldUnsupportedReason::UnresolvedBookmark),
        None => Some(FieldUnsupportedReason::NoComputedResult),
    }
}

fn unsupported_toc_entry_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::TocEntry {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(
        super::fields::supports_toc_entry_field_syntax(instruction),
    ))
}

fn unsupported_page_reason_hint(
    instruction: &str,
    ctx: &Ctx<'_>,
) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::Page {
        return None;
    }
    if !super::fields::supports_page_field_syntax(instruction) {
        return Some(FieldUnsupportedReason::UnsupportedSwitch);
    }
    if ctx
        .last_page_field_unsupported_display_format
        .borrow()
        .unwrap_or(false)
    {
        Some(FieldUnsupportedReason::UnsupportedSwitch)
    } else {
        Some(FieldUnsupportedReason::NoComputedResult)
    }
}

fn unsupported_reference_index_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    let FieldKind::ReferenceIndex(kind) = FieldKind::from_instruction(instruction) else {
        return None;
    };
    if is_generated_reference_index_kind(&kind) {
        return Some(unsupported_opaque_field_reason_hint(
            instruction,
            is_generated_reference_index_kind,
        ));
    }
    if is_reference_index_marker_kind(&kind) {
        return Some(FieldUnsupportedReason::UnsupportedSwitch);
    }
    None
}

fn is_generated_reference_index_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "BIBLIOGRAPHY" | "CITATION" | "INDEX" | "TOA"
    )
}

fn is_reference_index_marker_kind(kind: &str) -> bool {
    matches!(kind.to_ascii_uppercase().as_str(), "RD" | "TA" | "XE")
}

fn unsupported_document_structure_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    let FieldKind::DocumentStructure(kind) = FieldKind::from_instruction(instruction) else {
        return None;
    };
    if kind.eq_ignore_ascii_case("REVNUM") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_revision_number_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("SECTION") || kind.eq_ignore_ascii_case("SECTIONPAGES") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::is_section_field_instruction(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("STYLEREF") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_style_ref_field_syntax(instruction),
        ));
    }
    Some(FieldUnsupportedReason::NoComputedResult)
}

fn unsupported_dynamic_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    let FieldKind::Dynamic(kind) = FieldKind::from_instruction(instruction) else {
        return None;
    };
    if kind == "=" {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_formula_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("COMPARE") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_compare_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("IF") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_if_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("QUOTE") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_quote_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("FILLIN") || kind.eq_ignore_ascii_case("ASK") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_prompt_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("SET") {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_set_field_syntax(instruction),
        ));
    }
    if kind.eq_ignore_ascii_case("NEXT")
        || kind.eq_ignore_ascii_case("NEXTIF")
        || kind.eq_ignore_ascii_case("SKIPIF")
    {
        return Some(unsupported_syntax_field_reason_hint(
            super::fields::supports_merge_control_field_syntax(instruction),
        ));
    }
    Some(FieldUnsupportedReason::NoComputedResult)
}

fn unsupported_compatibility_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Compatibility(_)
    ) {
        return None;
    }
    Some(unsupported_opaque_field_reason_hint(
        instruction,
        is_compatibility_kind,
    ))
}

fn is_compatibility_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "ADDIN" | "DATA" | "GLOSSARY" | "HTMLACTIVEX" | "PRIVATE"
    )
}

fn unsupported_inserted_content_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::InsertedContent(_)
    ) {
        return None;
    }
    Some(unsupported_opaque_field_reason_hint(
        instruction,
        is_inserted_content_kind,
    ))
}

fn is_inserted_content_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "AUTOTEXT"
            | "AUTOTEXTLIST"
            | "DATABASE"
            | "DDE"
            | "DDEAUTO"
            | "EMBED"
            | "IMPORT"
            | "INCLUDE"
            | "INCLUDEPICTURE"
            | "INCLUDETEXT"
            | "LINK"
    )
}

fn unsupported_mail_merge_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::MailMerge(_)
    ) {
        return None;
    }
    Some(unsupported_opaque_field_reason_hint(
        instruction,
        is_mail_merge_kind,
    ))
}

fn unsupported_opaque_field_reason_hint(
    instruction: &str,
    is_kind: fn(&str) -> bool,
) -> FieldUnsupportedReason {
    if opaque_field_syntax(instruction, is_kind) {
        FieldUnsupportedReason::NoComputedResult
    } else {
        FieldUnsupportedReason::UnsupportedSwitch
    }
}

fn is_mail_merge_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "ADDRESSBLOCK" | "GREETINGLINE" | "MERGEREC" | "MERGESEQ"
    )
}

fn unsupported_barcode_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Barcode(_)
    ) {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(barcode_field_syntax(
        instruction,
    )))
}

fn unsupported_form_field_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::FormField(_)
    ) {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(
        legacy_form_field_syntax(instruction).is_some(),
    ))
}

fn unsupported_syntax_field_reason_hint(valid_syntax: bool) -> FieldUnsupportedReason {
    if valid_syntax {
        FieldUnsupportedReason::NoComputedResult
    } else {
        FieldUnsupportedReason::UnsupportedSwitch
    }
}

fn unsupported_display_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Display(_)
    ) {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(
        super::fields::supports_display_field_syntax(instruction),
    ))
}

fn unsupported_action_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Action(_)
    ) {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(
        super::fields::supports_action_field_syntax(instruction),
    ))
}

fn unsupported_sequence_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::Sequence {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(
        super::fields::supports_sequence_field_syntax(instruction),
    ))
}

fn unsupported_numbering_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Numbering(_)
    ) {
        return None;
    }
    Some(unsupported_syntax_field_reason_hint(
        super::fields::supports_numbering_field_syntax(instruction),
    ))
}

fn unsupported_document_info_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if !matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::DocumentInfo(_)
    ) {
        return None;
    }
    (!super::fields::supports_document_info_field_syntax(instruction))
        .then_some(FieldUnsupportedReason::UnsupportedSwitch)
}

fn unsupported_filename_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::Filename {
        return None;
    }
    (!super::fields::supports_filename_field_syntax(instruction))
        .then_some(FieldUnsupportedReason::UnsupportedSwitch)
}

fn unsupported_hyperlink_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::Hyperlink {
        return None;
    }
    hyperlink_instr_url(instruction)
        .is_none()
        .then_some(FieldUnsupportedReason::UnsupportedSwitch)
}

fn unsupported_merge_field_reason_hint(instruction: &str) -> Option<FieldUnsupportedReason> {
    if FieldKind::from_instruction(instruction) != FieldKind::MergeField {
        return None;
    }
    (!merge_field_syntax(instruction)).then_some(FieldUnsupportedReason::UnsupportedSwitch)
}

fn computed_simple_field_result(
    instruction: &str,
    ctx: &Ctx<'_>,
    current_result: &str,
) -> Option<String> {
    ctx.last_formula_field_used_table_context.set(false);
    let (ref_position, note_ref_position) = ref_field_positions(instruction, ctx);
    let ref_result = {
        let field_bookmarks = ctx.field_bookmarks.borrow();
        let ref_ctx = super::fields::RefResultContext {
            bookmarks: ctx.ref_targets,
            ref_positions: ctx.ref_position_context,
            ref_numbers: ctx.ref_number_context,
            note_refs: ctx.note_ref_context,
            field_bookmarks: &field_bookmarks,
        };
        super::fields::computed_ref_result(instruction, &ref_ctx, ref_position, note_ref_position)
    };
    ref_result
        .or_else(|| {
            let position = if FieldKind::from_instruction(instruction) == FieldKind::Page {
                let index = {
                    let mut cursor = ctx.page_field_cursor.borrow_mut();
                    let index = *cursor;
                    *cursor += 1;
                    index
                };
                ctx.last_page_field_unsupported_display_format.replace(Some(
                    ctx.page_ref_context
                        .page_field_uses_unsupported_display_format(index),
                ));
                ctx.page_ref_context.page_field_position(index)
            } else {
                None
            };
            super::fields::computed_page_result(instruction, position)
        })
        .or_else(|| {
            let (position, order) =
                if FieldKind::from_instruction(instruction) == FieldKind::PageRef {
                    let index = {
                        let mut cursor = ctx.page_ref_field_cursor.borrow_mut();
                        let index = *cursor;
                        *cursor += 1;
                        index
                    };
                    (
                        ctx.page_ref_context.field_position(index),
                        ctx.page_ref_context.field_order(index),
                    )
                } else {
                    (None, None)
                };
            super::fields::computed_page_ref_result(
                instruction,
                ctx.page_ref_context,
                position,
                order,
            )
        })
        .or_else(|| {
            let position = if FieldKind::from_instruction(instruction) == FieldKind::NoteRef {
                let index = {
                    let mut cursor = ctx.note_ref_field_cursor.borrow_mut();
                    let index = *cursor;
                    *cursor += 1;
                    index
                };
                ctx.note_ref_context.field_position(index)
            } else {
                None
            };
            super::fields::computed_note_ref_result(instruction, ctx.note_ref_context, position)
        })
        .or_else(|| {
            if FieldKind::from_instruction(instruction) == FieldKind::Sequence {
                let heading_scope = *ctx.sequence_heading_counts.borrow();
                let mut counters = ctx.sequence_counters.borrow_mut();
                let mut heading_scopes = ctx.sequence_heading_scopes.borrow_mut();
                super::fields::computed_sequence_result_with_heading_scope(
                    instruction,
                    &mut counters,
                    Some(heading_scope),
                    &mut heading_scopes,
                )
            } else {
                None
            }
        })
        .or_else(|| super::fields::computed_toc_entry_result(instruction))
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Numbering(kind)
                    if kind == "AUTONUM"
                        || kind == "AUTONUMLGL"
                        || kind == "AUTONUMOUT"
                        || kind == "BIDIOUTLINE"
            ) {
                let mut counter = ctx.autonum_counter.borrow_mut();
                super::fields::computed_numbering_result(instruction, &mut counter)
            } else {
                None
            }
        })
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Numbering(kind) if kind == "LISTNUM"
            ) {
                let mut counter = ctx.listnum_counter.borrow_mut();
                super::fields::computed_listnum_result(instruction, &mut counter)
            } else {
                None
            }
        })
        .or_else(|| {
            let position = if super::fields::is_section_field_instruction(instruction) {
                let index = {
                    let mut cursor = ctx.section_field_cursor.borrow_mut();
                    let index = *cursor;
                    *cursor += 1;
                    index
                };
                ctx.section_context.field_position(index)
            } else {
                None
            };
            super::fields::computed_section_result(instruction, position)
        })
        .or_else(|| {
            super::fields::computed_revision_number_result(instruction, ctx.core_properties)
        })
        .or_else(|| {
            let position = if super::fields::is_style_ref_field_instruction(instruction) {
                let index = {
                    let mut cursor = ctx.style_ref_field_cursor.borrow_mut();
                    let index = *cursor;
                    *cursor += 1;
                    index
                };
                ctx.style_ref_context.field_position(index)
            } else {
                None
            };
            super::fields::computed_style_ref_result(instruction, ctx.style_ref_context, position)
        })
        .or_else(|| computed_dynamic_field_result(instruction, ctx))
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Dynamic(kind) if kind == "ASK"
            ) {
                let mut field_bookmarks = ctx.field_bookmarks.borrow_mut();
                super::fields::computed_ask_result(instruction, &mut field_bookmarks)
            } else {
                None
            }
        })
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Dynamic(kind) if kind == "SET"
            ) {
                let mut field_bookmarks = ctx.field_bookmarks.borrow_mut();
                super::fields::computed_set_result(instruction, &mut field_bookmarks)
            } else {
                None
            }
        })
        .or_else(|| {
            super::fields::computed_document_info_result(
                instruction,
                ctx.core_properties,
                ctx.custom_properties,
                ctx.document_variables,
                ctx.extended_properties,
                ctx.file_size_bytes,
            )
        })
        .or_else(|| super::fields::computed_reference_index_result(instruction))
        .or_else(|| super::fields::computed_display_result(instruction))
        .or_else(|| super::fields::computed_action_result(instruction))
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::FormField(_)
            ) {
                let index = {
                    let mut cursor = ctx.form_field_cursor.borrow_mut();
                    let index = *cursor;
                    *cursor += 1;
                    index
                };
                super::fields::computed_legacy_form_result(
                    instruction,
                    current_result,
                    ctx.legacy_form_context,
                    index,
                )
            } else {
                None
            }
        })
        .or_else(|| {
            let (position, note_ref_position) =
                if super::fields::is_direct_bookmark_ref_field_instruction(instruction) {
                    let index = {
                        let mut cursor = ctx.ref_field_cursor.borrow_mut();
                        let index = *cursor;
                        *cursor += 1;
                        index
                    };
                    (
                        ctx.ref_position_context.field_position(index),
                        ctx.note_ref_context.ref_field_position(index),
                    )
                } else {
                    (None, None)
                };
            let field_bookmarks = ctx.field_bookmarks.borrow();
            let ref_ctx = super::fields::RefResultContext {
                bookmarks: ctx.ref_targets,
                ref_positions: ctx.ref_position_context,
                ref_numbers: ctx.ref_number_context,
                note_refs: ctx.note_ref_context,
                field_bookmarks: &field_bookmarks,
            };
            super::fields::computed_direct_bookmark_ref_result(
                instruction,
                &ref_ctx,
                position,
                note_ref_position,
            )
        })
        .or_else(|| {
            super::fields::computed_toc_result(instruction, ctx.toc_entries, ctx.bookmark_names)
        })
}

fn computed_dynamic_field_result(instruction: &str, ctx: &Ctx<'_>) -> Option<String> {
    if matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Dynamic(kind) if kind == "="
    ) {
        let index = {
            let mut cursor = ctx.formula_field_cursor.borrow_mut();
            let index = *cursor;
            *cursor += 1;
            index
        };
        if let Some(result) = ctx.table_formula_context.field_result(index) {
            ctx.last_formula_field_used_table_context.set(true);
            return Some(result);
        }
        let field_bookmarks = ctx.field_bookmarks.borrow();
        return super::fields::computed_formula_result_with_bookmark_context(
            instruction,
            ctx.ref_targets,
            &field_bookmarks,
        );
    }
    if matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Dynamic(kind) if kind == "IF" || kind == "COMPARE"
    ) {
        let field_bookmarks = ctx.field_bookmarks.borrow();
        return super::fields::computed_if_compare_result_with_bookmark_context(
            instruction,
            ctx.ref_targets,
            &field_bookmarks,
        );
    }
    if matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Dynamic(kind) if kind == "NEXTIF" || kind == "SKIPIF"
    ) {
        let field_bookmarks = ctx.field_bookmarks.borrow();
        return super::fields::computed_merge_control_result_with_bookmark_context(
            instruction,
            ctx.ref_targets,
            &field_bookmarks,
        );
    }
    let field_bookmarks = ctx.field_bookmarks.borrow();
    super::fields::computed_dynamic_result_with_bookmarks(instruction, &field_bookmarks)
}

fn ref_field_positions(
    instruction: &str,
    ctx: &Ctx<'_>,
) -> (
    Option<super::fields::RefFieldPosition>,
    Option<super::fields::NoteRefFieldPosition>,
) {
    if FieldKind::from_instruction(instruction) != FieldKind::Ref {
        return (None, None);
    }
    let index = {
        let mut cursor = ctx.ref_field_cursor.borrow_mut();
        let index = *cursor;
        *cursor += 1;
        index
    };
    (
        ctx.ref_position_context.field_position(index),
        ctx.note_ref_context.ref_field_position(index),
    )
}

fn apply_sequence_heading_scope(pp: &PPr, ctx: &Ctx<'_>, applied: &mut bool) {
    if *applied {
        return;
    }
    let Some(level) = sequence_heading_level(pp, ctx.styles) else {
        return;
    };
    let mut counts = ctx.sequence_heading_counts.borrow_mut();
    counts[usize::from(level - 1)] = counts[usize::from(level - 1)].saturating_add(1);
    *applied = true;
}

fn sequence_heading_level(pp: &PPr, styles: &Styles) -> Option<u8> {
    match pp.outline {
        Some(level) if level <= 8 => Some(level + 1),
        Some(_) => None,
        None => pp
            .style_id
            .as_deref()
            .and_then(|style_id| styles.heading_level(style_id)),
    }
}

/// Extract a URL from a `HYPERLINK "…"` field instruction (matches the `.doc`
/// field-code parser).
pub(crate) fn hyperlink_instr_url(instr: &str) -> Option<String> {
    let target = crate::annotation::hyperlink_field_target(instr)?;
    Some(if hyperlink_instr_uses_anchor_target(instr) {
        format!("#{target}")
    } else {
        target
    })
}

fn hyperlink_instr_uses_anchor_target(instr: &str) -> bool {
    let tokens = instruction_parts(instr);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("HYPERLINK") {
        return false;
    }
    let Some(first_arg) = parts.next() else {
        return false;
    };
    let lower = first_arg.to_ascii_lowercase();
    lower == "\\l" || lower.starts_with("\\l")
}

/// Resolve paragraph-level properties (heading level, alignment, list) from the
/// collected `w:pPr` fields — mirroring `assemble.rs::take_paragraph` precedence
/// (explicit outline level wins; a heading suppresses list rendering).
fn resolve_tab_stops(changes: impl IntoIterator<Item = TabStop>) -> Vec<TabStop> {
    let mut resolved: Vec<TabStop> = Vec::new();
    for change in changes {
        resolved.retain(|stop| (stop.position_pt - change.position_pt).abs() >= 0.01);
        if change.alignment != TabAlignment::Clear && resolved.len() < MAX_TAB_STOPS {
            resolved.push(change);
        }
    }
    resolved.sort_by(|left, right| left.position_pt.total_cmp(&right.position_pt));
    resolved
}

fn finalize_paragraph(
    runs: Vec<Run>,
    pp: PPr,
    ctx: &Ctx<'_>,
) -> (
    Paragraph,
    PaginationHint,
    Vec<TabStop>,
    Option<LineSpacingHint>,
) {
    let PPr {
        style_id,
        num,
        jc,
        outline,
        layout: direct_layout,
        mut indent,
        indent_start_pt,
        indent_end_pt,
        bidi,
        keep_next,
        keep_lines,
        widow_control,
        tab_stops,
        section: _,
    } = pp;
    let inherited = ctx.styles.paragraph_props(style_id.as_deref());
    let mut resolved_layout = inherited.layout;
    resolved_layout.overlay(direct_layout);
    let resolved_tab_stops =
        resolve_tab_stops(inherited.tab_stops.iter().copied().chain(tab_stops));
    let pagination = PaginationHint {
        keep_next: keep_next.or(inherited.keep_next).unwrap_or(false),
        keep_lines: keep_lines.or(inherited.keep_lines).unwrap_or(false),
        widow_control: widow_control.or(inherited.widow_control).unwrap_or(true),
    };
    // A paragraph style may declare list membership; the paragraph's own
    // `w:numPr` wins when it has one.
    let num = num.or_else(|| inherited.num.clone());
    let bidi = bidi.or(inherited.bidi).unwrap_or(false);
    let jc = jc.or(inherited.jc);
    let direct_logical_left = if bidi { indent_end_pt } else { indent_start_pt };
    let direct_logical_right = if bidi { indent_start_pt } else { indent_end_pt };
    let inherited_logical_left = if bidi {
        inherited.indent_end_pt
    } else {
        inherited.indent_start_pt
    };
    let inherited_logical_right = if bidi {
        inherited.indent_start_pt
    } else {
        inherited.indent_end_pt
    };
    indent.left_pt = indent
        .left_pt
        .or(direct_logical_left)
        .or(inherited.indent_left_pt)
        .or(inherited_logical_left);
    indent.right_pt = indent
        .right_pt
        .or(direct_logical_right)
        .or(inherited.indent_right_pt)
        .or(inherited_logical_right);
    resolved_layout.apply_indent(&mut indent);
    // A heading is determined by its style. `w:outlineLvl` records an outline
    // position only, which Word also sets on ordinary body paragraphs, so it no
    // longer promotes a paragraph to a heading on its own.
    let heading_level = style_id
        .as_deref()
        .and_then(|s| ctx.styles.heading_level(s));
    let style_name = style_id
        .as_deref()
        .and_then(|s| ctx.styles.name(s))
        .map(str::to_string);
    let style_id = style_id.filter(|style_id| !style_id.is_empty());
    let align = match jc.as_deref() {
        Some("center") => Align::Center,
        Some("left") => Align::Left,
        Some("right") => Align::Right,
        Some("start") => {
            if bidi {
                Align::Right
            } else {
                Align::Left
            }
        }
        Some("end") => {
            if bidi {
                Align::Left
            } else {
                Align::Right
            }
        }
        Some("both") | Some("distribute") => Align::Justify,
        _ if bidi => Align::Right,
        _ => Align::Left,
    };
    // A heading takes precedence over list-item rendering. `numId == "0"` is the
    // OOXML "no list" sentinel.
    let list = if heading_level.is_some() {
        None
    } else {
        match num {
            Some((num_id, ilvl)) if num_id != "0" => {
                let ordered = ctx.numbering.ordered(&num_id, ilvl).unwrap_or(true);
                // Advance the live counters (document order) and format the label.
                let label = {
                    let mut map = ctx.counters.borrow_mut();
                    let c = map.entry(num_id.clone()).or_insert([0; 9]);
                    ctx.numbering.label(&num_id, ilvl, c).unwrap_or_default()
                };
                Some(ListInfo {
                    level: ilvl,
                    ordered,
                    label,
                    marker_font: ctx.numbering.marker_font(&num_id, ilvl).map(str::to_owned),
                })
            }
            _ => None,
        }
    };
    let spacing = resolved_layout.spacing();
    let line_spacing = resolved_layout.line_spacing_hint();
    let shading = resolved_layout.shading();
    let page_break_before = resolved_layout.page_break_before();
    let paragraph = Paragraph {
        props: ParaProps {
            style_id,
            style_name,
            heading_level,
            align,
            outline_level: outline,
            list,
            spacing,
            indent,
            shading,
            page_break_before,
            bidi,
        },
        runs,
    };
    (paragraph, pagination, resolved_tab_stops, line_spacing)
}

const DEFAULT_HORIZONTAL_CELL_MARGIN_TWIPS: u32 = 115;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CellMarginSpec {
    top: Option<u32>,
    trailing: Option<u32>,
    bottom: Option<u32>,
    leading: Option<u32>,
}

#[derive(Clone, Copy, Default)]
struct MarginDeclaration {
    present: bool,
    value: Option<u32>,
}

impl MarginDeclaration {
    fn record(&mut self, value: Option<u32>) {
        self.present = true;
        if value.is_some() {
            self.value = value;
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ParsedCellMargins {
    top: Option<u32>,
    legacy_trailing: Option<u32>,
    trailing: MarginDeclaration,
    bottom: Option<u32>,
    legacy_leading: Option<u32>,
    leading: MarginDeclaration,
}

impl ParsedCellMargins {
    fn finish(self) -> CellMarginSpec {
        CellMarginSpec {
            top: self.top,
            trailing: if self.trailing.present {
                self.trailing.value
            } else {
                self.legacy_trailing
            },
            bottom: self.bottom,
            leading: if self.leading.present {
                self.leading.value
            } else {
                self.legacy_leading
            },
        }
    }
}

impl CellMarginSpec {
    pub(crate) fn overlay(&mut self, other: Self) {
        if other.top.is_some() {
            self.top = other.top;
        }
        if other.trailing.is_some() {
            self.trailing = other.trailing;
        }
        if other.bottom.is_some() {
            self.bottom = other.bottom;
        }
        if other.leading.is_some() {
            self.leading = other.leading;
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        self.top.is_none()
            && self.trailing.is_none()
            && self.bottom.is_none()
            && self.leading.is_none()
    }

    #[cfg(test)]
    pub(crate) fn logical_values(self) -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
        (self.top, self.trailing, self.bottom, self.leading)
    }
}

fn resolve_cell_margins(
    style: CellMarginSpec,
    table: CellMarginSpec,
    cell: CellMarginSpec,
    bidi_visual: bool,
    defaults_active: bool,
) -> Option<CellMargins> {
    if !defaults_active {
        return None;
    }
    let mut effective = CellMarginSpec {
        top: Some(0),
        trailing: Some(DEFAULT_HORIZONTAL_CELL_MARGIN_TWIPS),
        bottom: Some(0),
        leading: Some(DEFAULT_HORIZONTAL_CELL_MARGIN_TWIPS),
    };
    effective.overlay(style);
    effective.overlay(table);
    effective.overlay(cell);
    let top = effective.top.unwrap_or(0);
    let trailing = effective
        .trailing
        .unwrap_or(DEFAULT_HORIZONTAL_CELL_MARGIN_TWIPS);
    let bottom = effective.bottom.unwrap_or(0);
    let leading = effective
        .leading
        .unwrap_or(DEFAULT_HORIZONTAL_CELL_MARGIN_TWIPS);
    let (right, left) = if bidi_visual {
        (leading, trailing)
    } else {
        (trailing, leading)
    };
    Some(CellMargins {
        top,
        right,
        bottom,
        left,
    })
}

#[derive(Clone, Copy)]
enum CellStyleRegionSpec {
    Named(NamedCellStyleRegions),
    Transitional(TableCellStyleRegions),
}

impl CellStyleRegionSpec {
    fn resolve(self, bidi_visual: bool) -> TableCellStyleRegions {
        match self {
            Self::Transitional(regions) => regions,
            Self::Named(named) => named.resolve(bidi_visual),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct NamedCellStyleRegions {
    regions: TableCellStyleRegions,
    first_row_last_column: bool,
    first_row_first_column: bool,
    last_row_last_column: bool,
    last_row_first_column: bool,
}

impl NamedCellStyleRegions {
    fn resolve(self, bidi_visual: bool) -> TableCellStyleRegions {
        let mut regions = self.regions;
        if bidi_visual {
            regions.north_west = self.first_row_last_column;
            regions.north_east = self.first_row_first_column;
            regions.south_west = self.last_row_last_column;
            regions.south_east = self.last_row_first_column;
        } else {
            regions.north_west = self.first_row_first_column;
            regions.north_east = self.first_row_last_column;
            regions.south_west = self.last_row_first_column;
            regions.south_east = self.last_row_last_column;
        }
        regions
    }
}

/// A streamed cell before vertical-merge resolution.
struct CellRaw {
    blocks: Vec<Block>,
    pagination: Vec<Option<PaginationHint>>,
    line_spacing: Vec<Option<LineSpacingHint>>,
    nested_tables: Vec<Option<TablePaginationHints>>,
    tab_stops: Vec<Vec<TabStop>>,
    column_break_offsets: Vec<Vec<usize>>,
    col_span: u16,
    vmerge: VMerge,
    shading: Option<Color>,
    shading_declared: bool,
    valign: VCell,
    valign_declared: bool,
    width_pct: Option<f32>,
    width_pct_declared: bool,
    margins: CellMarginSpec,
    style_regions: Option<CellStyleRegionSpec>,
}

struct RowRaw {
    cells: Vec<CellRaw>,
    props: RowProps,
}

#[derive(Clone, Copy, Default)]
struct RowProps {
    header: Option<bool>,
    cant_split: Option<bool>,
    style_regions: Option<TableRowStyleRegions>,
    cell_margins: Option<CellMarginSpec>,
}

impl RowProps {
    fn merge(&mut self, other: Self) {
        if other.header.is_some() {
            self.header = other.header;
        }
        if other.cant_split.is_some() {
            self.cant_split = other.cant_split;
        }
        if other.style_regions.is_some() {
            self.style_regions = other.style_regions;
        }
        self.overlay_cell_margins(other.cell_margins);
    }

    fn overlay_cell_margins(&mut self, other: Option<CellMarginSpec>) {
        let Some(other) = other else {
            return;
        };
        if let Some(current) = &mut self.cell_margins {
            current.overlay(other);
        } else {
            self.cell_margins = Some(other);
        }
    }

    fn pagination(self, style_cant_split: Option<bool>) -> TableRowPaginationHint {
        TableRowPaginationHint {
            cant_split: self.cant_split.or(style_cant_split).unwrap_or(false),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum VMerge {
    None,
    Restart,
    Continue,
}

/// Read a `<w:tbl>` and resolve merges into a [`Table`].
fn read_table_block(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
) -> Option<(Block, TablePaginationHints)> {
    ctx.suspend_block_captures();
    let (table, pagination) = read_table(r, ctx, depth);
    ctx.resume_block_captures();
    if table.rows.is_empty() {
        None
    } else {
        ctx.capture_table_block_hints(&pagination);
        Some((Block::Table(table), pagination))
    }
}

fn read_table(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> (Table, TablePaginationHints) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return (Table::default(), TablePaginationHints::default());
    }
    let mut rows: Vec<RowRaw> = Vec::new();
    let mut props = TableProps::default();
    let mut grid_widths = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"tblPr" => props = read_tblpr(r),
                b"tblGrid" => grid_widths = read_tbl_grid(r).unwrap_or_default(),
                b"tr" => rows.push(read_row(r, ctx, depth)),
                b"AlternateContent" => rows.extend(read_table_alternate_content_rows(
                    r,
                    ctx,
                    depth + 1,
                    &mut props,
                )),
                name if is_current_table_structural_wrapper(name) => {}
                _ => skip_subtree(r),
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tbl" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let row_count = rows.len();
    let table_look = props.look.unwrap_or_else(TableLook::word_default);
    let row_band_size = props
        .row_band_size
        .or_else(|| ctx.styles.table_row_band_size(props.style_id.as_deref()))
        // Word uses zero, rather than ECMA-376's one, when the property is omitted.
        .unwrap_or(0);
    let col_band_size = props
        .col_band_size
        .or_else(|| ctx.styles.table_col_band_size(props.style_id.as_deref()))
        // Word also uses zero for an omitted column-band size.
        .unwrap_or(0);
    let style_geometry = ctx.styles.table_geometry(props.style_id.as_deref());
    if !props.fixed_layout_declared {
        props.fixed_layout = style_geometry.fixed_layout.unwrap_or(props.fixed_layout);
    }
    if !props.bidi_visual_declared {
        props.bidi_visual = style_geometry.bidi_visual.unwrap_or(props.bidi_visual);
    }
    let row_regions: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            row.props.style_regions.unwrap_or_else(|| {
                table_look.row_regions(
                    index,
                    row_count,
                    row_band_size,
                    col_band_size,
                    grid_widths.len().max(
                        rows.iter()
                            .map(|row| {
                                row.cells
                                    .iter()
                                    .map(|cell| cell.col_span as usize)
                                    .sum::<usize>()
                            })
                            .max()
                            .unwrap_or(0),
                    ),
                    row.props.header.unwrap_or(false),
                    props.bidi_visual,
                )
            })
        })
        .collect();
    let row_pagination = rows
        .iter()
        .zip(&row_regions)
        .map(|(row, &regions)| {
            let style_cant_split = ctx
                .styles
                .table_row_cant_split_for_regions(props.style_id.as_deref(), regions);
            row.props.pagination(style_cant_split)
        })
        .collect();
    let style_cell_props = ctx.styles.table_cell_props(props.style_id.as_deref());
    // A table style's geometry fills in only what the table left unset.
    props.width_pct = props.width_pct.or(style_geometry.width_pct);
    props.indent_twips = props.indent_twips.or(style_geometry.indent_twips);
    props.align = props.align.or(style_geometry.align);
    // A table style's borders apply unless the table declares its own.
    if !props.borders_declared {
        if let Some(borders) = ctx.styles.table_borders(props.style_id.as_deref()) {
            props.border_color = borders.0;
            props.border_colors = borders.1;
            props.border_size_eighths = borders.2;
            props.border_sizes = borders.3;
            props.border_style = borders.4;
            props.border_styles = borders.5;
        }
    }
    let (
        table,
        cell_pagination,
        cell_line_spacing,
        cell_column_breaks,
        nested_pagination,
        cell_tab_stops,
    ) = build_table(
        rows,
        props,
        grid_widths,
        style_cell_props,
        row_regions,
        table_look,
        col_band_size,
    );
    (
        table,
        TablePaginationHints {
            rows: row_pagination,
            cells: cell_pagination,
            cell_line_spacing,
            cell_column_breaks,
            nested: nested_pagination,
            cell_tabs: cell_tab_stops,
        },
    )
}

fn read_tbl_grid(r: &mut Xml<'_>) -> Option<Vec<u32>> {
    let mut widths = Vec::new();
    let mut valid = true;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblGridChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"gridCol" => {
                if widths.len() >= MAX_TABLE_GRID_COLS {
                    valid = false;
                } else if let Some(width) = attr_u32(&e, b"w").filter(|width| *width > 0) {
                    widths.push(width);
                } else {
                    valid = false;
                }
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"gridCol" => {
                if widths.len() >= MAX_TABLE_GRID_COLS {
                    valid = false;
                } else if let Some(width) = attr_u32(&e, b"w").filter(|width| *width > 0) {
                    widths.push(width);
                } else {
                    valid = false;
                }
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tblGrid" => break,
            Ok(Event::Eof) | Err(_) => {
                valid = false;
                break;
            }
            _ => {}
        }
    }
    (valid && !widths.is_empty()).then_some(widths)
}

fn is_current_table_structural_wrapper(name: &[u8]) -> bool {
    matches!(
        name,
        b"sdt" | b"sdtContent" | b"customXml" | b"smartTag" | b"ins" | b"moveTo"
    )
}

fn read_table_alternate_content_rows(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    props: &mut TableProps,
) -> Vec<RowRaw> {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        rows.extend(read_table_alternate_content_branch_rows(
                            r,
                            ctx,
                            depth + 1,
                            name,
                            props,
                        ));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    rows
}

fn read_table_alternate_content_branch_rows(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    branch: &[u8],
    props: &mut TableProps,
) -> Vec<RowRaw> {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return Vec::new();
    }
    let mut rows = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"tblPr" => *props = read_tblpr(r),
                b"tr" => rows.push(read_row(r, ctx, depth)),
                b"AlternateContent" => {
                    rows.extend(read_table_alternate_content_rows(r, ctx, depth + 1, props))
                }
                name if is_current_table_structural_wrapper(name) => {}
                _ => skip_subtree(r),
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    rows
}

#[derive(Default)]
struct TableProps {
    style_id: Option<String>,
    borders_declared: bool,
    fixed_layout_declared: bool,
    bidi_visual_declared: bool,
    look: Option<TableLook>,
    row_band_size: Option<u8>,
    col_band_size: Option<u8>,
    cell_margins: CellMarginSpec,
    bidi_visual: bool,
    fixed_layout: bool,
    indent_twips: Option<i32>,
    align: Option<Align>,
    width_pct: Option<f32>,
    border_color: Option<Color>,
    border_colors: TableBorderColors,
    border_size_eighths: Option<u16>,
    border_sizes: TableBorderSizes,
    border_style: Option<TableBorderStyle>,
    border_styles: TableBorderStyles,
}

#[derive(Clone, Copy, Default)]
struct TableLook {
    first_row: bool,
    last_row: bool,
    first_column: bool,
    last_column: bool,
    horizontal_banding: bool,
    vertical_banding: bool,
}

impl TableLook {
    fn word_default() -> Self {
        // Word treats an omitted tblLook as 0x04A0: first row, first column,
        // no vertical banding, and horizontal banding enabled.
        Self {
            first_row: true,
            last_row: false,
            first_column: true,
            last_column: false,
            horizontal_banding: true,
            vertical_banding: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row_regions(
        self,
        index: usize,
        row_count: usize,
        row_band_size: u8,
        col_band_size: u8,
        column_count: usize,
        is_header: bool,
        bidi_visual: bool,
    ) -> TableRowStyleRegions {
        let horizontal_band = self
            .horizontal_banding
            .then_some(row_band_size)
            .filter(|size| *size != 0)
            .map(|size| (index / size as usize) % 2);
        let vertical_band = self
            .vertical_banding
            .then_some(col_band_size)
            .filter(|size| *size != 0);
        let first_row = self.first_row && (index == 0 || is_header);
        let last_row = self.last_row && index.checked_add(1) == Some(row_count);
        let first_column = self.first_column && column_count != 0;
        let last_column = self.last_column && column_count != 0;
        let (west, east) = if bidi_visual {
            (last_column, first_column)
        } else {
            (first_column, last_column)
        };
        TableRowStyleRegions {
            first_row,
            last_row,
            first_column,
            last_column,
            band1_vertical: vertical_band.is_some(),
            band2_vertical: vertical_band.is_some_and(|size| column_count > size as usize),
            band1_horizontal: horizontal_band == Some(0),
            band2_horizontal: horizontal_band == Some(1),
            north_west: first_row && index == 0 && west,
            north_east: first_row && index == 0 && east,
            south_west: last_row && index.checked_add(1) == Some(row_count) && west,
            south_east: last_row && index.checked_add(1) == Some(row_count) && east,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cell_regions(
        self,
        row_regions: TableRowStyleRegions,
        row_index: usize,
        row_count: usize,
        col: usize,
        col_span: u16,
        col_count: usize,
        col_band_size: u8,
        bidi_visual: bool,
    ) -> TableCellStyleRegions {
        let end_col = col.saturating_add(col_span as usize);
        let first_column = self.first_column && col == 0;
        let last_column = self.last_column && end_col == col_count;
        let band = self
            .vertical_banding
            .then_some(col_band_size)
            .filter(|size| *size != 0)
            .map(|size| (col / size as usize) % 2);
        let north = self.first_row && row_index == 0;
        let south = self.last_row && row_index.checked_add(1) == Some(row_count);
        let (west, east) = if bidi_visual {
            (last_column, first_column)
        } else {
            (first_column, last_column)
        };
        let mut regions: TableCellStyleRegions = row_regions.into();
        regions.first_column = first_column;
        regions.last_column = last_column;
        regions.band1_vertical = band == Some(0);
        regions.band2_vertical = band == Some(1);
        regions.north_west = north && west;
        regions.north_east = north && east;
        regions.south_west = south && west;
        regions.south_east = south && east;
        regions
    }
}

fn read_table_look(e: &BytesStart<'_>) -> TableLook {
    let has_named_attributes = e.attributes().flatten().any(|attribute| {
        matches!(
            local(attribute.key.as_ref()),
            b"firstRow" | b"lastRow" | b"firstColumn" | b"lastColumn" | b"noHBand" | b"noVBand"
        )
    });
    if has_named_attributes {
        return TableLook {
            first_row: attr_local(e, b"firstRow").is_some_and(|value| toggle_on(Some(value))),
            last_row: attr_local(e, b"lastRow").is_some_and(|value| toggle_on(Some(value))),
            first_column: attr_local(e, b"firstColumn").is_some_and(|value| toggle_on(Some(value))),
            last_column: attr_local(e, b"lastColumn").is_some_and(|value| toggle_on(Some(value))),
            horizontal_banding: !attr_local(e, b"noHBand")
                .is_some_and(|value| toggle_on(Some(value))),
            vertical_banding: !attr_local(e, b"noVBand")
                .is_some_and(|value| toggle_on(Some(value))),
        };
    }

    // ISO/IEC 29500-4 14.4.11 assigns the six selectors to bits 5 through 10.
    let mask = attr_local_trimmed(e, b"val")
        .and_then(|value| u16::from_str_radix(&value, 16).ok())
        .unwrap_or(0);
    TableLook {
        first_row: mask & 0x0020 != 0,
        last_row: mask & 0x0040 != 0,
        first_column: mask & 0x0080 != 0,
        last_column: mask & 0x0100 != 0,
        horizontal_banding: mask & 0x0200 == 0,
        vertical_banding: mask & 0x0400 == 0,
    }
}

fn apply_tblpr_child(props: &mut TableProps, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"tblStyle" => props.style_id = attr_local_trimmed(e, b"val"),
        b"tblLook" => props.look = Some(read_table_look(e)),
        b"tblStyleRowBandSize" => {
            if let Some(size) = attr_u8(e, b"val").filter(|size| *size <= 3) {
                props.row_band_size = Some(size);
            }
        }
        b"tblStyleColBandSize" => {
            if let Some(size) = attr_u8(e, b"val").filter(|size| *size <= 3) {
                props.col_band_size = Some(size);
            }
        }
        b"bidiVisual" => {
            props.bidi_visual_declared = true;
            props.bidi_visual = toggle_on(attr_local(e, b"val"));
        }
        b"tblW" if attr_local_trimmed(e, b"type").is_some_and(|value| value == "pct") => {
            props.width_pct = attr_f32(e, b"w").map(|percentage| percentage / 5000.0);
        }
        b"tblLayout" => {
            props.fixed_layout_declared = true;
            props.fixed_layout =
                attr_local_trimmed(e, b"type").is_some_and(|value| value == "fixed");
        }
        b"tblInd" if type_defaults_to_dxa(e) => {
            props.indent_twips = attr_i32(e, b"w");
        }
        b"jc" => {
            props.align = match attr_local_trimmed(e, b"val").as_deref() {
                Some("center") => Some(Align::Center),
                Some("right") | Some("end") => Some(Align::Right),
                Some("both") => Some(Align::Justify),
                Some("left") | Some("start") => Some(Align::Left),
                _ => None,
            };
        }
        _ => {}
    }
}

/// Read `<w:tblPr>` layout metadata.
fn read_tblpr(r: &mut Xml<'_>) -> TableProps {
    let mut props = TableProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tblpr_alternate_content(r, &mut props, 0);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                props
                    .cell_margins
                    .overlay(read_cell_margins(r, b"tblCellMar", 0));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblBorders" => {
                let borders = read_tbl_borders(r);
                props.borders_declared = true;
                props.border_color = borders.0;
                props.border_colors = borders.1;
                props.border_size_eighths = borders.2;
                props.border_sizes = borders.3;
                props.border_style = borders.4;
                props.border_styles = borders.5;
            }
            Ok(Event::Start(e)) => {
                apply_tblpr_child(&mut props, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => apply_tblpr_child(&mut props, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tblPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_tblpr_alternate_content(r: &mut Xml<'_>, props: &mut TableProps, depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_tblpr_alternate_content_branch(r, props, name, depth + 1);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_tblpr_alternate_content_branch(
    r: &mut Xml<'_>,
    props: &mut TableProps,
    branch: &[u8],
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tblpr_alternate_content(r, props, depth + 1);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                props
                    .cell_margins
                    .overlay(read_cell_margins(r, b"tblCellMar", depth + 1));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblBorders" => {
                let borders = read_tbl_borders(r);
                props.borders_declared = true;
                props.border_color = borders.0;
                props.border_colors = borders.1;
                props.border_size_eighths = borders.2;
                props.border_sizes = borders.3;
                props.border_style = borders.4;
                props.border_styles = borders.5;
            }
            Ok(Event::Start(e)) => {
                apply_tblpr_child(props, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => apply_tblpr_child(props, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// Cell defaults a table style can declare. Declaration flags preserve an
/// explicit clear or unsupported nearer value so it can suppress inheritance.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(crate) struct TableStyleCellDefaults {
    shading: Option<Color>,
    shading_declared: bool,
    valign: Option<VCell>,
    valign_declared: bool,
    width_pct: Option<f32>,
    width_pct_declared: bool,
}

impl TableStyleCellDefaults {
    pub(crate) fn overlay(&mut self, other: Self) {
        if other.shading_declared {
            self.shading = other.shading;
            self.shading_declared = true;
        }
        if other.valign_declared {
            self.valign = other.valign;
            self.valign_declared = true;
        }
        if other.width_pct_declared {
            self.width_pct = other.width_pct;
            self.width_pct_declared = true;
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        !self.shading_declared && !self.valign_declared && !self.width_pct_declared
    }

    pub(crate) fn shading(self) -> Option<Color> {
        self.shading
    }

    pub(crate) fn valign(self) -> Option<VCell> {
        self.valign
    }

    pub(crate) fn width_pct(self) -> Option<f32> {
        self.width_pct
    }

    /// Record a `w:tcPr` child, reusing the direct cell reader's semantics.
    pub(crate) fn record(&mut self, e: &BytesStart<'_>) {
        let mut t = TcPr {
            gs: 1,
            vm: VMerge::None,
            shading: None,
            shading_declared: false,
            valign: VCell::Top,
            valign_declared: false,
            width_pct: None,
            width_pct_declared: false,
            margins: CellMarginSpec::default(),
            style_regions: None,
        };
        apply_tcpr_child(&mut t, e);
        self.overlay(Self {
            shading: t.shading,
            shading_declared: t.shading_declared,
            valign: t.valign_declared.then_some(t.valign),
            valign_declared: t.valign_declared,
            width_pct: t.width_pct,
            width_pct_declared: t.width_pct_declared,
        });
    }
}

/// Table-level geometry a table style can declare. Only values the model
/// already carries and whose absence is unambiguous participate.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(crate) struct TableStyleGeometry {
    pub(crate) width_pct: Option<f32>,
    pub(crate) indent_twips: Option<i32>,
    pub(crate) align: Option<Align>,
    pub(crate) fixed_layout: Option<bool>,
    pub(crate) bidi_visual: Option<bool>,
}

impl TableStyleGeometry {
    pub(crate) fn overlay(&mut self, other: Self) {
        if other.width_pct.is_some() {
            self.width_pct = other.width_pct;
        }
        if other.indent_twips.is_some() {
            self.indent_twips = other.indent_twips;
        }
        if other.align.is_some() {
            self.align = other.align;
        }
        if other.fixed_layout.is_some() {
            self.fixed_layout = other.fixed_layout;
        }
        if other.bidi_visual.is_some() {
            self.bidi_visual = other.bidi_visual;
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        self.width_pct.is_none()
            && self.indent_twips.is_none()
            && self.align.is_none()
            && self.fixed_layout.is_none()
            && self.bidi_visual.is_none()
    }

    /// Record a `w:tblPr` child, reusing the direct-table reader's semantics.
    pub(crate) fn record(&mut self, e: &BytesStart<'_>) {
        let mut props = TableProps::default();
        apply_tblpr_child(&mut props, e);
        self.overlay(Self {
            width_pct: props.width_pct,
            indent_twips: props.indent_twips,
            align: props.align,
            fixed_layout: props.fixed_layout_declared.then_some(props.fixed_layout),
            bidi_visual: props.bidi_visual_declared.then_some(props.bidi_visual),
        });
    }
}

pub(crate) type TableBorderTuple = (
    Option<Color>,
    TableBorderColors,
    Option<u16>,
    TableBorderSizes,
    Option<TableBorderStyle>,
    TableBorderStyles,
);

pub(crate) fn read_tbl_borders(
    r: &mut Xml<'_>,
) -> (
    Option<Color>,
    TableBorderColors,
    Option<u16>,
    TableBorderSizes,
    Option<TableBorderStyle>,
    TableBorderStyles,
) {
    let mut borders = TableBorderProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tbl_borders_alternate_content(r, &mut borders);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                borders.record(&e);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tblBorders" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    borders.finish()
}

struct TableBorderProps {
    color: Option<Color>,
    colors: TableBorderColors,
    color_seen: bool,
    color_consistent: bool,
    size: Option<u16>,
    sizes: TableBorderSizes,
    size_seen: bool,
    size_consistent: bool,
    style: Option<TableBorderStyle>,
    styles: TableBorderStyles,
    style_seen: bool,
    style_consistent: bool,
}

impl Default for TableBorderProps {
    fn default() -> Self {
        Self {
            color: None,
            colors: TableBorderColors::default(),
            color_seen: false,
            color_consistent: true,
            size: None,
            sizes: TableBorderSizes::default(),
            size_seen: false,
            size_consistent: true,
            style: None,
            styles: TableBorderStyles::default(),
            style_seen: false,
            style_consistent: true,
        }
    }
}

impl TableBorderProps {
    fn record(&mut self, e: &BytesStart<'_>) {
        let Some(side) = table_border_side(e) else {
            return;
        };
        if let Some(next) = attr_local(e, b"color").and_then(|v| parse_rgb_hex_color(&v)) {
            self.colors.set(side, next);
            self.color_seen = true;
            match self.color {
                Some(current) if current != next => self.color_consistent = false,
                None => self.color = Some(next),
                _ => {}
            }
        }
        if let Some(next) = attr_u16(e, b"sz").filter(|v| *v > 0) {
            self.sizes.set(side, next);
            self.size_seen = true;
            match self.size {
                Some(current) if current != next => self.size_consistent = false,
                None => self.size = Some(next),
                _ => {}
            }
        }
        if let Some(next) = attr_local(e, b"val").and_then(|v| TableBorderStyle::from_wml_value(&v))
        {
            self.styles.set(side, next);
            self.style_seen = true;
            match self.style {
                Some(current) if current != next => self.style_consistent = false,
                None => self.style = Some(next),
                _ => {}
            }
        }
    }

    fn finish(
        self,
    ) -> (
        Option<Color>,
        TableBorderColors,
        Option<u16>,
        TableBorderSizes,
        Option<TableBorderStyle>,
        TableBorderStyles,
    ) {
        let uniform_color = if self.color_seen && self.color_consistent {
            self.color
        } else {
            None
        };
        let uniform_size = if self.size_seen && self.size_consistent {
            self.size
        } else {
            None
        };
        let uniform_style = if self.style_seen && self.style_consistent {
            self.style
        } else {
            None
        };
        (
            uniform_color,
            self.colors,
            uniform_size,
            self.sizes,
            uniform_style,
            self.styles,
        )
    }
}

fn read_tbl_borders_alternate_content(r: &mut Xml<'_>, borders: &mut TableBorderProps) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_tbl_borders_alternate_content_branch(r, borders, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_tbl_borders_alternate_content_branch(
    r: &mut Xml<'_>,
    borders: &mut TableBorderProps,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tbl_borders_alternate_content(r, borders);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                borders.record(&e);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn table_border_side(e: &BytesStart<'_>) -> Option<TableBorderSide> {
    match local(e.name().as_ref()) {
        b"top" => Some(TableBorderSide::Top),
        b"left" => Some(TableBorderSide::Left),
        b"bottom" => Some(TableBorderSide::Bottom),
        b"right" => Some(TableBorderSide::Right),
        b"insideH" => Some(TableBorderSide::InsideHorizontal),
        b"insideV" => Some(TableBorderSide::InsideVertical),
        _ => None,
    }
}

/// Read a `<w:tr>` and its direct row properties.
fn read_row(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> RowRaw {
    let mut cells = Vec::new();
    let mut props = RowProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"tblPrEx" => {
                    let margins = read_tblprex_cell_margins(r, depth);
                    props.overlay_cell_margins(margins);
                }
                b"trPr" => props.merge(read_trpr(r)),
                b"tc" => cells.push(read_cell(r, ctx, depth + 1)),
                b"AlternateContent" => {
                    let (branch_cells, branch_props) =
                        read_row_alternate_content_cells(r, ctx, depth + 1);
                    cells.extend(branch_cells);
                    if let Some(value) = branch_props {
                        props.merge(value);
                    }
                }
                name if is_current_table_structural_wrapper(name) => {}
                _ => skip_subtree(r),
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    RowRaw { cells, props }
}

fn read_row_alternate_content_cells(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
) -> (Vec<CellRaw>, Option<RowProps>) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return (Vec::new(), None);
    }
    let mut cells = Vec::new();
    let mut props = None;
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        let (branch_cells, branch_props) =
                            read_row_alternate_content_branch_cells(r, ctx, depth + 1, name);
                        cells.extend(branch_cells);
                        if branch_props.is_some() {
                            props = branch_props;
                        }
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (cells, props)
}

fn read_row_alternate_content_branch_cells(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    branch: &[u8],
) -> (Vec<CellRaw>, Option<RowProps>) {
    let mut cells = Vec::new();
    let mut props: Option<RowProps> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"tblPrEx" => {
                    let mut merged = props.unwrap_or_default();
                    let margins = read_tblprex_cell_margins(r, depth);
                    merged.overlay_cell_margins(margins);
                    props = Some(merged);
                }
                b"trPr" => {
                    let mut merged = props.unwrap_or_default();
                    merged.merge(read_trpr(r));
                    props = Some(merged);
                }
                b"tc" => cells.push(read_cell(r, ctx, depth + 1)),
                b"AlternateContent" => {
                    let (branch_cells, branch_props) =
                        read_row_alternate_content_cells(r, ctx, depth + 1);
                    cells.extend(branch_cells);
                    if let Some(value) = branch_props {
                        let mut merged = props.unwrap_or_default();
                        merged.merge(value);
                        props = Some(merged);
                    }
                }
                name if is_current_table_structural_wrapper(name) => {}
                _ => skip_subtree(r),
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (cells, props)
}

fn read_tblprex_cell_margins(r: &mut Xml<'_>, depth: u32) -> Option<CellMarginSpec> {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return None;
    }
    let mut margins = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPrExChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tblprex_alternate_content(r, &mut margins, depth + 1);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                overlay_declared_cell_margins(
                    &mut margins,
                    read_cell_margins(r, b"tblCellMar", depth + 1),
                );
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                overlay_declared_cell_margins(&mut margins, CellMarginSpec::default());
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tblPrEx" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    margins
}

fn overlay_declared_cell_margins(
    margins: &mut Option<CellMarginSpec>,
    declaration: CellMarginSpec,
) {
    if let Some(current) = margins {
        current.overlay(declaration);
    } else {
        *margins = Some(declaration);
    }
}

fn read_tblprex_alternate_content(
    r: &mut Xml<'_>,
    margins: &mut Option<CellMarginSpec>,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_tblprex_alternate_content_branch(r, margins, name, depth + 1);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_tblprex_alternate_content_branch(
    r: &mut Xml<'_>,
    margins: &mut Option<CellMarginSpec>,
    branch: &[u8],
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPrExChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tblprex_alternate_content(r, margins, depth + 1);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                overlay_declared_cell_margins(
                    margins,
                    read_cell_margins(r, b"tblCellMar", depth + 1),
                );
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                overlay_declared_cell_margins(margins, CellMarginSpec::default());
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_row_style_regions(e: &BytesStart<'_>) -> Option<TableRowStyleRegions> {
    let has_named_attributes = e.attributes().flatten().any(|attribute| {
        matches!(
            local(attribute.key.as_ref()),
            b"firstRow"
                | b"lastRow"
                | b"firstColumn"
                | b"lastColumn"
                | b"oddVBand"
                | b"evenVBand"
                | b"oddHBand"
                | b"evenHBand"
                | b"firstRowLastColumn"
                | b"firstRowFirstColumn"
                | b"lastRowLastColumn"
                | b"lastRowFirstColumn"
        )
    });
    if has_named_attributes {
        return Some(TableRowStyleRegions {
            first_row: attr_local(e, b"firstRow").is_some_and(|value| toggle_on(Some(value))),
            last_row: attr_local(e, b"lastRow").is_some_and(|value| toggle_on(Some(value))),
            first_column: attr_local(e, b"firstColumn").is_some_and(|value| toggle_on(Some(value))),
            last_column: attr_local(e, b"lastColumn").is_some_and(|value| toggle_on(Some(value))),
            band1_vertical: attr_local(e, b"oddVBand").is_some_and(|value| toggle_on(Some(value))),
            band2_vertical: attr_local(e, b"evenVBand").is_some_and(|value| toggle_on(Some(value))),
            band1_horizontal: attr_local(e, b"oddHBand")
                .is_some_and(|value| toggle_on(Some(value))),
            band2_horizontal: attr_local(e, b"evenHBand")
                .is_some_and(|value| toggle_on(Some(value))),
            north_west: attr_local(e, b"firstRowFirstColumn")
                .is_some_and(|value| toggle_on(Some(value))),
            north_east: attr_local(e, b"firstRowLastColumn")
                .is_some_and(|value| toggle_on(Some(value))),
            south_west: attr_local(e, b"lastRowFirstColumn")
                .is_some_and(|value| toggle_on(Some(value))),
            south_east: attr_local(e, b"lastRowLastColumn")
                .is_some_and(|value| toggle_on(Some(value))),
        });
    }

    // ISO/IEC 29500-4 14.3.1.1 serializes all twelve flags in order.
    let value = attr_local_trimmed(e, b"val")?;
    let mask = value.as_bytes();
    if mask.len() != 12 || mask.iter().any(|bit| !matches!(bit, b'0' | b'1')) {
        return None;
    }
    Some(TableRowStyleRegions {
        first_row: mask[0] == b'1',
        last_row: mask[1] == b'1',
        first_column: mask[2] == b'1',
        last_column: mask[3] == b'1',
        band1_vertical: mask[4] == b'1',
        band2_vertical: mask[5] == b'1',
        band1_horizontal: mask[6] == b'1',
        band2_horizontal: mask[7] == b'1',
        north_east: mask[8] == b'1',
        north_west: mask[9] == b'1',
        south_east: mask[10] == b'1',
        south_west: mask[11] == b'1',
    })
}

fn read_cell_style_regions(e: &BytesStart<'_>) -> Option<CellStyleRegionSpec> {
    let has_named_attributes = e.attributes().flatten().any(|attribute| {
        matches!(
            local(attribute.key.as_ref()),
            b"firstRow"
                | b"lastRow"
                | b"firstColumn"
                | b"lastColumn"
                | b"oddVBand"
                | b"evenVBand"
                | b"oddHBand"
                | b"evenHBand"
                | b"firstRowLastColumn"
                | b"firstRowFirstColumn"
                | b"lastRowLastColumn"
                | b"lastRowFirstColumn"
        )
    });
    if has_named_attributes {
        let enabled = |name: &[u8]| attr_local(e, name).is_some_and(|value| toggle_on(Some(value)));
        return Some(CellStyleRegionSpec::Named(NamedCellStyleRegions {
            regions: TableCellStyleRegions {
                first_row: enabled(b"firstRow"),
                last_row: enabled(b"lastRow"),
                first_column: enabled(b"firstColumn"),
                last_column: enabled(b"lastColumn"),
                band1_vertical: enabled(b"oddVBand"),
                band2_vertical: enabled(b"evenVBand"),
                band1_horizontal: enabled(b"oddHBand"),
                band2_horizontal: enabled(b"evenHBand"),
                ..TableCellStyleRegions::default()
            },
            first_row_last_column: enabled(b"firstRowLastColumn"),
            first_row_first_column: enabled(b"firstRowFirstColumn"),
            last_row_last_column: enabled(b"lastRowLastColumn"),
            last_row_first_column: enabled(b"lastRowFirstColumn"),
        }));
    }

    // ISO/IEC 29500-4 14.4.10 stores the four corner flags as physical
    // NE/NW/SE/SW bits after the eight row, column, and band flags.
    let value = attr_local_trimmed(e, b"val")?;
    let mask = value.as_bytes();
    if mask.len() != 12 || mask.iter().any(|bit| !matches!(bit, b'0' | b'1')) {
        return None;
    }
    Some(CellStyleRegionSpec::Transitional(TableCellStyleRegions {
        first_row: mask[0] == b'1',
        last_row: mask[1] == b'1',
        first_column: mask[2] == b'1',
        last_column: mask[3] == b'1',
        band1_vertical: mask[4] == b'1',
        band2_vertical: mask[5] == b'1',
        band1_horizontal: mask[6] == b'1',
        band2_horizontal: mask[7] == b'1',
        north_east: mask[8] == b'1',
        north_west: mask[9] == b'1',
        south_east: mask[10] == b'1',
        south_west: mask[11] == b'1',
    }))
}

/// Read direct `<w:trPr>` pagination properties.
fn read_trpr(r: &mut Xml<'_>) -> RowProps {
    let mut props = RowProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_trpr_alternate_content(r) {
                    props.merge(value);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"tblHeader" =>
            {
                props.header = Some(toggle_on(attr_local(&e, b"val")));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"cantSplit" =>
            {
                props.cant_split = Some(toggle_on(attr_local(&e, b"val")));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"cnfStyle" =>
            {
                if let Some(regions) = read_row_style_regions(&e) {
                    props.style_regions = Some(regions);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"trPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_trpr_alternate_content(r: &mut Xml<'_>) -> Option<RowProps> {
    let mut took = false;
    let mut props = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        props = Some(read_trpr_alternate_content_branch(r, name));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_trpr_alternate_content_branch(r: &mut Xml<'_>, branch: &[u8]) -> RowProps {
    let mut props = RowProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_trpr_alternate_content(r) {
                    props.merge(value);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"tblHeader" =>
            {
                props.header = Some(toggle_on(attr_local(&e, b"val")));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"cantSplit" =>
            {
                props.cant_split = Some(toggle_on(attr_local(&e, b"val")));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"cnfStyle" =>
            {
                if let Some(regions) = read_row_style_regions(&e) {
                    props.style_regions = Some(regions);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

/// Read a `<w:tc>` → its block content + `gridSpan`/`vMerge`.
fn read_cell(r: &mut Xml<'_>, ctx: &Ctx<'_>, depth: u32) -> CellRaw {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return CellRaw {
            blocks: Vec::new(),
            pagination: Vec::new(),
            line_spacing: Vec::new(),
            nested_tables: Vec::new(),
            tab_stops: Vec::new(),
            column_break_offsets: Vec::new(),
            col_span: 1,
            vmerge: VMerge::None,
            shading: None,
            shading_declared: false,
            valign: VCell::Top,
            valign_declared: false,
            width_pct: None,
            width_pct_declared: false,
            margins: CellMarginSpec::default(),
            style_regions: None,
        };
    }
    let mut blocks = Vec::new();
    let mut pagination = Vec::new();
    let mut line_spacing = Vec::new();
    let mut nested_tables = Vec::new();
    let mut tab_stops = Vec::new();
    let mut column_break_offsets = Vec::new();
    let mut tc: Option<TcPr> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"tcPr" => tc = Some(read_tcpr(r)),
                b"p" => {
                    read_paragraph_block_batch(r, ctx, depth + 1).append_to(
                        &mut blocks,
                        &mut pagination,
                        &mut line_spacing,
                        &mut nested_tables,
                        &mut tab_stops,
                        &mut column_break_offsets,
                    );
                }
                b"tbl" => {
                    if let Some((table, table_pagination)) = read_table_block(r, ctx, depth + 1) {
                        pagination.push(None);
                        line_spacing.push(None);
                        nested_tables.push(Some(table_pagination));
                        tab_stops.push(Vec::new());
                        column_break_offsets.push(Vec::new());
                        blocks.push(table);
                    }
                }
                b"sdt" | b"sdtContent" | b"customXml" | b"smartTag" | b"ins" | b"moveTo" => {
                    read_blocks_with_pagination(r, ctx, depth + 1).append_to(
                        &mut blocks,
                        &mut pagination,
                        &mut line_spacing,
                        &mut nested_tables,
                        &mut tab_stops,
                        &mut column_break_offsets,
                    );
                }
                b"AlternateContent" => read_cell_alternate_content(r, ctx, depth + 1, &mut tc)
                    .append_to(
                        &mut blocks,
                        &mut pagination,
                        &mut line_spacing,
                        &mut nested_tables,
                        &mut tab_stops,
                        &mut column_break_offsets,
                    ),
                _ => skip_subtree(r),
            },
            Ok(Event::End(_)) | Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let tc = tc.unwrap_or(TcPr {
        gs: 1,
        vm: VMerge::None,
        shading: None,
        shading_declared: false,
        valign: VCell::Top,
        valign_declared: false,
        width_pct: None,
        width_pct_declared: false,
        margins: CellMarginSpec::default(),
        style_regions: None,
    });
    CellRaw {
        blocks,
        pagination,
        line_spacing,
        nested_tables,
        tab_stops,
        column_break_offsets,
        col_span: tc.gs,
        vmerge: tc.vm,
        shading: tc.shading,
        shading_declared: tc.shading_declared,
        valign: tc.valign,
        valign_declared: tc.valign_declared,
        width_pct: tc.width_pct,
        width_pct_declared: tc.width_pct_declared,
        margins: tc.margins,
        style_regions: tc.style_regions,
    }
}

fn read_cell_alternate_content(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    tc: &mut Option<TcPr>,
) -> BlockBatch {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return BlockBatch::default();
    }
    let mut batch = BlockBatch::default();
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        batch.extend(read_cell_alternate_content_branch(
                            r,
                            ctx,
                            depth + 1,
                            name,
                            tc,
                        ));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    batch
}

fn read_cell_alternate_content_branch(
    r: &mut Xml<'_>,
    ctx: &Ctx<'_>,
    depth: u32,
    branch: &[u8],
    tc: &mut Option<TcPr>,
) -> BlockBatch {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return BlockBatch::default();
    }
    let mut batch = BlockBatch::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"tcPr" => *tc = Some(read_tcpr(r)),
                b"p" => batch.extend(read_paragraph_block_batch(r, ctx, depth + 1)),
                b"tbl" => {
                    if let Some((table, pagination)) = read_table_block(r, ctx, depth + 1) {
                        batch.push_table(table, pagination);
                    }
                }
                b"sdt" => {
                    batch.extend(read_content_control_blocks_with_pagination(
                        r,
                        ctx,
                        depth + 1,
                    ));
                }
                b"sdtContent" | b"customXml" | b"smartTag" | b"ins" | b"moveTo" => {
                    batch.extend(read_blocks_with_pagination(r, ctx, depth + 1));
                }
                b"AlternateContent" => {
                    batch.extend(read_cell_alternate_content(r, ctx, depth + 1, tc));
                }
                _ => skip_subtree(r),
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    batch
}

/// Collected `<w:tcPr>` properties.
struct TcPr {
    gs: u16,
    vm: VMerge,
    shading: Option<Color>,
    shading_declared: bool,
    valign: VCell,
    valign_declared: bool,
    width_pct: Option<f32>,
    width_pct_declared: bool,
    margins: CellMarginSpec,
    style_regions: Option<CellStyleRegionSpec>,
}

/// Read `<w:tcPr>` → gridSpan / vMerge / shading / vAlign / width.
fn read_tcpr(r: &mut Xml<'_>) -> TcPr {
    let mut t = TcPr {
        gs: 1,
        vm: VMerge::None,
        shading: None,
        shading_declared: false,
        valign: VCell::Top,
        valign_declared: false,
        width_pct: None,
        width_pct_declared: false,
        margins: CellMarginSpec::default(),
        style_regions: None,
    };
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tcpr_alternate_content(r, &mut t, 0);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcMar" => {
                t.margins.overlay(read_cell_margins(r, b"tcMar", 0));
            }
            Ok(Event::Start(e)) => {
                apply_tcpr_child(&mut t, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => apply_tcpr_child(&mut t, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tcPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    t
}

fn read_tcpr_alternate_content(r: &mut Xml<'_>, t: &mut TcPr, depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_tcpr_alternate_content_branch(r, t, name, depth + 1);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_tcpr_alternate_content_branch(r: &mut Xml<'_>, t: &mut TcPr, branch: &[u8], depth: u32) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcMar" => {
                t.margins.overlay(read_cell_margins(r, b"tcMar", depth + 1));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_tcpr_alternate_content(r, t, depth + 1);
            }
            Ok(Event::Start(e)) => {
                apply_tcpr_child(t, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => apply_tcpr_child(t, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn apply_tcpr_child(t: &mut TcPr, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"gridSpan" => {
            if let Some(v) = attr_u16(e, b"val") {
                t.gs = v.max(1);
            }
        }
        b"vMerge" => {
            t.vm = match attr_local_trimmed(e, b"val").as_deref() {
                Some("restart") => VMerge::Restart,
                _ => VMerge::Continue, // present with "continue"/no val
            };
        }
        b"shd" => {
            t.shading_declared = true;
            let suppresses_fill = attr_local_trimmed(e, b"val").as_deref() == Some("nil")
                || attr_local_trimmed(e, b"fill").as_deref() == Some("auto");
            t.shading = (!suppresses_fill)
                .then(|| attr_local(e, b"fill").and_then(|v| parse_rgb_hex_color(&v)))
                .flatten();
        }
        b"vAlign" => {
            t.valign_declared = true;
            t.valign = match attr_local_trimmed(e, b"val").as_deref() {
                Some("center") => VCell::Center,
                Some("bottom") => VCell::Bottom,
                _ => VCell::Top,
            };
        }
        // `type="pct"` w:w is in fiftieths of a percent (5000 = 100%).
        // Other, malformed, or unbounded declarations suppress inherited
        // percentages but remain outside the model's percentage-only field.
        b"tcW" => {
            t.width_pct_declared = true;
            t.width_pct = if attr_local_trimmed(e, b"type").as_deref() == Some("pct") {
                attr_f32(e, b"w")
                    .filter(|value| value.is_finite() && (0.0..=5000.0).contains(value))
                    .map(|value| value / 5000.0)
            } else {
                None
            };
        }
        b"cnfStyle" => {
            if let Some(regions) = read_cell_style_regions(e) {
                t.style_regions = Some(regions);
            }
        }
        _ => {}
    }
}

pub(crate) fn read_cell_margins(r: &mut Xml<'_>, end: &[u8], depth: u32) -> CellMarginSpec {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return CellMarginSpec::default();
    }
    let mut margins = ParsedCellMargins::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_cell_margins_alternate_content(r, &mut margins, depth + 1);
            }
            Ok(Event::Start(e)) => {
                apply_cell_margin_side(&mut margins, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => apply_cell_margin_side(&mut margins, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    margins.finish()
}

fn read_cell_margins_alternate_content(
    r: &mut Xml<'_>,
    margins: &mut ParsedCellMargins,
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_cell_margins_alternate_content_branch(r, margins, name, depth + 1);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_cell_margins_alternate_content_branch(
    r: &mut Xml<'_>,
    margins: &mut ParsedCellMargins,
    branch: &[u8],
    depth: u32,
) {
    if depth > MAX_DEPTH {
        skip_subtree(r);
        return;
    }
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_cell_margins_alternate_content(r, margins, depth + 1);
            }
            Ok(Event::Start(e)) => {
                apply_cell_margin_side(margins, &e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) => apply_cell_margin_side(margins, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn cell_margin_value(e: &BytesStart<'_>) -> Option<u32> {
    match attr_local(e, b"type").as_deref().map(str::trim) {
        None | Some("dxa") => attr_u32(e, b"w"),
        Some("nil") => Some(0),
        Some(_) => None,
    }
}

fn apply_cell_margin_side(margins: &mut ParsedCellMargins, e: &BytesStart<'_>) {
    let name = e.name();
    let side = local(name.as_ref());
    if !matches!(
        side,
        b"top" | b"right" | b"end" | b"bottom" | b"left" | b"start"
    ) {
        return;
    }
    let value = cell_margin_value(e);
    match side {
        b"top" if value.is_some() => margins.top = value,
        b"right" if value.is_some() => margins.legacy_trailing = value,
        b"end" => margins.trailing.record(value),
        b"bottom" if value.is_some() => margins.bottom = value,
        b"left" if value.is_some() => margins.legacy_leading = value,
        b"start" => margins.leading.record(value),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn effective_cell_style_regions(
    explicit: Option<CellStyleRegionSpec>,
    table_look: TableLook,
    row_regions: TableRowStyleRegions,
    row_index: usize,
    row_count: usize,
    col: usize,
    col_span: u16,
    col_count: usize,
    col_band_size: u8,
    bidi_visual: bool,
) -> TableCellStyleRegions {
    explicit.map_or_else(
        || {
            table_look.cell_regions(
                row_regions,
                row_index,
                row_count,
                col,
                col_span,
                col_count,
                col_band_size,
                bidi_visual,
            )
        },
        |regions| regions.resolve(bidi_visual),
    )
}

/// Place cells over a running column index and resolve vertical merges
/// (`vMerge="restart"` opens a span, a later `vMerge` continuation at the same
/// starting column grows the owner's `row_span` and is dropped) — the OOXML
/// analogue of `table.rs` Phase B.
fn build_table(
    raw_rows: Vec<RowRaw>,
    props: TableProps,
    grid_widths: Vec<u32>,
    style_cell_props: TableStyleCellProps,
    row_regions: Vec<TableRowStyleRegions>,
    table_look: TableLook,
    col_band_size: u8,
) -> (
    Table,
    TableCellPaginationHints,
    TableCellLineSpacingHints,
    TableCellColumnBreakHints,
    TableCellNestedPaginationHints,
    TableCellTabStopHints,
) {
    let header_rows = raw_rows
        .iter()
        .take_while(|row| row.props.header.unwrap_or(false))
        .count();

    struct Placed {
        blocks: Vec<Block>,
        pagination: Vec<Option<PaginationHint>>,
        line_spacing: Vec<Option<LineSpacingHint>>,
        column_break_offsets: Vec<Vec<usize>>,
        nested_tables: Vec<Option<TablePaginationHints>>,
        tab_stops: Vec<Vec<TabStop>>,
        col: usize,
        col_span: u16,
        row_span: u16,
        is_header: bool,
        vmerge: VMerge,
        dropped: bool,
        shading: Option<Color>,
        shading_declared: bool,
        valign: VCell,
        valign_declared: bool,
        width_pct: Option<f32>,
        width_pct_declared: bool,
        row_cell_margins: Option<CellMarginSpec>,
        margins: CellMarginSpec,
        style_regions: Option<CellStyleRegionSpec>,
    }

    let mut grid: Vec<Vec<Placed>> = Vec::with_capacity(raw_rows.len());
    let mut model_grid_cols = 0usize;
    for raw_row in raw_rows {
        let header = raw_row.props.header.unwrap_or(false);
        let row_cell_margins = raw_row.props.cell_margins;
        let mut col = 0usize;
        let mut row = Vec::with_capacity(raw_row.cells.len());
        for c in raw_row.cells {
            let cs = c.col_span.max(1);
            row.push(Placed {
                blocks: c.blocks,
                pagination: c.pagination,
                line_spacing: c.line_spacing,
                column_break_offsets: c.column_break_offsets,
                nested_tables: c.nested_tables,
                tab_stops: c.tab_stops,
                col,
                col_span: cs,
                row_span: 1,
                is_header: header,
                vmerge: c.vmerge,
                dropped: false,
                shading: c.shading,
                shading_declared: c.shading_declared,
                valign: c.valign,
                valign_declared: c.valign_declared,
                width_pct: c.width_pct,
                width_pct_declared: c.width_pct_declared,
                row_cell_margins,
                margins: c.margins,
                style_regions: c.style_regions,
            });
            col += cs as usize;
        }
        model_grid_cols = model_grid_cols.max(col);
        grid.push(row);
    }

    let mut open: HashMap<usize, (usize, usize)> = HashMap::new();
    for r in 0..grid.len() {
        for o in 0..grid[r].len() {
            let col = grid[r][o].col;
            match grid[r][o].vmerge {
                VMerge::Restart => {
                    open.insert(col, (r, o));
                }
                VMerge::Continue => {
                    if let Some(&(rr, oo)) = open.get(&col) {
                        grid[rr][oo].row_span = grid[rr][oo].row_span.saturating_add(1);
                        grid[r][o].dropped = true;
                    } else {
                        // Continuation with no open restart → recover as its own
                        // cell that a following continuation may merge into.
                        open.insert(col, (r, o));
                    }
                }
                VMerge::None => {
                    open.remove(&col);
                }
            }
        }
    }

    let row_count = grid.len();
    let cell_margin_defaults_active = !props.cell_margins.is_empty()
        || grid.iter().enumerate().any(|(row_index, row)| {
            let row_regions = row_regions.get(row_index).copied().unwrap_or_default();
            row.iter().any(|cell| {
                let regions = effective_cell_style_regions(
                    cell.style_regions,
                    table_look,
                    row_regions,
                    row_index,
                    row_count,
                    cell.col,
                    cell.col_span,
                    model_grid_cols,
                    col_band_size,
                    props.bidi_visual,
                );
                let style_margins = style_cell_props.presentation_for_regions(regions).margins;
                !cell.dropped
                    && (!style_margins.is_empty()
                        || cell.row_cell_margins.is_some()
                        || !cell.margins.is_empty())
            })
        });
    let mut rows = Vec::with_capacity(grid.len());
    let mut table_cell_pagination = Vec::with_capacity(grid.len());
    let mut table_cell_line_spacing = Vec::with_capacity(grid.len());
    let mut table_cell_column_breaks = Vec::with_capacity(grid.len());
    let mut table_nested_pagination = Vec::with_capacity(grid.len());
    let mut table_cell_tab_stops = Vec::with_capacity(grid.len());
    for (row_index, row) in grid.into_iter().enumerate() {
        let row_regions = row_regions.get(row_index).copied().unwrap_or_default();
        let mut cells = Vec::with_capacity(row.len());
        let mut cell_pagination = Vec::with_capacity(row.len());
        let mut cell_line_spacing = Vec::with_capacity(row.len());
        let mut cell_column_breaks = Vec::with_capacity(row.len());
        let mut cell_nested_pagination = Vec::with_capacity(row.len());
        let mut cell_tab_stops = Vec::with_capacity(row.len());
        for p in row.into_iter().filter(|p| !p.dropped) {
            let regions = effective_cell_style_regions(
                p.style_regions,
                table_look,
                row_regions,
                row_index,
                row_count,
                p.col,
                p.col_span,
                model_grid_cols,
                col_band_size,
                props.bidi_visual,
            );
            let style_presentation = style_cell_props.presentation_for_regions(regions);
            cell_pagination.push(p.pagination);
            cell_line_spacing.push(p.line_spacing);
            cell_column_breaks.push(p.column_break_offsets);
            cell_nested_pagination.push(p.nested_tables);
            cell_tab_stops.push(p.tab_stops);
            cells.push(Cell {
                blocks: p.blocks,
                col_span: p.col_span,
                row_span: p.row_span,
                is_header: p.is_header,
                // The restart row's resolved style presentation fills only
                // properties the surviving cell left undeclared.
                shading: if p.shading_declared {
                    p.shading
                } else {
                    style_presentation.defaults.shading()
                },
                valign: if p.valign_declared {
                    p.valign
                } else {
                    style_presentation.defaults.valign().unwrap_or(p.valign)
                },
                width_pct: if p.width_pct_declared {
                    p.width_pct
                } else {
                    style_presentation.defaults.width_pct()
                },
                margins: resolve_cell_margins(
                    style_presentation.margins,
                    p.row_cell_margins.unwrap_or(props.cell_margins),
                    p.margins,
                    props.bidi_visual,
                    cell_margin_defaults_active,
                ),
            });
        }
        rows.push(Row { cells });
        table_cell_pagination.push(cell_pagination);
        table_cell_line_spacing.push(cell_line_spacing);
        table_cell_column_breaks.push(cell_column_breaks);
        table_nested_pagination.push(cell_nested_pagination);
        table_cell_tab_stops.push(cell_tab_stops);
    }
    (
        Table {
            rows,
            header_rows,
            col_widths_pct: normalize_table_grid_widths(&grid_widths, model_grid_cols),
            bidi_visual: props.bidi_visual,
            fixed_layout: props.fixed_layout,
            indent_twips: props.indent_twips,
            align: props.align,
            width_pct: props.width_pct,
            border_color: props.border_color,
            border_colors: props.border_colors,
            border_size_eighths: props.border_size_eighths,
            border_sizes: props.border_sizes,
            border_style: props.border_style,
            border_styles: props.border_styles,
        },
        table_cell_pagination,
        table_cell_line_spacing,
        table_cell_column_breaks,
        table_nested_pagination,
        table_cell_tab_stops,
    )
}

fn normalize_table_grid_widths(widths: &[u32], model_grid_cols: usize) -> Vec<f32> {
    if widths.len() != model_grid_cols || widths.is_empty() {
        return Vec::new();
    }
    let sum = widths.iter().map(|width| u64::from(*width)).sum::<u64>();
    if sum == 0 {
        return Vec::new();
    }
    widths
        .iter()
        .map(|width| (*width as f64 / sum as f64) as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Block, TabLeader, VertAlign};

    type ParsedWithRenderHints = (
        Vec<Block>,
        Vec<PaginationHint>,
        Vec<Vec<TabStop>>,
        Vec<Vec<TableRowPaginationHint>>,
        Vec<TableCellPaginationHints>,
        Vec<TableCellNestedPaginationHints>,
        Vec<TableCellLineSpacingHints>,
    );

    fn parse(xml: &str) -> Vec<Block> {
        parse_with_media(xml, HashMap::new())
    }

    fn parse_with_media(xml: &str, media: HashMap<String, Image>) -> Vec<Block> {
        parse_with_media_and_styles(xml, media, Styles::default())
    }

    fn parse_with_styles(xml: &str, styles_xml: &str) -> Vec<Block> {
        parse_with_media_and_styles(xml, HashMap::new(), super::super::styles::parse(styles_xml))
    }

    fn parse_with_media_and_styles(
        xml: &str,
        media: HashMap<String, Image>,
        styles: Styles,
    ) -> Vec<Block> {
        parse_with_media_styles_and_pagination(xml, media, styles, false).0
    }

    fn parse_with_media_styles_and_pagination(
        xml: &str,
        media: HashMap<String, Image>,
        styles: Styles,
        capture_pagination: bool,
    ) -> ParsedWithRenderHints {
        let numbering = Numbering::default();
        let rels = HashMap::new();
        let charts = HashMap::new();
        let ref_targets = HashMap::new();
        let ref_position_context = super::super::fields::RefPositionContext::default();
        let ref_number_context = super::super::fields::RefNumberContext::empty();
        let page_ref_context = super::super::fields::PageRefContext::empty();
        let note_ref_context = super::super::fields::NoteRefContext::empty();
        let section_context = super::super::fields::SectionContext::empty();
        let style_ref_context = super::super::fields::StyleRefContext::default();
        let legacy_form_context = super::super::fields::LegacyFormContext::default();
        let table_formula_context = super::super::fields::TableFormulaContext::default();
        let toc_entries = Vec::new();
        let bookmark_names = HashSet::new();
        let core_properties = crate::CoreProperties::default();
        let custom_properties = HashMap::new();
        let document_variables = HashMap::new();
        let extended_properties = HashMap::new();
        let ctx = Ctx {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            media: &media,
            charts: &charts,
            ref_targets: &ref_targets,
            ref_position_context: &ref_position_context,
            ref_number_context: &ref_number_context,
            page_ref_context: &page_ref_context,
            note_ref_context: &note_ref_context,
            section_context: &section_context,
            style_ref_context: &style_ref_context,
            legacy_form_context: &legacy_form_context,
            table_formula_context: &table_formula_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
            core_properties: &core_properties,
            custom_properties: &custom_properties,
            document_variables: &document_variables,
            extended_properties: &extended_properties,
            file_size_bytes: None,
            preserve_note_local_fields: false,
            ref_field_cursor: Default::default(),
            page_field_cursor: Default::default(),
            last_page_field_unsupported_display_format: Default::default(),
            page_ref_field_cursor: Default::default(),
            note_ref_field_cursor: Default::default(),
            section_field_cursor: Default::default(),
            style_ref_field_cursor: Default::default(),
            form_field_cursor: Default::default(),
            formula_field_cursor: Default::default(),
            last_formula_field_used_table_context: Default::default(),
            sequence_counters: Default::default(),
            sequence_heading_counts: Default::default(),
            sequence_heading_scopes: Default::default(),
            autonum_counter: Default::default(),
            listnum_counter: Default::default(),
            field_bookmarks: Default::default(),
            counters: Default::default(),
            paragraph_charts: Default::default(),
            section_column_capture: Default::default(),
            pagination_capture: Default::default(),
        };
        if capture_pagination {
            *ctx.pagination_capture.borrow_mut() = Some(PaginationCapture::default());
        }
        let blocks = parse_document(xml, &ctx);
        let (hints, tab_stops, table_rows, table_cells, table_nested, table_cell_line_spacing) =
            ctx.pagination_capture
                .borrow_mut()
                .take()
                .map(|capture| {
                    (
                        capture.hints,
                        capture.tab_stops,
                        capture.table_row_pagination,
                        capture.table_cell_pagination,
                        capture.table_nested_pagination,
                        capture.table_cell_line_spacing,
                    )
                })
                .unwrap_or_default();
        (
            blocks,
            hints,
            tab_stops,
            table_rows,
            table_cells,
            table_nested,
            table_cell_line_spacing,
        )
    }

    #[test]
    fn captures_resolved_top_level_pagination_hints_in_block_order() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="paragraph" w:styleId="Kept">
                    <w:pPr><w:keepNext/><w:keepLines/></w:pPr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>default widow</w:t></w:r></w:p>
            <w:sdt><w:sdtContent><w:p><w:pPr><w:pStyle w:val="Kept"/><w:keepNext w:val="0"/><w:widowControl w:val="off"/></w:pPr><w:r><w:t>styled</w:t></w:r></w:p></w:sdtContent></w:sdt>
            <w:tbl><w:tr><w:tc><w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
            <w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>before</w:t><w:br w:type="page"/><w:t>after</w:t></w:r></w:p>
        </w:body></w:document>"#;

        let (blocks, hints, _, _, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(blocks.len(), 6);
        assert_eq!(hints.len(), blocks.len());
        assert_eq!(
            hints,
            vec![
                PaginationHint {
                    widow_control: true,
                    ..PaginationHint::default()
                },
                PaginationHint {
                    keep_next: false,
                    keep_lines: true,
                    widow_control: false,
                },
                PaginationHint::default(),
                PaginationHint {
                    keep_lines: true,
                    widow_control: true,
                    ..PaginationHint::default()
                },
                PaginationHint::default(),
                PaginationHint {
                    keep_lines: true,
                    widow_control: true,
                    ..PaginationHint::default()
                },
            ]
        );
    }

    #[test]
    fn captures_resolved_style_and_direct_tab_stops_in_block_order() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:docDefaults><w:pPrDefault><w:pPr><w:tabs>
                    <w:tab w:val="left" w:pos="720"/>
                </w:tabs></w:pPr></w:pPrDefault></w:docDefaults>
                <w:style w:type="paragraph" w:styleId="TabBase"><w:pPr><w:tabs>
                    <w:tab w:val="decimal" w:pos="2880"/>
                </w:tabs></w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="TabDerived">
                    <w:basedOn w:val="TabBase"/><w:pPr><w:tabs>
                        <w:tab w:val="center" w:pos="1440"/>
                    </w:tabs></w:pPr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:pStyle w:val="TabDerived"/><w:tabs>
                <w:tab w:val="clear" w:pos="720"/>
                <w:tab w:val="right" w:pos="2160"/>
            </w:tabs></w:pPr><w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r></w:p>
            <w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
        </w:body></w:document>"#;

        let (blocks, hints, tab_stops, _, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(hints.len(), blocks.len());
        assert_eq!(tab_stops.len(), blocks.len());
        assert_eq!(
            tab_stops[0],
            vec![
                TabStop {
                    position_pt: 72.0,
                    alignment: TabAlignment::Center,
                    leader: TabLeader::None,
                },
                TabStop {
                    position_pt: 108.0,
                    alignment: TabAlignment::Right,
                    leader: TabLeader::None,
                },
                TabStop {
                    position_pt: 144.0,
                    alignment: TabAlignment::Decimal,
                    leader: TabLeader::None,
                },
            ]
        );
        assert!(tab_stops[1].is_empty());
    }

    #[test]
    fn captures_direct_table_row_pagination_in_source_order() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:p><w:r><w:t>before</w:t></w:r></w:p>
            <w:tbl>
                <w:tr><w:tc><w:p><w:r><w:t>default</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:p><w:r><w:t>kept</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cantSplit w:val="off"/></w:trPr><w:tc><w:p><w:r><w:t>disabled</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:trPrChange><w:trPr><w:cantSplit/></w:trPr></w:trPrChange></w:trPr><w:tc><w:p><w:r><w:t>historical</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:cantSplit w:val="0"/></mc:Choice>
                    <mc:Fallback><w:cantSplit/></mc:Fallback>
                </mc:AlternateContent></w:trPr><w:tc><w:p><w:r><w:t>choice off</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:cantSplit/></mc:Choice>
                    <mc:Fallback><w:cantSplit w:val="false"/></mc:Fallback>
                </mc:AlternateContent></w:trPr><w:tc><w:p><w:r><w:t>choice on</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:p><w:r><w:t>after</w:t></w:r></w:p>
        </w:body></w:document>"#;

        let (blocks, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);

        assert_eq!(table_rows.len(), blocks.len());
        assert!(table_rows[0].is_empty());
        assert_eq!(
            table_rows[1],
            vec![
                TableRowPaginationHint { cant_split: false },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: false },
                TableRowPaginationHint { cant_split: false },
                TableRowPaginationHint { cant_split: false },
                TableRowPaginationHint { cant_split: true },
            ]
        );
        assert!(table_rows[2].is_empty());
    }

    #[test]
    fn table_row_pagination_uses_table_style_with_direct_row_precedence() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="KeepBase">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:style>
                <w:style w:type="table" w:styleId="KeepDerived">
                    <w:basedOn w:val="KeepBase"/>
                </w:style>
                <w:style w:type="table" w:styleId="AllowDerived">
                    <w:basedOn w:val="KeepBase"/>
                    <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="KeepDerived"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>inherited on</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cantSplit w:val="off"/></w:trPr><w:tc><w:p><w:r><w:t>direct off</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="AllowDerived"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>inherited off</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:p><w:r><w:t>direct on</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:tblStyle w:val="KeepDerived"/></mc:Choice>
                    <mc:Fallback><w:tblStyle w:val="AllowDerived"/></mc:Fallback>
                </mc:AlternateContent></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>choice on</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblPrChange><w:tblPr><w:tblStyle w:val="KeepDerived"/></w:tblPr></w:tblPrChange></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>history ignored</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (blocks, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(table_rows.len(), blocks.len());
        assert_eq!(
            table_rows,
            vec![
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: true },
                ],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: false }],
            ]
        );
    }

    #[test]
    fn table_row_pagination_uses_first_row_conditional_table_style() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="ConditionalKeep">
                    <w:tblStylePr w:type="firstRow">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:tbl>
                <w:tblPr>
                    <w:tblStyle w:val="ConditionalKeep"/>
                    <w:tblLook w:firstRow="1"/>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>first</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>second</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![vec![
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: false },
            ]]
        );
    }

    #[test]
    fn table_row_pagination_uses_horizontal_style_bands() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="Banded">
                    <w:tblPr><w:tblStyleRowBandSize w:val="1"/></w:tblPr>
                    <w:tblStylePr w:type="band1Horz">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="band2Horz">
                        <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Banded"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1 again</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2 again</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![vec![
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: false },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: false },
            ]]
        );
    }

    #[test]
    fn horizontal_style_band_sizes_and_region_precedence_are_bounded() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="SizedBands">
                    <w:tblPr><w:tblStyleRowBandSize w:val="2"/></w:tblPr>
                    <w:tblStylePr w:type="band1Horz">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="band2Horz">
                        <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="firstRow">
                        <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="lastRow">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="OmittedSize">
                    <w:tblStylePr w:type="band1Horz">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="SizedBands"/><w:tblLook w:firstRow="1" w:lastRow="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>first overrides band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>last overrides band 1</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr>
                    <w:tblStyle w:val="SizedBands"/>
                    <w:tblStyleRowBandSize w:val="3"/>
                    <w:tblStyleRowBandSize w:val="9"/>
                    <w:tblLook w:firstRow="0" w:lastRow="0"/>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>band 2</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr>
                    <w:tblStyle w:val="SizedBands"/>
                    <w:tblStyleRowBandSize w:val="0"/>
                    <w:tblLook w:firstRow="0" w:lastRow="0"/>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>disabled</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>disabled</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr>
                    <w:tblStyle w:val="SizedBands"/>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14"><w:tblStyleRowBandSize w:val="2"/></mc:Choice>
                        <mc:Fallback><w:tblStyleRowBandSize w:val="3"/></mc:Fallback>
                    </mc:AlternateContent>
                    <w:tblLook w:firstRow="0" w:lastRow="0"/>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>choice band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>choice band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>choice band 2</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>choice band 2</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="OmittedSize"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Word default size zero</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="SizedBands"/><w:tblStyleRowBandSize w:val="1"/></w:tblPr>
                <w:tr><w:trPr><w:cantSplit w:val="off"/></w:trPr><w:tc><w:p><w:r><w:t>direct off</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:p><w:r><w:t>direct on</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr>
                    <w:tblStyle w:val="SizedBands"/>
                    <w:tblStyleRowBandSize w:val="1"/>
                    <w:tblPrChange><w:tblPr><w:tblStyleRowBandSize w:val="2"/></w:tblPr></w:tblPrChange>
                    <w:tblLook w:firstRow="0" w:lastRow="0"/>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>current size band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>current size band 2</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![
                vec![
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: true },
                ],
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: true },
                ],
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
            ]
        );
    }

    #[test]
    fn horizontal_style_bands_honor_table_look_and_explicit_row_masks() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="Bands">
                    <w:tblPr><w:tblStyleRowBandSize w:val="1"/></w:tblPr>
                    <w:tblStylePr w:type="band1Horz">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="band2Horz">
                        <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>omitted look</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>omitted look</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/><w:tblLook w:noHBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>named disabled</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/><w:tblLook w:val="0200"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>mask disabled</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/><w:tblLook w:noHBand="0" w:val="0200"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>named wins</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/><w:tblLook w:noHBand="1"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:oddHBand="1"/></w:trPr><w:tc><w:p><w:r><w:t>explicit band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:evenHBand="1"/></w:trPr><w:tc><w:p><w:r><w:t>explicit band 2</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/><w:tblLook w:noHBand="1"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:val="000000100000"/></w:trPr><w:tc><w:p><w:r><w:t>mask band 1</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:val="000000010000"/></w:trPr><w:tc><w:p><w:r><w:t>mask band 2</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:evenHBand="0" w:val="000000100000"/></w:trPr><w:tc><w:p><w:r><w:t>named row mask wins</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Bands"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:val="malformed"/></w:trPr><w:tc><w:p><w:r><w:t>malformed falls back</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: true }],
            ]
        );
    }

    #[test]
    fn table_row_pagination_uses_all_explicit_conditional_region_masks() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="AllRows">
                    <w:tblStylePr w:type="band1Vert"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="band2Vert"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstCol"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastCol"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="nwCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="swCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="CornersOnly">
                    <w:tblStylePr w:type="nwCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="swCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:oddVBand="1"/></w:trPr><w:tc><w:p><w:r><w:t>band one</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:evenVBand="1"/></w:trPr><w:tc><w:p><w:r><w:t>band two</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:firstColumn="1"/></w:trPr><w:tc><w:p><w:r><w:t>first column</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:lastColumn="1"/></w:trPr><w:tc><w:p><w:r><w:t>last column</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:firstRowFirstColumn="1"/></w:trPr><w:tc><w:p><w:r><w:t>north west</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:firstRowLastColumn="1"/></w:trPr><w:tc><w:p><w:r><w:t>north east</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:lastRowFirstColumn="1"/></w:trPr><w:tc><w:p><w:r><w:t>south west</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:lastRowLastColumn="1"/></w:trPr><w:tc><w:p><w:r><w:t>south east</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:val="111111111111"/></w:trPr><w:tc><w:p><w:r><w:t>all regions mask</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![vec![
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
                TableRowPaginationHint { cant_split: true },
            ]]
        );
    }

    #[test]
    fn table_row_pagination_uses_table_look_for_vertical_and_corner_regions() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="AllRows">
                    <w:tblPr><w:tblStyleColBandSize w:val="1"/></w:tblPr>
                    <w:tblStylePr w:type="band1Vert"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="band2Vert"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstCol"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastCol"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="nwCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="swCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:noHBand="1" w:noVBand="0"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>band one</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:noHBand="1" w:noVBand="0"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>band two a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>band two b</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:firstColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>first column</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>last column</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:firstRow="1" w:firstColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>north west</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:firstRow="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>north east</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:lastRow="1" w:firstColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>south west</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="AllRows"/><w:tblLook w:lastRow="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>south east</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl><w:tblPr><w:tblStyle w:val="CornersOnly"/><w:tblLook w:firstColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>corner selector disabled</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: false }],
            ]
        );
    }

    #[test]
    fn conditional_cell_regions_follow_table_look_bands_and_precedence() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="Regions">
                    <w:tblPr><w:tblStyleColBandSize w:val="2"/></w:tblPr>
                    <w:tblStylePr w:type="wholeTable"><w:tcPr><w:shd w:fill="010101"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="band1Vert"><w:tcPr><w:shd w:fill="210001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="band2Vert"><w:tcPr><w:shd w:fill="220002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:fill="410001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastCol"><w:tcPr><w:shd w:fill="420002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstRow"><w:tcPr><w:shd w:fill="510001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><w:tcPr><w:shd w:fill="520002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="nwCell"><w:tcPr><w:shd w:fill="710001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:tcPr><w:shd w:fill="720002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="swCell"><w:tcPr><w:shd w:fill="730003"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:tcPr><w:shd w:fill="740004"/></w:tcPr></w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="OmittedSize">
                    <w:tblStylePr w:type="wholeTable"><w:tcPr><w:shd w:fill="010101"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="band1Vert"><w:tcPr><w:shd w:fill="210001"/></w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let cells = |count: usize| {
            (0..count)
                .map(|index| format!(r#"<w:tc><w:p><w:r><w:t>{index}</w:t></w:r></w:p></w:tc>"#))
                .collect::<String>()
        };
        let xml = format!(
            r#"<w:document><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Regions"/><w:tblLook w:val="01E0"/></w:tblPr>
                    <w:tr>{}</w:tr><w:tr>{}</w:tr><w:tr>{}</w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr>
                        <w:tblStyle w:val="Regions"/>
                        <w:tblStyleColBandSize w:val="1"/>
                        <w:tblStyleColBandSize w:val="9"/>
                        <w:tblLook w:val="0000"/>
                    </w:tblPr>
                    <w:tr>{}</w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="OmittedSize"/><w:tblLook w:val="0000"/></w:tblPr>
                    <w:tr>{}</w:tr>
                </w:tbl>
            </w:body></w:document>"#,
            cells(4),
            cells(4),
            cells(4),
            cells(4),
            cells(1),
        );
        let blocks = parse_with_media_and_styles(&xml, HashMap::new(), styles);

        let tables = blocks
            .iter()
            .map(|block| match block {
                Block::Table(table) => table,
                _ => panic!("table"),
            })
            .collect::<Vec<_>>();
        let shades = |table: &Table| {
            table
                .rows
                .iter()
                .map(|row| {
                    row.cells
                        .iter()
                        .map(|cell| cell.shading)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let color = |value: u32| {
            Some(Color::rgb(
                ((value >> 16) & 0xff) as u8,
                ((value >> 8) & 0xff) as u8,
                (value & 0xff) as u8,
            ))
        };

        assert_eq!(
            shades(tables[0]),
            vec![
                vec![
                    color(0x710001),
                    color(0x510001),
                    color(0x510001),
                    color(0x720002)
                ],
                vec![
                    color(0x410001),
                    color(0x210001),
                    color(0x220002),
                    color(0x420002)
                ],
                vec![
                    color(0x730003),
                    color(0x520002),
                    color(0x520002),
                    color(0x740004)
                ],
            ]
        );
        assert_eq!(
            shades(tables[1]),
            vec![vec![
                color(0x210001),
                color(0x220002),
                color(0x210001),
                color(0x220002),
            ]]
        );
        assert_eq!(shades(tables[2]), vec![vec![color(0x010101)]]);
    }

    #[test]
    fn explicit_cell_masks_and_rtl_corners_keep_distinct_semantics() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="Corners">
                    <w:tblStylePr w:type="wholeTable"><w:tcPr><w:shd w:fill="010101"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:fill="110001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastCol"><w:tcPr><w:shd w:fill="120002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="nwCell"><w:tcPr><w:shd w:fill="210001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:tcPr><w:shd w:fill="220002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="swCell"><w:tcPr><w:shd w:fill="230003"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:tcPr><w:shd w:fill="240004"/></w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Corners"/><w:tblLook w:firstColumn="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr>
                    <w:tc><w:tcPr><w:cnfStyle w:firstColumn="0" w:val="001000000000"/></w:tcPr><w:p/></w:tc>
                    <w:tc><w:tcPr><mc:AlternateContent>
                        <mc:Choice Requires="w14"><w:cnfStyle w:lastColumn="1"/></mc:Choice>
                        <mc:Fallback><w:cnfStyle w:firstColumn="1"/></mc:Fallback>
                    </mc:AlternateContent></w:tcPr><w:p/></w:tc>
                    <w:tc><w:tcPr><w:cnfStyle w:firstRowFirstColumn="1"/></w:tcPr><w:p/></w:tc>
                    <w:tc><w:tcPr><w:cnfStyle w:val="000000001000"/></w:tcPr><w:p/></w:tc>
                    <w:tc><w:tcPr><w:tcPrChange><w:tcPr><w:cnfStyle w:firstColumn="1"/></w:tcPr></w:tcPrChange></w:tcPr><w:p/></w:tc>
                    <w:tc><w:tcPr><w:cnfStyle w:val="malformed"/></w:tcPr><w:p/></w:tc>
                </w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Corners"/><w:bidiVisual/><w:tblLook w:firstRow="1" w:lastRow="1" w:firstColumn="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Corners"/><w:bidiVisual/><w:tblLook w:firstRow="0" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                <w:tr>
                    <w:tc><w:tcPr><w:cnfStyle w:firstRowFirstColumn="1"/></w:tcPr><w:p/></w:tc>
                    <w:tc><w:tcPr><w:cnfStyle w:val="000000000100"/></w:tcPr><w:p/></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let blocks = parse_with_media_and_styles(xml, HashMap::new(), styles);
        let tables = blocks
            .iter()
            .map(|block| match block {
                Block::Table(table) => table,
                _ => panic!("table"),
            })
            .collect::<Vec<_>>();
        let color = |value: u32| {
            Some(Color::rgb(
                ((value >> 16) & 0xff) as u8,
                ((value >> 8) & 0xff) as u8,
                (value & 0xff) as u8,
            ))
        };
        let row_shades = |table: &Table, row: usize| {
            table.rows[row]
                .cells
                .iter()
                .map(|cell| cell.shading)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            row_shades(tables[0], 0),
            vec![
                color(0x010101),
                color(0x120002),
                color(0x210001),
                color(0x220002),
                color(0x010101),
                color(0x120002),
            ]
        );
        assert!(tables[1].bidi_visual);
        assert_eq!(
            row_shades(tables[1], 0),
            vec![color(0x220002), color(0x210001)]
        );
        assert_eq!(
            row_shades(tables[1], 1),
            vec![color(0x240004), color(0x230003)]
        );
        assert_eq!(
            row_shades(tables[2], 0),
            vec![color(0x220002), color(0x210001)]
        );
    }

    #[test]
    fn conditional_cell_regions_respect_headers_spans_and_merge_owners() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="Lifecycle">
                    <w:tblStylePr w:type="wholeTable"><w:tcPr><w:shd w:fill="010101"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstRow"><w:tcPr><w:shd w:fill="110001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:fill="120002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastCol"><w:tcPr><w:shd w:fill="130003"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="nwCell"><w:tcPr><w:shd w:fill="210001"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:tcPr><w:shd w:fill="220002"/></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:tcPr><w:shd w:fill="240004"/></w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let two_cells = r#"<w:tc><w:p/></w:tc><w:tc><w:p/></w:tc>"#;
        let xml = format!(
            r#"<w:document><w:body>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Lifecycle"/><w:tblLook w:firstRow="1" w:firstColumn="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                    <w:tr><w:trPr><w:tblHeader/></w:trPr>{two_cells}</w:tr>
                    <w:tr><w:trPr><w:tblHeader/></w:trPr>{two_cells}</w:tr>
                    <w:tr><w:trPr><w:tblHeader/><w:cnfStyle w:firstRow="0"/></w:trPr>{two_cells}</w:tr>
                    <w:tr>{two_cells}</w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Lifecycle"/><w:tblLook w:firstRow="1" w:firstColumn="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                    <w:tr><w:tc><w:tcPr><w:gridSpan w:val="3"/></w:tcPr><w:p/></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                    <w:tr><w:tc><w:p/></w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tblPr><w:tblStyle w:val="Lifecycle"/><w:tblLook w:firstRow="1" w:lastRow="1" w:firstColumn="1" w:lastColumn="1" w:noHBand="1" w:noVBand="1"/></w:tblPr>
                    <w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p/></w:tc></w:tr>
                    <w:tr><w:tc><w:tcPr><w:vMerge/><w:cnfStyle w:lastRowLastColumn="1"/></w:tcPr><w:p/></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#,
        );
        let blocks = parse_with_media_and_styles(&xml, HashMap::new(), styles);
        let tables = blocks
            .iter()
            .map(|block| match block {
                Block::Table(table) => table,
                _ => panic!("table"),
            })
            .collect::<Vec<_>>();
        let color = |value: u32| {
            Some(Color::rgb(
                ((value >> 16) & 0xff) as u8,
                ((value >> 8) & 0xff) as u8,
                (value & 0xff) as u8,
            ))
        };
        let row_shades = |table: &Table, row: usize| {
            table.rows[row]
                .cells
                .iter()
                .map(|cell| cell.shading)
                .collect::<Vec<_>>()
        };

        assert_eq!(tables[0].header_rows, 3);
        assert_eq!(
            row_shades(tables[0], 0),
            vec![color(0x210001), color(0x220002)]
        );
        assert_eq!(
            row_shades(tables[0], 1),
            vec![color(0x110001), color(0x110001)]
        );
        assert_eq!(
            row_shades(tables[0], 2),
            vec![color(0x120002), color(0x130003)]
        );
        assert_eq!(
            row_shades(tables[0], 3),
            vec![color(0x120002), color(0x130003)]
        );

        assert_eq!(row_shades(tables[1], 0), vec![color(0x220002)]);
        assert_eq!(
            row_shades(tables[1], 1),
            vec![color(0x120002), color(0x130003)]
        );
        assert_eq!(row_shades(tables[1], 2), vec![color(0x120002)]);

        assert_eq!(tables[2].rows[0].cells[0].row_span, 2);
        assert_eq!(tables[2].rows[0].cells[0].shading, color(0x220002));
        assert!(tables[2].rows[1].cells.is_empty());
    }

    #[test]
    fn conditional_table_row_style_selection_and_precedence_are_bounded() {
        let styles = super::super::styles::parse(
            r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
                <w:style w:type="table" w:styleId="Precedence">
                    <w:tblStylePr w:type="wholeTable">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="firstRow">
                        <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                    </w:tblStylePr>
                    <w:tblStylePr w:type="lastRow">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="Derived">
                    <w:basedOn w:val="Precedence"/>
                    <w:tblStylePr w:type="lastRow">
                        <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                    </w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="FirstOnly">
                    <w:tblStylePr w:type="firstRow">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="LastOnly">
                    <w:tblStylePr w:type="lastRow">
                        <w:trPr><w:cantSplit/></w:trPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Precedence"/><w:tblLook w:firstRow="1" w:lastRow="1"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>first off</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>whole on</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>last on</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Derived"/><w:tblLook w:val="0060"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>both, last off</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Precedence"/><w:tblLook w:firstRow="0" w:lastRow="0" w:val="0060"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>named attributes win</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="FirstOnly"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Word default first</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>not first</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Precedence"/><w:tblLook w:firstRow="0" w:lastRow="0"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:firstRow="1"/></w:trPr><w:tc><w:p><w:r><w:t>explicit first off</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:trPr><w:cnfStyle w:val="010000000000"/></w:trPr><w:tc><w:p><w:r><w:t>explicit last on</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="FirstOnly"/><w:tblLook w:firstRow="1"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:firstRow="0"/></w:trPr><w:tc><w:p><w:r><w:t>explicit mask suppresses position</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="FirstOnly"/><w:tblLook w:firstRow="1"/></w:tblPr>
                <w:tr><w:trPr><w:cnfStyle w:val="malformed"/></w:trPr><w:tc><w:p><w:r><w:t>malformed mask falls back</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="FirstOnly"/><w:tblLook w:firstRow="0"/></w:tblPr>
                <w:tr><w:trPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:cnfStyle w:firstRow="1"/></mc:Choice>
                    <mc:Fallback><w:cnfStyle w:firstRow="0"/></mc:Fallback>
                </mc:AlternateContent></w:trPr><w:tc><w:p><w:r><w:t>selected row mask</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="LastOnly"/><w:tblPrChange><w:tblPr><w:tblLook w:lastRow="1"/></w:tblPr></w:tblPrChange></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>history ignored</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Precedence"/><w:tblLook w:firstRow="1" w:lastRow="1"/></w:tblPr>
                <w:tr><w:trPr><w:cantSplit w:val="off"/></w:trPr><w:tc><w:p><w:r><w:t>direct off wins</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="Derived"/><w:tblLook w:firstRow="1" w:lastRow="1"/></w:tblPr>
                <w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:p><w:r><w:t>direct on wins</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblStyle w:val="FirstOnly"/><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:tblLook w:firstRow="1"/></mc:Choice>
                    <mc:Fallback><w:tblLook w:firstRow="0"/></mc:Fallback>
                </mc:AlternateContent></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>selected table look</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (_, _, _, table_rows, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(
            table_rows,
            vec![
                vec![
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: true },
                ],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![
                    TableRowPaginationHint { cant_split: false },
                    TableRowPaginationHint { cant_split: true },
                ],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: false }],
                vec![TableRowPaginationHint { cant_split: true }],
                vec![TableRowPaginationHint { cant_split: true }],
            ]
        );
    }

    #[test]
    fn captures_direct_table_cell_pagination_with_surviving_cell_alignment() {
        let styles = super::super::styles::parse(
            r#"<w:styles>
                <w:style w:type="paragraph" w:styleId="Kept">
                    <w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/></w:pPr>
                </w:style>
            </w:styles>"#,
        );
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>before</w:t></w:r></w:p>
            <w:tbl>
                <w:tr>
                    <w:tc><w:p><w:pPr><w:pStyle w:val="Kept"/><w:keepNext w:val="0"/></w:pPr><w:r><w:t>one</w:t><w:br w:type="page"/><w:t>two</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>owner</w:t></w:r></w:p></w:tc>
                    <w:tc><w:sdt><w:sdtContent><w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>wrapped ceiling</w:t></w:r></w:p></w:sdtContent></w:sdt></w:tc>
                </w:tr>
                <w:tr>
                    <w:tc><w:p><w:pPr><w:widowControl w:val="off"/></w:pPr><w:r><w:t>disabled</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>dropped continuation</w:t></w:r></w:p></w:tc>
                    <w:tc><w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>survivor</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
            <w:p><w:r><w:t>after</w:t></w:r></w:p>
        </w:body></w:document>"#;

        let (blocks, _, _, _, table_cells, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), styles, true);

        assert_eq!(table_cells.len(), blocks.len());
        assert!(table_cells[0].is_empty());
        assert_eq!(
            table_cells[1],
            vec![
                vec![
                    vec![
                        Some(PaginationHint {
                            keep_next: false,
                            keep_lines: true,
                            widow_control: false,
                        }),
                        None,
                        Some(PaginationHint {
                            keep_next: false,
                            keep_lines: true,
                            widow_control: false,
                        }),
                    ],
                    vec![Some(PaginationHint {
                        widow_control: true,
                        ..PaginationHint::default()
                    })],
                    vec![Some(PaginationHint {
                        keep_lines: true,
                        widow_control: true,
                        ..PaginationHint::default()
                    })],
                ],
                vec![
                    vec![Some(PaginationHint::default())],
                    vec![Some(PaginationHint {
                        keep_next: true,
                        widow_control: true,
                        ..PaginationHint::default()
                    })],
                ],
            ]
        );
        assert!(table_cells[2].is_empty());
    }

    #[test]
    fn captures_wrapped_table_cell_pagination_in_selected_block_order() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl><w:tr>
                <w:tc><w:sdt><w:sdtContent>
                    <w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>sdt</w:t></w:r></w:p>
                </w:sdtContent></w:sdt></w:tc>
                <w:tc><w:customXml>
                    <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>custom</w:t></w:r></w:p>
                </w:customXml></w:tc>
                <w:tc><w:smartTag>
                    <w:p><w:pPr><w:widowControl w:val="off"/></w:pPr><w:r><w:t>smart</w:t></w:r></w:p>
                </w:smartTag></w:tc>
                <w:tc><w:ins>
                    <w:p><w:pPr><w:keepLines w:val="off"/></w:pPr><w:r><w:t>inserted</w:t></w:r></w:p>
                </w:ins></w:tc>
                <w:tc><w:moveTo>
                    <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>moved</w:t></w:r></w:p>
                </w:moveTo></w:tc>
                <w:tc><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>choice</w:t></w:r></w:p>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:p><w:pPr><w:keepLines w:val="off"/></w:pPr><w:r><w:t>fallback</w:t></w:r></w:p>
                    </mc:Fallback>
                </mc:AlternateContent></w:tc>
                <w:tc><w:customXml><w:ins><w:sdtContent>
                    <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>before</w:t><w:br w:type="page"/><w:t>after</w:t></w:r></w:p>
                </w:sdtContent></w:ins></w:customXml></w:tc>
                <w:tc><w:customXml>
                    <w:tbl><w:tr><w:tc><w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>nested</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                </w:customXml></w:tc>
            </w:tr></w:tbl>
        </w:body></w:document>"#;

        let (blocks, _, _, _, table_cells, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);

        assert_eq!(blocks.len(), 1);
        assert_eq!(table_cells.len(), blocks.len());
        assert_eq!(
            table_cells[0][0],
            vec![
                vec![Some(PaginationHint {
                    keep_lines: true,
                    widow_control: true,
                    ..PaginationHint::default()
                })],
                vec![Some(PaginationHint {
                    keep_next: true,
                    widow_control: true,
                    ..PaginationHint::default()
                })],
                vec![Some(PaginationHint::default())],
                vec![Some(PaginationHint {
                    widow_control: true,
                    ..PaginationHint::default()
                })],
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
                vec![
                    Some(PaginationHint {
                        keep_next: true,
                        widow_control: true,
                        ..PaginationHint::default()
                    }),
                    None,
                    Some(PaginationHint {
                        keep_next: true,
                        widow_control: true,
                        ..PaginationHint::default()
                    }),
                ],
                vec![None],
            ]
        );
    }

    #[test]
    fn captures_nested_table_cell_pagination_recursively() {
        let xml = r#"<w:document><w:body>
            <w:tbl><w:tr><w:tc>
                <w:tbl><w:tr><w:tc>
                    <w:p><w:pPr><w:keepLines/><w:widowControl w:val="off"/></w:pPr>
                        <w:r><w:t>nested</w:t></w:r>
                    </w:p>
                </w:tc></w:tr></w:tbl>
                <w:p/>
            </w:tc></w:tr></w:tbl>
        </w:body></w:document>"#;

        let (blocks, _, _, _, _, nested_tables, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);

        let nested = nested_tables[0][0][0][0]
            .as_ref()
            .expect("nested table block carries recursive pagination state");
        assert_eq!(
            nested.rows,
            vec![TableRowPaginationHint { cant_split: false }]
        );
        assert_eq!(
            nested.cells,
            vec![vec![vec![Some(PaginationHint {
                keep_lines: true,
                widow_control: false,
                ..PaginationHint::default()
            })]]]
        );
        assert_eq!(nested.nested, vec![vec![vec![None]]]);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn table_cell_line_spacing_tracks_fragments_nested_tables_and_merge_owners() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tr>
                    <w:tc>
                        <w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
                        <w:p><w:pPr><w:spacing w:line="240" w:lineRule="exact"/></w:pPr>
                            <w:r><w:t>before</w:t><w:br w:type="page"/><w:t>after</w:t></w:r>
                        </w:p>
                    </w:tc>
                    <w:tc><w:customXml><mc:AlternateContent>
                        <mc:Choice Requires="w14">
                            <w:tbl><w:tr><w:tc>
                                <w:p><w:pPr><w:spacing w:line="800" w:lineRule="atLeast"/></w:pPr>
                                    <w:r><w:t>selected nested</w:t></w:r>
                                </w:p>
                            </w:tc></w:tr></w:tbl>
                        </mc:Choice>
                        <mc:Fallback>
                            <w:p><w:pPr><w:spacing w:line="1980" w:lineRule="exact"/></w:pPr>
                                <w:r><w:t>fallback</w:t></w:r>
                            </w:p>
                        </mc:Fallback>
                    </mc:AlternateContent></w:customXml></w:tc>
                </w:tr>
                <w:tr>
                    <w:tc>
                        <w:tcPr><w:vMerge/></w:tcPr>
                        <w:p><w:pPr><w:spacing w:line="2000" w:lineRule="exact"/></w:pPr>
                            <w:r><w:t>dropped continuation</w:t></w:r>
                        </w:p>
                    </w:tc>
                    <w:tc><w:p><w:pPr><w:spacing w:line="600" w:lineRule="atLeast"/></w:pPr>
                        <w:r><w:t>survivor</w:t></w:r>
                    </w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (blocks, _, _, _, _, nested_tables, table_cell_line_spacing) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);

        assert_eq!(table_cell_line_spacing.len(), blocks.len());
        assert_eq!(table_cell_line_spacing[0].len(), 2);
        assert_eq!(table_cell_line_spacing[0][0].len(), 2);
        assert_eq!(table_cell_line_spacing[0][1].len(), 1);
        assert_eq!(
            table_cell_line_spacing[0][0][0],
            vec![
                Some(LineSpacingHint::Exact(12.0)),
                None,
                Some(LineSpacingHint::Exact(12.0)),
            ]
        );
        assert_eq!(table_cell_line_spacing[0][0][1], vec![None]);
        assert_eq!(
            table_cell_line_spacing[0][1][0],
            vec![Some(LineSpacingHint::AtLeast(30.0))]
        );

        let nested = nested_tables[0][0][1][0]
            .as_ref()
            .expect("selected nested table carries line-spacing state");
        assert_eq!(
            nested.cell_line_spacing,
            vec![vec![vec![Some(LineSpacingHint::AtLeast(40.0))]]]
        );
        assert_eq!(nested.nested, vec![vec![vec![None]]]);
    }

    #[test]
    fn nested_table_pagination_tracks_selected_wrappers_and_merge_survivors() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tr>
                    <w:tc>
                        <w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
                        <w:tbl><w:tr><w:tc>
                            <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>owner</w:t></w:r></w:p>
                        </w:tc></w:tr></w:tbl>
                    </w:tc>
                    <w:tc><w:p><w:r><w:t>peer</w:t></w:r></w:p></w:tc>
                </w:tr>
                <w:tr>
                    <w:tc>
                        <w:tcPr><w:vMerge/></w:tcPr>
                        <w:tbl><w:tr><w:tc>
                            <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>dropped</w:t></w:r></w:p>
                        </w:tc></w:tr></w:tbl>
                    </w:tc>
                    <w:tc>
                        <w:tcPr><w:gridSpan w:val="2"/></w:tcPr>
                        <w:customXml><mc:AlternateContent>
                            <mc:Choice Requires="w14">
                                <w:tbl><w:tr><w:tc>
                                    <w:sdt><w:sdtContent>
                                        <w:tbl><w:tr><w:tc>
                                            <w:p><w:pPr><w:keepLines/><w:widowControl w:val="off"/></w:pPr>
                                                <w:r><w:t>deep</w:t></w:r>
                                            </w:p>
                                        </w:tc></w:tr></w:tbl>
                                    </w:sdtContent></w:sdt>
                                    <w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>trail</w:t></w:r></w:p>
                                </w:tc></w:tr></w:tbl>
                            </mc:Choice>
                            <mc:Fallback>
                                <w:tbl><w:tr><w:tc>
                                    <w:p><w:r><w:t>fallback</w:t></w:r></w:p>
                                </w:tc></w:tr></w:tbl>
                            </mc:Fallback>
                        </mc:AlternateContent></w:customXml>
                    </w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#;

        let (blocks, _, _, _, _, nested_tables, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);

        let Block::Table(outer) = &blocks[0] else {
            panic!("outer table");
        };
        assert_eq!(outer.rows[0].cells[0].row_span, 2);
        assert_eq!(outer.rows[1].cells.len(), 1);
        assert_eq!(outer.rows[1].cells[0].col_span, 2);
        let Block::Table(selected_table) = &outer.rows[1].cells[0].blocks[0] else {
            panic!("selected nested table");
        };
        let Block::Table(deep_table) = &selected_table.rows[0].cells[0].blocks[0] else {
            panic!("second nested table level");
        };
        let Block::Paragraph(deep_paragraph) = &deep_table.rows[0].cells[0].blocks[0] else {
            panic!("deep paragraph");
        };
        let Block::Paragraph(trail_paragraph) = &selected_table.rows[0].cells[0].blocks[1] else {
            panic!("selected trailing paragraph");
        };
        assert_eq!(deep_paragraph.text(), "deep");
        assert_eq!(trail_paragraph.text(), "trail");

        let outer_nested = &nested_tables[0];
        assert_eq!(outer_nested.len(), 2);
        assert_eq!(outer_nested[0].len(), 2);
        assert_eq!(outer_nested[1].len(), 1);
        let selected = outer_nested[1][0][0]
            .as_ref()
            .expect("surviving selected nested table");
        assert_eq!(
            selected.cells[0][0],
            vec![
                None,
                Some(PaginationHint {
                    keep_next: true,
                    widow_control: true,
                    ..PaginationHint::default()
                }),
            ]
        );
        let deep = selected.nested[0][0][0]
            .as_ref()
            .expect("second nested level");
        assert_eq!(
            deep.cells[0][0],
            vec![Some(PaginationHint {
                keep_lines: true,
                widow_control: false,
                ..PaginationHint::default()
            })]
        );
        assert_eq!(deep.nested, vec![vec![vec![None]]]);
    }

    #[test]
    fn deeply_nested_table_pagination_stops_at_the_depth_limit() {
        let nesting = MAX_DEPTH + 4;
        let mut xml = String::from("<w:document><w:body>");
        for _ in 0..nesting {
            xml.push_str("<w:tbl><w:tr><w:tc>");
        }
        xml.push_str("<w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>too deep</w:t></w:r></w:p>");
        for _ in 0..nesting {
            xml.push_str("</w:tc></w:tr></w:tbl>");
        }
        xml.push_str("</w:body></w:document>");

        let (blocks, _, _, _, _, nested_tables, table_cell_line_spacing) =
            parse_with_media_styles_and_pagination(&xml, HashMap::new(), Styles::default(), true);

        assert_eq!(blocks.len(), 1);
        assert_eq!(nested_tables.len(), blocks.len());
        assert_eq!(table_cell_line_spacing.len(), blocks.len());
    }

    #[test]
    fn malformed_nested_table_pagination_remains_aligned() {
        let xml = r#"<w:document><w:body><w:tbl><w:tr><w:tc>
            <w:tbl><w:tr><w:tc>
                <w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>truncated</w:t></w:r>"#;

        let (blocks, _, _, _, _, nested_tables, table_cell_line_spacing) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);

        assert_eq!(nested_tables.len(), blocks.len());
        assert_eq!(table_cell_line_spacing.len(), blocks.len());
    }

    #[test]
    fn deeply_wrapped_table_cell_pagination_stops_at_the_depth_limit() {
        let nesting = MAX_DEPTH + 4;
        let mut xml = String::from("<w:document><w:body><w:tbl><w:tr><w:tc>");
        for _ in 0..nesting {
            xml.push_str("<w:customXml>");
        }
        xml.push_str("<w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:t>too deep</w:t></w:r></w:p>");
        for _ in 0..nesting {
            xml.push_str("</w:customXml>");
        }
        xml.push_str("</w:tc></w:tr></w:tbl></w:body></w:document>");

        let (blocks, _, _, _, table_cells, _, _) =
            parse_with_media_styles_and_pagination(&xml, HashMap::new(), Styles::default(), true);

        assert_eq!(blocks.len(), 1);
        assert_eq!(table_cells.len(), blocks.len());
        assert_eq!(table_cells[0][0], vec![Vec::new()]);
    }

    #[test]
    fn hyperlink_instruction_rejects_trailing_non_switch_tokens() {
        assert_eq!(
            hyperlink_instr_url(r#"HYPERLINK "https://example.com" \o "tip""#).as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            hyperlink_instr_url(r#"HYPERLINK "https://example.com" "extra "#),
            None
        );
        assert_eq!(
            hyperlink_instr_url(r#"HYPERLINK "https://example.com" extra"#),
            None
        );
    }

    #[test]
    fn hyperlink_anchor_trims_ooxml_value() {
        let xml = r#"<w:document><w:body><w:p>
            <w:hyperlink w:anchor=" TargetBookmark "><w:r><w:t>Jump</w:t></w:r></w:hyperlink>
        </w:p></w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        assert!(matches!(
            &p.runs[0].field,
            FieldRole::Hyperlink { url } if url == "#TargetBookmark"
        ));
    }

    #[test]
    fn empty_unsupported_simple_fields_are_counted_in_model_inventory() {
        let xml = r#"<w:document><w:body><w:p>
            <w:fldSimple w:instr=" DOESNOTEXIST "/>
            <w:fldSimple w:instr=" CUSTOMEMPTY "></w:fldSimple>
        </w:p></w:body></w:document>"#;
        let blocks = parse(xml);

        let inventory = crate::report::feature_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 2);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![
                crate::FieldKindCount {
                    kind: FieldKind::Unknown("DOESNOTEXIST".to_string()),
                    count: 1,
                },
                crate::FieldKindCount {
                    kind: FieldKind::Unknown("CUSTOMEMPTY".to_string()),
                    count: 1,
                },
            ]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![crate::FieldEvaluationReasonCount {
                reason: crate::FieldEvaluationReason::UnknownField,
                count: 2,
            }]
        );
        #[cfg(feature = "render")]
        {
            let render_inventory = crate::report::render_inventory_for_model(&blocks);
            assert_eq!(render_inventory.fields, inventory.fields);
            assert_eq!(
                render_inventory.unsupported_field_kinds,
                inventory.unsupported_field_kinds
            );
            assert_eq!(
                render_inventory.unsupported_field_reasons,
                inventory.unsupported_field_reasons
            );
        }
    }

    #[test]
    fn empty_unsupported_complex_fields_are_counted_in_model_inventory() {
        let xml = r#"<w:document><w:body><w:p>
            <w:r><w:fldChar w:fldCharType="begin"/></w:r>
            <w:r><w:instrText> CUSTOMCOMPLEX </w:instrText></w:r>
            <w:r><w:fldChar w:fldCharType="end"/></w:r>
        </w:p></w:body></w:document>"#;
        let blocks = parse(xml);

        let inventory = crate::report::feature_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 1);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![crate::FieldKindCount {
                kind: FieldKind::Unknown("CUSTOMCOMPLEX".to_string()),
                count: 1,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![crate::FieldEvaluationReasonCount {
                reason: crate::FieldEvaluationReason::UnknownField,
                count: 1,
            }]
        );
        #[cfg(feature = "render")]
        {
            let render_inventory = crate::report::render_inventory_for_model(&blocks);
            assert_eq!(render_inventory.fields, inventory.fields);
            assert_eq!(
                render_inventory.unsupported_field_kinds,
                inventory.unsupported_field_kinds
            );
            assert_eq!(
                render_inventory.unsupported_field_reasons,
                inventory.unsupported_field_reasons
            );
        }
    }

    fn raw_merge_cell(vmerge: VMerge) -> CellRaw {
        CellRaw {
            blocks: Vec::new(),
            pagination: Vec::new(),
            line_spacing: Vec::new(),
            nested_tables: Vec::new(),
            tab_stops: Vec::new(),
            column_break_offsets: Vec::new(),
            col_span: 1,
            vmerge,
            shading: None,
            shading_declared: false,
            valign: VCell::Top,
            valign_declared: false,
            width_pct: None,
            width_pct_declared: false,
            margins: CellMarginSpec::default(),
            style_regions: None,
        }
    }

    #[test]
    fn paragraph_runs_with_emphasis() {
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>plain </w:t></w:r>
                 <w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r>
                 <w:r><w:rPr><w:i/></w:rPr><w:t> ital</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        assert_eq!(p.text(), "plain bold ital");
        assert!(p.runs[1].props.bold);
        assert!(p.runs[2].props.italic);
    }

    #[test]
    fn paragraph_and_run_string_attrs_trim_ooxml_values() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:jc w:val=" center "/></w:pPr>
                <w:r><w:rPr><w:rFonts w:ascii=" Arial " w:eastAsia=" 맑은 고딕 "/><w:highlight w:val=" yellow "/></w:rPr><w:t>Styled</w:t></w:r>
            </w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        assert_eq!(p.props.align, Align::Center);
        assert_eq!(p.runs[0].props.font.as_deref(), Some("맑은 고딕"));
        assert_eq!(p.runs[0].props.highlight.as_deref(), Some("yellow"));
    }

    #[test]
    fn extracts_textbox_text_once_across_alternate_content() {
        // The same box is serialized as a DrawingML Choice and a VML Fallback; its
        // text must be recovered exactly once.
        let xml = r#"<w:document><w:body>
            <w:p>
                <w:r><w:t>본문 </w:t></w:r>
                <w:r><w:drawing><mc:AlternateContent>
                    <mc:Choice Requires="wps"><wps:wsp><wps:txbx><w:txbxContent>
                        <w:p><w:r><w:t>박스 텍스트</w:t></w:r></w:p>
                    </w:txbxContent></wps:txbx></wps:wsp></mc:Choice>
                    <mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent>
                        <w:p><w:r><w:t>박스 텍스트</w:t></w:r></w:p>
                    </w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback>
                </mc:AlternateContent></w:drawing></w:r>
            </w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para")
        };
        let text = p.text();
        assert!(text.contains("본문"), "body run lost: {text:?}");
        assert!(
            text.contains("박스 텍스트"),
            "textbox text missing: {text:?}"
        );
        assert_eq!(
            text.matches("박스 텍스트").count(),
            1,
            "textbox text double-counted across AlternateContent: {text:?}"
        );
    }

    #[test]
    fn extracts_floating_shape_text_at_run_level() {
        // A floating shape sits as <w:r><mc:AlternateContent> (Choice=DrawingML,
        // Fallback=VML) directly under the run — its text box must be recovered
        // once, not skipped (the previous behavior dropped the whole shape).
        let xml = r#"<w:document><w:body><w:p>
            <w:r><w:t>본문 </w:t></w:r>
            <w:r><mc:AlternateContent>
                <mc:Choice Requires="wps"><w:drawing><wps:wsp><wps:txbx><w:txbxContent>
                    <w:p><w:r><w:t>도형 속 글자</w:t></w:r></w:p>
                </w:txbxContent></wps:txbx></wps:wsp></w:drawing></mc:Choice>
                <mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent>
                    <w:p><w:r><w:t>도형 속 글자</w:t></w:r></w:p>
                </w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback>
            </mc:AlternateContent></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("para")
        };
        let t = p.text();
        assert!(t.contains("본문"), "body lost: {t:?}");
        assert!(t.contains("도형 속 글자"), "shape text missing: {t:?}");
        assert_eq!(
            t.matches("도형 속 글자").count(),
            1,
            "shape text doubled: {t:?}"
        );
    }

    #[test]
    fn image_rotation_trims_ooxml_units() {
        let mut media = HashMap::new();
        media.insert("rIdImg".to_string(), Image::default());
        let xml = r#"<w:document><w:body><w:p><w:r><w:drawing><wp:inline>
            <a:blip r:embed="rIdImg"/>
            <a:xfrm rot=" 5400000 "/>
        </wp:inline></w:drawing></w:r></w:p></w:body></w:document>"#;
        let blocks = parse_with_media(xml, media);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        let image = p
            .runs
            .iter()
            .find_map(|run| run.image.as_ref())
            .expect("image run");
        assert_eq!(image.rotation_degrees, Some(90));
    }

    #[test]
    fn drawing_image_uses_selected_docpr_description_as_alt_text() {
        let mut media = HashMap::new();
        media.insert("rIdImg".to_string(), Image::default());
        let xml = r#"<w:document><w:body><w:p><w:r><w:drawing>
            <mc:AlternateContent>
                <mc:Choice Requires="w14"><wp:inline>
                    <wp:docPr id="1" name="Choice" descr=" Choice &amp; alt "/>
                    <a:blip r:embed="rIdImg"/>
                </wp:inline></mc:Choice>
                <mc:Fallback><wp:inline>
                    <wp:docPr id="2" name="Fallback" descr="Fallback alt"/>
                    <a:blip r:embed="rIdImg"/>
                </wp:inline></mc:Fallback>
            </mc:AlternateContent>
        </w:drawing></w:r></w:p></w:body></w:document>"#;
        let blocks = parse_with_media(xml, media);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        let image = p
            .runs
            .iter()
            .find_map(|run| run.image.as_ref())
            .expect("image run");
        assert_eq!(image.alt.as_deref(), Some("Choice & alt"));
    }

    #[test]
    fn drawing_alt_text_is_scoped_per_media_occurrence() {
        let mut media = HashMap::new();
        media.insert("rIdShared".to_string(), Image::default());
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:drawing><wp:inline>
                <wp:docPr id="1" name="First" descr="First alt"/>
                <a:blip r:embed="rIdShared"/>
            </wp:inline></w:drawing></w:r></w:p>
            <w:p><w:r><w:drawing><wp:inline>
                <wp:docPr id="2" name="Second" descr="Second alt"/>
                <a:blip r:embed="rIdShared"/>
            </wp:inline></w:drawing></w:r></w:p>
        </w:body></w:document>"#;
        let blocks = parse_with_media(xml, media);
        let observed = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .runs
                    .iter()
                    .find_map(|run| run.image.as_ref())
                    .and_then(|image| image.alt.as_deref()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(observed, ["First alt", "Second alt"]);
    }

    #[test]
    fn floating_image_offsets_trim_relative_from() {
        let mut media = HashMap::new();
        media.insert("rIdImg".to_string(), Image::default());
        let xml = r#"<w:document><w:body><w:p><w:r><w:drawing><wp:anchor>
            <wp:positionH relativeFrom=" page "><wp:posOffset>91440</wp:posOffset></wp:positionH>
            <wp:positionV relativeFrom=" page "><wp:posOffset>182880</wp:posOffset></wp:positionV>
            <a:blip r:embed="rIdImg"/>
        </wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#;
        let blocks = parse_with_media(xml, media);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        let image = p
            .runs
            .iter()
            .find_map(|run| run.image.as_ref())
            .expect("image run");
        assert_eq!(image.floating_offset_emu, Some((91440, 182880)));
    }

    #[test]
    fn scans_header_footer_references() {
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>x</w:t></w:r></w:p>
            <w:sectPr>
                <w:headerReference w:type="default" r:id="rIdH"/>
                <w:headerReference w:type=" " r:id=" rIdDefault "/>
                <w:headerReference w:type="first" r:id=" "/>
                <w:footerReference w:type="default" r:id="rIdF"/>
                <w:footerReference w:type="even" r:id=" "/>
                <w:headerReference w:type="first" r:id="rIdH1"/>
            </w:sectPr>
        </w:body></w:document>"#;
        let (headers, footers) = scan_hf_refs(xml);
        assert_eq!(
            headers,
            vec![
                HeaderFooterRef {
                    rel_id: "rIdH".to_string(),
                    type_name: "default".to_string()
                },
                HeaderFooterRef {
                    rel_id: "rIdDefault".to_string(),
                    type_name: "default".to_string()
                },
                HeaderFooterRef {
                    rel_id: "rIdH1".to_string(),
                    type_name: "first".to_string()
                }
            ]
        );
        assert_eq!(
            footers,
            vec![HeaderFooterRef {
                rel_id: "rIdF".to_string(),
                type_name: "default".to_string()
            }]
        );
    }

    #[test]
    fn scans_header_footer_references_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:headerReference w:type="default" r:id="rIdChoiceHeader"/>
                        <w:footerReference w:type="first" r:id="rIdChoiceFooter"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:headerReference w:type="default" r:id="rIdFallbackHeader"/>
                        <w:footerReference w:type="first" r:id="rIdFallbackFooter"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr>
        </w:body></w:document>"#;
        let (headers, footers) = scan_hf_refs(xml);
        assert_eq!(
            headers,
            vec![HeaderFooterRef {
                rel_id: "rIdChoiceHeader".to_string(),
                type_name: "default".to_string()
            }]
        );
        assert_eq!(
            footers,
            vec![HeaderFooterRef {
                rel_id: "rIdChoiceFooter".to_string(),
                type_name: "first".to_string()
            }]
        );
    }

    #[test]
    fn scans_header_footer_distances_by_revision_view_and_selected_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:del><w:sectPr><w:pgMar w:header="100" w:footer="200"/></w:sectPr></w:del>
            <w:ins><w:sectPr><w:pgMar w:header="300" w:footer="400"/></w:sectPr></w:ins>
            <w:sectPr>
                <w:sectPrChange><w:sectPr><w:pgMar w:header="500" w:footer="600"/></w:sectPr></w:sectPrChange>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:pgMar w:header="700" w:footer="800"/></mc:Choice>
                    <mc:Fallback><w:pgMar w:header="900" w:footer="1000"/></mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr>
        </w:body></w:document>"#;

        let accepted = scan_hf_ref_sections(xml);
        assert_eq!(accepted.len(), 2);
        assert_eq!(accepted[0].header_distance_twips, Some(300));
        assert_eq!(accepted[0].footer_distance_twips, Some(400));
        assert_eq!(accepted[1].header_distance_twips, Some(700));
        assert_eq!(accepted[1].footer_distance_twips, Some(800));

        let rejected = scan_hf_ref_sections_for_revision_reject(xml);
        assert_eq!(rejected.len(), 2);
        assert_eq!(rejected[0].header_distance_twips, Some(100));
        assert_eq!(rejected[0].footer_distance_twips, Some(200));
        assert_eq!(rejected[1].header_distance_twips, Some(700));
        assert_eq!(rejected[1].footer_distance_twips, Some(800));
    }

    #[test]
    fn header_footer_distance_scanner_ignores_invalid_unsigned_twips() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:sectPr><w:pgMar w:header="-1" w:footer="invalid"/></w:sectPr></w:pPr></w:p>
            <w:sectPr><w:pgMar w:header=" 720 " w:footer="4294967296"/></w:sectPr>
        </w:body></w:document>"#;
        let sections = scan_hf_ref_sections(xml);

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].header_distance_twips, None);
        assert_eq!(sections[0].footer_distance_twips, None);
        assert_eq!(sections[1].header_distance_twips, Some(720));
        assert_eq!(sections[1].footer_distance_twips, None);
    }

    #[test]
    fn scans_final_section_page_number_start_only() {
        let no_final_restart = r#"<w:document><w:body>
            <w:p><w:pPr><w:sectPr><w:pgNumType w:start="3" w:fmt="upperRoman"/></w:sectPr></w:pPr></w:p>
            <w:sectPr/>
        </w:body></w:document>"#;
        assert_eq!(scan_page_number_start(no_final_restart), None);
        assert_eq!(scan_page_number_format(no_final_restart), None);

        let final_restart = r#"<w:document><w:body>
            <w:p><w:pPr><w:sectPr><w:pgNumType w:start="3"/></w:sectPr></w:pPr></w:p>
            <w:sectPr><w:pgNumType w:start="7" w:fmt="decimalZero"/></w:sectPr>
        </w:body></w:document>"#;
        assert_eq!(scan_page_number_start(final_restart), Some(7));
        assert_eq!(
            scan_page_number_format(final_restart),
            Some(PageNumberFormat::DecimalZero)
        );
    }

    #[test]
    fn scans_final_section_page_number_uses_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:pgNumType w:start="7" w:fmt="decimalZero"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:pgNumType w:start="12" w:fmt="upperRoman"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr>
        </w:body></w:document>"#;
        assert_eq!(scan_page_number_start(xml), Some(7));
        assert_eq!(
            scan_page_number_format(xml),
            Some(PageNumberFormat::DecimalZero)
        );
    }

    #[test]
    fn page_orientation_trims_ooxml_value() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:sectPr><w:pgSz w:w="15840" w:h="12240" w:orient=" landscape "/></w:sectPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>
        </w:body></w:document>"#;
        assert!(scan_page_setup(xml).landscape);
        let blocks = parse(xml);
        let section = blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(section) => Some(section),
                _ => None,
            })
            .expect("section break");
        assert!(section.page.landscape);
    }

    #[test]
    fn page_setup_uses_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>
                        <w:pgMar w:left="720" w:right="1080" w:top="1440" w:bottom="1800"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:pgSz w:w="12240" w:h="15840"/>
                        <w:pgMar w:left="1440" w:right="1440" w:top="1440" w:bottom="1440"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr>
        </w:body></w:document>"#;
        let page = scan_page_setup(xml);

        assert!(page.landscape);
        assert_eq!(page.width_pt, 792.0);
        assert_eq!(page.height_pt, 612.0);
        assert_eq!(page.margin_pt, 36.0);
        assert_eq!(page.margin_left_pt, Some(36.0));
        assert_eq!(page.margin_right_pt, Some(54.0));
        assert_eq!(page.margin_top_pt, Some(72.0));
        assert_eq!(page.margin_bottom_pt, Some(90.0));
    }

    #[test]
    fn final_section_setup_scanners_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:cols w:num="2"/>
                        <w:textDirection w:val="tbRl"/>
                        <w:docGrid w:type="lines" w:linePitch="360" w:charSpace="120"/>
                        <w:titlePg/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:cols w:num="5"/>
                        <w:textDirection w:val="lrTb"/>
                        <w:docGrid w:type="snapToChars" w:linePitch="720" w:charSpace="240"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr>
        </w:body></w:document>"#;

        assert_eq!(scan_section_columns(xml), Some(2));
        assert_eq!(
            scan_section_text_direction(xml),
            Some(TextDirection::TopToBottomRightToLeft)
        );
        assert_eq!(
            scan_section_doc_grid(xml),
            Some(DocGrid {
                grid_type: DocGridType::Lines,
                line_pitch: Some(360),
                character_space: Some(120),
            })
        );
        assert!(scan_section_title_page(xml));
    }

    #[test]
    fn final_section_column_spacing_uses_equal_current_branch_only() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:cols w:num="2" w:equalWidth="true" w:space="960"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:cols w:num="2" w:space="240"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr>
        </w:body></w:document>"#;

        assert_eq!(scan_final_section_column_hints(xml).gap_pt, Some(48.0));

        let unequal = r#"<w:document><w:body><w:sectPr>
            <w:cols w:num="2" w:equalWidth="0" w:space="960">
                <w:col w:w="2400" w:space="480"/>
                <w:col w:w="4800"/>
            </w:cols>
        </w:sectPr></w:body></w:document>"#;
        assert_eq!(scan_final_section_column_hints(unequal).gap_pt, None);
    }

    #[test]
    fn final_section_column_separator_follows_on_off_and_default_values() {
        for value in ["true", "TRUE", "on", "1", "unexpected"] {
            let xml = format!(
                r#"<w:document><w:body><w:sectPr><w:cols w:num="2" w:sep="{value}"/></w:sectPr></w:body></w:document>"#
            );
            assert!(scan_final_section_column_hints(&xml).separator, "{value}");
        }
        for value in ["false", "FALSE", "off", "0"] {
            let xml = format!(
                r#"<w:document><w:body><w:sectPr><w:cols w:num="2" w:sep="{value}"/></w:sectPr></w:body></w:document>"#
            );
            assert!(!scan_final_section_column_hints(&xml).separator, "{value}");
        }

        let omitted =
            r#"<w:document><w:body><w:sectPr><w:cols w:num="2"/></w:sectPr></w:body></w:document>"#;
        assert!(!scan_final_section_column_hints(omitted).separator);

        let last_definition_wins = r#"<w:document><w:body><w:sectPr>
            <w:cols w:num="2" w:sep="1"/><w:cols w:num="2"/>
        </w:sectPr></w:body></w:document>"#;
        assert!(!scan_final_section_column_hints(last_definition_wins).separator);
    }

    #[test]
    fn section_column_separator_uses_selected_mce_branch_and_survives_bad_geometry() {
        let selected = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:sectPr>
            <mc:AlternateContent>
                <mc:Choice Requires="w14"><w:cols w:num="2" w:sep="on"/></mc:Choice>
                <mc:Fallback><w:cols w:num="2" w:sep="off"/></mc:Fallback>
            </mc:AlternateContent>
        </w:sectPr></w:body></w:document>"#;
        assert!(scan_final_section_column_hints(selected).separator);

        let malformed = r#"<w:document><w:body><w:sectPr>
            <w:cols w:equalWidth="0" w:sep="1"><w:col w:w="0"/></w:cols>
        </w:sectPr></w:body></w:document>"#;
        let malformed = scan_final_section_column_hints(malformed);
        assert_eq!(malformed.layout, None);
        assert!(malformed.separator);
    }

    #[test]
    fn final_section_column_rtl_follows_on_off_source_order_and_selected_mce_branch() {
        for value in ["1", "true", "on"] {
            let xml = format!("<w:document><w:body><w:sectPr><w:bidi w:val=\"{value}\"/></w:sectPr></w:body></w:document>");
            assert!(scan_final_section_column_hints(&xml).rtl, "{value}");
        }
        for value in ["0", "false", "off"] {
            let xml = format!("<w:document><w:body><w:sectPr><w:bidi w:val=\"{value}\"/></w:sectPr></w:body></w:document>");
            assert!(!scan_final_section_column_hints(&xml).rtl, "{value}");
        }
        assert!(
            scan_final_section_column_hints(
                "<w:document><w:body><w:sectPr><w:bidi></w:bidi></w:sectPr></w:body></w:document>"
            )
            .rtl
        );
        assert!(!scan_final_section_column_hints(
            "<w:document><w:body><w:sectPr><w:bidi/><w:bidi w:val=\"0\"/></w:sectPr></w:body></w:document>"
        )
        .rtl);

        let selected = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:sectPr>
            <mc:AlternateContent><mc:Choice Requires="w14"><w:bidi/></mc:Choice><mc:Fallback><w:bidi w:val="0"/></mc:Fallback></mc:AlternateContent>
        </w:sectPr></w:body></w:document>"#;
        assert!(scan_final_section_column_hints(selected).rtl);

        let empty_choice = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:sectPr>
            <mc:AlternateContent><mc:Choice Requires="w14"/><mc:Fallback><w:bidi/></mc:Fallback></mc:AlternateContent>
        </w:sectPr></w:body></w:document>"#;
        assert!(!scan_final_section_column_hints(empty_choice).rtl);
    }

    #[test]
    fn final_unequal_columns_use_direct_child_count() {
        let xml = r#"<w:document><w:body><w:sectPr>
            <w:cols w:num="9" w:equalWidth="0">
                <w:col w:w="2400" w:space="480"/>
                <w:col w:w="4800"/>
            </w:cols>
        </w:sectPr></w:body></w:document>"#;

        assert_eq!(scan_section_columns(xml), Some(2));
    }

    #[test]
    fn final_unequal_columns_preserve_bounded_direct_geometry() {
        let xml = r#"<w:document><w:body><w:sectPr>
            <w:cols w:num="9" w:equalWidth="false" w:space="960">
                <w:col w:w="2400" w:space="480"/>
                <w:col w:w="4800" w:space="120"/>
            </w:cols>
        </w:sectPr></w:body></w:document>"#;

        let hints = scan_final_section_column_hints(xml);
        let layout = hints.layout.expect("unequal column geometry");
        assert_eq!(layout.columns.len(), 2);
        assert_eq!(layout.columns[0].width_pt, 120.0);
        assert_eq!(layout.columns[0].space_after_pt, 24.0);
        assert_eq!(layout.columns[1].width_pt, 240.0);
        assert_eq!(layout.columns[1].space_after_pt, 6.0);
        assert_eq!(hints.gap_pt, None);
    }

    #[test]
    fn unequal_columns_reject_malformed_or_excessive_direct_geometry() {
        for cols in [
            r#"<w:col w:w="0"/><w:col w:w="2400"/>"#,
            r#"<w:col/><w:col w:w="2400"/>"#,
            r#"<w:col w:w="31681"/><w:col w:w="2400"/>"#,
            r#"<w:col w:w="2400" w:space="invalid"/><w:col w:w="2400"/>"#,
            r#"<w:col w:w="2400" w:space="31681"/><w:col w:w="2400"/>"#,
        ] {
            let xml = format!(
                r#"<w:document><w:body><w:sectPr><w:cols w:num="7" w:equalWidth="0">{cols}</w:cols></w:sectPr></w:body></w:document>"#
            );
            assert_eq!(scan_section_columns(&xml), None, "{cols}");
            assert_eq!(scan_final_section_column_hints(&xml).layout, None, "{cols}");
        }

        let sixty_four = (0..64)
            .map(|_| r#"<w:col w:w="2400"/>"#)
            .collect::<String>();
        let accepted = format!(
            r#"<w:document><w:body><w:sectPr><w:cols w:equalWidth="0">{sixty_four}</w:cols></w:sectPr></w:body></w:document>"#
        );
        assert_eq!(scan_section_columns(&accepted), Some(64));
        assert_eq!(
            scan_final_section_column_hints(&accepted)
                .layout
                .expect("64 direct columns")
                .columns
                .len(),
            64
        );

        let sixty_five = format!(r#"{sixty_four}<w:col w:w="2400"/>"#);
        let rejected = format!(
            r#"<w:document><w:body><w:sectPr><w:cols w:num="9" w:equalWidth="0">{sixty_five}</w:cols></w:sectPr></w:body></w:document>"#
        );
        assert_eq!(scan_section_columns(&rejected), None);
        assert_eq!(scan_final_section_column_hints(&rejected).layout, None);
    }

    #[test]
    fn unequal_columns_follow_selected_alternate_content_branch() {
        let xml = r#"<w:document><w:body><w:sectPr><mc:AlternateContent>
            <mc:Choice Requires="w14"><w:cols w:num="8" w:equalWidth="off">
                <w:col w:w="2000" w:space="400"/><w:col w:w="4000"/>
            </w:cols></mc:Choice>
            <mc:Fallback><w:cols w:num="5"/></mc:Fallback>
        </mc:AlternateContent></w:sectPr></w:body></w:document>"#;

        assert_eq!(scan_section_columns(xml), Some(2));
        let layout = scan_final_section_column_hints(xml)
            .layout
            .expect("selected Choice geometry");
        assert_eq!(layout.columns[0].width_pt, 100.0);
        assert_eq!(layout.columns[0].space_after_pt, 20.0);
        assert_eq!(layout.columns[1].width_pt, 200.0);
    }

    #[test]
    fn equal_columns_ignore_direct_geometry() {
        let xml = r#"<w:document><w:body><w:sectPr>
            <w:cols w:num="3" w:equalWidth="true" w:space="360">
                <w:col w:w="2400"/><w:col w:w="4800"/>
            </w:cols>
        </w:sectPr></w:body></w:document>"#;

        assert_eq!(scan_section_columns(xml), Some(3));
        let hints = scan_final_section_column_hints(xml);
        assert_eq!(hints.gap_pt, Some(18.0));
        assert_eq!(hints.layout, None);
    }

    #[test]
    fn section_props_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:p><w:pPr><w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:type w:val="nextPage"/>
                        <w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>
                        <w:pgNumType w:start="3" w:fmt="upperRoman"/>
                        <w:cols w:num="2"/>
                        <w:textDirection w:val="tbRl"/>
                        <w:docGrid w:type="lines" w:linePitch="360" w:charSpace="120"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:type w:val="continuous"/>
                        <w:pgSz w:w="12240" w:h="15840"/>
                        <w:pgNumType w:start="9" w:fmt="decimalZero"/>
                        <w:cols w:num="5"/>
                        <w:textDirection w:val="lrTb"/>
                        <w:docGrid w:type="snapToChars" w:linePitch="720" w:charSpace="240"/>
                        <w:titlePg/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sectPr></w:pPr></w:p>
        </w:body></w:document>"#;
        let section = parse(xml)
            .into_iter()
            .find_map(|block| match block {
                Block::SectionBreak(section) => Some(section),
                _ => None,
            })
            .expect("section break");

        assert_eq!(section.section_break, Some(SectionBreakKind::NextPage));
        assert!(section.page.landscape);
        assert_eq!(section.page_number_start, Some(3));
        assert_eq!(
            section.page_number_format,
            Some(PageNumberFormat::UpperRoman)
        );
        assert_eq!(section.columns, Some(2));
        assert_eq!(
            section.text_direction,
            Some(TextDirection::TopToBottomRightToLeft)
        );
        assert_eq!(
            section.doc_grid,
            Some(DocGrid {
                grid_type: DocGridType::Lines,
                line_pitch: Some(360),
                character_space: Some(120),
            })
        );
        assert!(!section.title_page);
    }

    #[test]
    fn parses_real_notes_skipping_separators() {
        let styles = Styles::default();
        let numbering = Numbering::default();
        let rels = HashMap::new();
        let media = HashMap::new();
        let charts = HashMap::new();
        let ref_targets = HashMap::new();
        let ref_position_context = super::super::fields::RefPositionContext::default();
        let ref_number_context = super::super::fields::RefNumberContext::empty();
        let page_ref_context = super::super::fields::PageRefContext::empty();
        let note_ref_context = super::super::fields::NoteRefContext::empty();
        let section_context = super::super::fields::SectionContext::empty();
        let style_ref_context = super::super::fields::StyleRefContext::default();
        let legacy_form_context = super::super::fields::LegacyFormContext::default();
        let table_formula_context = super::super::fields::TableFormulaContext::default();
        let toc_entries = Vec::new();
        let bookmark_names = HashSet::new();
        let core_properties = crate::CoreProperties::default();
        let custom_properties = HashMap::new();
        let document_variables = HashMap::new();
        let extended_properties = HashMap::new();
        let ctx = Ctx {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            media: &media,
            charts: &charts,
            ref_targets: &ref_targets,
            ref_position_context: &ref_position_context,
            ref_number_context: &ref_number_context,
            page_ref_context: &page_ref_context,
            note_ref_context: &note_ref_context,
            section_context: &section_context,
            style_ref_context: &style_ref_context,
            legacy_form_context: &legacy_form_context,
            table_formula_context: &table_formula_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
            core_properties: &core_properties,
            custom_properties: &custom_properties,
            document_variables: &document_variables,
            extended_properties: &extended_properties,
            file_size_bytes: None,
            preserve_note_local_fields: false,
            ref_field_cursor: Default::default(),
            page_field_cursor: Default::default(),
            last_page_field_unsupported_display_format: Default::default(),
            page_ref_field_cursor: Default::default(),
            note_ref_field_cursor: Default::default(),
            section_field_cursor: Default::default(),
            style_ref_field_cursor: Default::default(),
            form_field_cursor: Default::default(),
            formula_field_cursor: Default::default(),
            last_formula_field_used_table_context: Default::default(),
            sequence_counters: Default::default(),
            sequence_heading_counts: Default::default(),
            sequence_heading_scopes: Default::default(),
            autonum_counter: Default::default(),
            listnum_counter: Default::default(),
            field_bookmarks: Default::default(),
            counters: Default::default(),
            paragraph_charts: Default::default(),
            section_column_capture: Default::default(),
            pagination_capture: Default::default(),
        };
        let xml = r#"<w:footnotes>
            <w:footnote w:type=" separator " w:id="-1"><w:p><w:r><w:t>SEP</w:t></w:r></w:p></w:footnote>
            <w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:t>CONT</w:t></w:r></w:p></w:footnote>
            <w:footnote w:id="1"><w:p><w:r><w:t>실제 각주 내용</w:t></w:r></w:p></w:footnote>
        </w:footnotes>"#;
        let blocks = parse_notes(xml, &ctx, b"footnote");
        let text: String = blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some(p.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("|");
        assert_eq!(text, "실제 각주 내용", "got: {text:?}");
    }

    #[test]
    fn note_entries_use_single_alternate_content_branch() {
        let styles = Styles::default();
        let numbering = Numbering::default();
        let rels = HashMap::new();
        let media = HashMap::new();
        let charts = HashMap::new();
        let ref_targets = HashMap::new();
        let ref_position_context = super::super::fields::RefPositionContext::default();
        let ref_number_context = super::super::fields::RefNumberContext::empty();
        let page_ref_context = super::super::fields::PageRefContext::empty();
        let note_ref_context = super::super::fields::NoteRefContext::empty();
        let section_context = super::super::fields::SectionContext::empty();
        let style_ref_context = super::super::fields::StyleRefContext::default();
        let legacy_form_context = super::super::fields::LegacyFormContext::default();
        let table_formula_context = super::super::fields::TableFormulaContext::default();
        let toc_entries = Vec::new();
        let bookmark_names = HashSet::new();
        let core_properties = crate::CoreProperties::default();
        let custom_properties = HashMap::new();
        let document_variables = HashMap::new();
        let extended_properties = HashMap::new();
        let ctx = Ctx {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            media: &media,
            charts: &charts,
            ref_targets: &ref_targets,
            ref_position_context: &ref_position_context,
            ref_number_context: &ref_number_context,
            page_ref_context: &page_ref_context,
            note_ref_context: &note_ref_context,
            section_context: &section_context,
            style_ref_context: &style_ref_context,
            legacy_form_context: &legacy_form_context,
            table_formula_context: &table_formula_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
            core_properties: &core_properties,
            custom_properties: &custom_properties,
            document_variables: &document_variables,
            extended_properties: &extended_properties,
            file_size_bytes: None,
            preserve_note_local_fields: false,
            ref_field_cursor: Default::default(),
            page_field_cursor: Default::default(),
            last_page_field_unsupported_display_format: Default::default(),
            page_ref_field_cursor: Default::default(),
            note_ref_field_cursor: Default::default(),
            section_field_cursor: Default::default(),
            style_ref_field_cursor: Default::default(),
            form_field_cursor: Default::default(),
            formula_field_cursor: Default::default(),
            last_formula_field_used_table_context: Default::default(),
            sequence_counters: Default::default(),
            sequence_heading_counts: Default::default(),
            sequence_heading_scopes: Default::default(),
            autonum_counter: Default::default(),
            listnum_counter: Default::default(),
            field_bookmarks: Default::default(),
            counters: Default::default(),
            paragraph_charts: Default::default(),
            section_column_capture: Default::default(),
            pagination_capture: Default::default(),
        };
        let xml = r#"<w:footnotes xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <mc:AlternateContent>
                <mc:Choice Requires="w14">
                    <w:footnote w:id="1"><w:p><w:r><w:t>Choice note</w:t></w:r></w:p></w:footnote>
                </mc:Choice>
                <mc:Fallback>
                    <w:footnote w:id="9"><w:p><w:r><w:t>Fallback note</w:t></w:r></w:p></w:footnote>
                </mc:Fallback>
            </mc:AlternateContent>
        </w:footnotes>"#;

        let entries = parse_note_entries(xml, &ctx, b"footnote");
        let notes: Vec<_> = entries
            .iter()
            .map(|(id, blocks)| {
                let text = blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Paragraph(paragraph) => Some(paragraph.text()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("|");
                (id.as_str(), text)
            })
            .collect();

        assert_eq!(notes, vec![("1", "Choice note".to_string())]);
    }

    #[test]
    fn parses_header_part_blocks() {
        let styles = Styles::default();
        let numbering = Numbering::default();
        let rels = HashMap::new();
        let media = HashMap::new();
        let charts = HashMap::new();
        let ref_targets = HashMap::new();
        let ref_position_context = super::super::fields::RefPositionContext::default();
        let ref_number_context = super::super::fields::RefNumberContext::empty();
        let page_ref_context = super::super::fields::PageRefContext::empty();
        let note_ref_context = super::super::fields::NoteRefContext::empty();
        let section_context = super::super::fields::SectionContext::empty();
        let style_ref_context = super::super::fields::StyleRefContext::default();
        let legacy_form_context = super::super::fields::LegacyFormContext::default();
        let table_formula_context = super::super::fields::TableFormulaContext::default();
        let toc_entries = Vec::new();
        let bookmark_names = HashSet::new();
        let core_properties = crate::CoreProperties::default();
        let custom_properties = HashMap::new();
        let document_variables = HashMap::new();
        let extended_properties = HashMap::new();
        let ctx = Ctx {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            media: &media,
            charts: &charts,
            ref_targets: &ref_targets,
            ref_position_context: &ref_position_context,
            ref_number_context: &ref_number_context,
            page_ref_context: &page_ref_context,
            note_ref_context: &note_ref_context,
            section_context: &section_context,
            style_ref_context: &style_ref_context,
            legacy_form_context: &legacy_form_context,
            table_formula_context: &table_formula_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
            core_properties: &core_properties,
            custom_properties: &custom_properties,
            document_variables: &document_variables,
            extended_properties: &extended_properties,
            file_size_bytes: None,
            preserve_note_local_fields: false,
            ref_field_cursor: Default::default(),
            page_field_cursor: Default::default(),
            last_page_field_unsupported_display_format: Default::default(),
            page_ref_field_cursor: Default::default(),
            note_ref_field_cursor: Default::default(),
            section_field_cursor: Default::default(),
            style_ref_field_cursor: Default::default(),
            form_field_cursor: Default::default(),
            formula_field_cursor: Default::default(),
            last_formula_field_used_table_context: Default::default(),
            sequence_counters: Default::default(),
            sequence_heading_counts: Default::default(),
            sequence_heading_scopes: Default::default(),
            autonum_counter: Default::default(),
            listnum_counter: Default::default(),
            field_bookmarks: Default::default(),
            counters: Default::default(),
            paragraph_charts: Default::default(),
            section_column_capture: Default::default(),
            pagination_capture: Default::default(),
        };
        let xml = r#"<w:hdr><w:p><w:r><w:t>헤더 텍스트</w:t></w:r></w:p></w:hdr>"#;
        let blocks = parse_hdrftr(xml, &ctx);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para")
        };
        assert_eq!(p.text(), "헤더 텍스트");
    }

    #[test]
    fn reads_rich_char_para_and_cell_formatting() {
        use crate::model::{CellMargins, Color, VCell, VertAlign};
        let xml = r#"<w:document><w:body>
            <w:p>
                <w:pPr><w:spacing w:before="240" w:after="120" w:line="360"/><w:ind w:left="720" w:firstLine="240"/><w:shd w:fill=" EEEEEE "/></w:pPr>
                <w:r><w:rPr><w:rFonts w:ascii="Arial" w:eastAsia="맑은 고딕"/><w:sz w:val="24"/><w:color w:val=" FF0000 "/><w:vertAlign w:val=" superscript "/><w:caps/></w:rPr><w:t>빨강</w:t></w:r>
            </w:p>
            <w:tbl><w:tblPr><w:tblW w:w="4000" w:type=" pct "/><w:tblLayout w:type=" fixed "/><w:tblInd w:w="720" w:type=" dxa "/><w:jc w:val=" center "/></w:tblPr><w:tr><w:tc>
                <w:tcPr><w:shd w:fill=" DDDDDD "/><w:vAlign w:val=" center "/><w:tcW w:w="2500" w:type=" pct "/><w:tcMar><w:top w:w="120" w:type=" dxa "/><w:right w:w="240" w:type=" dxa "/></w:tcMar></w:tcPr>
                <w:p><w:r><w:t>셀</w:t></w:r></w:p>
            </w:tc></w:tr></w:tbl>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para")
        };
        let rp = &p.runs[0].props;
        assert_eq!(rp.font.as_deref(), Some("맑은 고딕")); // eastAsia preferred
        assert_eq!(rp.size_half_pt, Some(24));
        assert_eq!(rp.color, Some(Color { r: 255, g: 0, b: 0 }));
        assert_eq!(rp.vert_align, VertAlign::Super);
        assert!(rp.caps, "w:caps not captured");
        assert_eq!(p.props.spacing.before_pt, Some(12.0));
        assert_eq!(p.props.spacing.after_pt, Some(6.0));
        assert_eq!(p.props.spacing.line_pct, Some(1.5));
        assert_eq!(p.props.indent.left_pt, Some(36.0));
        assert_eq!(p.props.indent.first_line_pt, Some(12.0));
        assert_eq!(
            p.props.shading,
            Some(Color {
                r: 0xEE,
                g: 0xEE,
                b: 0xEE
            })
        );
        let Block::Table(t) = &blocks[1] else {
            panic!("table")
        };
        assert_eq!(t.width_pct, Some(0.8));
        assert!(t.fixed_layout);
        assert_eq!(t.indent_twips, Some(720));
        assert_eq!(t.align, Some(Align::Center));
        let c = &t.rows[0].cells[0];
        assert_eq!(
            c.shading,
            Some(Color {
                r: 0xDD,
                g: 0xDD,
                b: 0xDD
            })
        );
        assert_eq!(c.valign, VCell::Center);
        assert_eq!(c.width_pct, Some(0.5));
        assert_eq!(
            c.margins,
            Some(CellMargins {
                top: 120,
                right: 240,
                bottom: 0,
                left: DEFAULT_HORIZONTAL_CELL_MARGIN_TWIPS,
            })
        );
    }

    #[test]
    fn underline_none_trims_ooxml_value() {
        let xml = r#"<w:document><w:body><w:p>
            <w:r><w:rPr><w:u w:val=" none "/></w:rPr><w:t>off</w:t></w:r>
            <w:r><w:rPr><w:u/></w:rPr><w:t>on</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("para");
        };
        assert!(!p.runs[0].props.underline);
        assert!(p.runs[1].props.underline);
    }

    #[test]
    fn reads_rtl_onoff_properties() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:bidi/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Arial" w:cs="Noto Sans Arabic"/><w:rtl/></w:rPr><w:t>rtl</w:t></w:r></w:p>
            <w:p><w:pPr><w:bidi w:val="0"/></w:pPr><w:r><w:rPr><w:rtl w:val="0"/></w:rPr><w:t>ltr</w:t></w:r></w:p>
            <w:tbl><w:tblPr><w:bidiVisual/></w:tblPr><w:tr><w:tc><w:p><w:r><w:t>visual</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
            <w:tbl><w:tblPr><w:bidiVisual w:val="0"/></w:tblPr><w:tr><w:tc><w:p><w:r><w:t>logical</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
        </w:body></w:document>"#;
        let blocks = parse(xml);

        let Block::Paragraph(rtl_para) = &blocks[0] else {
            panic!("rtl paragraph")
        };
        assert!(rtl_para.props.bidi);
        assert!(rtl_para.runs[0].props.rtl);
        assert_eq!(
            rtl_para.runs[0].props.font.as_deref(),
            Some("Noto Sans Arabic")
        );

        let Block::Paragraph(ltr_para) = &blocks[1] else {
            panic!("ltr paragraph")
        };
        assert!(!ltr_para.props.bidi);
        assert!(!ltr_para.runs[0].props.rtl);

        let Block::Table(visual_table) = &blocks[2] else {
            panic!("visual table")
        };
        assert!(visual_table.bidi_visual);

        let Block::Table(logical_table) = &blocks[3] else {
            panic!("logical table")
        };
        assert!(!logical_table.bidi_visual);
    }

    #[test]
    fn resolves_logical_alignment_and_indents_by_paragraph_direction() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:jc w:val="start"/><w:ind w:start="720" w:end="1440"/></w:pPr><w:r><w:t>ltr start</w:t></w:r></w:p>
            <w:p><w:pPr><w:bidi/><w:jc w:val="start"/><w:ind w:start="720" w:end="1440"/></w:pPr><w:r><w:t>rtl start</w:t></w:r></w:p>
            <w:p><w:pPr><w:bidi/><w:jc w:val="end"/></w:pPr><w:r><w:t>rtl end</w:t></w:r></w:p>
            <w:p><w:pPr><w:bidi/><w:jc w:val="left"/></w:pPr><w:r><w:t>rtl physical left</w:t></w:r></w:p>
            <w:p><w:pPr><w:bidi/></w:pPr><w:r><w:t>rtl default</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let paragraphs = blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph(paragraph) => paragraph,
                _ => panic!("paragraph"),
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraphs[0].props.align, Align::Left);
        assert_eq!(paragraphs[0].props.indent.left_pt, Some(36.0));
        assert_eq!(paragraphs[0].props.indent.right_pt, Some(72.0));
        assert_eq!(paragraphs[1].props.align, Align::Right);
        assert_eq!(paragraphs[1].props.indent.left_pt, Some(72.0));
        assert_eq!(paragraphs[1].props.indent.right_pt, Some(36.0));
        assert_eq!(paragraphs[2].props.align, Align::Left);
        assert_eq!(paragraphs[3].props.align, Align::Left);
        assert_eq!(paragraphs[4].props.align, Align::Right);
    }

    #[test]
    fn resolves_inherited_rtl_paragraph_and_complex_script_font() {
        let styles_xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="RtlBody">
                <w:pPr><w:bidi/><w:jc w:val="start"/><w:ind w:start="720" w:end="1440"/></w:pPr>
                <w:rPr><w:rFonts w:ascii="Arial" w:cs="Noto Sans Arabic"/><w:rtl/></w:rPr>
            </w:style>
        </w:styles>"#;
        let document_xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:pStyle w:val="RtlBody"/></w:pPr><w:r><w:t>styled rtl</w:t></w:r></w:p>
            <w:p><w:pPr><w:pStyle w:val="RtlBody"/><w:bidi w:val="0"/><w:jc w:val="left"/></w:pPr><w:r><w:t>direct ltr</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let blocks = parse_with_styles(document_xml, styles_xml);
        let Block::Paragraph(inherited) = &blocks[0] else {
            panic!("inherited paragraph")
        };
        let Block::Paragraph(overridden) = &blocks[1] else {
            panic!("overridden paragraph")
        };

        assert!(inherited.props.bidi);
        assert_eq!(inherited.props.align, Align::Right);
        assert_eq!(inherited.props.indent.left_pt, Some(72.0));
        assert_eq!(inherited.props.indent.right_pt, Some(36.0));
        assert!(inherited.runs[0].props.rtl);
        assert_eq!(
            inherited.runs[0].props.font.as_deref(),
            Some("Noto Sans Arabic")
        );
        assert!(!overridden.props.bidi);
        assert_eq!(overridden.props.align, Align::Left);
    }

    #[test]
    fn run_props_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p>
            <w:r><w:rPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:b/>
                        <w:i/>
                        <w:u w:val="single"/>
                        <w:strike/>
                        <w:smallCaps/>
                        <w:caps/>
                        <w:rFonts w:ascii="Choice Latin" w:eastAsia="Choice Korean"/>
                        <w:sz w:val="28"/>
                        <w:color w:val="112233"/>
                        <w:highlight w:val="yellow"/>
                        <w:vertAlign w:val="superscript"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:b w:val="false"/>
                        <w:i w:val="false"/>
                        <w:u w:val="none"/>
                        <w:strike w:val="false"/>
                        <w:smallCaps w:val="false"/>
                        <w:caps w:val="false"/>
                        <w:rFonts w:ascii="Fallback Latin" w:eastAsia="Fallback Korean"/>
                        <w:sz w:val="20"/>
                        <w:color w:val="445566"/>
                        <w:highlight w:val="green"/>
                        <w:vertAlign w:val="subscript"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:rPr><w:t>Run properties</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("paragraph");
        };
        let props = &p.runs[0].props;

        assert!(props.bold);
        assert!(props.italic);
        assert!(props.underline);
        assert!(props.strike);
        assert!(props.small_caps);
        assert!(props.caps);
        assert_eq!(props.font.as_deref(), Some("Choice Korean"));
        assert_eq!(props.size_half_pt, Some(28));
        assert_eq!(
            props.color,
            Some(Color {
                r: 0x11,
                g: 0x22,
                b: 0x33
            })
        );
        assert_eq!(props.highlight.as_deref(), Some("yellow"));
        assert_eq!(props.vert_align, VertAlign::Super);
    }

    #[test]
    fn preserves_significant_whitespace_in_t() {
        let xml = r#"<w:document><w:body><w:p>
            <w:r><w:t xml:space="preserve">a </w:t></w:r><w:r><w:t>b</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("para")
        };
        assert_eq!(p.text(), "a b");
    }

    #[test]
    fn page_break_type_trims_ooxml_value() {
        let xml = r#"<w:document><w:body><w:p>
            <w:r><w:t>before</w:t><w:br w:type=" page "/><w:t>after</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let blocks = parse(xml);
        assert!(matches!(blocks.get(1), Some(Block::PageBreak)));
        let Block::Paragraph(after) = &blocks[2] else {
            panic!("paragraph after break");
        };
        assert_eq!(after.text(), "after");
    }

    #[test]
    fn paragraph_spacing_line_rule_trims_ooxml_value() {
        let xml = r#"<w:document><w:body><w:p>
            <w:pPr><w:spacing w:line="360" w:lineRule=" exact "/></w:pPr>
            <w:r><w:t>exact line spacing</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("paragraph");
        };

        assert_eq!(p.props.spacing.line_pct, None);
    }

    #[test]
    fn paragraph_props_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p>
            <w:pPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:jc w:val="center"/>
                        <w:spacing w:before="240" w:after="120" w:line="360"/>
                        <w:ind w:left="720" w:firstLine="240"/>
                        <w:shd w:fill="EEEEEE"/>
                        <w:pageBreakBefore w:val="true"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:jc w:val="right"/>
                        <w:spacing w:before="480" w:after="360" w:line="480"/>
                        <w:ind w:left="1440" w:firstLine="480"/>
                        <w:shd w:fill="111111"/>
                        <w:pageBreakBefore w:val="false"/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:pPr>
            <w:r><w:t>Paragraph properties</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("paragraph");
        };

        assert_eq!(p.props.align, Align::Center);
        assert_eq!(p.props.spacing.before_pt, Some(12.0));
        assert_eq!(p.props.spacing.after_pt, Some(6.0));
        assert_eq!(p.props.spacing.line_pct, Some(1.5));
        assert_eq!(p.props.indent.left_pt, Some(36.0));
        assert_eq!(p.props.indent.first_line_pt, Some(12.0));
        assert_eq!(
            p.props.shading,
            Some(Color {
                r: 0xEE,
                g: 0xEE,
                b: 0xEE
            })
        );
        assert!(p.props.page_break_before);
    }

    #[test]
    fn table_gridspan_and_vmerge() {
        // 2x2 grid: row 0 col 0 spans 2 columns (gridSpan) and starts a vertical
        // merge; row 1 col 0 continues it (dropped, owner row_span=2).
        let xml = r#"<w:document><w:body><w:tbl>
            <w:tr>
              <w:tc><w:tcPr><w:gridSpan w:val=" 2 "/><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
            </w:tr>
            <w:tr>
              <w:tc><w:tcPr><w:gridSpan w:val=" 2 "/><w:vMerge/></w:tcPr><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
            </w:tr>
        </w:tbl></w:body></w:document>"#;
        let Block::Table(t) = &parse(xml)[0] else {
            panic!("table")
        };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0].cells.len(), 1);
        assert_eq!(t.rows[0].cells[0].col_span, 2);
        assert_eq!(t.rows[0].cells[0].row_span, 2);
        assert_eq!(t.rows[1].cells.len(), 0); // continuation dropped
    }

    #[test]
    fn table_grid_widths_populate_model_proportions_in_logical_order() {
        let xml = r#"<w:document><w:body><w:tbl>
            <w:tblPr><w:bidiVisual/></w:tblPr>
            <w:tblGrid>
                <w:gridCol w:w=" 1200 "/>
                <w:gridCol w:w="2400"/>
                <w:gridCol w:w="3600"/>
            </w:tblGrid>
            <w:tr>
                <w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
            </w:tr>
        </w:tbl></w:body></w:document>"#;
        let Block::Table(table) = &parse(xml)[0] else {
            panic!("table")
        };

        assert!(table.bidi_visual);
        assert_eq!(table.rows[0].cells[0].col_span, 2);
        assert_eq!(table.col_widths_pct.len(), 3);
        for (actual, expected) in table
            .col_widths_pct
            .iter()
            .zip([1.0 / 6.0, 1.0 / 3.0, 1.0 / 2.0])
        {
            assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn table_grid_ignores_revision_history_and_rejects_incomplete_widths() {
        let xml = r#"<w:document><w:body>
            <w:tbl>
                <w:tblGrid>
                    <w:gridCol w:w="1000"/>
                    <w:gridCol w:w="3000"/>
                    <w:tblGridChange w:id="7">
                        <w:tblGrid>
                            <w:gridCol w:w="9000"/>
                            <w:gridCol w:w="1000"/>
                        </w:tblGrid>
                    </w:tblGridChange>
                </w:tblGrid>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblGrid><w:gridCol w:w="1000"/><w:gridCol/></w:tblGrid>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblGrid><w:gridCol w:w="0"/><w:gridCol w:w="1000"/></w:tblGrid>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="invalid"/></w:tblGrid>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let tables = blocks
            .iter()
            .map(|block| match block {
                Block::Table(table) => table,
                _ => panic!("table"),
            })
            .collect::<Vec<_>>();

        assert_eq!(tables.len(), 5);
        assert_eq!(tables[0].col_widths_pct, vec![0.25, 0.75]);
        for table in &tables[1..] {
            assert!(table.col_widths_pct.is_empty());
        }
    }

    #[test]
    fn excessive_table_grid_widths_fall_back_without_panicking() {
        let mut xml = String::from("<w:document><w:body><w:tbl><w:tblGrid>");
        for _ in 0..=MAX_TABLE_GRID_COLS {
            xml.push_str(r#"<w:gridCol w:w="1"/>"#);
        }
        xml.push_str("</w:tblGrid><w:tr>");
        for _ in 0..=MAX_TABLE_GRID_COLS {
            xml.push_str("<w:tc><w:p/></w:tc>");
        }
        xml.push_str("</w:tr></w:tbl></w:body></w:document>");

        let Block::Table(table) = &parse(&xml)[0] else {
            panic!("table")
        };
        assert_eq!(table.rows[0].cells.len(), MAX_TABLE_GRID_COLS + 1);
        assert!(table.col_widths_pct.is_empty());
    }

    #[test]
    fn table_vmerge_restart_trims_ooxml_value() {
        let xml = r#"<w:document><w:body><w:tbl>
            <w:tr>
              <w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
            </w:tr>
            <w:tr>
              <w:tc><w:tcPr><w:vMerge w:val=" restart "/></w:tcPr><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
            </w:tr>
            <w:tr>
              <w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc>
            </w:tr>
        </w:tbl></w:body></w:document>"#;
        let Block::Table(t) = &parse(xml)[0] else {
            panic!("table")
        };

        assert_eq!(t.rows[0].cells[0].row_span, 1);
        assert_eq!(t.rows[1].cells[0].text(), "B");
        assert_eq!(t.rows[1].cells[0].row_span, 2);
        assert_eq!(t.rows[2].cells.len(), 0);
    }

    #[test]
    fn vertical_merge_row_span_saturates_instead_of_overflowing() {
        let mut rows = Vec::with_capacity(u16::MAX as usize + 1);
        rows.push(RowRaw {
            cells: vec![raw_merge_cell(VMerge::Restart)],
            props: RowProps::default(),
        });
        rows.extend((0..u16::MAX as usize).map(|_| RowRaw {
            cells: vec![raw_merge_cell(VMerge::Continue)],
            props: RowProps::default(),
        }));

        let row_regions = vec![TableRowStyleRegions::default(); rows.len()];
        let table = build_table(
            rows,
            TableProps::default(),
            Vec::new(),
            TableStyleCellProps::default(),
            row_regions,
            TableLook::word_default(),
            0,
        )
        .0;

        assert_eq!(table.rows[0].cells[0].row_span, u16::MAX);
    }

    #[test]
    fn block_level_sdt_content_is_not_lost() {
        // A content control (w:sdt) wrapping body paragraphs is a transparent
        // block container — its paragraphs must survive, not be skipped.
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>before</w:t></w:r></w:p>
            <w:sdt><w:sdtPr></w:sdtPr><w:sdtContent>
                <w:p><w:r><w:t>inside_sdt</w:t></w:r></w:p>
                <w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
            </w:sdtContent></w:sdt>
            <w:p><w:r><w:t>after</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let joined = blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(p) => p.text(),
                Block::Table(t) => t.rows[0].cells[0]
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Paragraph(p) => Some(p.text()),
                        _ => None,
                    })
                    .collect(),
                Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {
                    String::new()
                }
            })
            .collect::<Vec<_>>()
            .join("|");
        assert_eq!(joined, "before|inside_sdt|cell|after");
    }

    #[test]
    fn content_control_binding_metadata_trims_ooxml_values() {
        let xml = r#"<w:document><w:body>
            <w:sdt><w:sdtPr>
                <w:alias w:val=" Bound alias "/>
                <w:tag w:val=" bound-tag "/>
                <w:dataBinding w:xpath=" /root/client " w:storeItemID=" {11111111-2222-3333-4444-555555555555} "/>
            </w:sdtPr><w:sdtContent>
                <w:p><w:r><w:t>Bound value</w:t></w:r></w:p>
            </w:sdtContent></w:sdt>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(paragraph) = &blocks[0] else {
            panic!("paragraph")
        };
        let control = paragraph.runs[0]
            .content_control
            .as_ref()
            .expect("content control metadata");
        assert_eq!(control.alias.as_deref(), Some("Bound alias"));
        assert_eq!(control.tag.as_deref(), Some("bound-tag"));
        assert_eq!(control.data_binding_xpath.as_deref(), Some("/root/client"));
        assert_eq!(
            control.data_binding_store_item_id.as_deref(),
            Some("{11111111-2222-3333-4444-555555555555}")
        );
    }

    #[test]
    fn content_control_metadata_uses_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sdt><w:sdtPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:alias w:val=" Choice alias "/>
                        <w:tag w:val=" choice-tag "/>
                        <w:dataBinding w:xpath=" /root/choice " w:storeItemID=" {11111111-2222-3333-4444-555555555555} "/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:alias w:val=" Fallback alias "/>
                        <w:tag w:val=" fallback-tag "/>
                        <w:dataBinding w:xpath=" /root/fallback " w:storeItemID=" {66666666-7777-8888-9999-AAAAAAAAAAAA} "/>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sdtPr><w:sdtContent>
                <w:p><w:r><w:t>Controlled value</w:t></w:r></w:p>
            </w:sdtContent></w:sdt>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(paragraph) = &blocks[0] else {
            panic!("paragraph")
        };
        let control = paragraph.runs[0]
            .content_control
            .as_ref()
            .expect("content control metadata");

        assert_eq!(control.alias.as_deref(), Some("Choice alias"));
        assert_eq!(control.tag.as_deref(), Some("choice-tag"));
        assert_eq!(control.data_binding_xpath.as_deref(), Some("/root/choice"));
        assert_eq!(
            control.data_binding_store_item_id.as_deref(),
            Some("{11111111-2222-3333-4444-555555555555}")
        );
    }

    #[test]
    fn block_content_control_uses_single_alternate_content_child_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:sdt>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:sdtPr>
                            <w:alias w:val=" Choice block "/>
                            <w:tag w:val="choice-block"/>
                        </w:sdtPr>
                        <w:sdtContent>
                            <w:p><w:r><w:t>Choice block</w:t></w:r></w:p>
                        </w:sdtContent>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:sdtPr>
                            <w:alias w:val="Fallback block"/>
                            <w:tag w:val="fallback-block"/>
                        </w:sdtPr>
                        <w:sdtContent>
                            <w:p><w:r><w:t>Fallback block</w:t></w:r></w:p>
                        </w:sdtContent>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sdt>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(paragraph) = &blocks[0] else {
            panic!("paragraph")
        };
        assert_eq!(paragraph.text(), "Choice block");
        let control = paragraph.runs[0]
            .content_control
            .as_ref()
            .expect("content control metadata");

        assert_eq!(control.alias.as_deref(), Some("Choice block"));
        assert_eq!(control.tag.as_deref(), Some("choice-block"));
    }

    #[test]
    fn run_content_control_uses_single_alternate_content_child_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:p><w:sdt>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:sdtPr>
                            <w:alias w:val=" Choice run "/>
                            <w:tag w:val="choice-run"/>
                        </w:sdtPr>
                        <w:sdtContent>
                            <w:r><w:t>Choice run</w:t></w:r>
                        </w:sdtContent>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:sdtPr>
                            <w:alias w:val="Fallback run"/>
                            <w:tag w:val="fallback-run"/>
                        </w:sdtPr>
                        <w:sdtContent>
                            <w:r><w:t>Fallback run</w:t></w:r>
                        </w:sdtContent>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:sdt></w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(paragraph) = &blocks[0] else {
            panic!("paragraph")
        };
        assert_eq!(paragraph.text(), "Choice run");
        let control = paragraph.runs[0]
            .content_control
            .as_ref()
            .expect("content control metadata");

        assert_eq!(control.alias.as_deref(), Some("Choice run"));
        assert_eq!(control.tag.as_deref(), Some("choice-run"));
    }

    #[test]
    fn content_control_blank_metadata_is_ignored() {
        let xml = r#"<w:document><w:body>
            <w:sdt><w:sdtPr>
                <w:alias w:val=" "/>
                <w:tag w:val=" "/>
                <w:dataBinding w:xpath=" " w:storeItemID=" "/>
            </w:sdtPr><w:sdtContent>
                <w:p><w:r><w:t>Plain value</w:t></w:r></w:p>
            </w:sdtContent></w:sdt>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Paragraph(paragraph) = &blocks[0] else {
            panic!("paragraph")
        };

        assert!(paragraph.runs[0].content_control.is_none());
    }

    #[test]
    fn deeply_nested_tables_do_not_overflow_the_stack() {
        // Thousands of nested table cells (cf. POI deep-table-cell.docx) must be
        // bounded by MAX_DEPTH and skipped iteratively, not recursed to a crash.
        let depth = 4000;
        let mut xml = String::from("<w:document><w:body>");
        for _ in 0..depth {
            xml.push_str("<w:tbl><w:tr><w:tc>");
        }
        xml.push_str("<w:p><w:r><w:t>deep</w:t></w:r></w:p>");
        for _ in 0..depth {
            xml.push_str("</w:tc></w:tr></w:tbl>");
        }
        xml.push_str("</w:body></w:document>");
        let blocks = parse(&xml); // returns instead of overflowing
        assert!(!blocks.is_empty());
    }

    #[test]
    fn deeply_nested_textboxes_do_not_overflow_the_stack() {
        // The drawing → txbxContent → paragraph → run → drawing … cycle must be
        // bounded by the same MAX_DEPTH budget (threaded across the drawing
        // boundary), not recursed to a stack overflow on hostile input.
        let depth = 4000;
        let mut xml = String::from("<w:document><w:body>");
        for _ in 0..depth {
            xml.push_str("<w:p><w:r><w:drawing><w:txbxContent>");
        }
        xml.push_str("<w:p><w:r><w:t>deep</w:t></w:r></w:p>");
        for _ in 0..depth {
            xml.push_str("</w:txbxContent></w:drawing></w:r></w:p>");
        }
        xml.push_str("</w:body></w:document>");
        let _ = parse(&xml); // must return, not abort
    }

    #[test]
    fn skips_field_and_deletion_but_keeps_body() {
        // w:del (tracked deletion) content must not appear; w:ins must.
        let xml = r#"<w:document><w:body><w:p>
            <w:del><w:r><w:delText>gone</w:delText></w:r></w:del>
            <w:ins><w:r><w:t>kept</w:t></w:r></w:ins>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("para")
        };
        assert_eq!(p.text(), "kept");
    }

    #[test]
    fn paragraph_property_change_keeps_visible_current_text() {
        let xml = r#"<w:document><w:body><w:p>
            <w:pPr>
                <w:pPrChange><w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange>
            </w:pPr>
            <w:r><w:t>Property change</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("para")
        };
        assert_eq!(p.text(), "Property change");
    }

    #[test]
    fn paragraph_layout_ignores_nested_property_containers() {
        let xml = r#"<w:document><w:body><w:p>
            <w:pPr>
                <w:spacing w:before="120" w:after="240"/>
                <w:ind w:firstLine="200"/>
                <w:shd w:val="clear" w:fill="112233"/>
                <w:pageBreakBefore w:val="0"/>
                <w:rPr>
                    <w:spacing w:before="480"/><w:ind w:hanging="500"/>
                    <w:shd w:val="clear" w:fill="AABBCC"/><w:pageBreakBefore/>
                </w:rPr>
                <w:tcPr>
                    <w:spacing w:after="600"/><w:ind w:hanging="700"/>
                    <w:shd w:val="clear" w:fill="BBCCDD"/><w:pageBreakBefore/>
                </w:tcPr>
                <w:numPr>
                    <w:ilvl w:val="0"/><w:numId w:val="0"/>
                    <w:spacing w:before="720"/><w:ind w:hanging="800"/>
                    <w:shd w:val="clear" w:fill="CCDDEE"/><w:pageBreakBefore/>
                </w:numPr>
            </w:pPr>
            <w:r><w:t>Current layout</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let Block::Paragraph(paragraph) = &parse(xml)[0] else {
            panic!("paragraph")
        };

        assert_eq!(paragraph.props.spacing.before_pt, Some(6.0));
        assert_eq!(paragraph.props.spacing.after_pt, Some(12.0));
        assert_eq!(paragraph.props.indent.first_line_pt, Some(10.0));
        assert_eq!(paragraph.props.indent.hanging_pt, None);
        assert_eq!(paragraph.props.shading, Some(Color::rgb(0x11, 0x22, 0x33)));
        assert!(!paragraph.props.page_break_before);
    }

    #[test]
    fn deeply_nested_paragraph_alternate_content_is_bounded() {
        let mut xml = String::from(
            r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
                <w:p><w:pPr><w:spacing w:before="120" w:after="240"/>"#,
        );
        for _ in 0..(MAX_DEPTH + 8) {
            xml.push_str("<mc:AlternateContent><mc:Choice Requires=\"w14\">");
        }
        xml.push_str("<w:spacing w:before=\"480\"/>");
        for _ in 0..(MAX_DEPTH + 8) {
            xml.push_str(
                "</mc:Choice><mc:Fallback><w:spacing w:before=\"600\"/></mc:Fallback></mc:AlternateContent>",
            );
        }
        xml.push_str(
            r#"<w:ind w:firstLine="200"/></w:pPr>
                <w:r><w:t>Bounded layout</w:t></w:r>
            </w:p></w:body></w:document>"#,
        );

        let Block::Paragraph(paragraph) = &parse(&xml)[0] else {
            panic!("paragraph")
        };
        assert_eq!(paragraph.props.spacing.before_pt, Some(6.0));
        assert_eq!(paragraph.props.spacing.after_pt, Some(12.0));
        assert_eq!(paragraph.props.indent.first_line_pt, Some(10.0));
    }

    #[test]
    fn paragraph_numbering_and_tabs_use_one_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:p><w:pPr>
                <w:numPr><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:ilvl w:val="2"/><w:numId w:val="7"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:ilvl w:val="4"/><w:numId w:val="9"/>
                    </mc:Fallback>
                </mc:AlternateContent></w:numPr>
                <w:tabs><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:tab w:val="right" w:pos="1440"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:tab w:val="center" w:pos="2880"/>
                    </mc:Fallback>
                </mc:AlternateContent></w:tabs>
            </w:pPr><w:r><w:t>Selected branch</w:t></w:r></w:p>
            <w:p><w:pPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback><w:spacing w:before="240"/></mc:Fallback>
                </mc:AlternateContent>
                <w:numPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback>
                        <w:ilvl w:val="4"/><w:numId w:val="9"/>
                    </mc:Fallback>
                </mc:AlternateContent></w:numPr>
                <w:tabs><mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback>
                        <w:tab w:val="center" w:pos="2880"/>
                    </mc:Fallback>
                </mc:AlternateContent></w:tabs>
            </w:pPr><w:r><w:t>Empty selected branch</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let (blocks, _, tab_stops, _, _, _, _) =
            parse_with_media_styles_and_pagination(xml, HashMap::new(), Styles::default(), true);
        let Block::Paragraph(paragraph) = &blocks[0] else {
            panic!("paragraph")
        };

        assert_eq!(
            paragraph.props.list.as_ref().map(|list| list.level),
            Some(2)
        );
        assert_eq!(
            tab_stops[0],
            vec![TabStop {
                position_pt: 72.0,
                alignment: TabAlignment::Right,
                leader: TabLeader::None,
            }]
        );
        let Block::Paragraph(empty_choice) = &blocks[1] else {
            panic!("second paragraph")
        };
        assert_eq!(empty_choice.props.list, None);
        assert_eq!(empty_choice.props.spacing.before_pt, None);
        assert!(tab_stops[1].is_empty());
    }

    #[test]
    fn section_empty_alternate_content_choice_does_not_apply_fallback() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:p><w:pPr><w:sectPr>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback>
                        <w:cols w:num="5"/>
                        <w:docGrid w:type="lines" w:linePitch="360"/>
                    </mc:Fallback>
                </mc:AlternateContent>
                <w:type w:val="nextPage"/>
            </w:sectPr></w:pPr><w:r><w:t>Empty choice</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let section = parse(xml)
            .into_iter()
            .find_map(|block| match block {
                Block::SectionBreak(section) => Some(section),
                _ => None,
            })
            .expect("section break");

        assert_eq!(section.section_break, Some(SectionBreakKind::NextPage));
        assert_eq!(section.columns, None);
        assert_eq!(section.doc_grid, None);
    }

    #[test]
    fn section_properties_ignore_unknown_nested_containers() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr><w:sectPr>
                <w:unknown>
                    <w:cols w:num="5"/>
                    <w:docGrid w:type="lines" w:linePitch="360"/>
                    <w:titlePg/>
                </w:unknown>
                <w:type w:val="nextPage"/>
                <w:cols w:num="2"/>
            </w:sectPr></w:pPr><w:r><w:t>Scoped section</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let section = parse(xml)
            .into_iter()
            .find_map(|block| match block {
                Block::SectionBreak(section) => Some(section),
                _ => None,
            })
            .expect("section break");

        assert_eq!(section.section_break, Some(SectionBreakKind::NextPage));
        assert_eq!(section.columns, Some(2));
        assert_eq!(section.doc_grid, None);
        assert!(!section.title_page);
    }

    #[test]
    fn deeply_nested_section_alternate_content_is_bounded_and_recovers() {
        let mut xml = String::from(
            r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
                <w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/>"#,
        );
        for _ in 0..(MAX_DEPTH + 8) {
            xml.push_str("<mc:AlternateContent><mc:Choice Requires=\"w14\">");
        }
        xml.push_str(r#"<w:docGrid w:type="lines" w:linePitch="360"/>"#);
        for _ in 0..(MAX_DEPTH + 8) {
            xml.push_str(
                "</mc:Choice><mc:Fallback><w:docGrid w:type=\"snapToChars\"/></mc:Fallback></mc:AlternateContent>",
            );
        }
        xml.push_str(
            r#"<w:cols w:num="3"/></w:sectPr></w:pPr>
                <w:r><w:t>Bounded section</w:t></w:r>
            </w:p></w:body></w:document>"#,
        );

        let section = parse(&xml)
            .into_iter()
            .find_map(|block| match block {
                Block::SectionBreak(section) => Some(section),
                _ => None,
            })
            .expect("section break");
        assert_eq!(section.section_break, Some(SectionBreakKind::NextPage));
        assert_eq!(section.doc_grid, None);
        assert_eq!(section.columns, Some(3));
    }

    #[test]
    fn non_finite_twips_do_not_enter_paragraph_or_section_models() {
        let xml = r#"<w:document><w:body>
            <w:p><w:pPr>
                <w:ind w:left="NaN" w:right="inf" w:start="-inf" w:end="1e999"/>
                <w:sectPr>
                    <w:pgSz w:w="NaN" w:h="12240"/>
                    <w:pgMar w:left="inf" w:right="-inf" w:top="1e999" w:bottom="NaN"/>
                </w:sectPr>
            </w:pPr><w:r><w:t>Finite model</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let paragraph = blocks
            .iter()
            .find_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("paragraph");
        let section = blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(section) => Some(section),
                _ => None,
            })
            .expect("section break");

        assert_eq!(paragraph.props.indent.left_pt, None);
        assert_eq!(paragraph.props.indent.right_pt, None);
        assert!(section.page.width_pt.is_finite());
        assert!(section.page.height_pt.is_finite());
        assert_eq!(section.page.margin_left_pt, None);
        assert_eq!(section.page.margin_right_pt, None);
        assert_eq!(section.page.margin_top_pt, None);
        assert_eq!(section.page.margin_bottom_pt, None);
    }

    #[test]
    fn run_property_change_keeps_visible_current_text() {
        let xml = r#"<w:document><w:body><w:p><w:r>
            <w:rPr>
                <w:rPrChange><w:rPr><w:b/></w:rPr></w:rPrChange>
            </w:rPr>
            <w:t>Run property change</w:t>
        </w:r></w:p></w:body></w:document>"#;
        let Block::Paragraph(p) = &parse(xml)[0] else {
            panic!("para")
        };
        assert_eq!(p.text(), "Run property change");
    }

    #[test]
    fn table_property_changes_keep_visible_current_cells() {
        let xml = r#"<w:document><w:body><w:tbl>
            <w:tblPr>
                <w:tblPrChange><w:tblPr><w:tblW w:w="0"/></w:tblPr></w:tblPrChange>
            </w:tblPr>
            <w:tr>
                <w:trPr>
                    <w:trPrChange><w:trPr><w:tblHeader/></w:trPr></w:trPrChange>
                </w:trPr>
                <w:tc>
                    <w:tcPr>
                        <w:tcPrChange><w:tcPr><w:vAlign w:val="center"/></w:tcPr></w:tcPrChange>
                    </w:tcPr>
                    <w:p><w:r><w:t>Cell property change</w:t></w:r></w:p>
                </w:tc>
            </w:tr>
        </w:tbl></w:body></w:document>"#;
        let Block::Table(table) = &parse(xml)[0] else {
            panic!("table")
        };
        let Block::Paragraph(p) = &table.rows[0].cells[0].blocks[0] else {
            panic!("cell paragraph")
        };
        assert_eq!(p.text(), "Cell property change");
    }

    #[test]
    fn table_props_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14">
                            <w:tblW w:type="pct" w:w="4000"/>
                            <w:tblLayout w:type="fixed"/>
                            <w:jc w:val="center"/>
                        </mc:Choice>
                        <mc:Fallback>
                            <w:tblW w:type="pct" w:w="7000"/>
                            <w:tblLayout w:type="autofit"/>
                            <w:jc w:val="right"/>
                        </mc:Fallback>
                    </mc:AlternateContent>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Table properties</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let Block::Table(table) = &parse(xml)[0] else {
            panic!("table")
        };

        assert_eq!(table.width_pct, Some(0.8));
        assert!(table.fixed_layout);
        assert_eq!(table.align, Some(Align::Center));
    }

    #[test]
    fn table_logical_alignment_accepts_start_and_end_in_selected_paths() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:jc w:val="start"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Direct start</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:jc w:val="end"/></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Direct end</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14"><w:jc w:val="end"/></mc:Choice>
                        <mc:Fallback><w:jc w:val="start"/></mc:Fallback>
                    </mc:AlternateContent>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Selected end</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let alignments = blocks
            .iter()
            .map(|block| match block {
                Block::Table(table) => table.align,
                _ => panic!("table"),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            alignments,
            vec![Some(Align::Left), Some(Align::Right), Some(Align::Right)]
        );
    }

    #[test]
    fn table_border_props_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr>
                    <w:tblBorders>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w14">
                                <w:top w:val="double" w:sz="8" w:color="112233"/>
                                <w:left w:val="double" w:sz="8" w:color="112233"/>
                                <w:bottom w:val="double" w:sz="8" w:color="112233"/>
                                <w:right w:val="double" w:sz="8" w:color="112233"/>
                                <w:insideH w:val="double" w:sz="8" w:color="112233"/>
                                <w:insideV w:val="double" w:sz="8" w:color="112233"/>
                            </mc:Choice>
                            <mc:Fallback>
                                <w:top w:val="dotted" w:sz="12" w:color="445566"/>
                                <w:left w:val="dotted" w:sz="12" w:color="445566"/>
                                <w:bottom w:val="dotted" w:sz="12" w:color="445566"/>
                                <w:right w:val="dotted" w:sz="12" w:color="445566"/>
                                <w:insideH w:val="dotted" w:sz="12" w:color="445566"/>
                                <w:insideV w:val="dotted" w:sz="12" w:color="445566"/>
                            </mc:Fallback>
                        </mc:AlternateContent>
                    </w:tblBorders>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>Table borders</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let Block::Table(table) = &parse(xml)[0] else {
            panic!("table")
        };

        let choice_color = Color {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        };
        assert_eq!(table.border_color, Some(choice_color));
        assert_eq!(table.border_colors.top, Some(choice_color));
        assert_eq!(table.border_size_eighths, Some(8));
        assert_eq!(table.border_sizes.top, Some(8));
        assert_eq!(table.border_style, Some(TableBorderStyle::Double));
        assert_eq!(table.border_styles.top, Some(TableBorderStyle::Double));
    }

    #[test]
    fn table_cell_margins_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tr>
                    <w:tc>
                        <w:tcPr>
                            <w:tcMar>
                                <mc:AlternateContent>
                                    <mc:Choice Requires="w14">
                                        <w:top w:w="120" w:type="dxa"/>
                                        <w:right w:w="240" w:type="dxa"/>
                                        <w:bottom w:w="360" w:type="dxa"/>
                                        <w:left w:w="480" w:type="dxa"/>
                                    </mc:Choice>
                                    <mc:Fallback>
                                        <w:top w:w="720" w:type="dxa"/>
                                        <w:right w:w="840" w:type="dxa"/>
                                        <w:bottom w:w="960" w:type="dxa"/>
                                        <w:left w:w="1080" w:type="dxa"/>
                                    </mc:Fallback>
                                </mc:AlternateContent>
                            </w:tcMar>
                        </w:tcPr>
                        <w:p><w:r><w:t>Margin cell</w:t></w:r></w:p>
                    </w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let Block::Table(table) = &parse(xml)[0] else {
            panic!("table")
        };

        assert_eq!(
            table.rows[0].cells[0].margins,
            Some(CellMargins {
                top: 120,
                right: 240,
                bottom: 360,
                left: 480,
            })
        );
    }

    #[test]
    fn table_row_props_use_single_alternate_content_branch() {
        let xml = r#"<w:document xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tr>
                    <w:trPr>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w14"><w:cantSplit/></mc:Choice>
                            <mc:Fallback><w:tblHeader/></mc:Fallback>
                        </mc:AlternateContent>
                    </w:trPr>
                    <w:tc><w:p><w:r><w:t>Body row</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
            <w:tbl>
                <w:tr>
                    <w:trPr>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w14"><w:tblHeader/></mc:Choice>
                            <mc:Fallback><w:cantSplit/></mc:Fallback>
                        </mc:AlternateContent>
                    </w:trPr>
                    <w:tc><w:p><w:r><w:t>Header row</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#;
        let blocks = parse(xml);
        let Block::Table(body_table) = &blocks[0] else {
            panic!("first table")
        };
        let Block::Table(header_table) = &blocks[1] else {
            panic!("second table")
        };

        assert_eq!(body_table.header_rows, 0);
        assert!(!body_table.rows[0].cells[0].is_header);
        assert_eq!(header_table.header_rows, 1);
        assert!(header_table.rows[0].cells[0].is_header);
    }
}
