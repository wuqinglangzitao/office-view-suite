//! Second pass: turn the piece table + character/paragraph properties + list
//! tables into the rich [`DocModel`].
//!
//! This never runs for the fast [`crate::Document::text`] path; it is built only
//! when a caller asks for the model or an exporter. It decodes the pieces a
//! second time into a CP-aligned `(units, fcs, prms)` stream (each UTF-16 code
//! unit tagged with its source `WordDocument` byte offset and PCD modifier) so
//! character properties can be attached per run.

use std::collections::HashMap;

use encoding_rs::Encoding;

use crate::chpx::{highlight_name, Chp, ChpxTable, PcdPrm1Patch};
use crate::clx::Piece;
use crate::fib::{self, Fib};
use crate::list::Numberer;
use crate::model::{
    normalize_field_instruction, Align, Block, CharProps, DocGrid, DocGridType, DocMeta, DocModel,
    DocSetup, FieldRole, Image, Indent, LineSpacingHint, ListInfo, PageNumberFormat, PageSetup,
    PaginationHint, ParaProps, Paragraph, RunningSurfaceDistanceHints, SectionBreakKind,
    SectionColumnHint, SectionColumnLayoutHints, SectionSetup, SourceRegion, SourceRegionKind,
    Spacing, Stats, TableCellLineSpacingHints, TableCellPaginationHints, TableRowPaginationHint,
    TextDirection,
};
#[cfg(feature = "docx")]
use crate::model::{
    RunningBlockPaginationHints, RunningSurfaceColumnBreakHints, RunningSurfacePaginationHints,
    TableCellColumnBreakHints,
};
#[cfg(any(feature = "docx", feature = "render"))]
use crate::model::{
    RunningSurfaceLineSpacingHints, RunningSurfaceTabStopHints,
    RunningSurfaceTableCellTabStopHints, TabStop, TableCellTabStopHints,
};
use crate::papx::{
    PapxTable, ParagraphIndentOverrides, ParagraphJustification, ParagraphLineSpacing,
    ParagraphSpacingOverrides,
};
use crate::stsh::StyleSheet;
use crate::table::{self, CellBuild, RowBuild};
use crate::util::{u16le, u32le};

/// Immutable source structures threaded through legacy model assembly: the decoded
/// `(units, fcs, prms)` stream plus the property/style/font tables. Bundled so the region
/// builders take one borrow instead of a long parallel argument list. The
/// `Numberer` is passed alongside because it is mutated per paragraph.
struct LegacySource<'a> {
    units: &'a [u16],
    fcs: &'a [u32],
    prms: &'a [u16],
    prm1_patches: &'a [Option<PcdPrm1Patch>],
    papx: &'a PapxTable,
    chpx: &'a ChpxTable,
    stylesheet: &'a StyleSheet,
    data: &'a [u8],
    fonts: &'a [String],
}

/// Positional descriptor for one region emitted by [`push_legacy_region`]: which CP
/// span of the source stream to assemble and how to tag/keep the resulting blocks.
struct RegionSpec {
    kind: SourceRegionKind,
    source_start_cp: usize,
    source_len_cp: usize,
    source_story_index: Option<usize>,
    include_empty: bool,
    suppress_terminal_page_break: bool,
}

/// Parsed structures needed to build the model, passed to [`build_model`].
pub(crate) struct BuildInputs<'a> {
    pub word: &'a [u8],
    pub table: &'a [u8],
    pub pieces: &'a [Piece],
    pub enc: &'static Encoding,
    pub papx: &'a PapxTable,
    pub chpx: &'a ChpxTable,
    pub prm1_patches: &'a [Option<PcdPrm1Patch>],
    pub stylesheet: &'a StyleSheet,
    pub data: &'a [u8],
    pub fonts: &'a [String],
    pub fib: &'a Fib,
}

pub(crate) struct LegacyBuildOutput {
    pub(crate) model: DocModel,
    #[cfg(feature = "docx")]
    pub(crate) promoted_running_block_ranges: Vec<(usize, usize)>,
    pub(crate) pagination_hints: Vec<PaginationHint>,
    pub(crate) line_spacing_hints: Vec<Option<LineSpacingHint>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) tab_stops: Vec<Vec<TabStop>>,
    #[cfg(feature = "render")]
    pub(crate) render_tab_stops: Vec<Vec<TabStop>>,
    #[cfg(feature = "docx")]
    pub(crate) note_reference_anchors: Vec<Vec<LegacyNoteReferenceAnchor>>,
    pub(crate) column_break_offsets: Vec<Vec<usize>>,
    #[cfg(feature = "docx")]
    pub(crate) table_cell_column_break_offsets: Vec<TableCellColumnBreakHints>,
    pub(crate) section_column_gap_pt: Vec<Option<f32>>,
    pub(crate) final_section_column_gap_pt: Option<f32>,
    pub(crate) section_column_layouts: Vec<Option<SectionColumnLayoutHints>>,
    pub(crate) final_section_column_layout: Option<SectionColumnLayoutHints>,
    pub(crate) section_column_separators: Vec<bool>,
    pub(crate) final_section_column_separator: bool,
    pub(crate) section_column_rtl: Vec<bool>,
    pub(crate) final_section_column_rtl: bool,
    pub(crate) table_row_pagination: Vec<Vec<TableRowPaginationHint>>,
    pub(crate) table_cell_pagination: Vec<TableCellPaginationHints>,
    pub(crate) table_cell_line_spacing: Vec<TableCellLineSpacingHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) table_cell_tab_stops: Vec<TableCellTabStopHints>,
    #[cfg(feature = "render")]
    pub(crate) render_table_cell_tab_stops: Vec<TableCellTabStopHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) running_line_spacing_hints: Vec<RunningSurfaceLineSpacingHints>,
    #[cfg(feature = "docx")]
    pub(crate) running_pagination_hints: Vec<RunningSurfacePaginationHints>,
    #[cfg(feature = "docx")]
    pub(crate) running_column_break_offsets: Vec<RunningSurfaceColumnBreakHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) running_tab_stops: Vec<RunningSurfaceTabStopHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    pub(crate) running_table_cell_tab_stops: Vec<RunningSurfaceTableCellTabStopHints>,
    pub(crate) running_surface_distances: Vec<RunningSurfaceDistanceHints>,
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegacyNoteReferenceAnchor {
    pub(crate) source_cp: usize,
    pub(crate) text_offset: usize,
}

pub(crate) fn build_model_with_render_hints(
    inputs: BuildInputs<'_>,
    numberer: &mut Numberer<'_>,
) -> LegacyBuildOutput {
    let BuildInputs {
        word,
        table,
        pieces,
        enc,
        papx,
        chpx,
        prm1_patches,
        stylesheet,
        data,
        fonts,
        fib,
    } = inputs;
    let (units, fcs, prms) = decode_with_fc_and_prm(word, pieces, enc);
    let section_spans = legacy_section_spans(word, table, fib.ccp_text as usize);
    let src = LegacySource {
        units: &units,
        fcs: &fcs,
        prms: &prms,
        prm1_patches,
        papx,
        chpx,
        stylesheet,
        data,
        fonts,
    };
    let LegacyRegionOutput {
        blocks,
        regions,
        mut pagination_hints,
        mut line_spacing_hints,
        #[cfg(any(feature = "docx", feature = "render"))]
        tab_stops,
        #[cfg(feature = "render")]
        note_tab_stops,
        #[cfg(feature = "docx")]
        note_reference_anchors,
        column_break_offsets,
        #[cfg(feature = "docx")]
        table_cell_column_break_offsets,
        mut table_row_pagination,
        mut table_cell_pagination,
        mut table_cell_line_spacing,
        #[cfg(any(feature = "docx", feature = "render"))]
        table_cell_tab_stops,
        #[cfg(feature = "render")]
        note_table_cell_tab_stops,
        #[cfg(any(feature = "docx", feature = "render"))]
        running_tab_regions,
        #[cfg(feature = "docx")]
        running_column_break_regions,
        text_start: _,
    } = build_legacy_region_blocks(&src, numberer, fib, table, &section_spans);
    #[cfg(any(feature = "docx", feature = "render"))]
    let running_line_spacing_hints = legacy_running_line_spacing_from_regions(
        &blocks,
        &regions,
        &line_spacing_hints,
        &table_cell_line_spacing,
    );
    #[cfg(feature = "docx")]
    let running_pagination_hints = legacy_running_pagination_from_regions(
        &blocks,
        &regions,
        &pagination_hints,
        &table_row_pagination,
        &table_cell_pagination,
    );
    #[cfg(feature = "docx")]
    let running_column_break_offsets =
        legacy_running_column_breaks_from_regions(&blocks, &running_column_break_regions);
    clear_legacy_running_layout_hints(
        &blocks,
        &regions,
        &mut pagination_hints,
        &mut line_spacing_hints,
        &mut table_row_pagination,
        &mut table_cell_pagination,
        &mut table_cell_line_spacing,
    );
    #[cfg(any(feature = "docx", feature = "render"))]
    let (running_tab_stops, running_table_cell_tab_stops) =
        legacy_running_tab_stops_from_regions(&blocks, &running_tab_regions);
    #[cfg(feature = "render")]
    let render_tab_stops = overlay_aligned_nonempty(&tab_stops, &note_tab_stops);
    #[cfg(feature = "render")]
    let render_table_cell_tab_stops =
        overlay_aligned_nonempty(&table_cell_tab_stops, &note_table_cell_tab_stops);
    let mut blocks = blocks;
    let stats = compute_stats(&blocks);
    let (setup, promoted_running_block_ranges) =
        legacy_doc_setup_from_regions(&mut blocks, &regions, &section_spans);
    #[cfg(not(feature = "docx"))]
    let _ = promoted_running_block_ranges;
    let (section_column_gap_pt, final_section_column_gap_pt) =
        legacy_section_column_gap_hints(&blocks, &section_spans);
    let (section_column_layouts, final_section_column_layout) =
        legacy_section_column_layout_hints(&blocks, &section_spans);
    let (section_column_separators, final_section_column_separator) =
        legacy_section_column_separator_hints(&blocks, &section_spans);
    let (section_column_rtl, final_section_column_rtl) =
        legacy_section_column_rtl_hints(&blocks, &section_spans);
    let running_surface_distances = legacy_running_surface_distance_hints(&section_spans);
    LegacyBuildOutput {
        model: DocModel {
            blocks,
            regions,
            meta: DocMeta {
                codepage: fib.ansi_codepage(),
                lid: fib.lid,
                stats,
            },
            custom_properties: Default::default(),
            custom_xml_items: Vec::new(),
            setup,
        },
        #[cfg(feature = "docx")]
        promoted_running_block_ranges,
        pagination_hints,
        line_spacing_hints,
        #[cfg(any(feature = "docx", feature = "render"))]
        tab_stops,
        #[cfg(feature = "render")]
        render_tab_stops,
        #[cfg(feature = "docx")]
        note_reference_anchors,
        column_break_offsets,
        #[cfg(feature = "docx")]
        table_cell_column_break_offsets,
        section_column_gap_pt,
        final_section_column_gap_pt,
        section_column_layouts,
        final_section_column_layout,
        section_column_separators,
        final_section_column_separator,
        section_column_rtl,
        final_section_column_rtl,
        table_row_pagination,
        table_cell_pagination,
        table_cell_line_spacing,
        #[cfg(any(feature = "docx", feature = "render"))]
        table_cell_tab_stops,
        #[cfg(feature = "render")]
        render_table_cell_tab_stops,
        #[cfg(any(feature = "docx", feature = "render"))]
        running_line_spacing_hints,
        #[cfg(feature = "docx")]
        running_pagination_hints,
        #[cfg(feature = "docx")]
        running_column_break_offsets,
        #[cfg(any(feature = "docx", feature = "render"))]
        running_tab_stops,
        #[cfg(any(feature = "docx", feature = "render"))]
        running_table_cell_tab_stops,
        running_surface_distances,
    }
}

fn legacy_running_surface_distance_hints(
    section_spans: &[LegacySectionSpan],
) -> Vec<RunningSurfaceDistanceHints> {
    section_spans
        .iter()
        .map(|span| RunningSurfaceDistanceHints {
            header_pt: span.header_distance_pt,
            footer_pt: span.footer_distance_pt,
        })
        .collect()
}

fn legacy_section_column_gap_hints(
    blocks: &[Block],
    section_spans: &[LegacySectionSpan],
) -> (Vec<Option<f32>>, Option<f32>) {
    let mut hints = vec![None; blocks.len()];
    let mut ending_sections = section_spans.iter();
    for (index, block) in blocks.iter().enumerate() {
        if matches!(block, Block::SectionBreak(_)) {
            hints[index] = ending_sections.next().and_then(|span| span.column_gap_pt);
        }
    }
    let final_gap = section_spans.last().and_then(|span| span.column_gap_pt);
    (hints, final_gap)
}

fn legacy_section_column_layout_hints(
    blocks: &[Block],
    section_spans: &[LegacySectionSpan],
) -> (
    Vec<Option<SectionColumnLayoutHints>>,
    Option<SectionColumnLayoutHints>,
) {
    let mut hints = vec![None; blocks.len()];
    let mut ending_sections = section_spans.iter();
    for (index, block) in blocks.iter().enumerate() {
        if matches!(block, Block::SectionBreak(_)) {
            hints[index] = ending_sections
                .next()
                .and_then(|span| span.column_layout.clone());
        }
    }
    let final_layout = section_spans
        .last()
        .and_then(|span| span.column_layout.clone());
    (hints, final_layout)
}

fn legacy_section_column_separator_hints(
    blocks: &[Block],
    section_spans: &[LegacySectionSpan],
) -> (Vec<bool>, bool) {
    let mut hints = vec![false; blocks.len()];
    let mut ending_sections = section_spans.iter();
    for (index, block) in blocks.iter().enumerate() {
        if matches!(block, Block::SectionBreak(_)) {
            hints[index] = ending_sections
                .next()
                .is_some_and(|span| span.column_separator);
        }
    }
    let final_separator = section_spans
        .last()
        .is_some_and(|span| span.column_separator);
    (hints, final_separator)
}

fn legacy_section_column_rtl_hints(
    blocks: &[Block],
    section_spans: &[LegacySectionSpan],
) -> (Vec<bool>, bool) {
    let mut hints = vec![false; blocks.len()];
    let mut ending_sections = section_spans.iter();
    for (index, block) in blocks.iter().enumerate() {
        if matches!(block, Block::SectionBreak(_)) {
            hints[index] = ending_sections.next().is_some_and(|span| span.column_rtl);
        }
    }
    let final_rtl = section_spans.last().is_some_and(|span| span.column_rtl);
    (hints, final_rtl)
}

fn legacy_doc_setup_from_regions(
    blocks: &mut [Block],
    regions: &[SourceRegion],
    section_spans: &[LegacySectionSpan],
) -> (DocSetup, Vec<(usize, usize)>) {
    let section_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count()
        .saturating_add(1);
    if section_count > 1 {
        return legacy_doc_section_setups_from_regions(
            blocks,
            regions,
            section_count,
            section_spans,
        );
    }
    let (mut setup, promoted_ranges) = legacy_doc_flat_setup_from_regions(blocks, regions);
    if let [span] = section_spans {
        setup.page = span.page;
        setup.columns = span.columns;
        setup.title_page = span.title_page;
        setup.page_number_start = span.page_number_start;
        setup.page_number_format = span.page_number_format;
        setup.text_direction = span.text_direction;
        setup.doc_grid = span.doc_grid;
    }
    (setup, promoted_ranges)
}

fn legacy_doc_flat_setup_from_regions(
    blocks: &[Block],
    regions: &[SourceRegion],
) -> (DocSetup, Vec<(usize, usize)>) {
    let mut setup = DocSetup::default();
    let mut promoted_ranges = Vec::new();
    for region in regions.iter().filter(|region| {
        region.kind == SourceRegionKind::HeaderFooter && region.block_start < region.block_end
    }) {
        let start = region.block_start.min(blocks.len());
        let end = region.block_end.min(blocks.len());
        if start < end {
            let slot = legacy_header_footer_setup_slot(&mut setup, region.source_story_index);
            if slot.is_empty() {
                *slot = blocks[start..end].to_vec();
                promoted_ranges.push((start, end));
            }
        }
    }
    (setup, promoted_ranges)
}

fn legacy_doc_section_setups_from_regions(
    blocks: &mut [Block],
    regions: &[SourceRegion],
    section_count: usize,
    section_spans: &[LegacySectionSpan],
) -> (DocSetup, Vec<(usize, usize)>) {
    let mut section_setups = vec![SectionSetup::default(); section_count];
    let mut promoted_ranges = Vec::new();
    for (setup, span) in section_setups.iter_mut().zip(section_spans) {
        setup.page = span.page;
        setup.columns = span.columns;
        setup.title_page = span.title_page;
        setup.page_number_start = span.page_number_start;
        setup.page_number_format = span.page_number_format;
        setup.text_direction = span.text_direction;
        setup.doc_grid = span.doc_grid;
        setup.section_break = Some(span.section_break);
    }
    for region in regions.iter().filter(|region| {
        region.kind == SourceRegionKind::HeaderFooter && region.block_start < region.block_end
    }) {
        let start = region.block_start.min(blocks.len());
        let end = region.block_end.min(blocks.len());
        if start >= end {
            continue;
        }
        if region.source_story_index.is_none() {
            let mut promoted = false;
            for section_setup in &mut section_setups {
                if section_setup.header.is_empty() {
                    section_setup.header = blocks[start..end].to_vec();
                    promoted = true;
                }
            }
            if promoted {
                promoted_ranges.push((start, end));
            }
            continue;
        }
        let Some(section_index) = legacy_header_footer_section_index(region.source_story_index)
        else {
            continue;
        };
        let Some(section_setup) = section_setups.get_mut(section_index) else {
            continue;
        };
        let Some(slot) =
            legacy_header_footer_section_setup_slot(section_setup, region.source_story_index)
        else {
            continue;
        };
        if slot.is_empty() {
            *slot = blocks[start..end].to_vec();
            promoted_ranges.push((start, end));
        }
    }

    let mut section_index = 0usize;
    for block in blocks {
        let Block::SectionBreak(setup) = block else {
            continue;
        };
        let Some(section_setup) = section_setups.get(section_index) else {
            break;
        };
        *setup = section_setup.clone();
        section_index = section_index.saturating_add(1);
    }

    let mut setup = DocSetup::default();
    if let Some(final_section) = section_setups.last() {
        apply_legacy_section_setup_to_doc_setup(final_section, &mut setup);
    }
    (setup, promoted_ranges)
}

fn apply_legacy_section_setup_to_doc_setup(section: &SectionSetup, setup: &mut DocSetup) {
    setup.page = section.page;
    setup.columns = section.columns;
    setup.header = section.header.clone();
    setup.first_header = section.first_header.clone();
    setup.even_header = section.even_header.clone();
    setup.footer = section.footer.clone();
    setup.first_footer = section.first_footer.clone();
    setup.even_footer = section.even_footer.clone();
    setup.title_page = section.title_page;
    setup.page_number_start = section.page_number_start;
    setup.page_number_format = section.page_number_format;
    setup.text_direction = section.text_direction;
    setup.doc_grid = section.doc_grid;
}

fn build_legacy_region_blocks(
    src: &LegacySource<'_>,
    numberer: &mut Numberer<'_>,
    fib: &Fib,
    table: &[u8],
    section_spans: &[LegacySectionSpan],
) -> LegacyRegionOutput {
    let mut output = LegacyRegionOutput::default();
    let mut source_start_cp = 0usize;
    let header_stories = header_footer_story_ranges(fib, table);
    let has_header_footer_setup_stories = header_stories
        .iter()
        .any(|story| story.story_index >= HEADER_FOOTER_STORY_BASE);

    for (kind, source_len_cp) in legacy_region_specs(fib) {
        if kind == SourceRegionKind::Main && section_spans.len() > 1 {
            push_legacy_main_section_regions(
                src,
                numberer,
                &mut output,
                source_start_cp,
                section_spans,
            );
        } else if kind == SourceRegionKind::HeaderFooter && has_header_footer_setup_stories {
            for story in header_stories
                .iter()
                .filter(|story| story.story_index >= HEADER_FOOTER_STORY_BASE)
            {
                push_legacy_region(
                    src,
                    numberer,
                    &mut output,
                    RegionSpec {
                        kind,
                        source_start_cp: source_start_cp.saturating_add(story.start_cp),
                        source_len_cp: story.end_cp.saturating_sub(story.start_cp),
                        source_story_index: Some(story.story_index),
                        include_empty: false,
                        suppress_terminal_page_break: false,
                    },
                );
            }
        } else {
            push_legacy_region(
                src,
                numberer,
                &mut output,
                RegionSpec {
                    kind,
                    source_start_cp,
                    source_len_cp,
                    source_story_index: None,
                    include_empty: kind == SourceRegionKind::Main,
                    suppress_terminal_page_break: false,
                },
            );
        }

        source_start_cp = source_start_cp.saturating_add(source_len_cp);
    }

    output
}

fn push_legacy_main_section_regions(
    src: &LegacySource<'_>,
    numberer: &mut Numberer<'_>,
    output: &mut LegacyRegionOutput,
    source_start_cp: usize,
    section_spans: &[LegacySectionSpan],
) {
    for (index, span) in section_spans.iter().enumerate() {
        push_legacy_region(
            src,
            numberer,
            output,
            RegionSpec {
                kind: SourceRegionKind::Main,
                source_start_cp: source_start_cp.saturating_add(span.start_cp),
                source_len_cp: span.end_cp.saturating_sub(span.start_cp),
                source_story_index: None,
                include_empty: true,
                suppress_terminal_page_break: index + 1 < section_spans.len(),
            },
        );
        if index + 1 < section_spans.len() {
            output
                .blocks
                .push(Block::SectionBreak(legacy_section_break_setup(
                    span.page,
                    span.section_break,
                    span.columns,
                )));
            output.pagination_hints.push(PaginationHint::default());
            output.line_spacing_hints.push(None);
            #[cfg(any(feature = "docx", feature = "render"))]
            output.tab_stops.push(Vec::new());
            #[cfg(feature = "render")]
            output.note_tab_stops.push(Vec::new());
            #[cfg(feature = "docx")]
            output.note_reference_anchors.push(Vec::new());
            output.column_break_offsets.push(Vec::new());
            #[cfg(feature = "docx")]
            output.table_cell_column_break_offsets.push(Vec::new());
            output.table_row_pagination.push(Vec::new());
            output.table_cell_pagination.push(Vec::new());
            output.table_cell_line_spacing.push(Vec::new());
            #[cfg(any(feature = "docx", feature = "render"))]
            output.table_cell_tab_stops.push(Vec::new());
            #[cfg(feature = "render")]
            output.note_table_cell_tab_stops.push(Vec::new());
        }
    }
}

fn legacy_section_break_setup(
    page: PageSetup,
    section_break: SectionBreakKind,
    columns: Option<u16>,
) -> SectionSetup {
    SectionSetup {
        section_break: Some(section_break),
        page,
        columns,
        ..SectionSetup::default()
    }
}

fn push_legacy_region(
    src: &LegacySource<'_>,
    numberer: &mut Numberer<'_>,
    output: &mut LegacyRegionOutput,
    spec: RegionSpec,
) {
    let RegionSpec {
        kind,
        source_start_cp,
        source_len_cp,
        source_story_index,
        include_empty,
        suppress_terminal_page_break,
    } = spec;
    let block_start = output.blocks.len();
    let actual_start = source_start_cp.min(src.units.len()).min(src.fcs.len());
    let actual_end = source_start_cp
        .saturating_add(source_len_cp)
        .min(src.units.len())
        .min(src.fcs.len());
    let mut region_output = if actual_start < actual_end {
        let mut asm = Asm::new(
            src.papx,
            src.chpx,
            src.stylesheet,
            src.data,
            src.fonts,
            numberer,
        );
        asm.prm1_patches = src.prm1_patches;
        asm.suppress_terminal_page_break = suppress_terminal_page_break;
        let prm_start = actual_start.min(src.prms.len());
        let prm_end = actual_end.min(src.prms.len());
        asm.run_with_prms(
            &src.units[actual_start..actual_end],
            &src.fcs[actual_start..actual_end],
            &src.prms[prm_start..prm_end],
            actual_start,
        );
        asm.finish_with_render_hints()
    } else {
        LegacyBlockOutput::default()
    };
    let text_len = compute_stats(&region_output.blocks).text_chars;
    if kind == SourceRegionKind::Main {
        region_output = promote_legacy_manual_page_breaks(region_output);
    }
    #[cfg(feature = "render")]
    let mut note_tab_stops =
        if matches!(kind, SourceRegionKind::Footnote | SourceRegionKind::Endnote) {
            std::mem::take(&mut region_output.tab_stops)
        } else {
            vec![Vec::new(); region_output.blocks.len()]
        };
    #[cfg(feature = "render")]
    let mut note_table_cell_tab_stops =
        if matches!(kind, SourceRegionKind::Footnote | SourceRegionKind::Endnote) {
            std::mem::take(&mut region_output.table_cell_tab_stops)
        } else {
            vec![Vec::new(); region_output.blocks.len()]
        };
    #[cfg(feature = "docx")]
    let mut note_reference_anchors = if kind == SourceRegionKind::Main {
        std::mem::take(&mut region_output.note_reference_anchors)
    } else {
        vec![Vec::new(); region_output.blocks.len()]
    };
    #[cfg(any(feature = "docx", feature = "render"))]
    let running_tab_region = (kind == SourceRegionKind::HeaderFooter
        && !region_output.blocks.is_empty())
    .then(|| LegacyRunningTabRegion {
        source_story_index,
        tab_stops: std::mem::take(&mut region_output.tab_stops),
        table_cell_tab_stops: std::mem::take(&mut region_output.table_cell_tab_stops),
    });
    #[cfg(feature = "docx")]
    let running_column_break_region = (kind == SourceRegionKind::HeaderFooter
        && !region_output.blocks.is_empty())
    .then(|| LegacyRunningColumnBreakRegion {
        source_story_index,
        column_break_offsets: std::mem::take(&mut region_output.column_break_offsets),
    });
    #[cfg(any(feature = "docx", feature = "render"))]
    if kind != SourceRegionKind::Main {
        region_output.tab_stops = vec![Vec::new(); region_output.blocks.len()];
        region_output.table_cell_tab_stops = vec![Vec::new(); region_output.blocks.len()];
    }
    let mut column_break_offsets = if kind == SourceRegionKind::Main {
        std::mem::take(&mut region_output.column_break_offsets)
    } else {
        vec![Vec::new(); region_output.blocks.len()]
    };
    #[cfg(feature = "docx")]
    let mut table_cell_column_break_offsets = if kind == SourceRegionKind::Main {
        std::mem::take(&mut region_output.table_cell_column_break_offsets)
    } else {
        vec![Vec::new(); region_output.blocks.len()]
    };
    output.blocks.append(&mut region_output.blocks);
    output
        .pagination_hints
        .append(&mut region_output.pagination_hints);
    output
        .line_spacing_hints
        .append(&mut region_output.line_spacing_hints);
    #[cfg(any(feature = "docx", feature = "render"))]
    output.tab_stops.append(&mut region_output.tab_stops);
    #[cfg(feature = "render")]
    output.note_tab_stops.append(&mut note_tab_stops);
    #[cfg(feature = "docx")]
    output
        .note_reference_anchors
        .append(&mut note_reference_anchors);
    output
        .column_break_offsets
        .append(&mut column_break_offsets);
    #[cfg(feature = "docx")]
    output
        .table_cell_column_break_offsets
        .append(&mut table_cell_column_break_offsets);
    output
        .table_row_pagination
        .append(&mut region_output.table_row_pagination);
    output
        .table_cell_pagination
        .append(&mut region_output.table_cell_pagination);
    output
        .table_cell_line_spacing
        .append(&mut region_output.table_cell_line_spacing);
    #[cfg(any(feature = "docx", feature = "render"))]
    output
        .table_cell_tab_stops
        .append(&mut region_output.table_cell_tab_stops);
    #[cfg(feature = "render")]
    output
        .note_table_cell_tab_stops
        .append(&mut note_table_cell_tab_stops);
    #[cfg(any(feature = "docx", feature = "render"))]
    if let Some(region) = running_tab_region {
        output.running_tab_regions.push(region);
    }
    #[cfg(feature = "docx")]
    if let Some(region) = running_column_break_region {
        output.running_column_break_regions.push(region);
    }
    let block_end = output.blocks.len();

    if source_len_cp > 0 || include_empty {
        output.regions.push(SourceRegion {
            kind,
            source_story_index,
            block_start,
            block_end,
            source_start_cp,
            source_len_cp,
            text_start: output.text_start,
            text_len,
        });
    }

    output.text_start = output.text_start.saturating_add(text_len);
}

#[derive(Default)]
struct LegacyRegionOutput {
    blocks: Vec<Block>,
    regions: Vec<SourceRegion>,
    pagination_hints: Vec<PaginationHint>,
    line_spacing_hints: Vec<Option<LineSpacingHint>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: Vec<Vec<TabStop>>,
    #[cfg(feature = "render")]
    note_tab_stops: Vec<Vec<TabStop>>,
    #[cfg(feature = "docx")]
    note_reference_anchors: Vec<Vec<LegacyNoteReferenceAnchor>>,
    column_break_offsets: Vec<Vec<usize>>,
    #[cfg(feature = "docx")]
    table_cell_column_break_offsets: Vec<TableCellColumnBreakHints>,
    table_row_pagination: Vec<Vec<TableRowPaginationHint>>,
    table_cell_pagination: Vec<TableCellPaginationHints>,
    table_cell_line_spacing: Vec<TableCellLineSpacingHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    table_cell_tab_stops: Vec<TableCellTabStopHints>,
    #[cfg(feature = "render")]
    note_table_cell_tab_stops: Vec<TableCellTabStopHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    running_tab_regions: Vec<LegacyRunningTabRegion>,
    #[cfg(feature = "docx")]
    running_column_break_regions: Vec<LegacyRunningColumnBreakRegion>,
    text_start: usize,
}

#[cfg(any(feature = "docx", feature = "render"))]
struct LegacyRunningTabRegion {
    source_story_index: Option<usize>,
    tab_stops: Vec<Vec<TabStop>>,
    table_cell_tab_stops: Vec<TableCellTabStopHints>,
}

#[cfg(feature = "docx")]
struct LegacyRunningColumnBreakRegion {
    source_story_index: Option<usize>,
    column_break_offsets: Vec<Vec<usize>>,
}

#[cfg(feature = "render")]
fn overlay_aligned_nonempty<T: Clone>(base: &[Vec<T>], overlay: &[Vec<T>]) -> Vec<Vec<T>> {
    if base.len() != overlay.len() {
        return base.to_vec();
    }
    base.iter()
        .zip(overlay)
        .map(|(base, overlay)| {
            if overlay.is_empty() {
                base.clone()
            } else {
                overlay.clone()
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeaderStoryRange {
    story_index: usize,
    start_cp: usize,
    end_cp: usize,
}

const HEADER_FOOTER_STORY_BASE: usize = 6;
const FIB_FCLCB_PLCF_SED: usize = 6;
const SED_RECORD_LEN: usize = 12;
const MAX_LEGACY_SECTION_COLUMNS: usize = 44;
const SPRM_S_DXA_COL_WIDTH: u16 = 0xF203;
const SPRM_S_DXA_COL_SPACING: u16 = 0xF204;
const SPRM_S_F_EVENLY_SPACED: u16 = 0x3005;
const SPRM_S_BKC: u16 = 0x3009;
const SPRM_S_F_TITLE_PAGE: u16 = 0x300A;
const SPRM_S_C_COLUMNS: u16 = 0x500B;
const SPRM_S_DXA_COLUMNS: u16 = 0x900C;
const SPRM_S_NFC_PGN: u16 = 0x300E;
const SPRM_S_F_PGN_RESTART: u16 = 0x3011;
const SPRM_S_L_BETWEEN: u16 = 0x3019;
const SPRM_S_F_BIDI: u16 = 0x3228;
const SPRM_S_DYA_HDR_TOP: u16 = 0xB017;
const SPRM_S_DYA_HDR_BOTTOM: u16 = 0xB018;
const SPRM_S_PGN_START_97: u16 = 0x501C;
const SPRM_S_B_ORIENTATION: u16 = 0x301D;
const SPRM_S_XA_PAGE: u16 = 0xB01F;
const SPRM_S_YA_PAGE: u16 = 0xB020;
const SPRM_S_DXA_LEFT: u16 = 0xB021;
const SPRM_S_DXA_RIGHT: u16 = 0xB022;
const SPRM_S_DYA_TOP: u16 = 0x9023;
const SPRM_S_DYA_BOTTOM: u16 = 0x9024;
const SPRM_S_DXT_CHAR_SPACE: u16 = 0x7030;
const SPRM_S_DYA_LINE_PITCH: u16 = 0x9031;
const SPRM_S_CLM: u16 = 0x5032;
const SPRM_S_TEXT_FLOW: u16 = 0x5033;
const SPRM_S_PGN_START: u16 = 0x7044;

fn legacy_header_footer_setup_slot(
    setup: &mut DocSetup,
    story_index: Option<usize>,
) -> &mut Vec<Block> {
    let Some(position) = legacy_header_footer_story_position(story_index) else {
        return &mut setup.header;
    };
    match position {
        0 => &mut setup.even_header,
        1 => &mut setup.header,
        2 => &mut setup.even_footer,
        3 => &mut setup.footer,
        4 => &mut setup.first_header,
        _ => &mut setup.first_footer,
    }
}

fn legacy_header_footer_section_setup_slot(
    setup: &mut SectionSetup,
    story_index: Option<usize>,
) -> Option<&mut Vec<Block>> {
    match legacy_header_footer_story_position(story_index)? {
        0 => Some(&mut setup.even_header),
        1 => Some(&mut setup.header),
        2 => Some(&mut setup.even_footer),
        3 => Some(&mut setup.footer),
        4 => Some(&mut setup.first_header),
        _ => Some(&mut setup.first_footer),
    }
}

fn legacy_header_footer_story_position(story_index: Option<usize>) -> Option<usize> {
    story_index?
        .checked_sub(HEADER_FOOTER_STORY_BASE)
        .map(|index| index % 6)
}

fn legacy_header_footer_section_index(story_index: Option<usize>) -> Option<usize> {
    story_index?
        .checked_sub(HEADER_FOOTER_STORY_BASE)
        .map(|index| index / 6)
}

#[cfg(feature = "docx")]
fn legacy_running_pagination_from_regions(
    blocks: &[Block],
    regions: &[SourceRegion],
    pagination_hints: &[PaginationHint],
    table_row_pagination: &[Vec<TableRowPaginationHint>],
    table_cell_pagination: &[TableCellPaginationHints],
) -> Vec<RunningSurfacePaginationHints> {
    debug_assert_eq!(blocks.len(), pagination_hints.len());
    debug_assert_eq!(blocks.len(), table_row_pagination.len());
    debug_assert_eq!(blocks.len(), table_cell_pagination.len());
    let section_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count()
        .saturating_add(1);
    let mut sections = vec![RunningSurfacePaginationHints::default(); section_count];

    for region in regions.iter().filter(|region| {
        region.kind == SourceRegionKind::HeaderFooter && region.block_start < region.block_end
    }) {
        let start = region.block_start.min(blocks.len());
        let end = region.block_end.min(blocks.len());
        let Some(paragraphs) = pagination_hints.get(start..end).map(|hints| hints.to_vec()) else {
            continue;
        };
        let Some(table_rows) = table_row_pagination
            .get(start..end)
            .map(|hints| hints.to_vec())
        else {
            continue;
        };
        let Some(table_cells) = table_cell_pagination
            .get(start..end)
            .map(|hints| hints.to_vec())
        else {
            continue;
        };
        if paragraphs.is_empty() {
            continue;
        }
        let captured = RunningBlockPaginationHints {
            paragraphs,
            table_rows,
            table_cells,
        };

        if section_count == 1 {
            let slot = legacy_running_pagination_slot(&mut sections[0], region.source_story_index);
            if slot.paragraphs.is_empty() {
                *slot = captured;
            }
            continue;
        }
        if region.source_story_index.is_none() {
            for section in &mut sections {
                if section.header.paragraphs.is_empty() {
                    section.header = captured.clone();
                }
            }
            continue;
        }
        let Some(section_index) = legacy_header_footer_section_index(region.source_story_index)
        else {
            continue;
        };
        let Some(section) = sections.get_mut(section_index) else {
            continue;
        };
        let Some(slot) = legacy_running_pagination_section_slot(section, region.source_story_index)
        else {
            continue;
        };
        if slot.paragraphs.is_empty() {
            *slot = captured;
        }
    }

    sections
}

#[cfg(feature = "docx")]
fn legacy_running_pagination_slot(
    hints: &mut RunningSurfacePaginationHints,
    story_index: Option<usize>,
) -> &mut RunningBlockPaginationHints {
    match legacy_header_footer_story_position(story_index) {
        Some(0) => &mut hints.even_header,
        Some(1) | None => &mut hints.header,
        Some(2) => &mut hints.even_footer,
        Some(3) => &mut hints.footer,
        Some(4) => &mut hints.first_header,
        _ => &mut hints.first_footer,
    }
}

#[cfg(feature = "docx")]
fn legacy_running_pagination_section_slot(
    hints: &mut RunningSurfacePaginationHints,
    story_index: Option<usize>,
) -> Option<&mut RunningBlockPaginationHints> {
    match legacy_header_footer_story_position(story_index)? {
        0 => Some(&mut hints.even_header),
        1 => Some(&mut hints.header),
        2 => Some(&mut hints.even_footer),
        3 => Some(&mut hints.footer),
        4 => Some(&mut hints.first_header),
        _ => Some(&mut hints.first_footer),
    }
}

#[cfg(feature = "docx")]
fn legacy_running_column_breaks_from_regions(
    blocks: &[Block],
    regions: &[LegacyRunningColumnBreakRegion],
) -> Vec<RunningSurfaceColumnBreakHints> {
    let section_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count()
        .saturating_add(1);
    let mut sections = vec![RunningSurfaceColumnBreakHints::default(); section_count];

    for region in regions
        .iter()
        .filter(|region| !region.column_break_offsets.is_empty())
    {
        if section_count == 1 {
            let slot =
                legacy_running_column_break_slot(&mut sections[0], region.source_story_index);
            if slot.is_empty() {
                *slot = region.column_break_offsets.clone();
            }
            continue;
        }
        if region.source_story_index.is_none() {
            for section in &mut sections {
                if section.header.is_empty() {
                    section.header = region.column_break_offsets.clone();
                }
            }
            continue;
        }
        let Some(section_index) = legacy_header_footer_section_index(region.source_story_index)
        else {
            continue;
        };
        let Some(section) = sections.get_mut(section_index) else {
            continue;
        };
        let Some(slot) =
            legacy_running_column_break_section_slot(section, region.source_story_index)
        else {
            continue;
        };
        if slot.is_empty() {
            *slot = region.column_break_offsets.clone();
        }
    }

    sections
}

#[cfg(feature = "docx")]
fn legacy_running_column_break_slot(
    hints: &mut RunningSurfaceColumnBreakHints,
    story_index: Option<usize>,
) -> &mut Vec<Vec<usize>> {
    match legacy_header_footer_story_position(story_index) {
        Some(0) => &mut hints.even_header,
        Some(1) | None => &mut hints.header,
        Some(2) => &mut hints.even_footer,
        Some(3) => &mut hints.footer,
        Some(4) => &mut hints.first_header,
        _ => &mut hints.first_footer,
    }
}

#[cfg(feature = "docx")]
fn legacy_running_column_break_section_slot(
    hints: &mut RunningSurfaceColumnBreakHints,
    story_index: Option<usize>,
) -> Option<&mut Vec<Vec<usize>>> {
    match legacy_header_footer_story_position(story_index)? {
        0 => Some(&mut hints.even_header),
        1 => Some(&mut hints.header),
        2 => Some(&mut hints.even_footer),
        3 => Some(&mut hints.footer),
        4 => Some(&mut hints.first_header),
        _ => Some(&mut hints.first_footer),
    }
}

fn clear_legacy_running_layout_hints(
    blocks: &[Block],
    regions: &[SourceRegion],
    pagination_hints: &mut [PaginationHint],
    line_spacing_hints: &mut [Option<LineSpacingHint>],
    table_row_pagination: &mut [Vec<TableRowPaginationHint>],
    table_cell_pagination: &mut [TableCellPaginationHints],
    table_cell_line_spacing: &mut [TableCellLineSpacingHints],
) {
    for region in regions
        .iter()
        .filter(|region| region.kind == SourceRegionKind::HeaderFooter)
    {
        let start = region.block_start.min(blocks.len());
        let end = region.block_end.min(blocks.len());
        for index in start..end {
            if let Some(hint) = pagination_hints.get_mut(index) {
                *hint = PaginationHint {
                    widow_control: true,
                    ..PaginationHint::default()
                };
            }
            if let Some(hint) = line_spacing_hints.get_mut(index) {
                *hint = None;
            }
            if let Some(rows) = table_row_pagination.get_mut(index) {
                for row in rows {
                    row.cant_split = false;
                }
            }
            if let Some(rows) = table_cell_pagination.get_mut(index) {
                for cells in rows {
                    for blocks in cells {
                        for paragraph in blocks {
                            *paragraph = None;
                        }
                    }
                }
            }
            if let Some(rows) = table_cell_line_spacing.get_mut(index) {
                for cells in rows {
                    for blocks in cells {
                        for paragraph in blocks {
                            *paragraph = None;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
fn legacy_running_line_spacing_from_regions(
    blocks: &[Block],
    regions: &[SourceRegion],
    line_spacing_hints: &[Option<LineSpacingHint>],
    table_cell_line_spacing: &[TableCellLineSpacingHints],
) -> Vec<RunningSurfaceLineSpacingHints> {
    debug_assert_eq!(blocks.len(), line_spacing_hints.len());
    debug_assert_eq!(blocks.len(), table_cell_line_spacing.len());
    let section_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count()
        .saturating_add(1);
    let mut sections = vec![RunningSurfaceLineSpacingHints::default(); section_count];

    for region in regions.iter().filter(|region| {
        region.kind == SourceRegionKind::HeaderFooter && region.block_start < region.block_end
    }) {
        let start = region.block_start.min(blocks.len());
        let end = region.block_end.min(blocks.len());
        let Some(spacing) = line_spacing_hints
            .get(start..end)
            .map(|slice| slice.to_vec())
        else {
            continue;
        };
        let Some(table_cell_spacing) = table_cell_line_spacing
            .get(start..end)
            .map(|slice| slice.to_vec())
        else {
            continue;
        };
        if spacing.is_empty() {
            continue;
        }

        if section_count == 1 {
            let (slot, table_cell_slot) =
                legacy_running_line_spacing_slots(&mut sections[0], region.source_story_index);
            if slot.is_empty() {
                *slot = spacing;
                *table_cell_slot = table_cell_spacing;
            }
            continue;
        }
        if region.source_story_index.is_none() {
            for section in &mut sections {
                if section.header.is_empty() {
                    section.header = spacing.clone();
                    section.header_table_cells = table_cell_spacing.clone();
                }
            }
            continue;
        }
        let Some(section_index) = legacy_header_footer_section_index(region.source_story_index)
        else {
            continue;
        };
        let Some(section) = sections.get_mut(section_index) else {
            continue;
        };
        let Some((slot, table_cell_slot)) =
            legacy_running_line_spacing_section_slots(section, region.source_story_index)
        else {
            continue;
        };
        if slot.is_empty() {
            *slot = spacing;
            *table_cell_slot = table_cell_spacing;
        }
    }

    sections
}

#[cfg(any(feature = "docx", feature = "render"))]
fn legacy_running_line_spacing_slots(
    hints: &mut RunningSurfaceLineSpacingHints,
    story_index: Option<usize>,
) -> (
    &mut Vec<Option<LineSpacingHint>>,
    &mut Vec<TableCellLineSpacingHints>,
) {
    let Some(position) = legacy_header_footer_story_position(story_index) else {
        return (&mut hints.header, &mut hints.header_table_cells);
    };
    match position {
        0 => (&mut hints.even_header, &mut hints.even_header_table_cells),
        1 => (&mut hints.header, &mut hints.header_table_cells),
        2 => (&mut hints.even_footer, &mut hints.even_footer_table_cells),
        3 => (&mut hints.footer, &mut hints.footer_table_cells),
        4 => (&mut hints.first_header, &mut hints.first_header_table_cells),
        _ => (&mut hints.first_footer, &mut hints.first_footer_table_cells),
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
fn legacy_running_line_spacing_section_slots(
    hints: &mut RunningSurfaceLineSpacingHints,
    story_index: Option<usize>,
) -> Option<(
    &mut Vec<Option<LineSpacingHint>>,
    &mut Vec<TableCellLineSpacingHints>,
)> {
    match legacy_header_footer_story_position(story_index)? {
        0 => Some((&mut hints.even_header, &mut hints.even_header_table_cells)),
        1 => Some((&mut hints.header, &mut hints.header_table_cells)),
        2 => Some((&mut hints.even_footer, &mut hints.even_footer_table_cells)),
        3 => Some((&mut hints.footer, &mut hints.footer_table_cells)),
        4 => Some((&mut hints.first_header, &mut hints.first_header_table_cells)),
        _ => Some((&mut hints.first_footer, &mut hints.first_footer_table_cells)),
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
fn legacy_running_tab_stops_from_regions(
    blocks: &[Block],
    regions: &[LegacyRunningTabRegion],
) -> (
    Vec<RunningSurfaceTabStopHints>,
    Vec<RunningSurfaceTableCellTabStopHints>,
) {
    let section_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count()
        .saturating_add(1);
    let mut paragraph_sections = vec![RunningSurfaceTabStopHints::default(); section_count];
    let mut table_sections = vec![RunningSurfaceTableCellTabStopHints::default(); section_count];

    for region in regions.iter().filter(|region| !region.tab_stops.is_empty()) {
        debug_assert_eq!(region.tab_stops.len(), region.table_cell_tab_stops.len());
        if section_count == 1 {
            let (paragraph_slot, table_slot) = legacy_running_tab_slots(
                &mut paragraph_sections[0],
                &mut table_sections[0],
                region.source_story_index,
            );
            if paragraph_slot.is_empty() {
                *paragraph_slot = region.tab_stops.clone();
                *table_slot = region.table_cell_tab_stops.clone();
            }
            continue;
        }

        if region.source_story_index.is_none() {
            for (paragraph, table) in paragraph_sections.iter_mut().zip(&mut table_sections) {
                if paragraph.header.is_empty() {
                    paragraph.header = region.tab_stops.clone();
                    table.header = region.table_cell_tab_stops.clone();
                }
            }
            continue;
        }
        let Some(section_index) = legacy_header_footer_section_index(region.source_story_index)
        else {
            continue;
        };
        let Some((paragraph, table)) = paragraph_sections
            .get_mut(section_index)
            .zip(table_sections.get_mut(section_index))
        else {
            continue;
        };
        let Some((paragraph_slot, table_slot)) =
            legacy_running_tab_section_slots(paragraph, table, region.source_story_index)
        else {
            continue;
        };
        if paragraph_slot.is_empty() {
            *paragraph_slot = region.tab_stops.clone();
            *table_slot = region.table_cell_tab_stops.clone();
        }
    }

    (paragraph_sections, table_sections)
}

#[cfg(any(feature = "docx", feature = "render"))]
fn legacy_running_tab_slots<'a>(
    paragraph: &'a mut RunningSurfaceTabStopHints,
    table: &'a mut RunningSurfaceTableCellTabStopHints,
    story_index: Option<usize>,
) -> (
    &'a mut Vec<Vec<TabStop>>,
    &'a mut Vec<TableCellTabStopHints>,
) {
    match legacy_header_footer_story_position(story_index) {
        Some(0) => (&mut paragraph.even_header, &mut table.even_header),
        Some(1) => (&mut paragraph.header, &mut table.header),
        Some(2) => (&mut paragraph.even_footer, &mut table.even_footer),
        Some(3) => (&mut paragraph.footer, &mut table.footer),
        Some(4) => (&mut paragraph.first_header, &mut table.first_header),
        Some(_) => (&mut paragraph.first_footer, &mut table.first_footer),
        None => (&mut paragraph.header, &mut table.header),
    }
}

#[cfg(any(feature = "docx", feature = "render"))]
fn legacy_running_tab_section_slots<'a>(
    paragraph: &'a mut RunningSurfaceTabStopHints,
    table: &'a mut RunningSurfaceTableCellTabStopHints,
    story_index: Option<usize>,
) -> Option<(
    &'a mut Vec<Vec<TabStop>>,
    &'a mut Vec<TableCellTabStopHints>,
)> {
    match legacy_header_footer_story_position(story_index)? {
        0 => Some((&mut paragraph.even_header, &mut table.even_header)),
        1 => Some((&mut paragraph.header, &mut table.header)),
        2 => Some((&mut paragraph.even_footer, &mut table.even_footer)),
        3 => Some((&mut paragraph.footer, &mut table.footer)),
        4 => Some((&mut paragraph.first_header, &mut table.first_header)),
        _ => Some((&mut paragraph.first_footer, &mut table.first_footer)),
    }
}

fn header_footer_story_ranges(fib: &Fib, table: &[u8]) -> Vec<HeaderStoryRange> {
    if fib.ccp_hdd == 0 || fib.lcb_plcf_hdd < 12 {
        return Vec::new();
    }
    let Some(slice) = table.get(fib.fc_plcf_hdd..fib.fc_plcf_hdd.saturating_add(fib.lcb_plcf_hdd))
    else {
        return Vec::new();
    };
    let cp_count = slice.len() / 4;
    if cp_count < 3 {
        return Vec::new();
    }
    let story_count = cp_count.saturating_sub(2);
    let hdd_len = fib.ccp_hdd as usize;
    let mut stories = Vec::new();
    for story_index in 0..story_count {
        let start = u32le(slice, story_index * 4).unwrap_or(0) as usize;
        let end = u32le(slice, (story_index + 1) * 4).unwrap_or(0) as usize;
        let start = start.min(hdd_len);
        let end = end.min(hdd_len);
        if start < end {
            stories.push(HeaderStoryRange {
                story_index,
                start_cp: start,
                end_cp: end,
            });
        }
    }
    stories
}

#[derive(Debug, Clone, PartialEq)]
struct LegacySectionSpan {
    start_cp: usize,
    end_cp: usize,
    page: PageSetup,
    columns: Option<u16>,
    column_gap_pt: Option<f32>,
    column_layout: Option<SectionColumnLayoutHints>,
    column_separator: bool,
    column_rtl: bool,
    title_page: bool,
    page_number_start: Option<u32>,
    page_number_format: Option<PageNumberFormat>,
    text_direction: Option<TextDirection>,
    doc_grid: Option<DocGrid>,
    header_distance_pt: Option<f32>,
    footer_distance_pt: Option<f32>,
    section_break: SectionBreakKind,
}

#[derive(Debug, Clone, PartialEq)]
struct LegacySectionProperties {
    page: PageSetup,
    columns: Option<u16>,
    column_gap_pt: Option<f32>,
    column_layout: Option<SectionColumnLayoutHints>,
    column_separator: bool,
    column_rtl: bool,
    title_page: bool,
    page_number_start: Option<u32>,
    page_number_format: Option<PageNumberFormat>,
    text_direction: Option<TextDirection>,
    doc_grid: Option<DocGrid>,
    header_distance_pt: Option<f32>,
    footer_distance_pt: Option<f32>,
    section_break: SectionBreakKind,
}

fn legacy_section_spans(word: &[u8], table: &[u8], main_len_cp: usize) -> Vec<LegacySectionSpan> {
    parse_legacy_section_spans(word, table, main_len_cp).unwrap_or_default()
}

fn parse_legacy_section_spans(
    word: &[u8],
    table: &[u8],
    main_len_cp: usize,
) -> Option<Vec<LegacySectionSpan>> {
    let (fc, lcb) = fib::fc_lcb_pair(word, FIB_FCLCB_PLCF_SED)?;
    if lcb < 4 {
        return None;
    }
    let payload_len = lcb.checked_sub(4)?;
    let section_width = 4usize.checked_add(SED_RECORD_LEN)?;
    if payload_len % section_width != 0 {
        return None;
    }
    let section_count = payload_len / section_width;
    if section_count == 0 {
        return None;
    }
    let end = fc.checked_add(lcb)?;
    let slice = table.get(fc..end)?;
    let cp_bytes = section_count.checked_add(1)?.checked_mul(4)?;
    if cp_bytes.checked_add(section_count.checked_mul(SED_RECORD_LEN)?)? != lcb {
        return None;
    }

    let mut cps = Vec::with_capacity(section_count + 1);
    for index in 0..=section_count {
        cps.push(u32le(slice, index * 4)? as usize);
    }
    // [MS-DOC] 2.8.26 permits the final CP at or beyond the main-story end.
    if cps.first().copied() != Some(0)
        || cps
            .last()
            .copied()
            .is_none_or(|last_cp| last_cp < main_len_cp)
    {
        return None;
    }
    let mut spans = Vec::with_capacity(section_count);
    for (index, pair) in cps.windows(2).enumerate() {
        let [start_cp, end_cp] = pair else {
            return None;
        };
        let bounded_end_cp = (*end_cp).min(main_len_cp);
        if start_cp >= end_cp || *start_cp >= bounded_end_cp {
            return None;
        }
        let record_offset = cp_bytes.checked_add(index.checked_mul(SED_RECORD_LEN)?)?;
        let fc_sepx = u32le(slice, record_offset.checked_add(2)?)? as i32;
        let properties = legacy_sepx_section_properties_at(word, fc_sepx);
        spans.push(LegacySectionSpan {
            start_cp: *start_cp,
            end_cp: bounded_end_cp,
            page: properties.page,
            columns: properties.columns,
            column_gap_pt: properties.column_gap_pt,
            column_layout: properties.column_layout,
            column_separator: properties.column_separator,
            column_rtl: properties.column_rtl,
            title_page: properties.title_page,
            page_number_start: properties.page_number_start,
            page_number_format: properties.page_number_format,
            text_direction: properties.text_direction,
            doc_grid: properties.doc_grid,
            header_distance_pt: properties.header_distance_pt,
            footer_distance_pt: properties.footer_distance_pt,
            section_break: properties.section_break,
        });
    }
    Some(spans)
}

fn legacy_sepx_section_properties_at(word: &[u8], fc_sepx: i32) -> LegacySectionProperties {
    usize::try_from(fc_sepx)
        .ok()
        .filter(|offset| *offset != 0)
        .and_then(|offset| parse_legacy_sepx_section_properties(word, offset))
        .unwrap_or_else(legacy_section_properties_default)
}

fn parse_legacy_sepx_section_properties(
    word: &[u8],
    offset: usize,
) -> Option<LegacySectionProperties> {
    let cb = u16le(word, offset)? as i16;
    let cb = usize::try_from(cb).ok()?;
    let start = offset.checked_add(2)?;
    let end = start.checked_add(cb)?;
    scan_legacy_section_grpprl(word.get(start..end)?)
}

fn scan_legacy_section_grpprl(grpprl: &[u8]) -> Option<LegacySectionProperties> {
    let mut properties = legacy_section_properties_default();
    // [MS-DOC] 2.6.4 defaults to equal spacing and stores the count minus one.
    let mut column_count = None;
    let mut column_gap_pt = None;
    let mut column_width_twips = [None; MAX_LEGACY_SECTION_COLUMNS];
    let mut column_spacing_twips = [None; MAX_LEGACY_SECTION_COLUMNS];
    let mut columns_evenly_spaced = true;
    let mut page_number_restart = false;
    let mut page_number_start = None;
    // [MS-DOC] 2.6.4 and 2.9.237 require a valid line pitch for every enabled mode.
    let mut doc_grid_type = None;
    let mut doc_grid_line_pitch = None;
    let mut doc_grid_character_space = None;
    let mut pos = 0usize;
    while pos < grpprl.len() {
        let sprm = u16le(grpprl, pos)?;
        let operand_start = pos.checked_add(2)?;
        let operand_len = legacy_sprm_operand_len(sprm, grpprl, operand_start)?;
        let operand_end = operand_start.checked_add(operand_len)?;
        let operand = grpprl.get(operand_start..operand_end)?;

        match sprm {
            SPRM_S_DXA_COL_WIDTH => {
                let index = usize::from(*operand.first()?);
                if index < MAX_LEGACY_SECTION_COLUMNS {
                    if let Some(value @ 718..=31_680) = u16le(operand, 1) {
                        column_width_twips[index] = Some(value);
                    }
                }
            }
            SPRM_S_DXA_COL_SPACING => {
                let index = usize::from(*operand.first()?);
                if index < MAX_LEGACY_SECTION_COLUMNS {
                    if let Some(value @ 0..=31_680) = u16le(operand, 1) {
                        column_spacing_twips[index] = Some(value);
                    }
                }
            }
            SPRM_S_F_EVENLY_SPACED => match operand.first().copied() {
                Some(0) => columns_evenly_spaced = false,
                Some(1) => columns_evenly_spaced = true,
                _ => {}
            },
            SPRM_S_BKC => match operand.first().copied() {
                // Continuous/new-column cannot be represented by the shared model.
                Some(0..=2) => properties.section_break = SectionBreakKind::NextPage,
                Some(3) => properties.section_break = SectionBreakKind::EvenPage,
                Some(4) => properties.section_break = SectionBreakKind::OddPage,
                _ => {}
            },
            SPRM_S_F_TITLE_PAGE => match operand.first().copied() {
                Some(0) => properties.title_page = false,
                Some(1) => properties.title_page = true,
                _ => {}
            },
            SPRM_S_C_COLUMNS => {
                if let Some(value @ 0..=43) = u16le(operand, 0) {
                    column_count = value.checked_add(1);
                }
            }
            SPRM_S_DXA_COLUMNS => {
                let value = u16le(operand, 0)? as i16;
                if (0..=31_680).contains(&value) {
                    column_gap_pt = Some(f32::from(value) / 20.0);
                }
            }
            SPRM_S_NFC_PGN => {
                if let Some(format) = operand.first().copied().and_then(legacy_page_number_format) {
                    properties.page_number_format = Some(format);
                }
            }
            SPRM_S_F_PGN_RESTART => match operand.first().copied() {
                Some(0) => page_number_restart = false,
                Some(1) => page_number_restart = true,
                _ => {}
            },
            SPRM_S_L_BETWEEN => match operand.first().copied() {
                Some(0) => properties.column_separator = false,
                Some(1) => properties.column_separator = true,
                _ => {}
            },
            SPRM_S_F_BIDI => match operand.first().copied() {
                Some(0) => properties.column_rtl = false,
                Some(1) => properties.column_rtl = true,
                _ => {}
            },
            SPRM_S_DYA_HDR_TOP => {
                if let Some(value @ 0..=31_680) = u16le(operand, 0) {
                    properties.header_distance_pt = Some(twips_to_points(value));
                }
            }
            SPRM_S_DYA_HDR_BOTTOM => {
                if let Some(value @ 0..=31_680) = u16le(operand, 0) {
                    properties.footer_distance_pt = Some(twips_to_points(value));
                }
            }
            SPRM_S_PGN_START_97 => {
                if let Some(value) = u16le(operand, 0) {
                    page_number_start = Some(u32::from(value));
                }
            }
            SPRM_S_B_ORIENTATION => match operand.first().copied() {
                // [MS-DOC] 2.9.236: 1 = portrait, 2 = landscape.
                Some(1) => properties.page.landscape = false,
                Some(2) => properties.page.landscape = true,
                _ => {}
            },
            SPRM_S_XA_PAGE => {
                if let Some(value @ 144..=31_680) = u16le(operand, 0) {
                    properties.page.width_pt = twips_to_points(value);
                }
            }
            SPRM_S_YA_PAGE => {
                if let Some(value @ 144..=31_680) = u16le(operand, 0) {
                    properties.page.height_pt = twips_to_points(value);
                }
            }
            SPRM_S_DXA_LEFT => {
                if let Some(value @ 0..=31_680) = u16le(operand, 0) {
                    properties.page.margin_left_pt = Some(twips_to_points(value));
                }
            }
            SPRM_S_DXA_RIGHT => {
                if let Some(value @ 0..=31_680) = u16le(operand, 0) {
                    properties.page.margin_right_pt = Some(twips_to_points(value));
                }
            }
            SPRM_S_DYA_TOP => {
                let value = u16le(operand, 0)? as i16;
                if (0..=31_665).contains(&value) {
                    properties.page.margin_top_pt = Some(f32::from(value) / 20.0);
                }
            }
            SPRM_S_DYA_BOTTOM => {
                let value = u16le(operand, 0)? as i16;
                if (0..=31_665).contains(&value) {
                    properties.page.margin_bottom_pt = Some(f32::from(value) / 20.0);
                }
            }
            SPRM_S_DXT_CHAR_SPACE => {
                let value = u32le(operand, 0)? as i32;
                if (-670_925..=6_488_064).contains(&value) {
                    // The shared model is unsigned; a valid negative value must
                    // still clear an earlier representable source-order value.
                    doc_grid_character_space = u32::try_from(value).ok();
                }
            }
            SPRM_S_DYA_LINE_PITCH => {
                if let Some(value @ 1..=31_680) = u16le(operand, 0) {
                    doc_grid_line_pitch = Some(u32::from(value));
                }
            }
            SPRM_S_CLM => {
                if let Some(value) = u16le(operand, 0) {
                    match value {
                        0 => doc_grid_type = None,
                        1 => doc_grid_type = Some(DocGridType::LinesAndChars),
                        2 => doc_grid_type = Some(DocGridType::Lines),
                        3 => doc_grid_type = Some(DocGridType::SnapToChars),
                        _ => {}
                    }
                }
            }
            SPRM_S_TEXT_FLOW => {
                if let Some(direction) = u16le(operand, 0).and_then(legacy_text_direction) {
                    properties.text_direction = Some(direction);
                }
            }
            SPRM_S_PGN_START => {
                if let Some(value @ 0..=2_147_483_646) = u32le(operand, 0) {
                    page_number_start = Some(value);
                }
            }
            _ => {}
        }
        pos = operand_end;
    }
    if columns_evenly_spaced {
        properties.columns = column_count;
        properties.column_gap_pt = column_count
            .is_some_and(|count| count > 1)
            .then_some(column_gap_pt)
            .flatten();
    } else if column_count == Some(1) {
        properties.columns = Some(1);
    } else if let Some(layout) =
        legacy_custom_column_layout(column_count, &column_width_twips, &column_spacing_twips)
    {
        properties.columns = u16::try_from(layout.columns.len()).ok();
        properties.column_layout = Some(layout);
    }
    properties.page_number_start =
        page_number_restart.then_some(page_number_start.unwrap_or(0).max(1));
    properties.doc_grid = doc_grid_type
        .zip(doc_grid_line_pitch)
        .map(|(grid_type, line_pitch)| DocGrid {
            grid_type,
            line_pitch: Some(line_pitch),
            character_space: doc_grid_character_space,
        });
    Some(properties)
}

fn legacy_custom_column_layout(
    column_count: Option<u16>,
    widths_twips: &[Option<u16>; MAX_LEGACY_SECTION_COLUMNS],
    spacing_twips: &[Option<u16>; MAX_LEGACY_SECTION_COLUMNS],
) -> Option<SectionColumnLayoutHints> {
    let count = usize::from(column_count?);
    if !(2..=MAX_LEGACY_SECTION_COLUMNS).contains(&count) {
        return None;
    }
    let mut columns = Vec::with_capacity(count);
    for index in 0..count {
        columns.push(SectionColumnHint {
            width_pt: twips_to_points(widths_twips[index]?),
            space_after_pt: if index + 1 < count {
                twips_to_points(spacing_twips[index].unwrap_or(0))
            } else {
                0.0
            },
        });
    }
    Some(SectionColumnLayoutHints { columns })
}

fn legacy_text_direction(text_flow: u16) -> Option<TextDirection> {
    // [MS-DOC] 2.6.4 uses [MS-ODRAW] 2.4.5 MSOTXFL. The `A`/`N`
    // distinction carries the glyph rotation represented by ECMA-376 Part 4
    // 14.11.7's transitional `V` directions; Word's value-5 behavior advances
    // subsequent lines to the right, matching `tbLrV`.
    match text_flow {
        0 => Some(TextDirection::LeftToRightTopToBottom),
        1 => Some(TextDirection::TopToBottomRightToLeft),
        2 => Some(TextDirection::BottomToTopLeftToRight),
        3 => Some(TextDirection::TopToBottomRightToLeftVertical),
        4 => Some(TextDirection::LeftToRightTopToBottomVertical),
        5 => Some(TextDirection::TopToBottomLeftToRightVertical),
        _ => None,
    }
}

fn legacy_page_number_format(nfc: u8) -> Option<PageNumberFormat> {
    match nfc {
        0x00 | 0x28 => Some(PageNumberFormat::Decimal),
        0x01 => Some(PageNumberFormat::UpperRoman),
        0x02 => Some(PageNumberFormat::LowerRoman),
        0x03 => Some(PageNumberFormat::UpperLetter),
        0x04 => Some(PageNumberFormat::LowerLetter),
        0x05 => Some(PageNumberFormat::Ordinal),
        0x06 => Some(PageNumberFormat::CardinalText),
        0x07 => Some(PageNumberFormat::OrdinalText),
        0x0E => Some(PageNumberFormat::DecimalFullWidth),
        0x0F => Some(PageNumberFormat::DecimalHalfWidth),
        0x12 => Some(PageNumberFormat::DecimalEnclosedCircle),
        0x13 => Some(PageNumberFormat::DecimalFullWidth2),
        0x16 => Some(PageNumberFormat::DecimalZero),
        0x18 => Some(PageNumberFormat::Ganada),
        0x19 => Some(PageNumberFormat::Chosung),
        0x1A => Some(PageNumberFormat::DecimalEnclosedFullstop),
        0x1B => Some(PageNumberFormat::DecimalEnclosedParen),
        0x29 => Some(PageNumberFormat::KoreanDigital),
        0x2A => Some(PageNumberFormat::KoreanCounting),
        0x2B => Some(PageNumberFormat::KoreanLegal),
        0x2C => Some(PageNumberFormat::KoreanDigital2),
        0x39 => Some(PageNumberFormat::NumberInDash),
        // [MS-DOC] permits fallback for non-counting formats; other formats absent
        // from the shared enum use the same deterministic decimal ceiling.
        0x08..=0x3B | 0xFF => Some(PageNumberFormat::Decimal),
        _ => None,
    }
}

fn legacy_section_properties_default() -> LegacySectionProperties {
    LegacySectionProperties {
        page: legacy_section_page_setup_default(),
        columns: None,
        column_gap_pt: None,
        column_layout: None,
        column_separator: false,
        column_rtl: false,
        title_page: false,
        page_number_start: None,
        page_number_format: None,
        text_direction: None,
        doc_grid: None,
        header_distance_pt: None,
        footer_distance_pt: None,
        section_break: SectionBreakKind::NextPage,
    }
}

fn legacy_section_page_setup_default() -> PageSetup {
    // [MS-DOC] 2.6.4 defaults omitted legacy section sizes to US Letter.
    PageSetup {
        width_pt: 612.0,
        height_pt: 792.0,
        ..PageSetup::default()
    }
}

fn legacy_sprm_operand_len(sprm: u16, data: &[u8], operand_start: usize) -> Option<usize> {
    match (sprm >> 13) & 0x7 {
        0 | 1 => Some(1),
        2 | 4 | 5 => Some(2),
        3 => Some(4),
        7 => Some(3),
        6 => Some(1usize.checked_add(usize::from(*data.get(operand_start)?))?),
        _ => None,
    }
}

fn twips_to_points(value: u16) -> f32 {
    f32::from(value) / 20.0
}

fn legacy_region_specs(fib: &Fib) -> [(SourceRegionKind, usize); 6] {
    [
        (SourceRegionKind::Main, fib.ccp_text as usize),
        (SourceRegionKind::Footnote, fib.ccp_ftn as usize),
        (SourceRegionKind::HeaderFooter, fib.ccp_hdd as usize),
        (SourceRegionKind::Annotation, fib.ccp_atn as usize),
        (SourceRegionKind::Endnote, fib.ccp_edn as usize),
        (SourceRegionKind::TextBox, fib.ccp_txbx as usize),
    ]
}

/// Decode every piece in CP order into UTF-16 code units, recording each unit's
/// source byte offset in the `WordDocument` stream (so CHPX/PAPX FC lookups land
/// on the right character).
pub(crate) fn decode_with_fc(
    word: &[u8],
    pieces: &[Piece],
    enc: &'static Encoding,
) -> (Vec<u16>, Vec<u32>) {
    let (units, fcs, _) = decode_piece_stream(word, pieces, enc, false);
    (units, fcs)
}

fn decode_with_fc_and_prm(
    word: &[u8],
    pieces: &[Piece],
    enc: &'static Encoding,
) -> (Vec<u16>, Vec<u32>, Vec<u16>) {
    decode_piece_stream(word, pieces, enc, true)
}

fn decode_piece_stream(
    word: &[u8],
    pieces: &[Piece],
    enc: &'static Encoding,
    track_prms: bool,
) -> (Vec<u16>, Vec<u32>, Vec<u16>) {
    let mut units: Vec<u16> = Vec::new();
    let mut fcs: Vec<u32> = Vec::new();
    let mut prms: Vec<u16> = Vec::new();
    // Bound cumulative decoded bytes (see `text::decode_pieces`): valid pieces partition the
    // stream (total ≤ word.len()), but overlapping pieces in a crafted piece table would
    // re-decode it per piece — a quadratic memory/CPU DoS. Stop once the budget is reached.
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
            // Decode the whole 8-bit slice (handles multi-byte cp949/cp932), then
            // assign each char its source FC by re-encoding to count its bytes.
            let text = enc.decode(slice).0;
            let mut fc = p.fc as u32;
            let mut tmp = [0u8; 4];
            let mut ubuf = [0u16; 2];
            for ch in text.chars() {
                let chs = ch.encode_utf8(&mut tmp);
                // Re-encode to recover the source byte width. An undecodable byte
                // decodes to U+FFFD, which `encode` would turn into a multi-byte
                // numeric character reference (`&#65533;`) — that would over-count
                // and shift every following FC, misattributing CHPX runs. Guard
                // it: on a round-trip error the source was a single bad byte, and
                // no char in any supported ANSI codepage is wider than 2 bytes.
                let (eb, _, had_err) = enc.encode(chs);
                let blen = if had_err { 1 } else { eb.len().clamp(1, 2) } as u32;
                for u in ch.encode_utf16(&mut ubuf) {
                    push_decoded_unit(&mut units, &mut fcs, &mut prms, track_prms, *u, fc, p.prm);
                }
                fc = fc.saturating_add(blen);
            }
        } else {
            let byte_len = p.cch.saturating_mul(2);
            let end = p.fc.saturating_add(byte_len).min(word.len());
            let Some(slice) = word.get(p.fc..end) else {
                continue;
            };
            consumed = consumed.saturating_add(slice.len());
            for (i, c) in slice.chunks_exact(2).enumerate() {
                push_decoded_unit(
                    &mut units,
                    &mut fcs,
                    &mut prms,
                    track_prms,
                    u16::from_le_bytes([c[0], c[1]]),
                    (p.fc + i * 2) as u32,
                    p.prm,
                );
            }
        }
    }
    (units, fcs, prms)
}

#[inline]
fn push_decoded_unit(
    units: &mut Vec<u16>,
    fcs: &mut Vec<u32>,
    prms: &mut Vec<u16>,
    track_prms: bool,
    unit: u16,
    fc: u32,
    prm: u16,
) {
    units.push(unit);
    fcs.push(fc);
    if track_prms {
        prms.push(prm);
    }
}

// Word control characters.
const CELL_MARK: u16 = 0x07;
const PARA_MARK: u16 = 0x0D;
const FIELD_BEGIN: u16 = 0x13;
const FIELD_SEP: u16 = 0x14;
const FIELD_END: u16 = 0x15;

/// Streaming assembler over the `(units, fcs)` stream.
struct Asm<'a, 'l> {
    papx: &'a PapxTable,
    chpx: &'a ChpxTable,
    prm1_patches: &'a [Option<PcdPrm1Patch>],
    stylesheet: &'a StyleSheet,
    data: &'a [u8],
    fonts: &'a [String],
    numberer: &'a mut Numberer<'l>,

    blocks: Vec<Block>,
    pagination_hints: Vec<PaginationHint>,
    line_spacing_hints: Vec<Option<LineSpacingHint>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: Vec<Vec<TabStop>>,
    #[cfg(feature = "docx")]
    note_reference_anchors: Vec<Vec<LegacyNoteReferenceAnchor>>,
    column_break_offsets: Vec<Vec<usize>>,
    #[cfg(feature = "docx")]
    table_cell_column_break_offsets: Vec<TableCellColumnBreakHints>,
    page_break_offsets: Vec<Vec<usize>>,
    table_row_pagination: Vec<Vec<TableRowPaginationHint>>,
    table_cell_pagination: Vec<TableCellPaginationHints>,
    table_cell_line_spacing: Vec<TableCellLineSpacingHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    table_cell_tab_stops: Vec<TableCellTabStopHints>,

    // Current run being coalesced. `run_chp` is the (cheap, `Copy`) source the current
    // `run_props` was built from — comparing it per code unit avoids rebuilding the owned
    // `CharProps` (which clones the font name) and `FieldRole` (which clones the URL) for
    // every character. The URL is constant within a run because every `active_url` change
    // happens at a field mark, which flushes the run first.
    run_buf: Vec<u16>,
    run_chp: Chp,
    run_props: CharProps,
    run_field: FieldRole,

    // Current paragraph's runs.
    para_runs: Vec<Run_>,
    para_text_chars: usize,
    #[cfg(feature = "docx")]
    para_note_reference_anchors: Vec<LegacyNoteReferenceAnchor>,
    para_column_break_offsets: Vec<usize>,
    para_page_break_offsets: Vec<usize>,

    // A non-final PlcfSed range ends in a section mark, not a manual page break.
    suppress_terminal_page_break: bool,

    // Table-building state.
    cur_rows: Vec<RowBuild>,
    cur_row_pagination: Vec<TableRowPaginationHint>,
    cur_table_bidi_visual: Option<bool>,
    cur_row_cells: Vec<CellBuild>,
    cell_blocks: Vec<Block>,
    cell_pagination: Vec<Option<PaginationHint>>,
    cell_line_spacing: Vec<Option<LineSpacingHint>>,
    #[cfg(feature = "docx")]
    cell_column_break_offsets: Vec<Vec<usize>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    cell_tab_stops: Vec<Vec<TabStop>>,

    // Field state. `field_stack` holds one entry per currently-open field
    // (`0x13`..`0x15`), each recording whether that field has passed its `0x14`
    // separator and the instruction parsed at that point. Text is visible only
    // when *every* open field has seen its separator: if any enclosing field is
    // still in its instruction part, the text (even a nested field's result)
    // belongs to that instruction and is dropped. This makes a field with no
    // separator at all, and text after any field ends, correctly return to
    // visible-content mode — a plain bool could never be un-stuck and silently
    // swallowed all trailing text.
    field_stack: Vec<FieldState>,
    // Count of `field_stack` entries still in their instruction part (not yet
    // separated). `in_instruction()` is `unseparated != 0` — an O(1) replacement
    // for scanning `field_stack` per code unit, which a crafted run of N field
    // markers + N text chars turned into O(N²) work (CPU DoS via the model APIs).
    unseparated: usize,
    // Per-document inline-picture cache + byte budget. A crafted `.doc` can point
    // many picture runs (`0x01`) at the same `fcPic`, so without dedup the same
    // `Data` payload is rescanned and recopied per run — O(runs × payload). Cache
    // each `fcPic`'s extraction (scan once) and cap total materialized image bytes
    // (legit image bytes live once in `Data`, so ≤ ~2×data.len()).
    img_cache: HashMap<u32, Image>,
    img_budget: usize,
}

#[derive(Default)]
struct FieldState {
    separated: bool,
    instr_buf: Vec<u16>,
    role: FieldRole,
}

// Local alias to the model Run (avoid a name clash with the field below).
use crate::model::Run as Run_;

fn resolve_paragraph_indent(source: ParagraphIndentOverrides, bidi: bool) -> Indent {
    let logical_left_twips = source
        .logical_left_twips
        .map(|left| i32::from(left) + i32::from(source.nest_twips.unwrap_or(0)));
    let logical_right_twips = source.logical_right_twips.map(i32::from);
    let (left_twips, right_twips) = if bidi {
        (logical_right_twips, logical_left_twips)
    } else {
        (logical_left_twips, logical_right_twips)
    };
    let points = |twips: Option<i32>| {
        twips
            .filter(|value| *value != 0)
            .map(|value| value as f32 / 20.0)
    };
    let (first_line_pt, hanging_pt) = match source.first_line_twips.map(i32::from) {
        Some(value) if value > 0 => (Some(value as f32 / 20.0), None),
        Some(value) if value < 0 => (None, Some((-value) as f32 / 20.0)),
        _ => (None, None),
    };
    Indent {
        left_pt: points(left_twips),
        right_pt: points(right_twips),
        first_line_pt,
        hanging_pt,
    }
}

fn resolve_paragraph_spacing(source: ParagraphSpacingOverrides) -> Spacing {
    Spacing {
        before_pt: Some(source.before_twips.unwrap_or(0) as f32 / 20.0),
        after_pt: Some(source.after_twips.unwrap_or(0) as f32 / 20.0),
        line_pct: match source.line {
            Some(ParagraphLineSpacing::ProportionalTwips(value)) => Some(value as f32 / 240.0),
            Some(
                ParagraphLineSpacing::ExactTwips(_)
                | ParagraphLineSpacing::AtLeastTwips(_)
                | ParagraphLineSpacing::Unrepresentable,
            ) => None,
            None => Some(1.0),
        },
    }
}

fn resolve_absolute_line_spacing(source: ParagraphSpacingOverrides) -> Option<LineSpacingHint> {
    match source.line {
        Some(ParagraphLineSpacing::ExactTwips(value)) => {
            Some(LineSpacingHint::Exact(value as f32 / 20.0))
        }
        Some(ParagraphLineSpacing::AtLeastTwips(value)) => {
            Some(LineSpacingHint::AtLeast(value as f32 / 20.0))
        }
        Some(
            ParagraphLineSpacing::ProportionalTwips(_) | ParagraphLineSpacing::Unrepresentable,
        )
        | None => None,
    }
}

struct CompletedLegacyParagraph {
    paragraph: Paragraph,
    pagination: PaginationHint,
    line_spacing_hint: Option<LineSpacingHint>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: Vec<TabStop>,
    #[cfg(feature = "docx")]
    note_reference_anchors: Vec<LegacyNoteReferenceAnchor>,
    column_break_offsets: Vec<usize>,
    page_break_offsets: Vec<usize>,
}

impl<'a, 'l> Asm<'a, 'l> {
    fn new(
        papx: &'a PapxTable,
        chpx: &'a ChpxTable,
        stylesheet: &'a StyleSheet,
        data: &'a [u8],
        fonts: &'a [String],
        numberer: &'a mut Numberer<'l>,
    ) -> Self {
        Asm {
            papx,
            chpx,
            prm1_patches: &[],
            stylesheet,
            data,
            fonts,
            numberer,
            blocks: Vec::new(),
            pagination_hints: Vec::new(),
            line_spacing_hints: Vec::new(),
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops: Vec::new(),
            #[cfg(feature = "docx")]
            note_reference_anchors: Vec::new(),
            column_break_offsets: Vec::new(),
            #[cfg(feature = "docx")]
            table_cell_column_break_offsets: Vec::new(),
            page_break_offsets: Vec::new(),
            table_row_pagination: Vec::new(),
            table_cell_pagination: Vec::new(),
            table_cell_line_spacing: Vec::new(),
            #[cfg(any(feature = "docx", feature = "render"))]
            table_cell_tab_stops: Vec::new(),
            run_buf: Vec::new(),
            run_chp: Chp::default(),
            run_props: CharProps::default(),
            run_field: FieldRole::None,
            para_runs: Vec::new(),
            para_text_chars: 0,
            #[cfg(feature = "docx")]
            para_note_reference_anchors: Vec::new(),
            para_column_break_offsets: Vec::new(),
            para_page_break_offsets: Vec::new(),
            suppress_terminal_page_break: false,
            cur_rows: Vec::new(),
            cur_row_pagination: Vec::new(),
            cur_table_bidi_visual: None,
            cur_row_cells: Vec::new(),
            cell_blocks: Vec::new(),
            cell_pagination: Vec::new(),
            cell_line_spacing: Vec::new(),
            #[cfg(feature = "docx")]
            cell_column_break_offsets: Vec::new(),
            #[cfg(any(feature = "docx", feature = "render"))]
            cell_tab_stops: Vec::new(),
            field_stack: Vec::new(),
            unseparated: 0,
            img_cache: HashMap::new(),
            img_budget: data.len().saturating_mul(2).saturating_add(1 << 20),
        }
    }

    /// We are in field-instruction (drop) mode iff *any* open field has not yet
    /// passed its `0x14` separator — a nested field's result is still part of the
    /// enclosing field's instruction. Empty stack ⇒ visible body content. Tracked
    /// as a counter (not a per-call scan of `field_stack`) so this stays O(1).
    fn in_instruction(&self) -> bool {
        self.unseparated != 0
    }

    fn active_field_role(&self) -> FieldRole {
        if self.in_instruction() {
            return FieldRole::None;
        }
        self.field_stack
            .last()
            .map(|field| field.role.clone())
            .unwrap_or_default()
    }

    fn push_instruction_unit(&mut self, u: u16) {
        if let Some(field) = self
            .field_stack
            .iter_mut()
            .rev()
            .find(|field| !field.separated)
        {
            field.instr_buf.push(u);
        }
    }

    #[cfg(test)]
    fn run(&mut self, units: &[u16], fcs: &[u32]) {
        self.run_with_prms(units, fcs, &[], 0);
    }

    fn run_with_prms(&mut self, units: &[u16], fcs: &[u32], prms: &[u16], source_cp_base: usize) {
        for (i, &u) in units.iter().enumerate() {
            let fc = fcs.get(i).copied().unwrap_or(0);
            let prm = prms.get(i).copied().unwrap_or(0);
            let terminal_section_mark =
                self.suppress_terminal_page_break && i + 1 == units.len() && u == 0x000C;
            match u {
                FIELD_BEGIN => {
                    self.flush_run();
                    self.field_stack.push(FieldState::default());
                    self.unseparated += 1;
                }
                FIELD_SEP => {
                    // Mark the innermost field as separated → its result follows.
                    let n = self.field_stack.len();
                    if n > 0 && !self.field_stack[n - 1].separated {
                        self.field_stack[n - 1].separated = true;
                        self.unseparated -= 1;
                    }
                    if let Some(field) = self.field_stack.last_mut() {
                        let instr = String::from_utf16_lossy(&field.instr_buf);
                        field.role = field_role_from_instruction(&instr);
                    }
                    self.flush_run();
                }
                FIELD_END => {
                    self.flush_run();
                    if let Some(field) = self.field_stack.pop() {
                        if !field.separated {
                            self.unseparated -= 1;
                        }
                    }
                }
                _ if self.in_instruction() => self.push_instruction_unit(u),
                PARA_MARK => self.end_paragraph(fc, false),
                CELL_MARK => self.end_paragraph(fc, true),
                0x0001 => self.picture(fc),
                _ => self.push_content(
                    u,
                    fc,
                    prm,
                    terminal_section_mark,
                    source_cp_base.saturating_add(i),
                ),
            }
        }
    }

    /// An inline picture special char (`0x01`): if the run is a real picture
    /// (`fSpec` + `sprmCPicLocation`), extract it into an image run; otherwise
    /// (embedded OLE object, form field) drop it.
    fn picture(&mut self, fc: u32) {
        let Some(fc_pic) = self.chpx.pic_at(fc) else {
            return;
        };
        self.flush_run();
        let img = self.extract_image(fc_pic);
        self.para_runs.push(Run_ {
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

    /// Resolve the picture at `fc_pic`, scanning the `Data` stream at most once per
    /// location (cache) and bounding total materialized image bytes (budget). Once
    /// the budget is spent, further pictures become metadata-only placeholders — so
    /// a crafted `.doc` aliasing one payload across many runs stays O(input), not
    /// O(runs × payload), without dropping images in any real document.
    fn extract_image(&mut self, fc_pic: u32) -> Image {
        if !self.img_cache.contains_key(&fc_pic) {
            // PICF total size (lcb @ fcPic) bounds the scan; charge it before scanning
            // so even payloads with no recognizable raster cost the budget once.
            let lcb = crate::util::u32le(self.data, fc_pic as usize).unwrap_or(0) as usize;
            let img = if lcb == 0 || lcb > self.img_budget {
                Image::default()
            } else {
                self.img_budget = self.img_budget.saturating_sub(lcb);
                crate::image::extract(self.data, fc_pic)
            };
            self.img_cache.insert(fc_pic, img);
        }
        // Per-run copy: charge the emitted bytes; over budget ⇒ metadata-only
        // placeholder (no byte clone) so N references to one payload stay bounded.
        let n = self
            .img_cache
            .get(&fc_pic)
            .and_then(|i| i.bytes.as_ref())
            .map_or(0, |b| b.len());
        if n == 0 || n > self.img_budget {
            let c = self.img_cache.get(&fc_pic).expect("inserted above");
            return Image {
                alt: c.alt.clone(),
                bytes: None,
                mime: c.mime.clone(),
                width_px: c.width_px,
                height_px: c.height_px,
                rotation_degrees: c.rotation_degrees,
                floating_offset_emu: c.floating_offset_emu,
                drawing_layout: c.drawing_layout.clone(),
            };
        }
        self.img_budget -= n;
        self.img_cache.get(&fc_pic).cloned().unwrap_or_default()
    }

    /// Append a content code unit to the current run, splitting the run when the
    /// character properties or field role change.
    fn push_content(
        &mut self,
        u: u16,
        fc: u32,
        prm: u16,
        terminal_section_mark: bool,
        _source_cp: usize,
    ) {
        if terminal_section_mark {
            return;
        }
        #[cfg(feature = "docx")]
        if u == 0x0002 {
            let buffered_chars = String::from_utf16_lossy(&self.run_buf).chars().count();
            self.para_note_reference_anchors
                .push(LegacyNoteReferenceAnchor {
                    source_cp: _source_cp,
                    text_offset: self.para_text_chars.saturating_add(buffered_chars),
                });
            return;
        }
        // Map Word control characters to plain text; drop the unrenderable ones.
        let mapped: Option<u16> = match u {
            0x0B | 0x0C | 0x0E => Some(0x000A), // line / page / column break → newline
            0x1E => Some(0x002D),               // non-breaking hyphen → '-'
            0xA0 => Some(0x0020),               // non-breaking space → ' '
            0x1F => None,                       // optional hyphen → drop
            0x01 | 0x02 | 0x08 => None,         // picture / footnote / object anchors (Slice 5)
            c if c < 0x20 && c != b'\t' as u16 => None, // other C0 controls
            c => Some(c),
        };
        let Some(unit) = mapped else { return };
        let is_column_break = u == 0x000E;
        let is_page_break = u == 0x000C;

        let mut chp = self.chpx.chp_at(fc);
        chp.apply_pcd_prm(prm, self.prm1_patches);
        let paragraph_style = self.papx.style_at(fc).0;
        let mut inherited = self.stylesheet.typography(paragraph_style);
        if let Some(style) = chp.character_style {
            inherited = self.stylesheet.character_typography(style).inherit_typography(inherited);
        }
        chp = chp.inherit_typography(inherited);
        chp.ftc = chp.font_for(unit);
        chp.normalize_model_defaults();
        // Start a new run only when the (cheap) char properties change or after a flush
        // (e.g. a field mark, which is also the only place `active_url` changes). The owned
        // `CharProps`/`FieldRole` — which clone the font name and URL — are then built once
        // per run, not once per code unit (the latter was O(metadata × text) work).
        if self.run_buf.is_empty() || chp != self.run_chp {
            self.flush_run();
            self.run_chp = chp;
            self.run_props = CharProps {
                bold: chp.bold,
                italic: chp.italic,
                underline: chp.underline,
                strike: chp.strike,
                hidden: chp.hidden,
                size_half_pt: chp.size_half_pt,
                color: chp.color,
                highlight: chp.highlight.and_then(highlight_name).map(str::to_owned),
                vert_align: chp.vert_align.unwrap_or_default(),
                small_caps: chp.small_caps.unwrap_or(false),
                caps: chp.caps.unwrap_or(false),
                rtl: chp.rtl.unwrap_or(false),
                font: chp.ftc.and_then(|ftc| crate::ffn::name_of(self.fonts, ftc)),
            };
            self.run_field = self.active_field_role();
        }
        if is_page_break || (is_column_break && !chp.hidden) {
            self.flush_run();
            if is_page_break {
                self.para_page_break_offsets.push(self.para_text_chars);
            } else {
                self.para_column_break_offsets.push(self.para_text_chars);
            }
        }
        self.run_buf.push(unit);
    }

    fn flush_run(&mut self) {
        if self.run_buf.is_empty() {
            return;
        }
        let text = String::from_utf16_lossy(&self.run_buf);
        self.run_buf.clear();
        self.para_text_chars = self.para_text_chars.saturating_add(text.chars().count());
        self.para_runs.push(Run_ {
            text,
            props: self.run_props.clone(),
            field: self.run_field.clone(),
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

    /// Finalize the runs collected so far into a [`Paragraph`] with list info.
    fn take_paragraph(&mut self, fc: u32) -> CompletedLegacyParagraph {
        self.flush_run();
        let runs = std::mem::take(&mut self.para_runs);
        self.para_text_chars = 0;
        #[cfg(feature = "docx")]
        let note_reference_anchors = std::mem::take(&mut self.para_note_reference_anchors);
        let column_break_offsets = std::mem::take(&mut self.para_column_break_offsets);
        let page_break_offsets = std::mem::take(&mut self.para_page_break_offsets);
        let (ilfo, ilvl) = self.papx.list_at(fc);
        let list = if ilfo > 0 {
            self.numberer.label(ilfo, ilvl).map(|label| ListInfo {
                level: ilvl,
                ordered: !label.trim().is_empty(),
                label,
                marker_font: None,
            })
        } else {
            None
        };
        // Heading level: an explicit outline level on the paragraph wins
        // (0..8 → h1..h9, 9 → body); otherwise the paragraph style decides.
        let (istd, outlvl) = self.papx.style_at(fc);
        let layout = self
            .stylesheet
            .paragraph_layout(istd)
            .apply(self.papx.paragraph_layout_overrides_at(fc));
        let bidi = layout.bidi.unwrap_or(false);
        let indent = resolve_paragraph_indent(
            self.stylesheet
                .paragraph_indent(istd)
                .apply(self.papx.paragraph_indent_overrides_at(fc)),
            bidi,
        );
        let source_spacing = self
            .stylesheet
            .paragraph_spacing(istd)
            .apply(self.papx.paragraph_spacing_overrides_at(fc));
        let spacing = resolve_paragraph_spacing(source_spacing);
        let line_spacing_hint = resolve_absolute_line_spacing(source_spacing);
        #[cfg(any(feature = "docx", feature = "render"))]
        let tab_stops = self
            .stylesheet
            .paragraph_tab_stops(istd)
            .and_then(|inherited| self.papx.resolve_paragraph_tab_stops_at(fc, inherited))
            .map(|stops| stops.into_iter().map(|stop| stop.to_model()).collect())
            .unwrap_or_default();
        let source_pagination = self
            .stylesheet
            .paragraph_pagination(istd)
            .apply(self.papx.paragraph_pagination_overrides_at(fc));
        let shading = self
            .papx
            .paragraph_shading_at(fc)
            .or_else(|| self.stylesheet.paragraph_shading(istd))
            .and_then(|shading| shading.flat_color());
        let heading_level = match outlvl {
            Some(o) if o <= 8 => Some(o + 1),
            Some(_) => None,
            None => self.stylesheet.heading_level(istd),
        };
        let align = match layout.justification {
            Some(ParagraphJustification::PhysicalLeft) => Align::Left,
            Some(ParagraphJustification::Center) => Align::Center,
            Some(ParagraphJustification::PhysicalRight) => Align::Right,
            Some(ParagraphJustification::Justify) => Align::Justify,
            Some(ParagraphJustification::LogicalEnd) => {
                if bidi {
                    Align::Left
                } else {
                    Align::Right
                }
            }
            Some(ParagraphJustification::LogicalStart) => {
                if bidi {
                    Align::Right
                } else {
                    Align::Left
                }
            }
            // The modern paragraph-justification default is logical start.
            // The indented value keeps that edge while its indent stays a ceiling.
            Some(ParagraphJustification::UnsupportedIndented) | None => {
                if bidi {
                    Align::Right
                } else {
                    Align::Left
                }
            }
        };
        let style_name = self.stylesheet.name(istd).map(str::to_string);
        // A heading takes precedence over list-item rendering.
        let list = if heading_level.is_some() { None } else { list };
        let paragraph = Paragraph {
            props: ParaProps {
                style_name,
                heading_level,
                align,
                outline_level: outlvl,
                list,
                spacing,
                indent,
                shading,
                page_break_before: source_pagination.page_break_before,
                bidi,
                ..Default::default()
            },
            runs,
        };
        let pagination = PaginationHint {
            keep_next: source_pagination.keep_next,
            keep_lines: source_pagination.keep_lines,
            widow_control: source_pagination.widow_control,
        };
        CompletedLegacyParagraph {
            paragraph,
            pagination,
            line_spacing_hint,
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops,
            #[cfg(feature = "docx")]
            note_reference_anchors,
            column_break_offsets,
            page_break_offsets,
        }
    }

    /// Handle a paragraph (`0x0D`) or cell (`0x07`) mark: finalize the paragraph
    /// and route it into the body or the current table.
    fn end_paragraph(&mut self, fc: u32, is_cell_mark: bool) {
        let (in_table, ttp) = self.papx.at(fc);
        let CompletedLegacyParagraph {
            paragraph: para,
            pagination,
            line_spacing_hint,
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops,
            #[cfg(feature = "docx")]
            note_reference_anchors,
            column_break_offsets,
            page_break_offsets,
        } = self.take_paragraph(fc);

        if !in_table {
            self.flush_table();
            if !para.is_blank()
                || !column_break_offsets.is_empty()
                || !page_break_offsets.is_empty()
            {
                self.blocks.push(Block::Paragraph(para));
                self.pagination_hints.push(pagination);
                self.line_spacing_hints.push(line_spacing_hint);
                #[cfg(any(feature = "docx", feature = "render"))]
                self.tab_stops.push(tab_stops);
                #[cfg(feature = "docx")]
                self.note_reference_anchors.push(note_reference_anchors);
                self.column_break_offsets.push(column_break_offsets);
                #[cfg(feature = "docx")]
                self.table_cell_column_break_offsets.push(Vec::new());
                self.page_break_offsets.push(page_break_offsets);
                self.table_row_pagination.push(Vec::new());
                self.table_cell_pagination.push(Vec::new());
                self.table_cell_line_spacing.push(Vec::new());
                #[cfg(any(feature = "docx", feature = "render"))]
                self.table_cell_tab_stops.push(Vec::new());
            }
            return;
        }

        // A 0x0D inside a table starts a new paragraph within the SAME cell; a
        // 0x07 closes the cell (and, when it is the row terminator, the row).
        if !is_cell_mark {
            self.cell_blocks.push(Block::Paragraph(para));
            self.cell_pagination.push(Some(pagination));
            self.cell_line_spacing.push(line_spacing_hint);
            #[cfg(feature = "docx")]
            self.cell_column_break_offsets.push(column_break_offsets);
            #[cfg(any(feature = "docx", feature = "render"))]
            self.cell_tab_stops.push(tab_stops);
            return;
        }
        // The row-terminating paragraph (`fTtp`) is an empty marker, not a real
        // cell — don't emit it as a phantom trailing column.
        let blank_terminator = ttp && para.is_blank() && self.cell_blocks.is_empty();
        if !blank_terminator {
            self.cell_blocks.push(Block::Paragraph(para));
            self.cell_pagination.push(Some(pagination));
            self.cell_line_spacing.push(line_spacing_hint);
            #[cfg(feature = "docx")]
            self.cell_column_break_offsets.push(column_break_offsets);
            #[cfg(any(feature = "docx", feature = "render"))]
            self.cell_tab_stops.push(tab_stops);
            self.cur_row_cells.push(CellBuild {
                blocks: std::mem::take(&mut self.cell_blocks),
                pagination: std::mem::take(&mut self.cell_pagination),
                line_spacing: std::mem::take(&mut self.cell_line_spacing),
                #[cfg(feature = "docx")]
                column_break_offsets: std::mem::take(&mut self.cell_column_break_offsets),
                #[cfg(any(feature = "docx", feature = "render"))]
                tab_stops: std::mem::take(&mut self.cell_tab_stops),
            });
        } else {
            self.cell_blocks.clear();
            self.cell_pagination.clear();
            self.cell_line_spacing.clear();
            #[cfg(feature = "docx")]
            self.cell_column_break_offsets.clear();
            #[cfg(any(feature = "docx", feature = "render"))]
            self.cell_tab_stops.clear();
        }
        if ttp {
            // The row definition (column geometry + merge flags) is carried on the
            // TTP paragraph's grpprl.
            let def = self.papx.table_def_at(fc).cloned();
            let header = self.papx.table_header_at(fc);
            let bidi_visual = self.papx.table_bidi_visual_at(fc);
            let row = RowBuild {
                cells: std::mem::take(&mut self.cur_row_cells),
                def,
                header,
            };
            let row_pagination = TableRowPaginationHint {
                cant_split: self.papx.table_cant_split_at(fc),
            };
            if self
                .cur_table_bidi_visual
                .is_some_and(|current| current != bidi_visual)
            {
                self.flush_table();
            }
            self.cur_table_bidi_visual.get_or_insert(bidi_visual);
            self.cur_rows.push(row);
            self.cur_row_pagination.push(row_pagination);
        }
    }

    /// Emit any in-progress table as a block, resolving cell merges.
    fn flush_table(&mut self) {
        // A dangling row (no row terminator) is still real tabular data.
        if !self.cur_row_cells.is_empty() {
            self.cur_rows.push(RowBuild {
                cells: std::mem::take(&mut self.cur_row_cells),
                def: None,
                header: false,
            });
            self.cur_row_pagination
                .push(TableRowPaginationHint::default());
        }
        self.cell_blocks.clear();
        self.cell_pagination.clear();
        self.cell_line_spacing.clear();
        #[cfg(feature = "docx")]
        self.cell_column_break_offsets.clear();
        #[cfg(any(feature = "docx", feature = "render"))]
        self.cell_tab_stops.clear();
        let bidi_visual = self.cur_table_bidi_visual.take().unwrap_or(false);
        if !self.cur_rows.is_empty() {
            let built =
                table::build_with_direction(std::mem::take(&mut self.cur_rows), bidi_visual);
            let row_pagination = std::mem::take(&mut self.cur_row_pagination);
            if !built.table.rows.is_empty() {
                debug_assert_eq!(row_pagination.len(), built.table.rows.len());
                debug_assert_eq!(built.cell_pagination.len(), built.table.rows.len());
                debug_assert_eq!(built.cell_line_spacing.len(), built.table.rows.len());
                #[cfg(any(feature = "docx", feature = "render"))]
                debug_assert_eq!(built.cell_tab_stops.len(), built.table.rows.len());
                self.blocks.push(Block::Table(built.table));
                self.pagination_hints.push(PaginationHint::default());
                self.line_spacing_hints.push(None);
                #[cfg(any(feature = "docx", feature = "render"))]
                self.tab_stops.push(Vec::new());
                #[cfg(feature = "docx")]
                self.note_reference_anchors.push(Vec::new());
                self.column_break_offsets.push(Vec::new());
                #[cfg(feature = "docx")]
                self.table_cell_column_break_offsets
                    .push(built.cell_column_breaks);
                self.page_break_offsets.push(Vec::new());
                self.table_row_pagination.push(row_pagination);
                self.table_cell_pagination.push(built.cell_pagination);
                self.table_cell_line_spacing.push(built.cell_line_spacing);
                #[cfg(any(feature = "docx", feature = "render"))]
                self.table_cell_tab_stops.push(built.cell_tab_stops);
            }
        }
    }

    /// Flush trailing paragraph/table state at end of stream.
    fn finish_with_render_hints(mut self) -> LegacyBlockOutput {
        // A trailing paragraph with no final mark.
        if !self.para_runs.is_empty() || !self.run_buf.is_empty() {
            let CompletedLegacyParagraph {
                paragraph: para,
                pagination,
                line_spacing_hint,
                #[cfg(any(feature = "docx", feature = "render"))]
                tab_stops,
                #[cfg(feature = "docx")]
                note_reference_anchors,
                column_break_offsets,
                page_break_offsets,
            } = self.take_paragraph(u32::MAX);
            if !para.is_blank()
                || !column_break_offsets.is_empty()
                || !page_break_offsets.is_empty()
            {
                self.blocks.push(Block::Paragraph(para));
                self.pagination_hints.push(pagination);
                self.line_spacing_hints.push(line_spacing_hint);
                #[cfg(any(feature = "docx", feature = "render"))]
                self.tab_stops.push(tab_stops);
                #[cfg(feature = "docx")]
                self.note_reference_anchors.push(note_reference_anchors);
                self.column_break_offsets.push(column_break_offsets);
                #[cfg(feature = "docx")]
                self.table_cell_column_break_offsets.push(Vec::new());
                self.page_break_offsets.push(page_break_offsets);
                self.table_row_pagination.push(Vec::new());
                self.table_cell_pagination.push(Vec::new());
                self.table_cell_line_spacing.push(Vec::new());
                #[cfg(any(feature = "docx", feature = "render"))]
                self.table_cell_tab_stops.push(Vec::new());
            }
        }
        self.flush_table();
        debug_assert_eq!(self.pagination_hints.len(), self.blocks.len());
        debug_assert_eq!(self.line_spacing_hints.len(), self.blocks.len());
        #[cfg(any(feature = "docx", feature = "render"))]
        debug_assert_eq!(self.tab_stops.len(), self.blocks.len());
        #[cfg(feature = "docx")]
        debug_assert_eq!(self.note_reference_anchors.len(), self.blocks.len());
        debug_assert_eq!(self.column_break_offsets.len(), self.blocks.len());
        #[cfg(feature = "docx")]
        debug_assert_eq!(
            self.table_cell_column_break_offsets.len(),
            self.blocks.len()
        );
        debug_assert_eq!(self.page_break_offsets.len(), self.blocks.len());
        debug_assert_eq!(self.table_row_pagination.len(), self.blocks.len());
        debug_assert_eq!(self.table_cell_pagination.len(), self.blocks.len());
        debug_assert_eq!(self.table_cell_line_spacing.len(), self.blocks.len());
        #[cfg(any(feature = "docx", feature = "render"))]
        debug_assert_eq!(self.table_cell_tab_stops.len(), self.blocks.len());
        LegacyBlockOutput {
            blocks: self.blocks,
            pagination_hints: self.pagination_hints,
            line_spacing_hints: self.line_spacing_hints,
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops: self.tab_stops,
            #[cfg(feature = "docx")]
            note_reference_anchors: self.note_reference_anchors,
            column_break_offsets: self.column_break_offsets,
            #[cfg(feature = "docx")]
            table_cell_column_break_offsets: self.table_cell_column_break_offsets,
            page_break_offsets: self.page_break_offsets,
            table_row_pagination: self.table_row_pagination,
            table_cell_pagination: self.table_cell_pagination,
            table_cell_line_spacing: self.table_cell_line_spacing,
            #[cfg(any(feature = "docx", feature = "render"))]
            table_cell_tab_stops: self.table_cell_tab_stops,
        }
    }

    #[cfg(test)]
    fn finish(self) -> Vec<Block> {
        self.finish_with_render_hints().blocks
    }
}

#[derive(Default)]
struct LegacyBlockOutput {
    blocks: Vec<Block>,
    pagination_hints: Vec<PaginationHint>,
    line_spacing_hints: Vec<Option<LineSpacingHint>>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: Vec<Vec<TabStop>>,
    #[cfg(feature = "docx")]
    note_reference_anchors: Vec<Vec<LegacyNoteReferenceAnchor>>,
    column_break_offsets: Vec<Vec<usize>>,
    #[cfg(feature = "docx")]
    table_cell_column_break_offsets: Vec<TableCellColumnBreakHints>,
    page_break_offsets: Vec<Vec<usize>>,
    table_row_pagination: Vec<Vec<TableRowPaginationHint>>,
    table_cell_pagination: Vec<TableCellPaginationHints>,
    table_cell_line_spacing: Vec<TableCellLineSpacingHints>,
    #[cfg(any(feature = "docx", feature = "render"))]
    table_cell_tab_stops: Vec<TableCellTabStopHints>,
}

fn promote_legacy_manual_page_breaks(output: LegacyBlockOutput) -> LegacyBlockOutput {
    let LegacyBlockOutput {
        blocks,
        pagination_hints,
        line_spacing_hints,
        #[cfg(any(feature = "docx", feature = "render"))]
        tab_stops,
        #[cfg(feature = "docx")]
        note_reference_anchors,
        column_break_offsets,
        #[cfg(feature = "docx")]
        table_cell_column_break_offsets,
        page_break_offsets,
        table_row_pagination,
        table_cell_pagination,
        table_cell_line_spacing,
        #[cfg(any(feature = "docx", feature = "render"))]
        table_cell_tab_stops,
    } = output;
    debug_assert_eq!(pagination_hints.len(), blocks.len());
    debug_assert_eq!(line_spacing_hints.len(), blocks.len());
    #[cfg(any(feature = "docx", feature = "render"))]
    debug_assert_eq!(tab_stops.len(), blocks.len());
    #[cfg(feature = "docx")]
    debug_assert_eq!(note_reference_anchors.len(), blocks.len());
    debug_assert_eq!(column_break_offsets.len(), blocks.len());
    #[cfg(feature = "docx")]
    debug_assert_eq!(table_cell_column_break_offsets.len(), blocks.len());
    debug_assert_eq!(page_break_offsets.len(), blocks.len());
    debug_assert_eq!(table_row_pagination.len(), blocks.len());
    debug_assert_eq!(table_cell_pagination.len(), blocks.len());
    debug_assert_eq!(table_cell_line_spacing.len(), blocks.len());
    #[cfg(any(feature = "docx", feature = "render"))]
    debug_assert_eq!(table_cell_tab_stops.len(), blocks.len());

    let mut pagination_hints = pagination_hints.into_iter();
    let mut line_spacing_hints = line_spacing_hints.into_iter();
    #[cfg(any(feature = "docx", feature = "render"))]
    let mut tab_stops = tab_stops.into_iter();
    #[cfg(feature = "docx")]
    let mut note_reference_anchors = note_reference_anchors.into_iter();
    let mut column_break_offsets = column_break_offsets.into_iter();
    #[cfg(feature = "docx")]
    let mut table_cell_column_break_offsets = table_cell_column_break_offsets.into_iter();
    let mut page_break_offsets = page_break_offsets.into_iter();
    let mut table_row_pagination = table_row_pagination.into_iter();
    let mut table_cell_pagination = table_cell_pagination.into_iter();
    let mut table_cell_line_spacing = table_cell_line_spacing.into_iter();
    #[cfg(any(feature = "docx", feature = "render"))]
    let mut table_cell_tab_stops = table_cell_tab_stops.into_iter();
    let mut promoted = LegacyBlockOutput::default();

    for block in blocks {
        let pagination = pagination_hints.next().unwrap_or_default();
        let line_spacing_hint = line_spacing_hints.next().unwrap_or_default();
        #[cfg(any(feature = "docx", feature = "render"))]
        let paragraph_tab_stops = tab_stops.next().unwrap_or_default();
        #[cfg(feature = "docx")]
        let paragraph_note_anchors = note_reference_anchors.next().unwrap_or_default();
        let column_offsets = column_break_offsets.next().unwrap_or_default();
        #[cfg(feature = "docx")]
        let cell_column_offsets = table_cell_column_break_offsets.next().unwrap_or_default();
        let page_offsets = page_break_offsets.next().unwrap_or_default();
        let row_pagination = table_row_pagination.next().unwrap_or_default();
        let cell_pagination = table_cell_pagination.next().unwrap_or_default();
        let cell_line_spacing = table_cell_line_spacing.next().unwrap_or_default();
        #[cfg(any(feature = "docx", feature = "render"))]
        let cell_tab_stops = table_cell_tab_stops.next().unwrap_or_default();
        match block {
            Block::Paragraph(paragraph) if !page_offsets.is_empty() => {
                debug_assert!(row_pagination.is_empty());
                debug_assert!(cell_pagination.is_empty());
                debug_assert!(cell_line_spacing.is_empty());
                #[cfg(feature = "docx")]
                debug_assert!(cell_column_offsets.is_empty());
                #[cfg(any(feature = "docx", feature = "render"))]
                debug_assert!(cell_tab_stops.is_empty());
                promote_legacy_paragraph_page_breaks(
                    paragraph,
                    pagination,
                    line_spacing_hint,
                    #[cfg(any(feature = "docx", feature = "render"))]
                    &paragraph_tab_stops,
                    &column_offsets,
                    &page_offsets,
                    &mut promoted,
                );
            }
            block => {
                debug_assert!(page_offsets.is_empty());
                promoted.blocks.push(block);
                promoted.pagination_hints.push(pagination);
                promoted.line_spacing_hints.push(line_spacing_hint);
                #[cfg(any(feature = "docx", feature = "render"))]
                promoted.tab_stops.push(paragraph_tab_stops);
                #[cfg(feature = "docx")]
                promoted.note_reference_anchors.push(paragraph_note_anchors);
                promoted.column_break_offsets.push(column_offsets);
                #[cfg(feature = "docx")]
                promoted
                    .table_cell_column_break_offsets
                    .push(cell_column_offsets);
                promoted.page_break_offsets.push(Vec::new());
                promoted.table_row_pagination.push(row_pagination);
                promoted.table_cell_pagination.push(cell_pagination);
                promoted.table_cell_line_spacing.push(cell_line_spacing);
                #[cfg(any(feature = "docx", feature = "render"))]
                promoted.table_cell_tab_stops.push(cell_tab_stops);
            }
        }
    }
    promoted
}

fn promote_legacy_paragraph_page_breaks(
    paragraph: Paragraph,
    pagination: PaginationHint,
    line_spacing_hint: Option<LineSpacingHint>,
    #[cfg(any(feature = "docx", feature = "render"))] tab_stops: &[TabStop],
    column_break_offsets: &[usize],
    page_break_offsets: &[usize],
    output: &mut LegacyBlockOutput,
) {
    let props = paragraph.props;
    let mut current = Paragraph {
        props: props.clone(),
        runs: Vec::new(),
    };
    let mut page_breaks = page_break_offsets.iter().copied().peekable();
    let mut source_chars = 0usize;
    let mut segment_start = 0usize;
    let hints = LegacyParagraphPromotionHints {
        pagination,
        line_spacing: line_spacing_hint,
        #[cfg(any(feature = "docx", feature = "render"))]
        tab_stops,
        column_break_offsets,
    };

    for run in paragraph.runs {
        if run.text.is_empty() {
            current.runs.push(run);
            continue;
        }
        let mut text = String::new();
        for ch in run.text.chars() {
            if page_breaks.peek().copied() == Some(source_chars) {
                debug_assert_eq!(ch, '\n');
                push_legacy_run_fragment(&mut current, &run, &mut text);
                push_legacy_page_break_segment(
                    output,
                    &mut current,
                    &props,
                    hints,
                    segment_start..source_chars,
                );
                output.blocks.push(Block::PageBreak);
                output.pagination_hints.push(PaginationHint::default());
                output.line_spacing_hints.push(None);
                #[cfg(any(feature = "docx", feature = "render"))]
                output.tab_stops.push(Vec::new());
                #[cfg(feature = "docx")]
                output.note_reference_anchors.push(Vec::new());
                output.column_break_offsets.push(Vec::new());
                #[cfg(feature = "docx")]
                output.table_cell_column_break_offsets.push(Vec::new());
                output.page_break_offsets.push(Vec::new());
                output.table_row_pagination.push(Vec::new());
                output.table_cell_pagination.push(Vec::new());
                output.table_cell_line_spacing.push(Vec::new());
                #[cfg(any(feature = "docx", feature = "render"))]
                output.table_cell_tab_stops.push(Vec::new());
                page_breaks.next();
                source_chars = source_chars.saturating_add(1);
                segment_start = source_chars;
            } else {
                text.push(ch);
                source_chars = source_chars.saturating_add(1);
            }
        }
        push_legacy_run_fragment(&mut current, &run, &mut text);
    }

    debug_assert!(page_breaks.next().is_none());
    push_legacy_page_break_segment(
        output,
        &mut current,
        &props,
        hints,
        segment_start..source_chars,
    );
}

fn push_legacy_run_fragment(current: &mut Paragraph, run: &Run_, text: &mut String) {
    if text.is_empty() {
        return;
    }
    let mut fragment = run.clone();
    fragment.text = std::mem::take(text);
    current.runs.push(fragment);
}

#[derive(Clone, Copy)]
struct LegacyParagraphPromotionHints<'a> {
    pagination: PaginationHint,
    line_spacing: Option<LineSpacingHint>,
    #[cfg(any(feature = "docx", feature = "render"))]
    tab_stops: &'a [TabStop],
    column_break_offsets: &'a [usize],
}

fn push_legacy_page_break_segment(
    output: &mut LegacyBlockOutput,
    current: &mut Paragraph,
    props: &ParaProps,
    hints: LegacyParagraphPromotionHints<'_>,
    segment: std::ops::Range<usize>,
) {
    if current.runs.is_empty() {
        return;
    }
    let paragraph = std::mem::replace(
        current,
        Paragraph {
            props: props.clone(),
            runs: Vec::new(),
        },
    );
    output.blocks.push(Block::Paragraph(paragraph));
    output.pagination_hints.push(hints.pagination);
    output.line_spacing_hints.push(hints.line_spacing);
    #[cfg(any(feature = "docx", feature = "render"))]
    output.tab_stops.push(hints.tab_stops.to_vec());
    #[cfg(feature = "docx")]
    output.note_reference_anchors.push(Vec::new());
    output.column_break_offsets.push(
        hints
            .column_break_offsets
            .iter()
            .copied()
            .filter(|offset| segment.contains(offset))
            .map(|offset| offset - segment.start)
            .collect(),
    );
    #[cfg(feature = "docx")]
    output.table_cell_column_break_offsets.push(Vec::new());
    output.page_break_offsets.push(Vec::new());
    output.table_row_pagination.push(Vec::new());
    output.table_cell_pagination.push(Vec::new());
    output.table_cell_line_spacing.push(Vec::new());
    #[cfg(any(feature = "docx", feature = "render"))]
    output.table_cell_tab_stops.push(Vec::new());
}

/// Extract the target URL from a `HYPERLINK` field instruction, e.g.
/// `HYPERLINK "https://example.com" \o "tooltip"` → `https://example.com`.
fn parse_hyperlink(instr: &str) -> Option<String> {
    crate::annotation::hyperlink_field_target(instr)
}

fn field_role_from_instruction(instr: &str) -> FieldRole {
    if let Some(url) = parse_hyperlink(instr) {
        return FieldRole::Hyperlink { url };
    }
    let instruction = normalize_field_instruction(instr);
    if instruction.is_empty() {
        FieldRole::None
    } else {
        FieldRole::Simple { instruction }
    }
}

/// Aggregate paragraph/table/figure/character counts over a block tree. Shared
/// with the `.docx` path so both backends report stats identically.
pub(crate) fn compute_stats(blocks: &[Block]) -> Stats {
    let mut s = Stats::default();
    count_blocks(blocks, &mut s);
    s
}

fn count_blocks(blocks: &[Block], s: &mut Stats) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                s.paragraphs = s.paragraphs.saturating_add(1);
                s.text_chars += p.text().chars().count();
                for r in &p.runs {
                    if r.image.is_some() {
                        s.figures = s.figures.saturating_add(1);
                    }
                }
            }
            Block::Image(_) => s.figures = s.figures.saturating_add(1),
            Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
            Block::Table(t) => {
                s.tables = s.tables.saturating_add(1);
                for row in &t.rows {
                    for cell in &row.cells {
                        count_blocks(&cell.blocks, s);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::Lists;

    /// Run the assembler over a bare unit stream (FCs = 1:1 with index, no
    /// styling/list tables) and return the resulting blocks.
    fn run_units(units: &[u16]) -> Vec<Block> {
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);
        asm.run(units, &fcs);
        asm.finish()
    }

    fn push_test_section_column_count(grpprl: &mut Vec<u8>, columns_minus_one: u16) {
        grpprl.extend_from_slice(&SPRM_S_C_COLUMNS.to_le_bytes());
        grpprl.extend_from_slice(&columns_minus_one.to_le_bytes());
    }

    fn push_test_section_evenly_spaced(grpprl: &mut Vec<u8>, evenly_spaced: u8) {
        grpprl.extend_from_slice(&SPRM_S_F_EVENLY_SPACED.to_le_bytes());
        grpprl.push(evenly_spaced);
    }

    fn push_test_section_indexed_column_value(
        grpprl: &mut Vec<u8>,
        sprm: u16,
        index: u8,
        twips: u16,
    ) {
        grpprl.extend_from_slice(&sprm.to_le_bytes());
        grpprl.push(index);
        grpprl.extend_from_slice(&twips.to_le_bytes());
    }

    #[test]
    fn legacy_assembly_aligns_row_pagination_with_emitted_blocks() {
        let units = [b'A' as u16, CELL_MARK, CELL_MARK, b'X' as u16, PARA_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries(&[
            (2, true, false, false),
            (3, true, true, true),
            (5, false, false, false),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert!(matches!(assembled.blocks[0], Block::Table(_)));
        assert!(matches!(assembled.blocks[1], Block::Paragraph(_)));
        assert_eq!(assembled.table_row_pagination.len(), assembled.blocks.len());
        assert_eq!(assembled.table_row_pagination[0].len(), 1);
        assert!(assembled.table_row_pagination[0][0].cant_split);
        assert!(assembled.table_row_pagination[1].is_empty());
    }

    #[test]
    fn legacy_assembly_records_manual_column_break_offsets() {
        let units = [
            b'A' as u16,
            0x000E,
            b'B' as u16,
            0x000E,
            b'C' as u16,
            PARA_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(assembled.blocks.len(), 1);
        let Block::Paragraph(paragraph) = &assembled.blocks[0] else {
            panic!("legacy block must be a paragraph");
        };
        assert_eq!(paragraph.text(), "A\nB\nC");
        assert_eq!(assembled.column_break_offsets, vec![vec![1, 3]]);
    }

    #[test]
    fn legacy_assembly_keeps_marker_only_column_break_paragraphs() {
        let units = [0x000E, PARA_MARK, b'X' as u16, PARA_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(assembled.blocks.len(), 2);
        assert_eq!(assembled.column_break_offsets, vec![vec![0], vec![]]);
        let Block::Paragraph(marker) = &assembled.blocks[0] else {
            panic!("marker block must be a paragraph");
        };
        assert_eq!(marker.text(), "\n");
    }

    #[test]
    fn legacy_assembly_aligns_table_cell_column_break_offsets() {
        let units = [
            b'A' as u16,
            0x000E,
            CELL_MARK,
            CELL_MARK,
            b'X' as u16,
            PARA_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries(&[
            (3, true, false, false),
            (4, true, true, true),
            (6, false, false, false),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert!(matches!(assembled.blocks[0], Block::Table(_)));
        assert!(matches!(assembled.blocks[1], Block::Paragraph(_)));
        assert_eq!(assembled.column_break_offsets, vec![vec![], vec![]]);
        #[cfg(feature = "docx")]
        assert_eq!(
            assembled.table_cell_column_break_offsets,
            vec![vec![vec![vec![vec![1]]]], Vec::new()]
        );
    }

    #[test]
    fn legacy_page_break_promotion_remaps_column_offsets() {
        let units = [
            b'A' as u16,
            0x000E,
            b'B' as u16,
            0x000C,
            b'C' as u16,
            0x000E,
            b'D' as u16,
            PARA_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let mut assembled = asm.finish_with_render_hints();
        assert_eq!(assembled.column_break_offsets, vec![vec![1, 5]]);
        assert_eq!(assembled.page_break_offsets, vec![vec![3]]);
        assembled.line_spacing_hints[0] = Some(LineSpacingHint::Exact(8.0));
        #[cfg(any(feature = "docx", feature = "render"))]
        {
            assembled.tab_stops[0] = vec![TabStop {
                position_pt: 54.0,
                alignment: crate::model::TabAlignment::Right,
                leader: crate::model::TabLeader::Dot,
            }];
        }

        let promoted = promote_legacy_manual_page_breaks(assembled);
        let [Block::Paragraph(before), Block::PageBreak, Block::Paragraph(after)] =
            promoted.blocks.as_slice()
        else {
            panic!("manual page break must split the paragraph");
        };
        assert_eq!(before.text(), "A\nB");
        assert_eq!(after.text(), "C\nD");
        assert_eq!(
            promoted.column_break_offsets,
            vec![vec![1], vec![], vec![1]]
        );
        assert_eq!(
            promoted.line_spacing_hints,
            vec![
                Some(LineSpacingHint::Exact(8.0)),
                None,
                Some(LineSpacingHint::Exact(8.0))
            ]
        );
        #[cfg(any(feature = "docx", feature = "render"))]
        assert_eq!(
            promoted.tab_stops,
            vec![
                vec![TabStop {
                    position_pt: 54.0,
                    alignment: crate::model::TabAlignment::Right,
                    leader: crate::model::TabLeader::Dot,
                }],
                vec![],
                vec![TabStop {
                    position_pt: 54.0,
                    alignment: crate::model::TabAlignment::Right,
                    leader: crate::model::TabLeader::Dot,
                }],
            ]
        );
        assert!(promoted.page_break_offsets.iter().all(Vec::is_empty));
    }

    #[test]
    fn legacy_page_break_promotion_keeps_repeated_marker_only_breaks() {
        let units = [0x000C, 0x000C, b'X' as u16, PARA_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let promoted = promote_legacy_manual_page_breaks(asm.finish_with_render_hints());

        assert!(matches!(promoted.blocks[0], Block::PageBreak));
        assert!(matches!(promoted.blocks[1], Block::PageBreak));
        let Block::Paragraph(after) = &promoted.blocks[2] else {
            panic!("text after repeated page breaks must remain a paragraph");
        };
        assert_eq!(after.text(), "X");
    }

    #[test]
    fn legacy_assembly_does_not_promote_table_cell_page_breaks() {
        let units = [
            b'A' as u16,
            0x000C,
            b'B' as u16,
            CELL_MARK,
            CELL_MARK,
            b'X' as u16,
            PARA_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries(&[
            (4, true, false, false),
            (5, true, true, true),
            (7, false, false, false),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        let Block::Table(table) = &assembled.blocks[0] else {
            panic!("first block must remain a table");
        };
        assert_eq!(table.rows[0].cells[0].text(), "A\nB");
        assert!(assembled.page_break_offsets.iter().all(Vec::is_empty));
    }

    #[test]
    fn legacy_assembly_maps_and_aligns_direct_paragraph_pagination() {
        let units = [
            b'A' as u16,
            PARA_MARK,
            b'B' as u16,
            PARA_MARK,
            b'C' as u16,
            PARA_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let first = crate::papx::ParagraphPagination {
            keep_next: true,
            keep_lines: true,
            page_break_before: true,
            widow_control: false,
        };
        let second = crate::papx::ParagraphPagination::default();
        let third = crate::papx::ParagraphPagination {
            widow_control: false,
            ..crate::papx::ParagraphPagination::default()
        };
        let papx = PapxTable::from_test_entries_with_pagination(&[
            (2, false, false, false, first),
            (4, false, false, false, second),
            (6, false, false, false, third),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(assembled.blocks.len(), 3);
        assert_eq!(
            assembled.pagination_hints,
            vec![
                PaginationHint {
                    keep_next: true,
                    keep_lines: true,
                    widow_control: false,
                },
                PaginationHint {
                    widow_control: true,
                    ..PaginationHint::default()
                },
                PaginationHint::default(),
            ]
        );
        assert!(assembled.table_cell_pagination.iter().all(Vec::is_empty));
        let Block::Paragraph(first_paragraph) = &assembled.blocks[0] else {
            panic!("first block must be a paragraph");
        };
        assert!(first_paragraph.props.page_break_before);
        let Block::Paragraph(second_paragraph) = &assembled.blocks[1] else {
            panic!("second block must be a paragraph");
        };
        assert!(!second_paragraph.props.page_break_before);
    }

    #[test]
    fn logical_indents_map_signed_edges_and_zero_by_direction() {
        let source = ParagraphIndentOverrides {
            logical_left_twips: Some(-720),
            logical_right_twips: Some(360),
            nest_twips: Some(-120),
            first_line_twips: Some(-360),
        };
        assert_eq!(
            resolve_paragraph_indent(source, false),
            Indent {
                left_pt: Some(-42.0),
                right_pt: Some(18.0),
                first_line_pt: None,
                hanging_pt: Some(18.0),
            }
        );
        assert_eq!(
            resolve_paragraph_indent(source, true),
            Indent {
                left_pt: Some(18.0),
                right_pt: Some(-42.0),
                first_line_pt: None,
                hanging_pt: Some(18.0),
            }
        );
        assert_eq!(
            resolve_paragraph_indent(
                ParagraphIndentOverrides {
                    logical_left_twips: Some(0),
                    logical_right_twips: Some(0),
                    nest_twips: Some(0),
                    first_line_twips: Some(0),
                },
                false,
            ),
            Indent::default()
        );
        assert_eq!(
            resolve_paragraph_indent(
                ParagraphIndentOverrides {
                    nest_twips: Some(120),
                    ..ParagraphIndentOverrides::default()
                },
                false,
            ),
            Indent::default()
        );
        let style = ParagraphIndentOverrides {
            logical_left_twips: Some(720),
            logical_right_twips: Some(1440),
            first_line_twips: Some(-360),
            ..ParagraphIndentOverrides::default()
        };
        let direct = ParagraphIndentOverrides {
            logical_left_twips: Some(1000),
            nest_twips: Some(120),
            first_line_twips: Some(240),
            ..ParagraphIndentOverrides::default()
        };
        assert_eq!(
            resolve_paragraph_indent(style.apply(direct), true),
            Indent {
                left_pt: Some(72.0),
                right_pt: Some(56.0),
                first_line_pt: Some(12.0),
                hanging_pt: None,
            }
        );
    }

    #[test]
    fn legacy_assembly_aligns_table_cell_paragraph_pagination() {
        let units = [b'A' as u16, PARA_MARK, b'B' as u16, CELL_MARK, CELL_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let first = crate::papx::ParagraphPagination {
            keep_next: true,
            page_break_before: true,
            widow_control: false,
            ..crate::papx::ParagraphPagination::default()
        };
        let second = crate::papx::ParagraphPagination {
            keep_lines: true,
            ..crate::papx::ParagraphPagination::default()
        };
        let papx = PapxTable::from_test_entries_with_pagination(&[
            (2, true, false, false, first),
            (4, true, false, false, second),
            (
                5,
                true,
                true,
                false,
                crate::papx::ParagraphPagination::default(),
            ),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(assembled.blocks.len(), 1);
        assert_eq!(
            assembled.table_cell_pagination,
            vec![vec![vec![vec![
                Some(PaginationHint {
                    keep_next: true,
                    widow_control: false,
                    ..PaginationHint::default()
                }),
                Some(PaginationHint {
                    keep_lines: true,
                    widow_control: true,
                    ..PaginationHint::default()
                }),
            ]]]]
        );
        let Block::Table(table) = &assembled.blocks[0] else {
            panic!("assembled block must be a table");
        };
        let Block::Paragraph(first_paragraph) = &table.rows[0].cells[0].blocks[0] else {
            panic!("first cell block must be a paragraph");
        };
        assert!(first_paragraph.props.page_break_before);
    }

    #[test]
    fn legacy_assembly_resolves_table_cell_style_pagination_before_direct_overrides() {
        let units = [b'A' as u16, PARA_MARK, b'B' as u16, CELL_MARK, CELL_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries_with_style_pagination(&[
            (
                2,
                true,
                false,
                false,
                1,
                crate::papx::ParagraphPaginationOverrides {
                    keep_next: Some(false),
                    page_break_before: Some(false),
                    ..Default::default()
                },
            ),
            (4, true, false, false, 1, Default::default()),
            (5, true, true, false, 0, Default::default()),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::from_test_pagination(vec![
            crate::papx::ParagraphPagination::default(),
            crate::papx::ParagraphPagination {
                keep_next: true,
                keep_lines: true,
                page_break_before: true,
                widow_control: false,
            },
        ]);
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(
            assembled.table_cell_pagination,
            vec![vec![vec![vec![
                Some(PaginationHint {
                    keep_next: false,
                    keep_lines: true,
                    widow_control: false,
                }),
                Some(PaginationHint {
                    keep_next: true,
                    keep_lines: true,
                    widow_control: false,
                }),
            ]]]]
        );
        let Block::Table(table) = &assembled.blocks[0] else {
            panic!("assembled block must be a table");
        };
        let Block::Paragraph(first) = &table.rows[0].cells[0].blocks[0] else {
            panic!("first cell block must be a paragraph");
        };
        let Block::Paragraph(second) = &table.rows[0].cells[0].blocks[1] else {
            panic!("second cell block must be a paragraph");
        };
        assert!(!first.props.page_break_before);
        assert!(second.props.page_break_before);
    }

    #[test]
    fn dangling_legacy_row_defaults_to_splittable() {
        let units = [b'A' as u16, CELL_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries(&[(2, true, false, true)]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(assembled.table_row_pagination.len(), 1);
        assert_eq!(assembled.table_row_pagination[0].len(), 1);
        assert!(!assembled.table_row_pagination[0][0].cant_split);
    }

    #[test]
    fn legacy_row_pagination_stays_aligned_across_separated_tables() {
        let units = [
            b'A' as u16,
            CELL_MARK,
            CELL_MARK,
            b'X' as u16,
            PARA_MARK,
            b'B' as u16,
            CELL_MARK,
            CELL_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries(&[
            (2, true, false, false),
            (3, true, true, true),
            (5, false, false, false),
            (7, true, false, false),
            (8, true, true, false),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert!(matches!(
            assembled.blocks.as_slice(),
            [Block::Table(_), Block::Paragraph(_), Block::Table(_)]
        ));
        assert_eq!(assembled.table_row_pagination.len(), 3);
        assert!(assembled.table_row_pagination[0][0].cant_split);
        assert!(assembled.table_row_pagination[1].is_empty());
        assert!(!assembled.table_row_pagination[2][0].cant_split);
    }

    #[test]
    fn legacy_table_direction_splits_rows_without_misaligning_sidecars() {
        let units = [
            b'A' as u16,
            CELL_MARK,
            CELL_MARK,
            b'B' as u16,
            CELL_MARK,
            CELL_MARK,
            b'C' as u16,
            CELL_MARK,
            CELL_MARK,
        ];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries_with_table_bidi(&[
            (2, true, false, false, false),
            (3, true, true, true, true),
            (5, true, false, false, false),
            (6, true, true, false, true),
            (8, true, false, false, false),
            (9, true, true, true, false),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        assert_eq!(assembled.blocks.len(), 2);
        let Block::Table(first) = &assembled.blocks[0] else {
            panic!("first block must be a table");
        };
        let Block::Table(second) = &assembled.blocks[1] else {
            panic!("second block must be a table");
        };
        assert!(first.bidi_visual);
        assert!(!second.bidi_visual);
        assert_eq!(first.rows.len(), 2);
        assert_eq!(second.rows.len(), 1);
        assert_eq!(
            assembled.table_row_pagination,
            vec![
                vec![
                    TableRowPaginationHint { cant_split: true },
                    TableRowPaginationHint { cant_split: false },
                ],
                vec![TableRowPaginationHint { cant_split: true }],
            ]
        );
        assert_eq!(assembled.table_cell_pagination[0].len(), 2);
        assert_eq!(assembled.table_cell_pagination[1].len(), 1);
    }

    #[test]
    fn dangling_legacy_row_inherits_the_active_table_direction() {
        let units = [b'A' as u16, CELL_MARK, CELL_MARK, b'B' as u16, CELL_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let papx = PapxTable::from_test_entries_with_table_bidi(&[
            (2, true, false, false, false),
            (3, true, true, false, true),
            (5, true, false, false, false),
        ]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run(&units, &fcs);
        let assembled = asm.finish_with_render_hints();

        let [Block::Table(table)] = assembled.blocks.as_slice() else {
            panic!("rows must remain one table");
        };
        assert!(table.bidi_visual);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(assembled.table_row_pagination[0].len(), 2);
        assert_eq!(assembled.table_cell_pagination[0].len(), 2);
    }

    #[test]
    fn legacy_page_number_format_maps_bounded_msonfc_subset() {
        let exact = [
            (0x00, PageNumberFormat::Decimal),
            (0x01, PageNumberFormat::UpperRoman),
            (0x02, PageNumberFormat::LowerRoman),
            (0x03, PageNumberFormat::UpperLetter),
            (0x04, PageNumberFormat::LowerLetter),
            (0x05, PageNumberFormat::Ordinal),
            (0x06, PageNumberFormat::CardinalText),
            (0x07, PageNumberFormat::OrdinalText),
            (0x0E, PageNumberFormat::DecimalFullWidth),
            (0x0F, PageNumberFormat::DecimalHalfWidth),
            (0x12, PageNumberFormat::DecimalEnclosedCircle),
            (0x13, PageNumberFormat::DecimalFullWidth2),
            (0x16, PageNumberFormat::DecimalZero),
            (0x18, PageNumberFormat::Ganada),
            (0x19, PageNumberFormat::Chosung),
            (0x1A, PageNumberFormat::DecimalEnclosedFullstop),
            (0x1B, PageNumberFormat::DecimalEnclosedParen),
            (0x28, PageNumberFormat::Decimal),
            (0x29, PageNumberFormat::KoreanDigital),
            (0x2A, PageNumberFormat::KoreanCounting),
            (0x2B, PageNumberFormat::KoreanLegal),
            (0x2C, PageNumberFormat::KoreanDigital2),
            (0x39, PageNumberFormat::NumberInDash),
        ];

        for (nfc, expected) in exact {
            assert_eq!(legacy_page_number_format(nfc), Some(expected));
        }
        for fallback in [0x08, 0x0A, 0x17, 0x2D, 0x3A, 0x3B, 0xFF] {
            assert_eq!(
                legacy_page_number_format(fallback),
                Some(PageNumberFormat::Decimal)
            );
        }
        for invalid in [0x3C, 0x7F, 0xFE] {
            assert_eq!(legacy_page_number_format(invalid), None);
        }
    }

    #[test]
    fn legacy_sepx_scanner_keeps_last_valid_values() {
        let mut grpprl = vec![
            0x00, 0xC0, 0x02, 0xAA, 0xBB, // unknown variable-length sprm
            0x1D, 0x30, 0x02, // landscape
        ];
        for value in [12_240u16, 15_840, 100] {
            grpprl.extend_from_slice(&SPRM_S_XA_PAGE.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        }
        grpprl.extend_from_slice(&SPRM_S_DXA_LEFT.to_le_bytes());
        grpprl.extend_from_slice(&0u16.to_le_bytes());
        for value in [1_440i16, -720] {
            grpprl.extend_from_slice(&SPRM_S_DYA_TOP.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        }
        grpprl.extend_from_slice(&SPRM_S_B_ORIENTATION.to_le_bytes());
        grpprl.push(0);

        let properties = scan_legacy_section_grpprl(&grpprl).unwrap();

        assert_eq!(properties.page.width_pt, 792.0);
        assert_eq!(properties.page.margin_left_pt, Some(0.0));
        assert_eq!(properties.page.margin_top_pt, Some(72.0));
        assert!(properties.page.landscape);
        assert_eq!(properties.section_break, SectionBreakKind::NextPage);

        let push_break = |grpprl: &mut Vec<u8>, value| {
            grpprl.extend_from_slice(&SPRM_S_BKC.to_le_bytes());
            grpprl.push(value);
        };
        push_break(&mut grpprl, 3);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().section_break,
            SectionBreakKind::EvenPage
        );
        push_break(&mut grpprl, 5);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().section_break,
            SectionBreakKind::EvenPage
        );
        push_break(&mut grpprl, 2);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().section_break,
            SectionBreakKind::NextPage
        );
        push_break(&mut grpprl, 3);
        push_break(&mut grpprl, 1);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().section_break,
            SectionBreakKind::NextPage
        );
        push_break(&mut grpprl, 4);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().section_break,
            SectionBreakKind::OddPage
        );
        push_break(&mut grpprl, 0);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().section_break,
            SectionBreakKind::NextPage
        );
    }

    #[test]
    fn legacy_sepx_scanner_preserves_bounded_running_surface_distances() {
        let mut grpprl = Vec::new();
        let push_distance = |grpprl: &mut Vec<u8>, sprm: u16, value: u16| {
            grpprl.extend_from_slice(&sprm.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };

        let defaults = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(defaults.header_distance_pt, None);
        assert_eq!(defaults.footer_distance_pt, None);

        push_distance(&mut grpprl, SPRM_S_DYA_HDR_TOP, 0);
        push_distance(&mut grpprl, SPRM_S_DYA_HDR_BOTTOM, 31_680);
        let bounds = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(bounds.header_distance_pt, Some(0.0));
        assert_eq!(bounds.footer_distance_pt, Some(1_584.0));

        push_distance(&mut grpprl, SPRM_S_DYA_HDR_TOP, 720);
        push_distance(&mut grpprl, SPRM_S_DYA_HDR_BOTTOM, 1_440);
        for invalid in [31_681, u16::MAX] {
            push_distance(&mut grpprl, SPRM_S_DYA_HDR_TOP, invalid);
            push_distance(&mut grpprl, SPRM_S_DYA_HDR_BOTTOM, invalid);
        }
        let last_valid = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(last_valid.header_distance_pt, Some(36.0));
        assert_eq!(last_valid.footer_distance_pt, Some(72.0));

        push_distance(&mut grpprl, SPRM_S_DYA_HDR_TOP, 2_000);
        push_distance(&mut grpprl, SPRM_S_DYA_HDR_BOTTOM, 3_000);
        let replaced = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(replaced.header_distance_pt, Some(100.0));
        assert_eq!(replaced.footer_distance_pt, Some(150.0));
    }

    #[test]
    fn legacy_sepx_scanner_preserves_bounded_column_counts() {
        let mut grpprl = Vec::new();
        let push_columns = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&SPRM_S_C_COLUMNS.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };

        assert_eq!(scan_legacy_section_grpprl(&grpprl).unwrap().columns, None);
        push_columns(&mut grpprl, 44);
        assert_eq!(scan_legacy_section_grpprl(&grpprl).unwrap().columns, None);
        push_columns(&mut grpprl, 0u16);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().columns,
            Some(1)
        );
        push_columns(&mut grpprl, 43);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().columns,
            Some(44)
        );
        for invalid in [44, u16::MAX] {
            push_columns(&mut grpprl, invalid);
            assert_eq!(
                scan_legacy_section_grpprl(&grpprl).unwrap().columns,
                Some(44)
            );
        }
    }

    #[test]
    fn legacy_sepx_scanner_applies_strict_equal_spacing_in_source_order() {
        let mut grpprl = Vec::new();
        let push_columns = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&SPRM_S_C_COLUMNS.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };
        let push_evenly_spaced = |grpprl: &mut Vec<u8>, value| {
            grpprl.extend_from_slice(&SPRM_S_F_EVENLY_SPACED.to_le_bytes());
            grpprl.push(value);
        };

        push_columns(&mut grpprl, 2u16);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().columns,
            Some(3)
        );
        push_evenly_spaced(&mut grpprl, 2);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().columns,
            Some(3)
        );
        push_evenly_spaced(&mut grpprl, 0);
        assert_eq!(scan_legacy_section_grpprl(&grpprl).unwrap().columns, None);
        push_columns(&mut grpprl, 4);
        push_evenly_spaced(&mut grpprl, 2);
        assert_eq!(scan_legacy_section_grpprl(&grpprl).unwrap().columns, None);
        push_evenly_spaced(&mut grpprl, 1);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().columns,
            Some(5)
        );
    }

    #[test]
    fn legacy_sepx_scanner_applies_column_separator_bool8_in_source_order() {
        let mut grpprl = Vec::new();
        let push_separator = |grpprl: &mut Vec<u8>, value| {
            grpprl.extend_from_slice(&SPRM_S_L_BETWEEN.to_le_bytes());
            grpprl.push(value);
        };

        assert!(
            !scan_legacy_section_grpprl(&grpprl)
                .unwrap()
                .column_separator
        );
        push_separator(&mut grpprl, 1);
        assert!(
            scan_legacy_section_grpprl(&grpprl)
                .unwrap()
                .column_separator
        );
        push_separator(&mut grpprl, 2);
        assert!(
            scan_legacy_section_grpprl(&grpprl)
                .unwrap()
                .column_separator
        );
        push_separator(&mut grpprl, 0);
        assert!(
            !scan_legacy_section_grpprl(&grpprl)
                .unwrap()
                .column_separator
        );
        push_separator(&mut grpprl, u8::MAX);
        assert!(
            !scan_legacy_section_grpprl(&grpprl)
                .unwrap()
                .column_separator
        );
        push_separator(&mut grpprl, 1);
        assert!(
            scan_legacy_section_grpprl(&grpprl)
                .unwrap()
                .column_separator
        );
    }

    #[test]
    fn legacy_sepx_scanner_applies_section_bidi_bool8_in_source_order() {
        let mut grpprl = Vec::new();
        let push_bidi = |grpprl: &mut Vec<u8>, value| {
            grpprl.extend_from_slice(&SPRM_S_F_BIDI.to_le_bytes());
            grpprl.push(value);
        };

        assert!(!scan_legacy_section_grpprl(&grpprl).unwrap().column_rtl);
        push_bidi(&mut grpprl, 1);
        assert!(scan_legacy_section_grpprl(&grpprl).unwrap().column_rtl);
        push_bidi(&mut grpprl, 2);
        assert!(scan_legacy_section_grpprl(&grpprl).unwrap().column_rtl);
        push_bidi(&mut grpprl, 0);
        assert!(!scan_legacy_section_grpprl(&grpprl).unwrap().column_rtl);
        push_bidi(&mut grpprl, u8::MAX);
        assert!(!scan_legacy_section_grpprl(&grpprl).unwrap().column_rtl);
        push_bidi(&mut grpprl, 1);
        assert!(scan_legacy_section_grpprl(&grpprl).unwrap().column_rtl);
    }

    #[test]
    fn legacy_sepx_scanner_recovers_complete_unequal_column_count() {
        let mut grpprl = Vec::new();
        push_test_section_column_count(&mut grpprl, 1);
        for (index, width_twips) in [(0, 2_000u16), (1, 4_000)] {
            push_test_section_indexed_column_value(
                &mut grpprl,
                SPRM_S_DXA_COL_WIDTH,
                index,
                width_twips,
            );
        }
        push_test_section_evenly_spaced(&mut grpprl, 0);

        let properties = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(properties.columns, Some(2));
        assert_eq!(
            properties.column_layout,
            Some(SectionColumnLayoutHints {
                columns: vec![
                    SectionColumnHint {
                        width_pt: 100.0,
                        space_after_pt: 0.0,
                    },
                    SectionColumnHint {
                        width_pt: 200.0,
                        space_after_pt: 0.0,
                    },
                ],
            })
        );
        assert_eq!(properties.column_gap_pt, None);
    }

    #[test]
    fn legacy_sepx_scanner_bounds_custom_columns_and_keeps_last_valid_values() {
        let mut grpprl = Vec::new();
        push_test_section_column_count(&mut grpprl, 1);
        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_WIDTH, 0, 718);
        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_WIDTH, 1, 31_680);
        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_SPACING, 0, 31_680);
        push_test_section_evenly_spaced(&mut grpprl, 0);

        let bounded = scan_legacy_section_grpprl(&grpprl).unwrap();
        let bounded_layout = bounded.column_layout.unwrap();
        assert_eq!(bounded_layout.columns[0].width_pt, 35.9);
        assert_eq!(bounded_layout.columns[0].space_after_pt, 1_584.0);
        assert_eq!(bounded_layout.columns[1].width_pt, 1_584.0);

        for (sprm, index, value) in [
            (SPRM_S_DXA_COL_WIDTH, 0, 717),
            (SPRM_S_DXA_COL_WIDTH, 1, 31_681),
            (SPRM_S_DXA_COL_WIDTH, 44, 2_000),
            (SPRM_S_DXA_COL_SPACING, 0, 31_681),
            (SPRM_S_DXA_COL_SPACING, 44, 720),
        ] {
            push_test_section_indexed_column_value(&mut grpprl, sprm, index, value);
        }
        let retained = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(retained.column_layout, Some(bounded_layout));

        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_WIDTH, 0, 1_440);
        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_SPACING, 0, 720);
        let replaced = scan_legacy_section_grpprl(&grpprl).unwrap();
        let replaced_layout = replaced.column_layout.unwrap();
        assert_eq!(replaced_layout.columns[0].width_pt, 72.0);
        assert_eq!(replaced_layout.columns[0].space_after_pt, 36.0);
    }

    #[test]
    fn legacy_sepx_scanner_requires_complete_final_custom_geometry() {
        let mut grpprl = Vec::new();
        push_test_section_column_count(&mut grpprl, 1);
        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_WIDTH, 0, 2_000);
        push_test_section_evenly_spaced(&mut grpprl, 0);

        let incomplete = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(incomplete.columns, None);
        assert_eq!(incomplete.column_layout, None);

        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_WIDTH, 1, 4_000);
        let complete = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(complete.columns, Some(2));
        assert_eq!(
            complete.column_layout.unwrap().columns[0].space_after_pt,
            0.0
        );

        push_test_section_evenly_spaced(&mut grpprl, 1);
        grpprl.extend_from_slice(&SPRM_S_DXA_COLUMNS.to_le_bytes());
        grpprl.extend_from_slice(&800u16.to_le_bytes());
        let equal = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(equal.columns, Some(2));
        assert_eq!(equal.column_gap_pt, Some(40.0));
        assert_eq!(equal.column_layout, None);

        push_test_section_evenly_spaced(&mut grpprl, 0);
        push_test_section_column_count(&mut grpprl, 2);
        let changed_count = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(changed_count.columns, None);
        assert_eq!(changed_count.column_layout, None);

        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_WIDTH, 2, 3_000);
        push_test_section_evenly_spaced(&mut grpprl, 2);
        let completed_changed_count = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(completed_changed_count.columns, Some(3));
        assert_eq!(
            completed_changed_count.column_layout.unwrap().columns.len(),
            3
        );

        push_test_section_column_count(&mut grpprl, 0);
        let single = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(single.columns, Some(1));
        assert_eq!(single.column_layout, None);
    }

    #[test]
    fn legacy_sepx_scanner_accepts_the_exact_custom_column_cap() {
        let mut grpprl = Vec::new();
        push_test_section_column_count(&mut grpprl, 43);
        for index in 0..44 {
            push_test_section_indexed_column_value(
                &mut grpprl,
                SPRM_S_DXA_COL_WIDTH,
                index,
                718 + u16::from(index),
            );
        }
        push_test_section_indexed_column_value(&mut grpprl, SPRM_S_DXA_COL_SPACING, 43, 1_440);
        push_test_section_evenly_spaced(&mut grpprl, 0);

        let capped = scan_legacy_section_grpprl(&grpprl).unwrap();
        let capped_layout = capped.column_layout.unwrap();
        assert_eq!(capped.columns, Some(44));
        assert_eq!(capped_layout.columns.len(), 44);
        assert_eq!(capped_layout.columns[43].width_pt, twips_to_points(761));
        assert_eq!(capped_layout.columns[43].space_after_pt, 0.0);

        push_test_section_column_count(&mut grpprl, 44);
        let invalid_count = scan_legacy_section_grpprl(&grpprl).unwrap();
        assert_eq!(invalid_count.columns, Some(44));
        assert_eq!(invalid_count.column_layout, Some(capped_layout));
    }

    #[test]
    fn legacy_sepx_scanner_preserves_bounded_equal_column_spacing() {
        let mut grpprl = Vec::new();
        let push_columns = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&SPRM_S_C_COLUMNS.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };
        let push_spacing = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&SPRM_S_DXA_COLUMNS.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };
        let push_evenly_spaced = |grpprl: &mut Vec<u8>, value| {
            grpprl.extend_from_slice(&SPRM_S_F_EVENLY_SPACED.to_le_bytes());
            grpprl.push(value);
        };

        push_spacing(&mut grpprl, 720);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().column_gap_pt,
            None
        );

        push_columns(&mut grpprl, 1);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().column_gap_pt,
            Some(36.0)
        );

        for invalid in [31_681, u16::MAX] {
            push_spacing(&mut grpprl, invalid);
            assert_eq!(
                scan_legacy_section_grpprl(&grpprl).unwrap().column_gap_pt,
                Some(36.0)
            );
        }

        push_evenly_spaced(&mut grpprl, 0);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().column_gap_pt,
            None
        );
        push_evenly_spaced(&mut grpprl, 1);
        push_spacing(&mut grpprl, 0);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().column_gap_pt,
            Some(0.0)
        );

        push_columns(&mut grpprl, 0);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().column_gap_pt,
            None
        );
    }

    #[test]
    fn legacy_sepx_scanner_preserves_bounded_document_grid_state() {
        let mut grpprl = Vec::new();
        let push_mode = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&SPRM_S_CLM.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };
        let push_line_pitch = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&SPRM_S_DYA_LINE_PITCH.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };
        let push_character_space = |grpprl: &mut Vec<u8>, value: i32| {
            grpprl.extend_from_slice(&SPRM_S_DXT_CHAR_SPACE.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };

        push_mode(&mut grpprl, 1);
        push_character_space(&mut grpprl, 40_960);
        assert_eq!(scan_legacy_section_grpprl(&grpprl).unwrap().doc_grid, None);

        push_line_pitch(&mut grpprl, 360);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().doc_grid,
            Some(DocGrid {
                grid_type: DocGridType::LinesAndChars,
                line_pitch: Some(360),
                character_space: Some(40_960),
            })
        );

        push_mode(&mut grpprl, 4);
        push_line_pitch(&mut grpprl, 0);
        push_line_pitch(&mut grpprl, 31_681);
        push_character_space(&mut grpprl, 6_488_065);
        push_character_space(&mut grpprl, -670_926);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().doc_grid,
            Some(DocGrid {
                grid_type: DocGridType::LinesAndChars,
                line_pitch: Some(360),
                character_space: Some(40_960),
            })
        );

        push_character_space(&mut grpprl, -4_096);
        push_line_pitch(&mut grpprl, 720);
        push_mode(&mut grpprl, 3);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().doc_grid,
            Some(DocGrid {
                grid_type: DocGridType::SnapToChars,
                line_pitch: Some(720),
                character_space: None,
            })
        );

        push_mode(&mut grpprl, 0);
        assert_eq!(scan_legacy_section_grpprl(&grpprl).unwrap().doc_grid, None);

        let mut lines_only = Vec::new();
        push_mode(&mut lines_only, 2);
        push_line_pitch(&mut lines_only, 480);
        assert_eq!(
            scan_legacy_section_grpprl(&lines_only).unwrap().doc_grid,
            Some(DocGrid {
                grid_type: DocGridType::Lines,
                line_pitch: Some(480),
                character_space: None,
            })
        );
    }

    #[test]
    fn legacy_sepx_scanner_preserves_bounded_text_direction_state() {
        use crate::model::TextDirection;

        let push_text_flow = |grpprl: &mut Vec<u8>, value: u16| {
            grpprl.extend_from_slice(&0x5033u16.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        };
        let expected = [
            TextDirection::LeftToRightTopToBottom,
            TextDirection::TopToBottomRightToLeft,
            TextDirection::BottomToTopLeftToRight,
            TextDirection::TopToBottomRightToLeftVertical,
            TextDirection::LeftToRightTopToBottomVertical,
            TextDirection::TopToBottomLeftToRightVertical,
        ];

        assert_eq!(
            scan_legacy_section_grpprl(&[]).unwrap().text_direction,
            None
        );
        for (value, expected) in expected.into_iter().enumerate() {
            let mut grpprl = Vec::new();
            push_text_flow(&mut grpprl, value as u16);
            assert_eq!(
                scan_legacy_section_grpprl(&grpprl).unwrap().text_direction,
                Some(expected)
            );
        }

        let mut grpprl = Vec::new();
        push_text_flow(&mut grpprl, 1);
        push_text_flow(&mut grpprl, 6);
        push_text_flow(&mut grpprl, u16::MAX);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().text_direction,
            Some(TextDirection::TopToBottomRightToLeft)
        );
        push_text_flow(&mut grpprl, 5);
        assert_eq!(
            scan_legacy_section_grpprl(&grpprl).unwrap().text_direction,
            Some(TextDirection::TopToBottomLeftToRightVertical)
        );
    }

    #[test]
    fn legacy_sepx_parser_rejects_malformed_payloads() {
        assert!(scan_legacy_section_grpprl(&[0x1D]).is_none());
        assert!(scan_legacy_section_grpprl(&[0x00, 0xC0, 0x02, 0xAA]).is_none());
        let [text_flow_lo, text_flow_hi] = SPRM_S_TEXT_FLOW.to_le_bytes();
        assert!(scan_legacy_section_grpprl(&[text_flow_lo, text_flow_hi, 0x01]).is_none());
        assert!(parse_legacy_sepx_section_properties(&(-1i16).to_le_bytes(), 0).is_none());

        let truncated = [4, 0, 0x1D, 0x30, 0x01];
        assert!(parse_legacy_sepx_section_properties(&truncated, 0).is_none());
        for fc_sepx in [-1, 0, i32::MAX] {
            let properties = legacy_sepx_section_properties_at(&truncated, fc_sepx);
            assert_eq!(properties.page.width_pt, 612.0);
            assert_eq!(properties.page.height_pt, 792.0);
            assert!(!properties.page.landscape);
            assert_eq!(properties.columns, None);
            assert_eq!(properties.text_direction, None);
            assert_eq!(properties.section_break, SectionBreakKind::NextPage);
        }
    }

    #[test]
    fn legacy_section_break_keeps_row_pagination_sidecar_aligned() {
        let units = [b'A' as u16, PARA_MARK, b'B' as u16, PARA_MARK];
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let prms = [0; 4];
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let source = LegacySource {
            units: &units,
            fcs: &fcs,
            prms: &prms,
            prm1_patches: &[],
            papx: &papx,
            chpx: &chpx,
            stylesheet: &stsh,
            data: &[],
            fonts: &[],
        };
        let mut output = LegacyRegionOutput::default();

        push_legacy_main_section_regions(
            &source,
            &mut numberer,
            &mut output,
            0,
            &[
                LegacySectionSpan {
                    start_cp: 0,
                    end_cp: 2,
                    page: PageSetup::default(),
                    columns: Some(2),
                    column_gap_pt: None,
                    column_layout: None,
                    column_separator: false,
                    column_rtl: false,
                    title_page: false,
                    page_number_start: None,
                    page_number_format: None,
                    text_direction: None,
                    doc_grid: None,
                    header_distance_pt: None,
                    footer_distance_pt: None,
                    section_break: SectionBreakKind::NextPage,
                },
                LegacySectionSpan {
                    start_cp: 2,
                    end_cp: 4,
                    page: PageSetup::default(),
                    columns: Some(3),
                    column_gap_pt: None,
                    column_layout: None,
                    column_separator: false,
                    column_rtl: false,
                    title_page: false,
                    page_number_start: None,
                    page_number_format: None,
                    text_direction: None,
                    doc_grid: None,
                    header_distance_pt: None,
                    footer_distance_pt: None,
                    section_break: SectionBreakKind::NextPage,
                },
            ],
        );

        assert!(matches!(
            output.blocks.as_slice(),
            [
                Block::Paragraph(_),
                Block::SectionBreak(_),
                Block::Paragraph(_)
            ]
        ));
        let Block::SectionBreak(setup) = &output.blocks[1] else {
            unreachable!()
        };
        assert_eq!(setup.columns, Some(2));
        assert_eq!(output.table_row_pagination.len(), output.blocks.len());
        assert!(output.table_row_pagination.iter().all(Vec::is_empty));
        assert_eq!(output.pagination_hints.len(), output.blocks.len());
        assert!(output.pagination_hints[0].widow_control);
        assert_eq!(output.pagination_hints[1], PaginationHint::default());
        assert!(output.pagination_hints[2].widow_control);
        assert_eq!(output.table_cell_pagination.len(), output.blocks.len());
        assert!(output.table_cell_pagination.iter().all(Vec::is_empty));
    }

    fn all_text(blocks: &[Block]) -> String {
        blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some(p.text()),
                _ => None,
            })
            .collect()
    }

    fn us(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    fn minimal_fib_for_header_footer(ccp_hdd: u32, lcb_plcf_hdd: usize) -> Fib {
        Fib {
            nfib: 0x00D9,
            lid: 0x0409,
            encrypted: false,
            obfuscated: false,
            complex: false,
            which_table_stream_one: false,
            fc_clx: 0,
            lcb_clx: 0,
            fc_plcf_bte_papx: 0,
            lcb_plcf_bte_papx: 0,
            fc_plcf_bte_chpx: 0,
            lcb_plcf_bte_chpx: 0,
            fc_stshf: 0,
            lcb_stshf: 0,
            fc_sttbf_ffn: 0,
            lcb_sttbf_ffn: 0,
            fc_plf_lst: 0,
            lcb_plf_lst: 0,
            fc_plf_lfo: 0,
            lcb_plf_lfo: 0,
            fc_plcf_hdd: 0,
            lcb_plcf_hdd,
            fc_plcfand_ref: 0,
            lcb_plcfand_ref: 0,
            fc_grp_xst_atn_owners: 0,
            lcb_grp_xst_atn_owners: 0,
            ccp_text: 0,
            ccp_ftn: 0,
            ccp_hdd,
            ccp_atn: 0,
            ccp_edn: 0,
            ccp_txbx: 0,
        }
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_running_line_spacing_maps_six_story_slots_per_section() {
        let mut blocks = vec![
            Block::PageBreak,
            Block::SectionBreak(SectionSetup::default()),
            Block::PageBreak,
        ];
        blocks.extend((0..12).map(|_| Block::PageBreak));
        let mut line_spacing = vec![None; blocks.len()];
        let mut table_cell_line_spacing = vec![Vec::new(); blocks.len()];
        let mut regions = Vec::new();
        for story_offset in 0..12 {
            let block_index = story_offset + 3;
            line_spacing[block_index] = Some(LineSpacingHint::Exact((story_offset + 1) as f32));
            table_cell_line_spacing[block_index] = vec![vec![vec![Some(
                LineSpacingHint::AtLeast((story_offset + 101) as f32),
            )]]];
            regions.push(SourceRegion {
                kind: SourceRegionKind::HeaderFooter,
                source_story_index: Some(HEADER_FOOTER_STORY_BASE + story_offset),
                block_start: block_index,
                block_end: block_index + 1,
                source_start_cp: story_offset,
                source_len_cp: 1,
                text_start: 0,
                text_len: 0,
            });
        }

        let mapped = legacy_running_line_spacing_from_regions(
            &blocks,
            &regions,
            &line_spacing,
            &table_cell_line_spacing,
        );
        let values = |hints: &RunningSurfaceLineSpacingHints| {
            [
                hints.even_header.first().copied().flatten(),
                hints.header.first().copied().flatten(),
                hints.even_footer.first().copied().flatten(),
                hints.footer.first().copied().flatten(),
                hints.first_header.first().copied().flatten(),
                hints.first_footer.first().copied().flatten(),
            ]
        };
        let table_values = |hints: &RunningSurfaceLineSpacingHints| {
            [
                &hints.even_header_table_cells,
                &hints.header_table_cells,
                &hints.even_footer_table_cells,
                &hints.footer_table_cells,
                &hints.first_header_table_cells,
                &hints.first_footer_table_cells,
            ]
            .map(|blocks| blocks[0][0][0][0])
        };

        assert_eq!(mapped.len(), 2);
        assert_eq!(
            values(&mapped[0]),
            [
                Some(LineSpacingHint::Exact(1.0)),
                Some(LineSpacingHint::Exact(2.0)),
                Some(LineSpacingHint::Exact(3.0)),
                Some(LineSpacingHint::Exact(4.0)),
                Some(LineSpacingHint::Exact(5.0)),
                Some(LineSpacingHint::Exact(6.0)),
            ]
        );
        assert_eq!(
            values(&mapped[1]),
            [
                Some(LineSpacingHint::Exact(7.0)),
                Some(LineSpacingHint::Exact(8.0)),
                Some(LineSpacingHint::Exact(9.0)),
                Some(LineSpacingHint::Exact(10.0)),
                Some(LineSpacingHint::Exact(11.0)),
                Some(LineSpacingHint::Exact(12.0)),
            ]
        );
        assert_eq!(
            table_values(&mapped[0]),
            [
                Some(LineSpacingHint::AtLeast(101.0)),
                Some(LineSpacingHint::AtLeast(102.0)),
                Some(LineSpacingHint::AtLeast(103.0)),
                Some(LineSpacingHint::AtLeast(104.0)),
                Some(LineSpacingHint::AtLeast(105.0)),
                Some(LineSpacingHint::AtLeast(106.0)),
            ]
        );
        assert_eq!(
            table_values(&mapped[1]),
            [
                Some(LineSpacingHint::AtLeast(107.0)),
                Some(LineSpacingHint::AtLeast(108.0)),
                Some(LineSpacingHint::AtLeast(109.0)),
                Some(LineSpacingHint::AtLeast(110.0)),
                Some(LineSpacingHint::AtLeast(111.0)),
                Some(LineSpacingHint::AtLeast(112.0)),
            ]
        );
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_running_line_spacing_repeats_unindexed_header_fallback() {
        let blocks = vec![
            Block::SectionBreak(SectionSetup::default()),
            Block::PageBreak,
        ];
        let regions = vec![SourceRegion {
            kind: SourceRegionKind::HeaderFooter,
            source_story_index: None,
            block_start: 1,
            block_end: 2,
            source_start_cp: 0,
            source_len_cp: 1,
            text_start: 0,
            text_len: 0,
        }];
        let line_spacing = vec![None, Some(LineSpacingHint::AtLeast(24.0))];
        let table_cell_line_spacing = vec![
            Vec::new(),
            vec![vec![vec![Some(LineSpacingHint::Exact(7.0))]]],
        ];

        let mapped = legacy_running_line_spacing_from_regions(
            &blocks,
            &regions,
            &line_spacing,
            &table_cell_line_spacing,
        );

        assert_eq!(mapped.len(), 2);
        for section in mapped {
            assert_eq!(section.header, vec![Some(LineSpacingHint::AtLeast(24.0))]);
            assert_eq!(
                section.header_table_cells,
                vec![vec![vec![vec![Some(LineSpacingHint::Exact(7.0))]]]]
            );
            assert!(section.even_header.is_empty());
            assert!(section.even_header_table_cells.is_empty());
            assert!(section.first_header.is_empty());
            assert!(section.first_header_table_cells.is_empty());
            assert!(section.footer.is_empty());
            assert!(section.footer_table_cells.is_empty());
            assert!(section.even_footer.is_empty());
            assert!(section.even_footer_table_cells.is_empty());
            assert!(section.first_footer.is_empty());
            assert!(section.first_footer_table_cells.is_empty());
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_running_column_breaks_map_six_story_slots_per_section_and_keep_first_owner() {
        let blocks = vec![
            Block::PageBreak,
            Block::SectionBreak(SectionSetup::default()),
            Block::PageBreak,
        ];
        let mut regions = (0..12)
            .map(|story_offset| LegacyRunningColumnBreakRegion {
                source_story_index: Some(HEADER_FOOTER_STORY_BASE + story_offset),
                column_break_offsets: vec![vec![story_offset + 1]],
            })
            .collect::<Vec<_>>();
        regions.push(LegacyRunningColumnBreakRegion {
            source_story_index: Some(HEADER_FOOTER_STORY_BASE),
            column_break_offsets: vec![vec![999]],
        });

        let mapped = legacy_running_column_breaks_from_regions(&blocks, &regions);
        let values = |hints: &RunningSurfaceColumnBreakHints| {
            [
                &hints.even_header,
                &hints.header,
                &hints.even_footer,
                &hints.footer,
                &hints.first_header,
                &hints.first_footer,
            ]
            .map(|blocks| blocks[0][0])
        };

        assert_eq!(mapped.len(), 2);
        assert_eq!(values(&mapped[0]), [1, 2, 3, 4, 5, 6]);
        assert_eq!(values(&mapped[1]), [7, 8, 9, 10, 11, 12]);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_running_column_breaks_repeat_unindexed_header_fallback() {
        let blocks = vec![
            Block::SectionBreak(SectionSetup::default()),
            Block::PageBreak,
        ];
        let regions = [LegacyRunningColumnBreakRegion {
            source_story_index: None,
            column_break_offsets: vec![vec![4]],
        }];

        let mapped = legacy_running_column_breaks_from_regions(&blocks, &regions);

        assert_eq!(mapped.len(), 2);
        for section in mapped {
            assert_eq!(section.header, vec![vec![4]]);
            assert!(section.even_header.is_empty());
            assert!(section.first_header.is_empty());
            assert!(section.footer.is_empty());
            assert!(section.even_footer.is_empty());
            assert!(section.first_footer.is_empty());
        }
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_running_tabs_map_six_story_slots_per_section_and_keep_first_owner() {
        let blocks = vec![
            Block::PageBreak,
            Block::SectionBreak(SectionSetup::default()),
            Block::PageBreak,
        ];
        let tab = |position_pt| TabStop {
            position_pt,
            alignment: crate::model::TabAlignment::Left,
            leader: crate::model::TabLeader::None,
        };
        let mut regions = (0..12)
            .map(|story_offset| LegacyRunningTabRegion {
                source_story_index: Some(HEADER_FOOTER_STORY_BASE + story_offset),
                tab_stops: vec![vec![tab((story_offset + 1) as f32)]],
                table_cell_tab_stops: vec![vec![vec![vec![vec![
                    tab((story_offset + 101) as f32),
                ]]]]],
            })
            .collect::<Vec<_>>();
        regions.push(LegacyRunningTabRegion {
            source_story_index: Some(HEADER_FOOTER_STORY_BASE),
            tab_stops: vec![vec![tab(999.0)]],
            table_cell_tab_stops: vec![vec![vec![vec![vec![tab(1_999.0)]]]]],
        });

        let (paragraphs, tables) = legacy_running_tab_stops_from_regions(&blocks, &regions);
        let paragraph_values = |hints: &RunningSurfaceTabStopHints| {
            [
                &hints.even_header,
                &hints.header,
                &hints.even_footer,
                &hints.footer,
                &hints.first_header,
                &hints.first_footer,
            ]
            .map(|blocks| blocks[0][0].position_pt)
        };
        let table_values = |hints: &RunningSurfaceTableCellTabStopHints| {
            [
                &hints.even_header,
                &hints.header,
                &hints.even_footer,
                &hints.footer,
                &hints.first_header,
                &hints.first_footer,
            ]
            .map(|blocks| blocks[0][0][0][0][0].position_pt)
        };

        assert_eq!(paragraphs.len(), 2);
        assert_eq!(tables.len(), 2);
        assert_eq!(
            paragraph_values(&paragraphs[0]),
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
        assert_eq!(
            paragraph_values(&paragraphs[1]),
            [7.0, 8.0, 9.0, 10.0, 11.0, 12.0]
        );
        assert_eq!(
            table_values(&tables[0]),
            [101.0, 102.0, 103.0, 104.0, 105.0, 106.0]
        );
        assert_eq!(
            table_values(&tables[1]),
            [107.0, 108.0, 109.0, 110.0, 111.0, 112.0]
        );
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    #[test]
    fn legacy_running_tabs_repeat_unindexed_header_fallback() {
        let blocks = vec![
            Block::SectionBreak(SectionSetup::default()),
            Block::PageBreak,
        ];
        let paragraph_tab = TabStop {
            position_pt: 36.0,
            alignment: crate::model::TabAlignment::Center,
            leader: crate::model::TabLeader::Dot,
        };
        let table_tab = TabStop {
            position_pt: 72.0,
            alignment: crate::model::TabAlignment::Right,
            leader: crate::model::TabLeader::Hyphen,
        };
        let regions = [LegacyRunningTabRegion {
            source_story_index: None,
            tab_stops: vec![Vec::new(), vec![paragraph_tab]],
            table_cell_tab_stops: vec![
                vec![vec![vec![vec![table_tab]]]],
                TableCellTabStopHints::new(),
            ],
        }];

        let (paragraphs, tables) = legacy_running_tab_stops_from_regions(&blocks, &regions);
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(tables.len(), 2);
        for (paragraph, table) in paragraphs.iter().zip(tables) {
            assert_eq!(paragraph.header, vec![Vec::new(), vec![paragraph_tab]]);
            assert_eq!(
                table.header,
                vec![vec![vec![vec![vec![table_tab]]]], Vec::new()]
            );
            assert!(paragraph.even_header.is_empty());
            assert!(paragraph.first_header.is_empty());
            assert!(paragraph.footer.is_empty());
            assert!(table.even_header.is_empty());
            assert!(table.first_header.is_empty());
            assert!(table.footer.is_empty());
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn legacy_note_tab_overlay_requires_exact_alignment() {
        let base = vec![vec![1], vec![2], vec![3]];
        let overlay = vec![Vec::new(), vec![20], Vec::new()];
        assert_eq!(
            overlay_aligned_nonempty(&base, &overlay),
            vec![vec![1], vec![20], vec![3]]
        );
        assert_eq!(
            overlay_aligned_nonempty(&base, &overlay[..2]),
            base,
            "a truncated note sidecar must not shift any value"
        );
    }

    #[test]
    fn field_without_separator_does_not_swallow_following_text() {
        // 0x13 "AB" 0x15 (field begin, instruction, end — NO 0x14 separator),
        // then body "CD". The body must survive: a single in-instruction bool
        // would stay stuck after 0x15 and drop everything after it.
        let mut units = vec![FIELD_BEGIN];
        units.extend(us("AB"));
        units.push(FIELD_END);
        units.extend(us("CD"));
        units.push(PARA_MARK);
        assert_eq!(all_text(&run_units(&units)), "CD");
    }

    #[test]
    fn legacy_header_footer_falls_back_when_plcf_hdd_has_no_setup_stories() {
        let mut units = us("HDR");
        units.push(PARA_MARK);
        let fcs: Vec<u32> = (0..units.len() as u32).collect();
        let prms = vec![0; units.len()];
        let mut plcf_hdd = Vec::new();
        for cp in [0u32, units.len() as u32, units.len() as u32] {
            plcf_hdd.extend_from_slice(&cp.to_le_bytes());
        }
        #[cfg(not(any(feature = "docx", feature = "render")))]
        let papx = PapxTable::default();
        #[cfg(any(feature = "docx", feature = "render"))]
        let papx = PapxTable::from_test_entries_with_tab_grpprls(&[(
            units.len() as u32,
            &[0x0D, 0xC6, 5, 0, 1, 0xD0, 0x02, 0],
        )]);
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let fib = minimal_fib_for_header_footer(units.len() as u32, plcf_hdd.len());

        let src = LegacySource {
            units: &units,
            fcs: &fcs,
            prms: &prms,
            prm1_patches: &[],
            papx: &papx,
            chpx: &chpx,
            stylesheet: &stsh,
            data: &[],
            fonts: &[],
        };
        let LegacyRegionOutput {
            blocks,
            regions,
            pagination_hints,
            line_spacing_hints,
            #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops,
            #[cfg(feature = "render")]
            note_tab_stops,
            #[cfg(feature = "docx")]
            note_reference_anchors,
            column_break_offsets,
            #[cfg(feature = "docx")]
            table_cell_column_break_offsets,
            table_row_pagination,
            table_cell_pagination,
            table_cell_line_spacing,
            #[cfg(any(feature = "docx", feature = "render"))]
            table_cell_tab_stops,
            #[cfg(feature = "render")]
            note_table_cell_tab_stops,
            #[cfg(any(feature = "docx", feature = "render"))]
            running_tab_regions,
            #[cfg(feature = "docx")]
            running_column_break_regions,
            text_start: _,
        } = build_legacy_region_blocks(&src, &mut numberer, &fib, &plcf_hdd, &[]);
        assert_eq!(pagination_hints.len(), blocks.len());
        assert_eq!(line_spacing_hints.len(), blocks.len());
        #[cfg(any(feature = "docx", feature = "render"))]
        {
            assert_eq!(tab_stops.len(), blocks.len());
            assert!(tab_stops.iter().all(Vec::is_empty));
        }
        assert_eq!(column_break_offsets.len(), blocks.len());
        #[cfg(feature = "docx")]
        assert_eq!(table_cell_column_break_offsets.len(), blocks.len());
        assert_eq!(table_row_pagination.len(), blocks.len());
        assert_eq!(table_cell_pagination.len(), blocks.len());
        assert_eq!(table_cell_line_spacing.len(), blocks.len());
        #[cfg(any(feature = "docx", feature = "render"))]
        {
            assert_eq!(table_cell_tab_stops.len(), blocks.len());
            assert!(table_cell_tab_stops.iter().all(Vec::is_empty));
            assert_eq!(running_tab_regions.len(), 1);
            assert_eq!(running_tab_regions[0].source_story_index, None);
            assert_eq!(running_tab_regions[0].tab_stops.len(), 1);
            assert_eq!(
                running_tab_regions[0].table_cell_tab_stops,
                vec![TableCellTabStopHints::new()]
            );
            assert_eq!(running_tab_regions[0].tab_stops[0].len(), 1);
            assert_eq!(running_tab_regions[0].tab_stops[0][0].position_pt, 36.0);

            let (running_tabs, running_table_tabs) =
                legacy_running_tab_stops_from_regions(&blocks, &running_tab_regions);
            assert_eq!(running_tabs.len(), 1);
            assert_eq!(running_tabs[0].header, running_tab_regions[0].tab_stops);
            assert_eq!(running_table_tabs.len(), 1);
            assert_eq!(
                running_table_tabs[0].header,
                running_tab_regions[0].table_cell_tab_stops
            );
        }
        #[cfg(feature = "render")]
        {
            assert_eq!(note_tab_stops, vec![Vec::new(); blocks.len()]);
            assert_eq!(
                note_table_cell_tab_stops,
                vec![TableCellTabStopHints::new(); blocks.len()]
            );
        }
        #[cfg(feature = "docx")]
        {
            assert_eq!(note_reference_anchors, vec![Vec::new(); blocks.len()]);
            assert_eq!(running_column_break_regions.len(), 1);
            assert_eq!(running_column_break_regions[0].source_story_index, None);
            assert_eq!(
                running_column_break_regions[0].column_break_offsets.len(),
                1
            );
            assert!(running_column_break_regions[0].column_break_offsets[0].is_empty());
        }

        let header_region = regions
            .iter()
            .find(|region| region.kind == SourceRegionKind::HeaderFooter)
            .expect("header/footer region should fall back to flat preservation");
        assert_eq!(header_region.source_story_index, None);
        assert_eq!(header_region.source_start_cp, 0);
        assert_eq!(header_region.source_len_cp, units.len());
        assert_eq!(header_region.text_len, 3);
        assert_eq!(
            all_text(&blocks[header_region.block_start..header_region.block_end]),
            "HDR"
        );
    }

    #[test]
    fn hyperlink_field_result_is_kept_and_linked() {
        // 0x13 ` HYPERLINK "http://x" ` 0x14 `link` 0x15, then body.
        let mut units = vec![FIELD_BEGIN];
        units.extend(us(" HYPERLINK \"http://x\" "));
        units.push(FIELD_SEP);
        units.extend(us("link"));
        units.push(FIELD_END);
        units.extend(us(" tail"));
        units.push(PARA_MARK);
        let blocks = run_units(&units);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("expected paragraph");
        };
        // The HYPERLINK instruction is dropped; only the result text + tail remain.
        assert_eq!(p.text(), "link tail");
        let linked = p
            .runs
            .iter()
            .find(|r| matches!(&r.field, FieldRole::Hyperlink { .. }));
        match linked.map(|r| (&r.text, &r.field)) {
            Some((t, FieldRole::Hyperlink { url })) => {
                assert_eq!(t, "link");
                assert_eq!(url, "http://x");
            }
            other => panic!("expected linked result run, got {other:?}"),
        }
        // The url does not leak onto the post-field tail.
        let tail = p.runs.iter().find(|r| r.text == " tail").unwrap();
        assert_eq!(tail.field, FieldRole::None);
    }

    #[test]
    fn mixed_case_hyperlink_field_result_is_linked() {
        let mut units = vec![FIELD_BEGIN];
        units.extend(us(" hYpErLiNk \"http://x\" "));
        units.push(FIELD_SEP);
        units.extend(us("link"));
        units.push(FIELD_END);
        units.push(PARA_MARK);
        let blocks = run_units(&units);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            &p.runs[0].field,
            FieldRole::Hyperlink { url } if url == "http://x"
        ));
        assert!(parse_hyperlink(" HYPERLINKBASE \"http://x\" ").is_none());
    }

    #[test]
    fn simple_field_result_keeps_instruction_on_result_run() {
        let mut units = vec![FIELD_BEGIN];
        units.extend(us(" PAGE "));
        units.push(FIELD_SEP);
        units.extend(us("7"));
        units.push(FIELD_END);
        units.extend(us(" tail"));
        units.push(PARA_MARK);
        let blocks = run_units(&units);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("expected paragraph");
        };

        assert_eq!(p.text(), "7 tail");
        let page = p.runs.iter().find(|r| r.text == "7").unwrap();
        assert_eq!(
            page.field,
            FieldRole::Simple {
                instruction: "PAGE".to_string()
            }
        );
        let tail = p.runs.iter().find(|r| r.text == " tail").unwrap();
        assert_eq!(tail.field, FieldRole::None);
    }

    #[test]
    fn nested_field_returns_to_outer_instruction_then_result() {
        // Outer field whose instruction itself contains a nested field:
        // 0x13 "A" 0x13 "B" 0x14 "C" 0x15 "D" 0x14 "RESULT" 0x15.
        // "A".."D" are all outer-instruction (dropped); only "RESULT" shows.
        let mut units = vec![FIELD_BEGIN];
        units.extend(us("A"));
        units.push(FIELD_BEGIN);
        units.extend(us("B"));
        units.push(FIELD_SEP);
        units.extend(us("C"));
        units.push(FIELD_END);
        units.extend(us("D"));
        units.push(FIELD_SEP);
        units.extend(us("RESULT"));
        units.push(FIELD_END);
        units.push(PARA_MARK);
        assert_eq!(all_text(&run_units(&units)), "RESULT");
    }

    #[test]
    fn many_separated_fields_then_text_stays_linear_and_visible() {
        // Adversarial field shape: N [FIELD_BEGIN, FIELD_SEP] pairs leave N separated fields
        // on the stack, then N text chars + a paragraph mark. The old per-code-unit
        // `field_stack` scan made this O(N²); the `unseparated` counter keeps it O(N).
        // All fields are separated, so the trailing text is visible content.
        let n = 100_000;
        let mut units = Vec::with_capacity(n * 2 + n + 1);
        for _ in 0..n {
            units.push(FIELD_BEGIN);
            units.push(FIELD_SEP);
        }
        units.resize(units.len() + n, b'A' as u16);
        units.push(PARA_MARK);
        let text = all_text(&run_units(&units));
        assert_eq!(text.len(), n);
        assert!(text.chars().all(|c| c == 'A'));
    }

    #[test]
    fn repeated_picture_runs_are_deduped_and_byte_budget_bounds_total() {
        // Data stream: one PICF (cbHeader=8) + 33-byte blip header + a PNG signature
        // (mirrors image.rs::finds_png_after_blip_header).
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let payload_len = 33 + png.len();
        let lcb = 8 + payload_len;
        let mut data = Vec::new();
        data.extend_from_slice(&(lcb as u32).to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes());
        data.extend_from_slice(&[0u8; 2]); // pad to cbHeader = 8
        data.extend_from_slice(&[0u8; 33]); // blip header
        data.extend_from_slice(&png);

        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &data, &[], &mut numberer);

        // Dedup: many runs at the same fcPic scan the Data once (one cache entry).
        let first = asm.extract_image(0);
        assert!(first.bytes.is_some(), "first extraction finds the PNG");
        for _ in 0..50 {
            let _ = asm.extract_image(0);
        }
        assert_eq!(asm.img_cache.len(), 1, "same fcPic scanned/cached once");

        // Byte budget bounds total materialized image bytes: once spent, further
        // picture runs become metadata-only placeholders instead of byte copies.
        let img_bytes = first.bytes.as_ref().unwrap().len();
        asm.img_budget = img_bytes; // room for exactly one more full copy
        assert!(asm.extract_image(0).bytes.is_some());
        let over = asm.extract_image(0);
        assert!(over.bytes.is_none(), "over-budget picture is a placeholder");
        assert_eq!(
            over.mime.as_deref(),
            Some("image/png"),
            "placeholder keeps mime"
        );
    }

    #[test]
    fn same_property_content_coalesces_into_one_run() {
        // Consecutive chars with identical (default) properties must coalesce into a single
        // run — confirming the per-run (not per-code-unit) property build still merges runs.
        let mut units = us("HELLOWORLD");
        units.push(PARA_MARK);
        let blocks = run_units(&units);
        let Block::Paragraph(p) = &blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(p.runs.len(), 1);
        assert_eq!(p.runs[0].text, "HELLOWORLD");
    }

    #[test]
    fn pcd_prm0_identity_splits_runs_when_units_share_an_fc() {
        let units = [b'A' as u16, b'B' as u16, PARA_MARK];
        let fcs = [0, 0, 0];
        let prms = [0x01AA, 0x01AC, 0];
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);

        asm.run_with_prms(&units, &fcs, &prms, 0);
        let blocks = asm.finish();
        let [Block::Paragraph(paragraph)] = blocks.as_slice() else {
            panic!("expected one paragraph");
        };
        assert_eq!(paragraph.runs.len(), 2);
        assert_eq!(paragraph.runs[0].text, "A");
        assert!(paragraph.runs[0].props.bold);
        assert!(!paragraph.runs[0].props.italic);
        assert_eq!(paragraph.runs[1].text, "B");
        assert!(!paragraph.runs[1].props.bold);
        assert!(paragraph.runs[1].props.italic);
    }

    #[test]
    fn pcd_prm1_identity_splits_runs_when_units_share_an_fc() {
        let units = [b'A' as u16, b'B' as u16, PARA_MARK];
        let fcs = [0, 0, 0];
        let prms = [1, 3, 0];
        let prm1_patches =
            crate::chpx::compile_pcd_prm1_patches(&[vec![0x35, 0x08, 1], vec![0x36, 0x08, 1]]);
        let papx = PapxTable::default();
        let chpx = ChpxTable::default();
        let stsh = StyleSheet::default();
        let lists = Lists::default();
        let mut numberer = Numberer::new(&lists);
        let mut asm = Asm::new(&papx, &chpx, &stsh, &[], &[], &mut numberer);
        asm.prm1_patches = &prm1_patches;

        asm.run_with_prms(&units, &fcs, &prms, 0);
        let blocks = asm.finish();
        let [Block::Paragraph(paragraph)] = blocks.as_slice() else {
            panic!("expected one paragraph");
        };
        assert_eq!(paragraph.runs.len(), 2);
        assert_eq!(paragraph.runs[0].text, "A");
        assert!(paragraph.runs[0].props.bold);
        assert!(!paragraph.runs[0].props.italic);
        assert_eq!(paragraph.runs[1].text, "B");
        assert!(!paragraph.runs[1].props.bold);
        assert!(paragraph.runs[1].props.italic);
    }

    #[test]
    fn rich_decode_keeps_fc_and_prm_aligned_across_encodings() {
        use crate::clx::Piece;
        // cp1252 piece: 'A', 0x81 (undefined → U+FFFD), 'B'. Each source byte is
        // one char, so FCs must be base, base+1, base+2 — not blown out by the
        // U+FFFD re-encoding into a numeric character reference. A following
        // UTF-16 surrogate pair carries one piece PRM on both code units.
        let base = 0x200usize;
        let mut word = vec![0u8; base];
        word.extend_from_slice(&[b'A', 0x81, b'B']);
        word.extend_from_slice(&[0x3D, 0xD8, 0x00, 0xDE]);
        let pieces = [
            Piece {
                cch: 3,
                fc: base,
                compressed: true,
                prm: 0x01AA,
            },
            Piece {
                cch: 2,
                fc: base + 3,
                compressed: false,
                prm: 0x01AC,
            },
        ];
        let (units, fcs, prms) = decode_with_fc_and_prm(&word, &pieces, encoding_rs::WINDOWS_1252);
        assert_eq!(units.len(), 5);
        assert_eq!(
            fcs,
            vec![
                base as u32,
                base as u32 + 1,
                base as u32 + 2,
                base as u32 + 3,
                base as u32 + 5,
            ]
        );
        assert_eq!(prms, vec![0x01AA, 0x01AA, 0x01AA, 0x01AC, 0x01AC]);
        assert_eq!(units[0], b'A' as u16);
        assert_eq!(units[2], b'B' as u16);
        assert_eq!(&units[3..], &[0xD83D, 0xDE00]);
    }

    #[test]
    fn rich_decode_keeps_prm_identity_when_pieces_share_an_fc() {
        use crate::clx::Piece;
        let word = b"X\0";
        let pieces = [
            Piece {
                cch: 1,
                fc: 0,
                compressed: false,
                prm: 0x01AA,
            },
            Piece {
                cch: 1,
                fc: 0,
                compressed: false,
                prm: 0x01AC,
            },
        ];

        let (units, fcs, prms) = decode_with_fc_and_prm(word, &pieces, encoding_rs::WINDOWS_1252);
        assert_eq!(units, vec![b'X' as u16, b'X' as u16]);
        assert_eq!(fcs, vec![0, 0]);
        assert_eq!(prms, vec![0x01AA, 0x01AC]);
    }

    #[test]
    fn hyperlink_instruction_parsing() {
        assert_eq!(
            parse_hyperlink(" HYPERLINK \"https://example.com\" \\o \"tip\" ").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(parse_hyperlink(" PAGE "), None);
        assert_eq!(
            parse_hyperlink(" HYPERLINK \\l \"anchor\" ").as_deref(),
            Some("anchor")
        );
        assert_eq!(parse_hyperlink(" HYPERLINK \\o \"tip\" "), None);
        assert_eq!(
            parse_hyperlink(" HYPERLINK \"https://example.com\" \"extra "),
            None
        );
        assert_eq!(
            parse_hyperlink(" HYPERLINK \"https://example.com\" extra "),
            None
        );
    }
}
