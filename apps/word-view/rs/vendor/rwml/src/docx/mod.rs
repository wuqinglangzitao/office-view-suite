//! Modern `.docx` (OOXML WordprocessingML) reading — the second Word backend.
//!
//! A `.docx` is a ZIP of XML parts: `word/document.xml` (the body — paragraphs,
//! runs, tables), `word/styles.xml` (style → heading level / name),
//! `word/numbering.xml` (list levels → ordered/bullet),
//! `word/_rels/document.xml.rels` (relationship id → hyperlink target / media
//! path), and `word/media/*` (image bytes).
//!
//! Everything is parsed into the **same** [`crate::model::DocModel`] the legacy
//! `.doc` path produces, so [`crate::Document::to_markdown`] /
//! [`crate::Document::to_html`] / [`crate::Document::images`] are shared and
//! `.doc` and `.docx` render identically. This is a *unification* play (one Word
//! crate, no JVM, no external `.docx` dependency) — see the README on how it
//! relates to the mature `docx-rs` crate.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::annotation::{
    document_property_key, Comment, Field, FieldKind, FloatingShape, HeaderFooter,
    HeaderFooterKind, Note, NoteKind, Revision, ShapeDistance, ShapeEffectExtent, ShapeExtent,
    ShapePoint, ShapePosition, ShapeWrapping, TextAnchor, TextBox,
};
use crate::assemble;
use crate::error::{Error, Result};
use crate::model::{AuthoredNote, Block, Chart, Color, CustomXmlItem, DocMeta, DocModel, Image};
use crate::text;
use crate::CoreProperties;

pub(crate) use self::xml_text::skip_subtree as skip_xml_subtree;
use self::xml_text::{inline_marker_text, read_i64_text, read_text, skip_subtree};

mod body;
pub(crate) mod chart;
mod comments;
pub(crate) mod fields;
mod numbering;
mod revisions;
mod styles;
mod xml_text;

pub(crate) fn parse_fields(xml: &str) -> Vec<Field> {
    let core_properties = CoreProperties::default();
    let custom_properties = HashMap::new();
    let document_variables = HashMap::new();
    let extended_properties = HashMap::new();
    fields::parse(
        xml,
        &styles::Styles::default(),
        &[],
        &numbering::Numbering::default(),
        fields::FieldDocumentProperties {
            core: &core_properties,
            custom: &custom_properties,
            variables: &document_variables,
            extended: &extended_properties,
            file_size_bytes: None,
        },
        false,
    )
}

pub(crate) fn header_footer_ref_ids(xml: &str) -> HashSet<String> {
    header_footer_ref_ids_from_sections(body::scan_hf_ref_sections(xml))
}

pub(crate) fn header_footer_ref_ids_for_revision_reject(xml: &str) -> HashSet<String> {
    header_footer_ref_ids_from_sections(body::scan_hf_ref_sections_for_revision_reject(xml))
}

fn header_footer_ref_ids_from_sections(sections: Vec<body::HeaderFooterRefs>) -> HashSet<String> {
    let mut ids = HashSet::new();
    for refs in sections {
        ids.extend(refs.headers.into_iter().map(|r| r.rel_id));
        ids.extend(refs.footers.into_iter().map(|r| r.rel_id));
    }
    ids
}

pub(crate) fn supports_display_field_syntax(instruction: &str) -> bool {
    fields::supports_display_field_syntax(instruction)
}

pub(crate) fn supports_computed_symbol_field_syntax(instruction: &str) -> bool {
    fields::supports_computed_symbol_field_syntax(instruction)
}

pub(crate) fn supports_action_field_syntax(instruction: &str) -> bool {
    fields::supports_action_field_syntax(instruction)
}

pub(crate) fn supports_reference_index_marker_syntax(instruction: &str) -> bool {
    fields::supports_reference_index_marker_syntax(instruction)
}

pub(crate) fn supports_toc_entry_field_syntax(instruction: &str) -> bool {
    fields::supports_toc_entry_field_syntax(instruction)
}

pub(crate) fn supports_hyperlink_field_syntax(instruction: &str) -> bool {
    body::hyperlink_instr_url(instruction).is_some()
}

pub(crate) fn supports_filename_field_syntax(instruction: &str) -> bool {
    fields::supports_filename_field_syntax(instruction)
}

pub(crate) fn supports_page_field_syntax(instruction: &str) -> bool {
    fields::supports_page_field_syntax(instruction)
}

pub(crate) fn page_field_unsupported_display_formats(xml: &str) -> Vec<bool> {
    let ref_targets = fields::ref_targets(xml);
    fields::page_ref_context(xml, &ref_targets).page_field_unsupported_display_formats()
}

pub(crate) fn supports_section_field_syntax(instruction: &str) -> bool {
    fields::is_section_field_instruction(instruction)
}

pub(crate) fn supports_numbering_field_syntax(instruction: &str) -> bool {
    fields::supports_numbering_field_syntax(instruction)
}

pub(crate) fn supports_compare_field_syntax(instruction: &str) -> bool {
    fields::supports_compare_field_syntax(instruction)
}

pub(crate) fn supports_if_field_syntax(instruction: &str) -> bool {
    fields::supports_if_field_syntax(instruction)
}

pub(crate) fn supports_context_free_if_compare_field_syntax(instruction: &str) -> bool {
    fields::supports_context_free_if_compare_field_syntax(instruction)
}

pub(crate) fn supports_context_free_formula_field_syntax(instruction: &str) -> bool {
    fields::supports_context_free_formula_field_syntax(instruction)
}

pub(crate) fn supports_context_free_fill_in_field_syntax(instruction: &str) -> bool {
    fields::supports_context_free_fill_in_field_syntax(instruction)
}

pub(crate) fn supports_context_free_display_field_syntax(instruction: &str) -> bool {
    fields::supports_context_free_display_field_syntax(instruction)
}

pub(crate) fn supports_quote_field_syntax(instruction: &str) -> bool {
    fields::supports_quote_field_syntax(instruction)
}

pub(crate) fn supports_prompt_field_syntax(instruction: &str) -> bool {
    fields::supports_prompt_field_syntax(instruction)
}

pub(crate) fn supports_set_field_syntax(instruction: &str) -> bool {
    fields::supports_set_field_syntax(instruction)
}

pub(crate) fn update_field_bookmarks_from_instruction(
    instruction: &str,
    field_bookmarks: &mut HashMap<String, String>,
) -> bool {
    fields::computed_set_result(instruction, field_bookmarks)
        .or_else(|| fields::computed_ask_result(instruction, field_bookmarks))
        .is_some()
}

pub(crate) fn supports_merge_control_field_syntax(instruction: &str) -> bool {
    fields::supports_merge_control_field_syntax(instruction)
}

pub(crate) fn supports_document_info_field_syntax(instruction: &str) -> bool {
    fields::supports_document_info_field_syntax(instruction)
}

pub(crate) fn supports_preserved_document_info_field_syntax(instruction: &str) -> bool {
    fields::supports_preserved_document_info_field_syntax(instruction)
}

pub(crate) fn supports_revision_number_field_syntax(instruction: &str) -> bool {
    fields::supports_revision_number_field_syntax(instruction)
}

pub(crate) fn preserved_note_local_ref_target(instruction: &str) -> Option<String> {
    fields::preserved_note_local_ref_target(instruction)
}

pub(crate) fn computed_preserved_note_local_ref_result(
    instruction: &str,
    bookmarks: &HashMap<String, String>,
) -> Option<String> {
    fields::computed_preserved_note_local_ref_result(instruction, bookmarks)
}

pub(crate) fn supports_formula_field_syntax(instruction: &str) -> bool {
    fields::supports_formula_field_syntax(instruction)
}

pub(crate) fn computed_span_free_table_formula_result(
    instruction: &str,
    rows: &[Vec<String>],
    row: usize,
    col: usize,
) -> Option<String> {
    fields::computed_span_free_table_formula_result(instruction, rows, row, col)
}

pub(crate) fn supports_sequence_field_syntax(instruction: &str) -> bool {
    fields::supports_sequence_field_syntax(instruction)
}

pub(crate) fn computed_preserved_sequence_reset_result(instruction: &str) -> Option<String> {
    fields::computed_preserved_sequence_reset_result(instruction)
}

pub(crate) fn computed_preserved_listnum_start_result(instruction: &str) -> Option<String> {
    fields::computed_preserved_listnum_start_result(instruction)
}

pub(crate) fn preserved_note_local_style_ref_target(instruction: &str) -> Option<String> {
    fields::preserved_note_local_style_ref_target(instruction)
}

pub(crate) fn computed_preserved_note_local_style_ref_result(
    instruction: &str,
    source_text: &str,
    source_before_field: bool,
) -> Option<String> {
    fields::computed_preserved_note_local_style_ref_result(
        instruction,
        source_text,
        source_before_field,
    )
}

pub(crate) fn supports_style_ref_field_syntax(instruction: &str) -> bool {
    fields::supports_style_ref_field_syntax(instruction)
}

pub(crate) fn note_ref_target_names(xml: &str) -> HashSet<String> {
    fields::note_ref_target_names(xml)
}

/// Relationship table: `Id` → `(Target, is_external)`.
type Rels = HashMap<String, (String, bool)>;

/// Detect the ZIP / OOXML magic (`PK\x03\x04`).
pub(crate) fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

/// A parsed `.docx`: the rich model (built eagerly — XML parsing is cheap, so
/// there is no lazy split like the `.doc` path) plus the derived flat text.
pub(crate) struct FreshConversionNotes {
    pub(crate) body_blocks: Vec<Block>,
    pub(crate) payloads: Vec<Vec<Option<crate::model::NoteWritePayload>>>,
}

pub(crate) struct DocxState {
    /// The **body-only** model (no footnote/endnote blocks). `Document::model()`
    /// re-appends `notes` for the read view; the lossy model is read/render only.
    pub model: DocModel,
    /// Footnote/endnote blocks, kept separate from `model.blocks` (their `.docx`
    /// parts are preserved on save, never inlined into the body).
    pub notes: Vec<Block>,
    /// Conversion-only body blocks and exact note-paragraph payloads.
    /// `None` keeps the historical flattened-note fallback.
    pub fresh_conversion_notes: Option<FreshConversionNotes>,
    /// Footnote/endnote side-table records parsed from `word/footnotes.xml` and
    /// `word/endnotes.xml`.
    pub note_records: Vec<Note>,
    /// Text-box side-table records parsed from body/note/header/footer
    /// `w:txbxContent` shapes.
    pub text_boxes: Vec<TextBox>,
    /// Floating shape geometry parsed from body/note/header/footer `wp:anchor` drawing markup.
    pub floating_shapes: Vec<FloatingShape>,
    /// Source-only paragraph pagination controls aligned to body model blocks.
    pub pagination_hints: Vec<crate::model::PaginationHint>,
    /// Source-only exact/minimum line spacing aligned to body model blocks.
    pub line_spacing_hints: Vec<Option<crate::model::LineSpacingHint>>,
    /// Renderer-only exact/minimum line spacing aligned to `notes` blocks.
    #[cfg(feature = "render")]
    pub note_line_spacing_hints: Vec<Option<crate::model::LineSpacingHint>>,
    /// Renderer-only explicit paragraph tab stops aligned to `notes` blocks.
    #[cfg(feature = "render")]
    pub note_tab_stops: Vec<Vec<crate::model::TabStop>>,
    /// Renderer-only table-cell tab stops aligned to `notes` blocks.
    #[cfg(feature = "render")]
    pub note_table_cell_tab_stops: Vec<crate::model::TableCellTabStopHints>,
    /// Source-only exact/minimum line spacing for section running surfaces.
    pub running_line_spacing_hints: Vec<crate::model::RunningSurfaceLineSpacingHints>,
    /// Source-only pagination controls for section running surfaces.
    pub running_pagination_hints: Vec<crate::model::RunningSurfacePaginationHints>,
    /// Source-only explicit paragraph tab stops for section running surfaces.
    pub running_tab_stops: Vec<crate::model::RunningSurfaceTabStopHints>,
    /// Source-only table-cell tab stops for section running surfaces.
    pub running_table_cell_tab_stops: Vec<crate::model::RunningSurfaceTableCellTabStopHints>,
    /// Source-only table-cell breaks and recursive table layout for section running surfaces.
    pub running_table_layout_hints: Vec<crate::model::RunningSurfaceTableLayoutHints>,
    /// Source-only manual column breaks in ordinary section running-surface paragraphs.
    pub running_column_break_offsets: Vec<crate::model::RunningSurfaceColumnBreakHints>,
    /// Source-only header/footer edge distances by section.
    pub running_surface_distances: Vec<crate::model::RunningSurfaceDistanceHints>,
    /// Source-only resolved explicit tab stops aligned to body model blocks.
    pub tab_stops: Vec<Vec<crate::model::TabStop>>,
    /// Source-only manual column-break character offsets within body paragraphs.
    pub column_break_offsets: Vec<Vec<usize>>,
    /// Source-only manual column-break offsets in direct paragraphs of top-level body tables.
    pub table_cell_column_break_offsets: Vec<crate::model::TableCellColumnBreakHints>,
    /// Source-only explicit equal-column gaps aligned to body model blocks.
    pub section_column_gap_pt: Vec<Option<f32>>,
    /// Source-only explicit unequal-column geometry aligned to body model blocks.
    pub section_column_layouts: Vec<Option<crate::model::SectionColumnLayoutHints>>,
    /// Source-only column-separator flags aligned to body model blocks.
    pub section_column_separators: Vec<bool>,
    /// Source-only right-to-left column population aligned to body model blocks.
    pub section_column_rtl: Vec<bool>,
    /// Source-only explicit equal-column gap for the final body section.
    pub final_section_column_gap_pt: Option<f32>,
    /// Source-only explicit unequal-column geometry for the final body section.
    pub final_section_column_layout: Option<crate::model::SectionColumnLayoutHints>,
    /// Source-only column-separator flag for the final body section.
    pub final_section_column_separator: bool,
    /// Source-only right-to-left population for the final body section.
    pub final_section_column_rtl: bool,
    /// Source-only effective table-row pagination controls aligned to body model blocks.
    pub table_row_pagination: Vec<Vec<crate::model::TableRowPaginationHint>>,
    /// Source-only direct table-cell paragraph controls aligned to body model blocks.
    pub table_cell_pagination: Vec<crate::model::TableCellPaginationHints>,
    /// Source-only table-cell exact/minimum line spacing aligned to body model blocks.
    pub table_cell_line_spacing: Vec<crate::model::TableCellLineSpacingHints>,
    /// Source-only recursive nested-table controls aligned to body model blocks.
    pub table_nested_pagination: Vec<crate::model::TableCellNestedPaginationHints>,
    /// Source-only resolved table-cell paragraph tab stops aligned to body model blocks.
    pub table_cell_tab_stops: Vec<crate::model::TableCellTabStopHints>,
    /// Renderer-only document default tab interval from `word/settings.xml`.
    #[cfg(feature = "render")]
    pub default_tab_stop_pt: Option<f32>,
    /// Exact running header/footer records parsed from referenced `.docx` parts.
    pub header_footers: Vec<HeaderFooter>,
    /// Core metadata parsed from `docProps/core.xml`.
    pub core_properties: CoreProperties,
    /// Full flat text: body, then footnotes/endnotes, then headers and footers.
    pub text: String,
    /// Just the main body (excludes notes and headers/footers).
    pub main_text: String,
    /// The retained OPC package (every part verbatim) — the source of truth for
    /// package-preserving `save()`. Element-tree edits mutate its `document.xml` in
    /// place; the lossy `model` above is the read/render view.
    pub package: crate::opc::Package,
    source_size_bytes: usize,
    /// Comments parsed from `word/comments.xml` and optional commentsExtended links.
    pub comments: Vec<Comment>,
    /// Fields parsed from body/note/header/footer content.
    pub fields: Vec<Field>,
    /// Tracked revisions parsed from body/note/header/footer content.
    pub revisions: Vec<Revision>,
}

impl std::fmt::Debug for DocxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocxState")
            .field("blocks", &self.model.blocks.len())
            .finish_non_exhaustive()
    }
}

/// Open and decode a `.docx` from its raw bytes.
pub(crate) fn open(bytes: &[u8]) -> Result<DocxState> {
    // Bound the entry count BEFORE `ZipArchive::new` (which eagerly collects the whole
    // central directory) — same authoritative limit the package layer enforces, so a
    // hostile archive can't amplify on the read path either.
    crate::opc::check_zip_entry_budget(bytes)?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| Error::Docx(format!("not a valid .docx (zip) container: {e}")))?;

    // All supplementary parts are best-effort: a missing styles/numbering/rels
    // part just means fewer headings/lists/links, never a failure.
    let rels = part(&mut zip, "word/_rels/document.xml.rels")
        .map(|s| parse_rels(&s))
        .unwrap_or_default();
    let styles = part(&mut zip, "word/styles.xml")
        .map(|s| styles::parse(&s))
        .unwrap_or_default();
    let numbering = part(&mut zip, "word/numbering.xml")
        .map(|s| numbering::parse(&s))
        .unwrap_or_default();
    let media = read_media(&mut zip, &rels);
    let charts = read_charts(&mut zip, &rels);

    // The body is the one required part.
    let doc_xml = part(&mut zip, "word/document.xml")
        .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
    let core_properties = part(&mut zip, "docProps/core.xml")
        .map(|s| parse_core_properties(&s))
        .unwrap_or_default();
    let custom_properties = part(&mut zip, "docProps/custom.xml")
        .map(|s| parse_custom_properties(&s))
        .unwrap_or_default();
    let custom_property_fields = custom_properties
        .iter()
        .map(|(key, value)| (document_property_key(key), value.clone()))
        .collect::<HashMap<_, _>>();
    let custom_xml_items = read_custom_xml_items(&mut zip);
    let extended_properties = part(&mut zip, "docProps/app.xml")
        .map(|s| parse_extended_properties(&s))
        .unwrap_or_default();
    let settings_xml = part(&mut zip, "word/settings.xml");
    let document_variables = settings_xml
        .as_deref()
        .map(parse_document_variables)
        .unwrap_or_default();
    let document_id = settings_xml.as_deref().and_then(parse_document_id);
    let preserve_legacy_form_cache = settings_xml
        .as_deref()
        .is_some_and(settings_preserves_legacy_form_cache);
    // Document-level footnote/endnote numbering (numStart/numFmt) applies to the
    // main body only; header/footer/note parts keep their own per-part local
    // numbering restarting at 1.
    let note_numbering = settings_xml
        .as_deref()
        .map(fields::note_numbering_from_settings)
        .unwrap_or_default();
    #[cfg(feature = "render")]
    let default_tab_stop_pt = settings_xml.as_deref().and_then(parse_default_tab_stop_pt);
    let document_properties = DocumentPropertyRefs {
        core: &core_properties,
        custom: &custom_property_fields,
        variables: &document_variables,
        extended: &extended_properties,
        file_size_bytes: Some(bytes.len()),
    };
    let field_properties = fields::FieldDocumentProperties {
        core: document_properties.core,
        custom: document_properties.custom,
        variables: document_properties.variables,
        extended: document_properties.extended,
        file_size_bytes: document_properties.file_size_bytes,
    };
    let raw_ref_targets =
        fields::ref_targets_with_properties(&doc_xml, field_properties, preserve_legacy_form_cache);
    let ref_position_context = fields::ref_position_context(&doc_xml, &numbering);
    let ref_number_context = fields::ref_number_context(&doc_xml, &numbering);
    let note_ref_context = fields::note_ref_context_with_numbering(
        &doc_xml,
        &raw_ref_targets,
        field_properties,
        preserve_legacy_form_cache,
        note_numbering,
    );
    let ref_targets = fields::ref_targets_with_note_context(
        &doc_xml,
        field_properties,
        preserve_legacy_form_cache,
        &note_ref_context,
    );
    let page_ref_context = fields::page_ref_context_with_properties(
        &doc_xml,
        &ref_targets,
        field_properties,
        preserve_legacy_form_cache,
    );
    let section_context = fields::section_context_with_properties(
        &doc_xml,
        &ref_targets,
        field_properties,
        preserve_legacy_form_cache,
    );
    let style_ref_context = fields::style_ref_context_with_properties(
        &doc_xml,
        &styles,
        &numbering,
        &fields::StyleRefResolutionSources {
            document_bookmarks: &ref_targets,
            note_refs: &note_ref_context,
            sections: &section_context,
        },
        field_properties,
        preserve_legacy_form_cache,
    );
    let legacy_form_context = fields::legacy_form_context(&doc_xml, preserve_legacy_form_cache);
    let table_formula_context = fields::table_formula_context_with_properties(
        &doc_xml,
        &ref_targets,
        &note_ref_context,
        &section_context,
        field_properties,
        preserve_legacy_form_cache,
    );
    let sequence_heading_context = fields::sequence_heading_context(&doc_xml, &styles);
    let toc_entries = fields::toc_entries_with_properties(
        &doc_xml,
        &styles,
        &ref_targets,
        &note_ref_context,
        &section_context,
        field_properties,
        preserve_legacy_form_cache,
    );
    let bookmark_names = fields::bookmark_names(&doc_xml);

    let ctx = body::Ctx {
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
        custom_properties: &custom_property_fields,
        document_variables: &document_variables,
        extended_properties: &extended_properties,
        file_size_bytes: Some(bytes.len()),
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
    ctx.begin_section_column_capture();
    ctx.begin_pagination_capture();
    let mut blocks = body::parse_document(&doc_xml, &ctx); // body only
    let body::BodySectionColumnHints {
        gaps: section_column_gap_pt,
        layouts: section_column_layouts,
        separators: section_column_separators,
        rtl: section_column_rtl,
    } = ctx.take_section_column_hints();
    let body_hints = ctx.take_layout_hints();
    let pagination_hints = body_hints.pagination;
    let line_spacing_hints = body_hints.line_spacing;
    let tab_stops = body_hints.tab_stops;
    let table_row_pagination = body_hints.table_rows;
    let table_cell_pagination = body_hints.table_cells;
    let table_cell_line_spacing = body_hints.table_cell_line_spacing;
    let table_cell_column_break_offsets = body_hints.table_cell_column_breaks;
    let table_cell_tab_stops = body_hints.table_cell_tabs;
    let column_break_offsets = body_hints.column_break_offsets;
    let table_nested_pagination = body_hints.table_nested;
    // Footnotes/endnotes live in their own parts. Keep them SEPARATE from the body
    // (not appended into `model.blocks`); their parts are preserved verbatim on save.
    // They are re-joined for the read/text views below and in `Document::model()`.
    let part_env = PartParseEnv {
        styles: &styles,
        numbering: &numbering,
        properties: document_properties,
        preserve_legacy_form_cache,
    };
    let mut note_part = read_notes(
        &mut zip,
        "word/footnotes.xml",
        b"footnote",
        NoteKind::Footnote,
        part_env,
    );
    let mut endnote_part = read_notes(
        &mut zip,
        "word/endnotes.xml",
        b"endnote",
        NoteKind::Endnote,
        part_env,
    );
    note_part.blocks.extend(endnote_part.blocks);
    note_part
        .source_entries
        .append(&mut endnote_part.source_entries);
    #[cfg(feature = "render")]
    note_part.line_spacing.extend(endnote_part.line_spacing);
    #[cfg(feature = "render")]
    note_part.tab_stops.extend(endnote_part.tab_stops);
    #[cfg(feature = "render")]
    note_part
        .table_cell_tab_stops
        .extend(endnote_part.table_cell_tab_stops);
    note_part.records.append(&mut endnote_part.records);
    note_part.revisions.extend(endnote_part.revisions);
    note_part
        .floating_shapes
        .extend(endnote_part.floating_shapes);
    note_part.text_boxes.extend(endnote_part.text_boxes);
    note_part.fields.extend(endnote_part.fields);
    extend_missing_comment_anchors(&mut note_part.comment_anchors, endnote_part.comment_anchors);
    attach_note_reference_anchors(
        &mut note_part.records,
        &doc_xml,
        &fields::FieldResolutionContext {
            properties: field_properties,
            document_bookmarks: &ref_targets,
            note_refs: &note_ref_context,
            sections: &section_context,
            style_refs: &style_ref_context,
            legacy_forms: &legacy_form_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
        },
    );
    let mut floating_shapes = read_floating_shapes(
        &doc_xml,
        ShapeFieldContext {
            properties: field_properties,
            document_bookmarks: &ref_targets,
            ref_positions: &ref_position_context,
            ref_numbers: &ref_number_context,
            page_refs: &page_ref_context,
            note_refs: &note_ref_context,
            sections: &section_context,
            legacy_forms: &legacy_form_context,
            table_formulas: &table_formula_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
            style_refs: &style_ref_context,
            sequence_headings: &sequence_heading_context,
        },
    );
    floating_shapes.extend(note_part.floating_shapes);
    let mut text_boxes = read_text_boxes(&doc_xml, &ctx, &floating_shapes);
    text_boxes.extend(note_part.text_boxes);
    // Running headers/footers referenced by the body's sectPr(s). `ctx` only holds
    // shared (&) borrows of rels/styles/numbering, so the &mut zip pass is fine.
    let HeaderFooterRead {
        sections: section_header_footers,
        final_section: final_header_footer,
        records: header_footers,
        comment_anchors: header_footer_comment_anchors,
        text_boxes: header_footer_text_boxes,
        revisions: header_footer_revisions,
        floating_shapes: header_footer_floating_shapes,
        fields: header_footer_fields,
    } = read_headers_footers(
        &mut zip,
        &doc_xml,
        &rels,
        &styles,
        &numbering,
        document_properties,
        preserve_legacy_form_cache,
    );
    floating_shapes.extend(header_footer_floating_shapes);
    text_boxes.extend(header_footer_text_boxes);
    let running_line_spacing_hints =
        running_line_spacing_by_model_section(&blocks, &section_header_footers);
    let running_pagination_hints =
        running_pagination_by_model_section(&blocks, &section_header_footers);
    let running_tab_stops = running_tab_stops_by_model_section(&blocks, &section_header_footers);
    let running_table_cell_tab_stops =
        running_table_cell_tab_stops_by_model_section(&blocks, &section_header_footers);
    let running_table_layout_hints =
        running_table_layout_by_model_section(&blocks, &section_header_footers);
    let running_column_break_offsets =
        running_column_breaks_by_model_section(&blocks, &section_header_footers);
    let running_surface_distances =
        running_surface_distances_by_model_section(&blocks, &section_header_footers);
    apply_section_header_footers(&mut blocks, &section_header_footers);
    let comments_xml = part(&mut zip, "word/comments.xml");
    let comments_ext_xml = part(&mut zip, "word/commentsExtended.xml");
    let mut comments = if let Some(xml) = comments_xml.as_deref() {
        let comments_section_context = fields::section_context_with_properties(
            xml,
            &ref_targets,
            field_properties,
            preserve_legacy_form_cache,
        );
        let comments_style_ref_context = fields::style_ref_context_with_properties(
            xml,
            &styles,
            &numbering,
            &fields::StyleRefResolutionSources {
                document_bookmarks: &ref_targets,
                note_refs: &note_ref_context,
                sections: &comments_section_context,
            },
            field_properties,
            preserve_legacy_form_cache,
        );
        let comments_legacy_form_context =
            fields::legacy_form_context(xml, preserve_legacy_form_cache);
        comments::parse(
            xml,
            &fields::FieldResolutionContext {
                properties: field_properties,
                document_bookmarks: &ref_targets,
                note_refs: &note_ref_context,
                sections: &comments_section_context,
                style_refs: &comments_style_ref_context,
                legacy_forms: &comments_legacy_form_context,
                toc_entries: &toc_entries,
                bookmark_names: &bookmark_names,
            },
        )
    } else {
        Vec::new()
    };
    if let (Some(comments_xml), Some(comments_ext_xml)) =
        (comments_xml.as_deref(), comments_ext_xml.as_deref())
    {
        comments::apply_extended_parent_ids(&mut comments, comments_xml, comments_ext_xml);
    }
    let field_resolution_context = fields::FieldResolutionContext {
        properties: field_properties,
        document_bookmarks: &ref_targets,
        note_refs: &note_ref_context,
        sections: &section_context,
        style_refs: &style_ref_context,
        legacy_forms: &legacy_form_context,
        toc_entries: &toc_entries,
        bookmark_names: &bookmark_names,
    };
    let fresh_conversion_notes = exact_fresh_conversion_notes(
        &blocks,
        &note_part.records,
        &note_part.source_entries,
        &doc_xml,
        &field_resolution_context,
    );
    let mut comment_anchors = comments::parse_anchors(&doc_xml, &field_resolution_context);
    extend_missing_comment_anchors(&mut comment_anchors, note_part.comment_anchors);
    extend_missing_comment_anchors(&mut comment_anchors, header_footer_comment_anchors);
    for comment in &mut comments {
        comment.anchor = comment_anchors.get(&comment.id).cloned();
    }
    let mut fields = fields::parse_with_note_numbering(
        &doc_xml,
        &styles,
        &toc_entries,
        &numbering,
        fields::FieldDocumentProperties {
            core: &core_properties,
            custom: &custom_property_fields,
            variables: &document_variables,
            extended: &extended_properties,
            file_size_bytes: Some(bytes.len()),
        },
        preserve_legacy_form_cache,
        note_numbering,
    );
    let mut revisions = revisions::parse(&doc_xml, &field_resolution_context);
    revisions.extend(note_part.revisions);
    revisions.extend(header_footer_revisions);
    // Stats reflect the full visible content (body + notes).
    let stats = {
        let mut all = blocks.clone();
        all.extend(note_part.blocks.iter().cloned());
        assemble::compute_stats(&all)
    };
    let model = DocModel {
        blocks, // body only
        regions: Vec::new(),
        // `.docx` text is Unicode (no ANSI codepage); these fields are not
        // meaningful here, unlike the `.doc` path's `lid`/codepage.
        meta: DocMeta {
            codepage: 0,
            lid: 0,
            stats,
        },
        custom_properties,
        custom_xml_items,
        setup: crate::model::DocSetup {
            page: body::scan_page_setup(&doc_xml),
            header: final_header_footer.header,
            first_header: final_header_footer.first_header,
            even_header: final_header_footer.even_header,
            footer: final_header_footer.footer,
            first_footer: final_header_footer.first_footer,
            even_footer: final_header_footer.even_footer,
            page_number_start: body::scan_page_number_start(&doc_xml),
            page_number_format: body::scan_page_number_format(&doc_xml),
            columns: body::scan_section_columns(&doc_xml),
            text_direction: body::scan_section_text_direction(&doc_xml),
            doc_grid: body::scan_section_doc_grid(&doc_xml),
            document_id,
            title_page: body::scan_section_title_page(&doc_xml),
            title: core_properties.title.clone(),
            subject: core_properties.subject.clone(),
            creator: core_properties.creator.clone(),
            description: core_properties.description.clone(),
            keywords: core_properties.keywords.clone(),
            category: core_properties.category.clone(),
            content_status: core_properties.content_status.clone(),
            last_modified_by: core_properties.last_modified_by.clone(),
            created: core_properties.created.clone(),
            modified: core_properties.modified.clone(),
            last_printed: core_properties.last_printed.clone(),
            revision: core_properties.revision.clone(),
            version: core_properties.version.clone(),
            ..crate::model::DocSetup::default()
        },
    };
    fields.extend(note_part.fields);
    fields.extend(header_footer_fields);
    let main_text = body_text(&model); // body only
                                       // Full text: body, then notes, then section/final headers/footers.
    let text = {
        let mut raw = String::new();
        flatten(&model.blocks, &mut raw);
        flatten(&note_part.blocks, &mut raw);
        flatten_header_footer_surfaces(&model, &mut raw);
        text::finalize(&raw)
    };
    // Retain the whole package verbatim for package-preserving editing/save. The
    // reader above is unchanged; this is an independent second pass over `bytes`.
    let package = crate::opc::Package::from_zip(bytes)?;
    let final_section_columns = body::scan_final_section_column_hints(&doc_xml);
    Ok(DocxState {
        model,
        notes: note_part.blocks,
        fresh_conversion_notes,
        text,
        main_text,
        package,
        source_size_bytes: bytes.len(),
        comments,
        note_records: note_part.records,
        text_boxes,
        floating_shapes,
        pagination_hints,
        line_spacing_hints,
        #[cfg(feature = "render")]
        note_line_spacing_hints: note_part.line_spacing,
        #[cfg(feature = "render")]
        note_tab_stops: note_part.tab_stops,
        #[cfg(feature = "render")]
        note_table_cell_tab_stops: note_part.table_cell_tab_stops,
        running_line_spacing_hints,
        running_pagination_hints,
        running_tab_stops,
        running_table_cell_tab_stops,
        running_table_layout_hints,
        running_column_break_offsets,
        running_surface_distances,
        tab_stops,
        column_break_offsets,
        table_cell_column_break_offsets,
        section_column_gap_pt,
        section_column_layouts,
        section_column_separators,
        section_column_rtl,
        final_section_column_gap_pt: final_section_columns.gap_pt,
        final_section_column_layout: final_section_columns.layout,
        final_section_column_separator: final_section_columns.separator,
        final_section_column_rtl: final_section_columns.rtl,
        table_row_pagination,
        table_cell_pagination,
        table_cell_line_spacing,
        table_nested_pagination,
        table_cell_tab_stops,
        #[cfg(feature = "render")]
        default_tab_stop_pt,
        header_footers,
        core_properties,
        fields,
        revisions,
    })
}

/// The bundled blank template bytes — a valid package this crate ships and tests.
const BLANK_DOCX: &[u8] = include_bytes!("../../assets/blank.docx");

/// A blank `.docx` state from the bundled template — backs [`crate::Document::new`].
/// Cannot fail in practice (a corrupt asset is caught by `new_from_template`); see
/// [`try_blank`] for the non-panicking variant.
pub(crate) fn blank() -> DocxState {
    open(BLANK_DOCX).expect("bundled assets/blank.docx is a valid package")
}

/// Fallible blank-template open — backs [`crate::Document::try_new`].
pub(crate) fn try_blank() -> Result<DocxState> {
    open(BLANK_DOCX)
}

/// Resolve and parse the header/footer parts referenced by the body's sectPr(s).
#[derive(Clone, Default)]
struct SectionHeaderFooter {
    header: Vec<Block>,
    first_header: Vec<Block>,
    even_header: Vec<Block>,
    footer: Vec<Block>,
    first_footer: Vec<Block>,
    even_footer: Vec<Block>,
    line_spacing: crate::model::RunningSurfaceLineSpacingHints,
    pagination: crate::model::RunningSurfacePaginationHints,
    tab_stops: crate::model::RunningSurfaceTabStopHints,
    table_cell_tab_stops: crate::model::RunningSurfaceTableCellTabStopHints,
    table_layout: crate::model::RunningSurfaceTableLayoutHints,
    column_breaks: crate::model::RunningSurfaceColumnBreakHints,
    distances: crate::model::RunningSurfaceDistanceHints,
}

#[derive(Default)]
struct HeaderFooterBlocks {
    default: Vec<Block>,
    first: Vec<Block>,
    even: Vec<Block>,
}

#[derive(Default)]
struct HeaderFooterLineSpacing {
    default: Vec<Option<crate::model::LineSpacingHint>>,
    default_table_cells: Vec<crate::model::TableCellLineSpacingHints>,
    first: Vec<Option<crate::model::LineSpacingHint>>,
    first_table_cells: Vec<crate::model::TableCellLineSpacingHints>,
    even: Vec<Option<crate::model::LineSpacingHint>>,
    even_table_cells: Vec<crate::model::TableCellLineSpacingHints>,
}

#[derive(Default)]
struct HeaderFooterPagination {
    default: crate::model::RunningBlockPaginationHints,
    first: crate::model::RunningBlockPaginationHints,
    even: crate::model::RunningBlockPaginationHints,
}

#[derive(Default)]
struct HeaderFooterTableCellTabStops {
    default: Vec<crate::model::TableCellTabStopHints>,
    first: Vec<crate::model::TableCellTabStopHints>,
    even: Vec<crate::model::TableCellTabStopHints>,
}

#[derive(Default)]
struct HeaderFooterTableLayout {
    default: crate::model::RunningTableLayoutHints,
    first: crate::model::RunningTableLayoutHints,
    even: crate::model::RunningTableLayoutHints,
}

#[derive(Default)]
struct HeaderFooterColumnBreaks {
    default: Vec<Vec<usize>>,
    first: Vec<Vec<usize>>,
    even: Vec<Vec<usize>>,
}

#[derive(Default)]
struct HeaderFooterTabStops {
    default: Vec<Vec<crate::model::TabStop>>,
    first: Vec<Vec<crate::model::TabStop>>,
    even: Vec<Vec<crate::model::TabStop>>,
}

struct HeaderFooterRead {
    sections: Vec<SectionHeaderFooter>,
    final_section: SectionHeaderFooter,
    records: Vec<HeaderFooter>,
    comment_anchors: HashMap<String, TextAnchor>,
    text_boxes: Vec<TextBox>,
    revisions: Vec<Revision>,
    floating_shapes: Vec<FloatingShape>,
    fields: Vec<Field>,
}

struct HeaderFooterPartRead {
    blocks: HeaderFooterBlocks,
    line_spacing: HeaderFooterLineSpacing,
    pagination: HeaderFooterPagination,
    tab_stops: HeaderFooterTabStops,
    table_cell_tab_stops: HeaderFooterTableCellTabStops,
    table_layout: HeaderFooterTableLayout,
    column_breaks: HeaderFooterColumnBreaks,
    records: Vec<HeaderFooter>,
    comment_anchors: HashMap<String, TextAnchor>,
    text_boxes: Vec<TextBox>,
    revisions: Vec<Revision>,
    floating_shapes: Vec<FloatingShape>,
    fields: Vec<Field>,
}

#[derive(Clone, Copy)]
struct DocumentPropertyRefs<'a> {
    core: &'a CoreProperties,
    custom: &'a HashMap<String, String>,
    variables: &'a HashMap<String, String>,
    extended: &'a HashMap<String, String>,
    file_size_bytes: Option<usize>,
}

/// Shared environment for parsing a supplementary `.docx` part (header/footer or
/// notes): the stylesheet and numbering tables, the document property refs, and
/// whether cached legacy-form results should be preserved. Bundled so the part
/// readers take one value instead of four parallel parameters.
#[derive(Clone, Copy)]
struct PartParseEnv<'a> {
    styles: &'a styles::Styles,
    numbering: &'a numbering::Numbering,
    properties: DocumentPropertyRefs<'a>,
    preserve_legacy_form_cache: bool,
}

fn read_headers_footers(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    doc_xml: &str,
    rels: &Rels,
    styles: &styles::Styles,
    numbering: &numbering::Numbering,
    properties: DocumentPropertyRefs<'_>,
    preserve_legacy_form_cache: bool,
) -> HeaderFooterRead {
    let part_env = PartParseEnv {
        styles,
        numbering,
        properties,
        preserve_legacy_form_cache,
    };
    let section_refs = body::scan_hf_ref_sections(doc_xml);
    let mut sections = Vec::with_capacity(section_refs.len());
    let mut records = Vec::new();
    let mut comment_anchors = HashMap::new();
    let mut text_boxes = Vec::new();
    let mut revisions = Vec::new();
    let mut floating_shapes = Vec::new();
    let mut field_entries = Vec::new();
    let mut seen_records = std::collections::HashSet::new();
    let mut seen_text_boxes = std::collections::HashSet::new();
    let mut inherited_header = Vec::new();
    let mut inherited_footer = Vec::new();
    let mut inherited_header_line_spacing = Vec::new();
    let mut inherited_header_pagination = crate::model::RunningBlockPaginationHints::default();
    let mut inherited_header_tab_stops = Vec::new();
    let mut inherited_header_table_cell_line_spacing = Vec::new();
    let mut inherited_header_table_cell_tab_stops = Vec::new();
    let mut inherited_header_table_layout = crate::model::RunningTableLayoutHints::default();
    let mut inherited_header_column_breaks = Vec::new();
    let mut inherited_footer_line_spacing = Vec::new();
    let mut inherited_footer_pagination = crate::model::RunningBlockPaginationHints::default();
    let mut inherited_footer_tab_stops = Vec::new();
    let mut inherited_footer_table_cell_line_spacing = Vec::new();
    let mut inherited_footer_table_cell_tab_stops = Vec::new();
    let mut inherited_footer_table_layout = crate::model::RunningTableLayoutHints::default();
    let mut inherited_footer_column_breaks = Vec::new();

    for refs in section_refs {
        let distances = crate::model::RunningSurfaceDistanceHints {
            header_pt: refs
                .header_distance_twips
                .map(|distance| distance as f32 / 20.0),
            footer_pt: refs
                .footer_distance_twips
                .map(|distance| distance as f32 / 20.0),
        };
        let header_has_default = has_default_header_footer_ref(&refs.headers);
        let footer_has_default = has_default_header_footer_ref(&refs.footers);
        let HeaderFooterPartRead {
            blocks: header_blocks,
            line_spacing: header_line_spacing,
            pagination: header_pagination_parts,
            tab_stops: header_tab_stops,
            table_cell_tab_stops: header_table_cell_tab_stops,
            table_layout: header_table_layout_parts,
            column_breaks: header_column_break_parts,
            records: header_records,
            comment_anchors: header_comment_anchors,
            text_boxes: header_text_boxes,
            revisions: header_revisions,
            floating_shapes: header_floating_shapes,
            fields: header_fields,
        } = read_hf_parts(
            zip,
            &refs.headers,
            HeaderFooterPartKind::Header,
            rels,
            part_env,
        );
        extend_unique_header_footer_records(&mut records, &mut seen_records, header_records);
        extend_missing_comment_anchors(&mut comment_anchors, header_comment_anchors);
        extend_unique_text_box_records(&mut text_boxes, &mut seen_text_boxes, header_text_boxes);
        extend_unique_revision_records(&mut revisions, header_revisions);
        extend_unique_floating_shape_records(&mut floating_shapes, header_floating_shapes);
        field_entries.extend(header_fields);
        let mut header = header_blocks.default;
        let mut header_spacing = header_line_spacing.default;
        let mut header_pagination = header_pagination_parts.default;
        let mut header_tabs = header_tab_stops.default;
        let mut header_table_cell_spacing = header_line_spacing.default_table_cells;
        let mut header_table_cell_tabs = header_table_cell_tab_stops.default;
        let mut header_table_layout = header_table_layout_parts.default;
        let mut header_column_breaks = header_column_break_parts.default;
        // Omitted odd/default refs inherit the previous section; an explicit
        // default ref, even when blank/unresolved, resets the inherited surface.
        if !header_has_default && !inherited_header.is_empty() {
            header = inherited_header.clone();
            header_spacing = inherited_header_line_spacing.clone();
            header_pagination = inherited_header_pagination.clone();
            header_tabs = inherited_header_tab_stops.clone();
            header_table_cell_spacing = inherited_header_table_cell_line_spacing.clone();
            header_table_cell_tabs = inherited_header_table_cell_tab_stops.clone();
            header_table_layout = inherited_header_table_layout.clone();
            header_column_breaks = inherited_header_column_breaks.clone();
        }
        if header_has_default || !header.is_empty() {
            inherited_header = header.clone();
            inherited_header_line_spacing = header_spacing.clone();
            inherited_header_pagination = header_pagination.clone();
            inherited_header_tab_stops = header_tabs.clone();
            inherited_header_table_cell_line_spacing = header_table_cell_spacing.clone();
            inherited_header_table_cell_tab_stops = header_table_cell_tabs.clone();
            inherited_header_table_layout = header_table_layout.clone();
            inherited_header_column_breaks = header_column_breaks.clone();
        }

        let HeaderFooterPartRead {
            blocks: footer_blocks,
            line_spacing: footer_line_spacing,
            pagination: footer_pagination_parts,
            tab_stops: footer_tab_stops,
            table_cell_tab_stops: footer_table_cell_tab_stops,
            table_layout: footer_table_layout_parts,
            column_breaks: footer_column_break_parts,
            records: footer_records,
            comment_anchors: footer_comment_anchors,
            text_boxes: footer_text_boxes,
            revisions: footer_revisions,
            floating_shapes: footer_floating_shapes,
            fields: footer_fields,
        } = read_hf_parts(
            zip,
            &refs.footers,
            HeaderFooterPartKind::Footer,
            rels,
            part_env,
        );
        extend_unique_header_footer_records(&mut records, &mut seen_records, footer_records);
        extend_missing_comment_anchors(&mut comment_anchors, footer_comment_anchors);
        extend_unique_text_box_records(&mut text_boxes, &mut seen_text_boxes, footer_text_boxes);
        extend_unique_revision_records(&mut revisions, footer_revisions);
        extend_unique_floating_shape_records(&mut floating_shapes, footer_floating_shapes);
        field_entries.extend(footer_fields);
        let mut footer = footer_blocks.default;
        let mut footer_spacing = footer_line_spacing.default;
        let mut footer_pagination = footer_pagination_parts.default;
        let mut footer_tabs = footer_tab_stops.default;
        let mut footer_table_cell_spacing = footer_line_spacing.default_table_cells;
        let mut footer_table_cell_tabs = footer_table_cell_tab_stops.default;
        let mut footer_table_layout = footer_table_layout_parts.default;
        let mut footer_column_breaks = footer_column_break_parts.default;
        // Same inheritance rule as headers.
        if !footer_has_default && !inherited_footer.is_empty() {
            footer = inherited_footer.clone();
            footer_spacing = inherited_footer_line_spacing.clone();
            footer_pagination = inherited_footer_pagination.clone();
            footer_tabs = inherited_footer_tab_stops.clone();
            footer_table_cell_spacing = inherited_footer_table_cell_line_spacing.clone();
            footer_table_cell_tabs = inherited_footer_table_cell_tab_stops.clone();
            footer_table_layout = inherited_footer_table_layout.clone();
            footer_column_breaks = inherited_footer_column_breaks.clone();
        }
        if footer_has_default || !footer.is_empty() {
            inherited_footer = footer.clone();
            inherited_footer_line_spacing = footer_spacing.clone();
            inherited_footer_pagination = footer_pagination.clone();
            inherited_footer_tab_stops = footer_tabs.clone();
            inherited_footer_table_cell_line_spacing = footer_table_cell_spacing.clone();
            inherited_footer_table_cell_tab_stops = footer_table_cell_tabs.clone();
            inherited_footer_table_layout = footer_table_layout.clone();
            inherited_footer_column_breaks = footer_column_breaks.clone();
        }
        sections.push(SectionHeaderFooter {
            header,
            first_header: header_blocks.first,
            even_header: header_blocks.even,
            footer,
            first_footer: footer_blocks.first,
            even_footer: footer_blocks.even,
            line_spacing: crate::model::RunningSurfaceLineSpacingHints {
                header: header_spacing,
                header_table_cells: header_table_cell_spacing,
                first_header: header_line_spacing.first,
                first_header_table_cells: header_line_spacing.first_table_cells,
                even_header: header_line_spacing.even,
                even_header_table_cells: header_line_spacing.even_table_cells,
                footer: footer_spacing,
                footer_table_cells: footer_table_cell_spacing,
                first_footer: footer_line_spacing.first,
                first_footer_table_cells: footer_line_spacing.first_table_cells,
                even_footer: footer_line_spacing.even,
                even_footer_table_cells: footer_line_spacing.even_table_cells,
            },
            pagination: crate::model::RunningSurfacePaginationHints {
                header: header_pagination,
                first_header: header_pagination_parts.first,
                even_header: header_pagination_parts.even,
                footer: footer_pagination,
                first_footer: footer_pagination_parts.first,
                even_footer: footer_pagination_parts.even,
            },
            tab_stops: crate::model::RunningSurfaceTabStopHints {
                header: header_tabs,
                first_header: header_tab_stops.first,
                even_header: header_tab_stops.even,
                footer: footer_tabs,
                first_footer: footer_tab_stops.first,
                even_footer: footer_tab_stops.even,
            },
            table_cell_tab_stops: crate::model::RunningSurfaceTableCellTabStopHints {
                header: header_table_cell_tabs,
                first_header: header_table_cell_tab_stops.first,
                even_header: header_table_cell_tab_stops.even,
                footer: footer_table_cell_tabs,
                first_footer: footer_table_cell_tab_stops.first,
                even_footer: footer_table_cell_tab_stops.even,
            },
            table_layout: crate::model::RunningSurfaceTableLayoutHints {
                header: header_table_layout,
                first_header: header_table_layout_parts.first,
                even_header: header_table_layout_parts.even,
                footer: footer_table_layout,
                first_footer: footer_table_layout_parts.first,
                even_footer: footer_table_layout_parts.even,
            },
            column_breaks: crate::model::RunningSurfaceColumnBreakHints {
                header: header_column_breaks,
                first_header: header_column_break_parts.first,
                even_header: header_column_break_parts.even,
                footer: footer_column_breaks,
                first_footer: footer_column_break_parts.first,
                even_footer: footer_column_break_parts.even,
            },
            distances,
        });
    }

    let final_section = sections.last().cloned().unwrap_or_default();
    HeaderFooterRead {
        sections,
        final_section,
        records,
        comment_anchors,
        text_boxes,
        revisions,
        floating_shapes,
        fields: field_entries,
    }
}

fn extend_unique_header_footer_records(
    records: &mut Vec<HeaderFooter>,
    seen: &mut std::collections::HashSet<String>,
    next: Vec<HeaderFooter>,
) {
    for record in next {
        if seen.insert(record.id.clone()) {
            records.push(record);
        }
    }
}

fn extend_unique_text_box_records(
    records: &mut Vec<TextBox>,
    seen: &mut std::collections::HashSet<String>,
    next: Vec<TextBox>,
) {
    for record in next {
        if seen.insert(record.id.clone()) {
            records.push(record);
        }
    }
}

fn extend_unique_revision_records(records: &mut Vec<Revision>, next: Vec<Revision>) {
    for record in next {
        if !records.contains(&record) {
            records.push(record);
        }
    }
}

fn extend_unique_floating_shape_records(
    records: &mut Vec<FloatingShape>,
    next: Vec<FloatingShape>,
) {
    for record in next {
        if !records.contains(&record) {
            records.push(record);
        }
    }
}

fn apply_section_header_footers(blocks: &mut [Block], sections: &[SectionHeaderFooter]) {
    if sections.is_empty() {
        return;
    }
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let section_count = if sections.len() > section_break_count {
        section_break_count
    } else {
        sections.len()
    };
    let mut section_iter = sections[..section_count].iter();
    for block in blocks {
        if let Block::SectionBreak(setup) = block {
            let Some(section) = section_iter.next() else {
                break;
            };
            setup.header = section.header.clone();
            setup.first_header = section.first_header.clone();
            setup.even_header = section.even_header.clone();
            setup.footer = section.footer.clone();
            setup.first_footer = section.first_footer.clone();
            setup.even_footer = section.even_footer.clone();
        }
    }
}

fn running_line_spacing_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfaceLineSpacingHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfaceLineSpacingHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.line_spacing.clone();
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.line_spacing.clone();
    }
    aligned
}

fn running_pagination_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfacePaginationHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfacePaginationHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.pagination.clone();
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.pagination.clone();
    }
    aligned
}

fn running_tab_stops_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfaceTabStopHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfaceTabStopHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.tab_stops.clone();
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.tab_stops.clone();
    }
    aligned
}

fn running_table_cell_tab_stops_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfaceTableCellTabStopHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfaceTableCellTabStopHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.table_cell_tab_stops.clone();
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.table_cell_tab_stops.clone();
    }
    aligned
}

fn running_table_layout_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfaceTableLayoutHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfaceTableLayoutHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.table_layout.clone();
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.table_layout.clone();
    }
    aligned
}

fn running_column_breaks_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfaceColumnBreakHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfaceColumnBreakHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.column_breaks.clone();
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.column_breaks.clone();
    }
    aligned
}

fn running_surface_distances_by_model_section(
    blocks: &[Block],
    sections: &[SectionHeaderFooter],
) -> Vec<crate::model::RunningSurfaceDistanceHints> {
    let section_break_count = blocks
        .iter()
        .filter(|block| matches!(block, Block::SectionBreak(_)))
        .count();
    let mut aligned =
        vec![crate::model::RunningSurfaceDistanceHints::default(); section_break_count + 1];
    for (target, source) in aligned
        .iter_mut()
        .take(section_break_count)
        .zip(sections.iter())
    {
        *target = source.distances;
    }
    if let (Some(target), Some(source)) = (aligned.last_mut(), sections.last()) {
        *target = source.distances;
    }
    aligned
}

fn has_default_header_footer_ref(refs: &[body::HeaderFooterRef]) -> bool {
    refs.iter()
        .any(|reference| normalized_header_footer_type(&reference.type_name) == "default")
}

/// Read each unique referenced header/footer part once (dedup by part name), with
/// its own `_rels`/media so links and images inside the part resolve correctly.
#[derive(Clone, Copy)]
enum HeaderFooterPartKind {
    Header,
    Footer,
}

fn read_hf_parts(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    refs: &[body::HeaderFooterRef],
    part_kind: HeaderFooterPartKind,
    rels: &Rels,
    env: PartParseEnv<'_>,
) -> HeaderFooterPartRead {
    let PartParseEnv {
        styles,
        numbering,
        properties,
        preserve_legacy_form_cache,
    } = env;
    let mut seen_blocks = std::collections::HashSet::new();
    let mut seen_records = std::collections::HashSet::new();
    let mut seen_text_boxes = std::collections::HashSet::new();
    let mut seen_revisions = std::collections::HashSet::new();
    let mut seen_floating_shapes = std::collections::HashSet::new();
    let mut blocks = HeaderFooterBlocks::default();
    let mut line_spacing = HeaderFooterLineSpacing::default();
    let mut pagination = HeaderFooterPagination::default();
    let mut tab_stops = HeaderFooterTabStops::default();
    let mut table_cell_tab_stops = HeaderFooterTableCellTabStops::default();
    let mut table_layout = HeaderFooterTableLayout::default();
    let mut column_breaks = HeaderFooterColumnBreaks::default();
    let mut records = Vec::new();
    let mut comment_anchors = HashMap::new();
    let mut text_boxes = Vec::new();
    let mut revisions = Vec::new();
    let mut floating_shapes = Vec::new();
    let mut field_entries = Vec::new();
    let mut seen_fields = std::collections::HashSet::new();
    for reference in refs {
        let Some((target, external)) = rels.get(&reference.rel_id) else {
            continue;
        };
        if *external {
            continue;
        }
        let path = normalize_part(target);
        let Some(xml) = part(zip, &path) else {
            continue;
        };
        let part_rels = part(zip, &part_rels_path(&path))
            .map(|s| parse_rels(&s))
            .unwrap_or_default();
        let part_media = read_media(zip, &part_rels);
        let part_charts = read_charts(zip, &part_rels);
        let field_properties = fields::FieldDocumentProperties {
            core: properties.core,
            custom: properties.custom,
            variables: properties.variables,
            extended: properties.extended,
            file_size_bytes: properties.file_size_bytes,
        };
        let raw_ref_targets =
            fields::ref_targets_with_properties(&xml, field_properties, preserve_legacy_form_cache);
        let ref_position_context = fields::ref_position_context(&xml, numbering);
        let ref_number_context = fields::ref_number_context(&xml, numbering);
        let page_ref_context = fields::PageRefContext::empty();
        let note_ref_context = fields::note_ref_context_with_properties(
            &xml,
            &raw_ref_targets,
            field_properties,
            preserve_legacy_form_cache,
        );
        let ref_targets = fields::ref_targets_with_note_context(
            &xml,
            field_properties,
            preserve_legacy_form_cache,
            &note_ref_context,
        );
        let section_context = fields::section_context_with_properties(
            &xml,
            &ref_targets,
            field_properties,
            preserve_legacy_form_cache,
        );
        let style_ref_context = fields::style_ref_context_with_properties(
            &xml,
            styles,
            numbering,
            &fields::StyleRefResolutionSources {
                document_bookmarks: &ref_targets,
                note_refs: &note_ref_context,
                sections: &section_context,
            },
            field_properties,
            preserve_legacy_form_cache,
        );
        let legacy_form_context = fields::legacy_form_context(&xml, preserve_legacy_form_cache);
        let table_formula_context = fields::table_formula_context_with_properties(
            &xml,
            &ref_targets,
            &note_ref_context,
            &section_context,
            field_properties,
            preserve_legacy_form_cache,
        );
        let sequence_heading_context = fields::sequence_heading_context(&xml, styles);
        let toc_entries = fields::toc_entries_with_properties(
            &xml,
            styles,
            &ref_targets,
            &note_ref_context,
            &section_context,
            field_properties,
            preserve_legacy_form_cache,
        );
        let bookmark_names = fields::bookmark_names(&xml);
        let field_resolution_context = fields::FieldResolutionContext {
            properties: field_properties,
            document_bookmarks: &ref_targets,
            note_refs: &note_ref_context,
            sections: &section_context,
            style_refs: &style_ref_context,
            legacy_forms: &legacy_form_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
        };
        let hf_ctx = body::Ctx {
            styles,
            numbering,
            rels: &part_rels,
            media: &part_media,
            charts: &part_charts,
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
            core_properties: properties.core,
            custom_properties: properties.custom,
            document_variables: properties.variables,
            extended_properties: properties.extended,
            file_size_bytes: properties.file_size_bytes,
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
        let type_name = normalized_header_footer_type(&reference.type_name);
        extend_missing_comment_anchors(
            &mut comment_anchors,
            comments::parse_anchors(&xml, &field_resolution_context),
        );
        if seen_text_boxes.insert((path.clone(), type_name.to_string())) {
            text_boxes.extend(read_text_boxes_with_prefix(
                &xml,
                &hf_ctx,
                &[],
                &format!("{path}#{type_name}-text-box"),
            ));
        }
        if seen_revisions.insert((path.clone(), type_name.to_string())) {
            revisions.extend(revisions::parse(&xml, &field_resolution_context));
        }
        if seen_floating_shapes.insert((path.clone(), type_name.to_string())) {
            floating_shapes.extend(read_floating_shapes(
                &xml,
                ShapeFieldContext {
                    properties: fields::FieldDocumentProperties {
                        core: properties.core,
                        custom: properties.custom,
                        variables: properties.variables,
                        extended: properties.extended,
                        file_size_bytes: properties.file_size_bytes,
                    },
                    document_bookmarks: &ref_targets,
                    ref_positions: &ref_position_context,
                    ref_numbers: &ref_number_context,
                    page_refs: &page_ref_context,
                    note_refs: &note_ref_context,
                    sections: &section_context,
                    legacy_forms: &legacy_form_context,
                    table_formulas: &table_formula_context,
                    toc_entries: &toc_entries,
                    bookmark_names: &bookmark_names,
                    style_refs: &style_ref_context,
                    sequence_headings: &sequence_heading_context,
                },
            ));
        }
        if seen_fields.insert((path.clone(), type_name.to_string())) {
            field_entries.extend(fields::parse(
                &xml,
                styles,
                &toc_entries,
                numbering,
                fields::FieldDocumentProperties {
                    core: properties.core,
                    custom: properties.custom,
                    variables: properties.variables,
                    extended: properties.extended,
                    file_size_bytes: properties.file_size_bytes,
                },
                preserve_legacy_form_cache,
            ));
        }
        hf_ctx.begin_pagination_capture();
        let part_blocks = body::parse_hdrftr(&xml, &hf_ctx);
        let part_layout_hints = hf_ctx.take_layout_hints();
        if seen_blocks.insert((path.clone(), type_name.to_string())) {
            match type_name {
                "first" => {
                    blocks.first.extend(part_blocks.clone());
                    pagination
                        .first
                        .paragraphs
                        .extend(part_layout_hints.pagination);
                    pagination
                        .first
                        .table_rows
                        .extend(part_layout_hints.table_rows);
                    pagination
                        .first
                        .table_cells
                        .extend(part_layout_hints.table_cells);
                    line_spacing.first.extend(part_layout_hints.line_spacing);
                    line_spacing
                        .first_table_cells
                        .extend(part_layout_hints.table_cell_line_spacing);
                    tab_stops.first.extend(part_layout_hints.tab_stops);
                    table_cell_tab_stops
                        .first
                        .extend(part_layout_hints.table_cell_tabs);
                    table_layout
                        .first
                        .cell_column_breaks
                        .extend(part_layout_hints.table_cell_column_breaks);
                    table_layout
                        .first
                        .nested_tables
                        .extend(part_layout_hints.table_nested);
                    column_breaks
                        .first
                        .extend(part_layout_hints.column_break_offsets);
                }
                "even" => {
                    blocks.even.extend(part_blocks.clone());
                    pagination
                        .even
                        .paragraphs
                        .extend(part_layout_hints.pagination);
                    pagination
                        .even
                        .table_rows
                        .extend(part_layout_hints.table_rows);
                    pagination
                        .even
                        .table_cells
                        .extend(part_layout_hints.table_cells);
                    line_spacing.even.extend(part_layout_hints.line_spacing);
                    line_spacing
                        .even_table_cells
                        .extend(part_layout_hints.table_cell_line_spacing);
                    tab_stops.even.extend(part_layout_hints.tab_stops);
                    table_cell_tab_stops
                        .even
                        .extend(part_layout_hints.table_cell_tabs);
                    table_layout
                        .even
                        .cell_column_breaks
                        .extend(part_layout_hints.table_cell_column_breaks);
                    table_layout
                        .even
                        .nested_tables
                        .extend(part_layout_hints.table_nested);
                    column_breaks
                        .even
                        .extend(part_layout_hints.column_break_offsets);
                }
                _ => {
                    blocks.default.extend(part_blocks.clone());
                    pagination
                        .default
                        .paragraphs
                        .extend(part_layout_hints.pagination);
                    pagination
                        .default
                        .table_rows
                        .extend(part_layout_hints.table_rows);
                    pagination
                        .default
                        .table_cells
                        .extend(part_layout_hints.table_cells);
                    line_spacing.default.extend(part_layout_hints.line_spacing);
                    line_spacing
                        .default_table_cells
                        .extend(part_layout_hints.table_cell_line_spacing);
                    tab_stops.default.extend(part_layout_hints.tab_stops);
                    table_cell_tab_stops
                        .default
                        .extend(part_layout_hints.table_cell_tabs);
                    table_layout
                        .default
                        .cell_column_breaks
                        .extend(part_layout_hints.table_cell_column_breaks);
                    table_layout
                        .default
                        .nested_tables
                        .extend(part_layout_hints.table_nested);
                    column_breaks
                        .default
                        .extend(part_layout_hints.column_break_offsets);
                }
            }
        }
        if seen_records.insert((path.clone(), type_name.to_string())) {
            let text = blocks_text(&part_blocks);
            if !text.is_empty() {
                records.push(HeaderFooter {
                    id: format!("{path}#{type_name}"),
                    kind: header_footer_kind(part_kind, type_name),
                    section: None,
                    text,
                });
            }
        }
    }
    HeaderFooterPartRead {
        blocks,
        line_spacing,
        pagination,
        tab_stops,
        table_cell_tab_stops,
        table_layout,
        column_breaks,
        records,
        comment_anchors,
        text_boxes,
        revisions,
        floating_shapes,
        fields: field_entries,
    }
}

fn extend_missing_comment_anchors(
    anchors: &mut HashMap<String, TextAnchor>,
    next: HashMap<String, TextAnchor>,
) {
    for (id, anchor) in next {
        anchors.entry(id).or_insert(anchor);
    }
}

fn normalized_header_footer_type(value: &str) -> &'static str {
    let value = value.trim();
    match value {
        "first" => "first",
        "even" => "even",
        _ => "default",
    }
}

fn header_footer_kind(part_kind: HeaderFooterPartKind, type_name: &str) -> HeaderFooterKind {
    match (part_kind, type_name) {
        (HeaderFooterPartKind::Header, "first") => HeaderFooterKind::FirstPageHeader,
        (HeaderFooterPartKind::Header, "even") => HeaderFooterKind::EvenPageHeader,
        (HeaderFooterPartKind::Header, _) => HeaderFooterKind::Header,
        (HeaderFooterPartKind::Footer, "first") => HeaderFooterKind::FirstPageFooter,
        (HeaderFooterPartKind::Footer, "even") => HeaderFooterKind::EvenPageFooter,
        (HeaderFooterPartKind::Footer, _) => HeaderFooterKind::Footer,
    }
}

#[derive(Default)]
struct NotePartRead {
    blocks: Vec<Block>,
    source_entries: Vec<NoteSourceEntry>,
    #[cfg(feature = "render")]
    line_spacing: Vec<Option<crate::model::LineSpacingHint>>,
    #[cfg(feature = "render")]
    tab_stops: Vec<Vec<crate::model::TabStop>>,
    #[cfg(feature = "render")]
    table_cell_tab_stops: Vec<crate::model::TableCellTabStopHints>,
    records: Vec<Note>,
    comment_anchors: HashMap<String, TextAnchor>,
    revisions: Vec<Revision>,
    floating_shapes: Vec<FloatingShape>,
    text_boxes: Vec<TextBox>,
    fields: Vec<Field>,
}

struct NoteSourceEntry {
    id: String,
    kind: NoteKind,
    payload: Option<crate::model::NoteWritePayload>,
}

/// Read a footnotes/endnotes part (if present) into its real notes' blocks, with
/// the part's own rels/media so links and images inside notes resolve.
fn read_notes(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
    tag: &[u8],
    kind: NoteKind,
    env: PartParseEnv<'_>,
) -> NotePartRead {
    let PartParseEnv {
        styles,
        numbering,
        properties,
        preserve_legacy_form_cache,
    } = env;
    let Some(xml) = part(zip, name) else {
        return NotePartRead::default();
    };
    let part_rels = part(zip, &part_rels_path(name))
        .map(|s| parse_rels(&s))
        .unwrap_or_default();
    let part_media = read_media(zip, &part_rels);
    let part_charts = read_charts(zip, &part_rels);
    let field_properties = fields::FieldDocumentProperties {
        core: properties.core,
        custom: properties.custom,
        variables: properties.variables,
        extended: properties.extended,
        file_size_bytes: properties.file_size_bytes,
    };
    let raw_ref_targets =
        fields::ref_targets_with_properties(&xml, field_properties, preserve_legacy_form_cache);
    let ref_position_context = fields::ref_position_context(&xml, numbering);
    let ref_number_context = fields::ref_number_context(&xml, numbering);
    let page_ref_context = fields::PageRefContext::empty();
    let note_ref_context = fields::note_ref_context_with_properties(
        &xml,
        &raw_ref_targets,
        field_properties,
        preserve_legacy_form_cache,
    );
    let ref_targets = fields::ref_targets_with_note_context(
        &xml,
        field_properties,
        preserve_legacy_form_cache,
        &note_ref_context,
    );
    let section_context = fields::section_context_with_properties(
        &xml,
        &ref_targets,
        field_properties,
        preserve_legacy_form_cache,
    );
    let style_ref_context = fields::style_ref_context_with_properties(
        &xml,
        styles,
        numbering,
        &fields::StyleRefResolutionSources {
            document_bookmarks: &ref_targets,
            note_refs: &note_ref_context,
            sections: &section_context,
        },
        field_properties,
        preserve_legacy_form_cache,
    );
    let legacy_form_context = fields::legacy_form_context(&xml, preserve_legacy_form_cache);
    let table_formula_context = fields::table_formula_context_with_properties(
        &xml,
        &ref_targets,
        &note_ref_context,
        &section_context,
        field_properties,
        preserve_legacy_form_cache,
    );
    let sequence_heading_context = fields::sequence_heading_context(&xml, styles);
    let toc_entries = fields::toc_entries_with_properties(
        &xml,
        styles,
        &ref_targets,
        &note_ref_context,
        &section_context,
        field_properties,
        preserve_legacy_form_cache,
    );
    let bookmark_names = fields::bookmark_names(&xml);
    let field_resolution_context = fields::FieldResolutionContext {
        properties: field_properties,
        document_bookmarks: &ref_targets,
        note_refs: &note_ref_context,
        sections: &section_context,
        style_refs: &style_ref_context,
        legacy_forms: &legacy_form_context,
        toc_entries: &toc_entries,
        bookmark_names: &bookmark_names,
    };
    let ctx = body::Ctx {
        styles,
        numbering,
        rels: &part_rels,
        media: &part_media,
        charts: &part_charts,
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
        core_properties: properties.core,
        custom_properties: properties.custom,
        document_variables: properties.variables,
        extended_properties: properties.extended,
        file_size_bytes: properties.file_size_bytes,
        preserve_note_local_fields: true,
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
    let mut blocks = Vec::new();
    let mut records = Vec::new();
    let comment_anchors = comments::parse_anchors(&xml, &field_resolution_context);
    let revisions = revisions::parse(&xml, &field_resolution_context);
    let floating_shapes = read_floating_shapes(
        &xml,
        ShapeFieldContext {
            properties: fields::FieldDocumentProperties {
                core: properties.core,
                custom: properties.custom,
                variables: properties.variables,
                extended: properties.extended,
                file_size_bytes: properties.file_size_bytes,
            },
            document_bookmarks: &ref_targets,
            ref_positions: &ref_position_context,
            ref_numbers: &ref_number_context,
            page_refs: &page_ref_context,
            note_refs: &note_ref_context,
            sections: &section_context,
            legacy_forms: &legacy_form_context,
            table_formulas: &table_formula_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
            style_refs: &style_ref_context,
            sequence_headings: &sequence_heading_context,
        },
    );
    let text_box_id_prefix = format!("{name}-text-box");
    let text_boxes = read_text_boxes_with_prefix(&xml, &ctx, &floating_shapes, &text_box_id_prefix);
    let fields = fields::parse(
        &xml,
        styles,
        &toc_entries,
        numbering,
        fields::FieldDocumentProperties {
            core: properties.core,
            custom: properties.custom,
            variables: properties.variables,
            extended: properties.extended,
            file_size_bytes: properties.file_size_bytes,
        },
        preserve_legacy_form_cache,
    );
    ctx.begin_pagination_capture();
    let note_entries = body::parse_note_entries(&xml, &ctx, tag);
    let layout_hints = ctx.take_layout_hints();
    let mut source_entries = Vec::with_capacity(note_entries.len());
    let mut block_offset = 0usize;
    for (id, note_blocks) in note_entries {
        let text = blocks_text(&note_blocks);
        let payload = note_write_payload(kind, &text, &note_blocks, &layout_hints, block_offset);
        block_offset = block_offset.saturating_add(note_blocks.len());
        source_entries.push(NoteSourceEntry {
            id: id.clone(),
            kind,
            payload,
        });
        records.push(Note {
            id,
            kind,
            text,
            anchor: None,
        });
        blocks.extend(note_blocks);
    }
    NotePartRead {
        blocks,
        source_entries,
        #[cfg(feature = "render")]
        line_spacing: layout_hints.line_spacing,
        #[cfg(feature = "render")]
        tab_stops: layout_hints.tab_stops,
        #[cfg(feature = "render")]
        table_cell_tab_stops: layout_hints.table_cell_tabs,
        records,
        comment_anchors,
        revisions,
        floating_shapes,
        text_boxes,
        fields,
    }
}

fn note_write_payload(
    kind: NoteKind,
    text: &str,
    blocks: &[Block],
    hints: &body::BodyLayoutHints,
    block_offset: usize,
) -> Option<crate::model::NoteWritePayload> {
    if blocks.is_empty() {
        return None;
    }
    let block_end = block_offset.checked_add(blocks.len())?;
    let mut table_pagination = Vec::with_capacity(blocks.len());
    for (local_index, block) in blocks.iter().enumerate() {
        match block {
            Block::Paragraph(_) | Block::PageBreak => table_pagination.push(None),
            Block::Chart(chart) if note_write_chart_supported(chart) => table_pagination.push(None),
            Block::Table(table) if note_write_table_shape_supported(table) => {
                let index = block_offset.checked_add(local_index)?;
                table_pagination.push(Some(crate::model::TablePaginationHints {
                    rows: hints.table_rows.get(index)?.clone(),
                    cells: hints.table_cells.get(index)?.clone(),
                    cell_line_spacing: hints.table_cell_line_spacing.get(index)?.clone(),
                    cell_column_breaks: hints.table_cell_column_breaks.get(index)?.clone(),
                    nested: hints.table_nested.get(index)?.clone(),
                    cell_tabs: hints.table_cell_tabs.get(index)?.clone(),
                }));
            }
            _ => return None,
        }
    }
    Some(crate::model::NoteWritePayload {
        kind,
        text: text.to_string(),
        blocks: blocks.to_vec(),
        pagination: hints.pagination.get(block_offset..block_end)?.to_vec(),
        line_spacing: hints.line_spacing.get(block_offset..block_end)?.to_vec(),
        tab_stops: hints.tab_stops.get(block_offset..block_end)?.to_vec(),
        column_break_offsets: hints
            .column_break_offsets
            .get(block_offset..block_end)?
            .to_vec(),
        table_pagination,
    })
}

fn note_write_table_shape_supported(table: &crate::model::Table) -> bool {
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
                    Block::Paragraph(_) | Block::PageBreak => {}
                    Block::Chart(chart) if note_write_chart_supported(chart) => {}
                    Block::Table(table) => pending.push(table),
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

pub(crate) fn note_write_chart_supported(chart: &crate::model::Chart) -> bool {
    !chart.series.is_empty()
        && chart.series.iter().all(|series| {
            !series.values.is_empty()
                && series.values.iter().all(|value| value.is_finite())
                && series.bubble_sizes.iter().all(|value| value.is_finite())
        })
}

fn read_text_boxes(
    doc_xml: &str,
    ctx: &body::Ctx<'_>,
    floating_shapes: &[FloatingShape],
) -> Vec<TextBox> {
    read_text_boxes_with_prefix(doc_xml, ctx, floating_shapes, "docx-text-box")
}

fn read_text_boxes_with_prefix(
    doc_xml: &str,
    ctx: &body::Ctx<'_>,
    floating_shapes: &[FloatingShape],
    id_prefix: &str,
) -> Vec<TextBox> {
    let text_boxes: Vec<_> = body::parse_text_boxes(doc_xml, ctx)
        .into_iter()
        .enumerate()
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let ordered_anchors = ordered_text_box_anchors(&text_boxes, floating_shapes);
    text_boxes
        .into_iter()
        .enumerate()
        .map(|(text_box_index, (index, text))| TextBox {
            id: format!("{id_prefix}-{index}"),
            anchor: ordered_anchors
                .get(text_box_index)
                .and_then(|anchor| anchor.clone())
                .or_else(|| text_box_anchor(&text, floating_shapes)),
            text,
        })
        .collect()
}

fn ordered_text_box_anchors(
    text_boxes: &[(usize, String)],
    floating_shapes: &[FloatingShape],
) -> Vec<Option<TextAnchor>> {
    let text_box_shapes: Vec<_> = floating_shapes
        .iter()
        .filter(|shape| shape.text.is_some() && shape.anchor_text.is_some())
        .collect();
    if text_boxes.len() != text_box_shapes.len()
        || !text_boxes
            .iter()
            .map(|(_, text)| text.as_str())
            .zip(&text_box_shapes)
            .all(|(text, shape)| shape.text.as_deref() == Some(text))
    {
        return vec![None; text_boxes.len()];
    }
    text_box_shapes
        .into_iter()
        .map(text_anchor_from_shape)
        .collect()
}

fn text_box_anchor(text: &str, floating_shapes: &[FloatingShape]) -> Option<TextAnchor> {
    let mut matches = floating_shapes.iter().filter(|shape| {
        shape.text.as_deref() == Some(text) && shape.anchor_text.as_deref().is_some()
    });
    let shape = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    text_anchor_from_shape(shape)
}

fn text_anchor_from_shape(shape: &FloatingShape) -> Option<TextAnchor> {
    Some(TextAnchor {
        id: shape.id.clone(),
        text: shape.anchor_text.clone()?,
    })
}

/// Read-only field-resolution context threaded through the floating-shape
/// readers and their `ShapeFieldCursor` helpers. Bundles the document property
/// refs and every field-family context into one borrow so the shape scanners do
/// not pass a dozen individual parameters.
#[derive(Clone, Copy)]
struct ShapeFieldContext<'a> {
    properties: fields::FieldDocumentProperties<'a>,
    document_bookmarks: &'a HashMap<String, String>,
    ref_positions: &'a fields::RefPositionContext,
    ref_numbers: &'a fields::RefNumberContext,
    page_refs: &'a fields::PageRefContext,
    note_refs: &'a fields::NoteRefContext,
    sections: &'a fields::SectionContext,
    legacy_forms: &'a fields::LegacyFormContext,
    table_formulas: &'a fields::TableFormulaContext,
    toc_entries: &'a [fields::TocEntry],
    bookmark_names: &'a HashSet<String>,
    style_refs: &'a fields::StyleRefContext,
    sequence_headings: &'a fields::SequenceHeadingContext,
}

/// The per-field positions a `ShapeFieldCursor` resolves for one instruction,
/// bundled so `computed_shape_context_field_result` takes one value instead of
/// eight parallel `Option`s.
#[derive(Default)]
struct ShapeFieldPositions {
    ref_position: Option<fields::RefFieldPosition>,
    page_position: Option<fields::PageRefPosition>,
    page_ref_position: Option<fields::PageRefPosition>,
    page_ref_order: Option<usize>,
    note_ref_position: Option<fields::NoteRefFieldPosition>,
    ref_note_position: Option<fields::NoteRefFieldPosition>,
    section_position: Option<fields::SectionFieldPosition>,
    style_ref_position: Option<fields::StyleRefFieldPosition>,
}

fn read_floating_shapes(doc_xml: &str, cx: ShapeFieldContext<'_>) -> Vec<FloatingShape> {
    let mut r = Reader::from_str(doc_xml);
    let mut shapes = Vec::new();
    let mut shape_field_cursor = ShapeFieldCursor::default();
    let mut scan_depth = 0usize;
    let mut in_body = false;
    let mut body_depth = 0usize;
    let mut body_block_candidate_depths = vec![0usize];
    let mut next_body_block_index = 0usize;
    let mut current_body_block_index = None;
    let mut current_body_block_depth = None;
    let mut current_body_block_text = String::new();
    let mut current_body_block_shapes = Vec::new();
    let mut anchor_complex_field = FloatingAnchorComplexField::default();
    let mut anchor_field_state = fields::ContextlessFieldState::with_document_and_note_context(
        cx.properties,
        cx.document_bookmarks,
        cx.note_refs,
    )
    .with_toc_context(cx.toc_entries, cx.bookmark_names)
    .with_section_context(cx.sections)
    .with_legacy_form_context_from(cx.legacy_forms, 0);
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_redundant_alternate_branch(
                    &mut alternate_content_stack,
                    scan_depth,
                    name,
                ) {
                    skip_subtree(&mut r);
                    continue;
                }
                if is_old_revision_content(name) {
                    skip_subtree(&mut r);
                    continue;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.push(AlternateContentState {
                        branch_depth: scan_depth + 1,
                        took_branch: false,
                    });
                }
                if name == b"fldSimple" {
                    shape_field_cursor.start_simple_field(&e, scan_depth + 1, cx);
                } else if name == b"fldChar" {
                    shape_field_cursor.apply_field_char(&e, cx);
                }
                if name == b"body" {
                    in_body = true;
                    body_depth = 0;
                    body_block_candidate_depths.clear();
                    body_block_candidate_depths.push(0);
                    current_body_block_index = None;
                    current_body_block_depth = None;
                    current_body_block_text.clear();
                    current_body_block_shapes.clear();
                    anchor_complex_field = FloatingAnchorComplexField::default();
                    anchor_field_state.clear();
                    alternate_content_stack.clear();
                    scan_depth += 1;
                    continue;
                }
                if in_body {
                    if current_body_block_index.is_none()
                        && body_block_candidate_depths.contains(&body_depth)
                        && is_transparent_body_block_container(name)
                    {
                        body_block_candidate_depths.push(body_depth + 1);
                    }
                    if current_body_block_index.is_none()
                        && body_block_candidate_depths.contains(&body_depth)
                        && is_body_block(name)
                    {
                        current_body_block_index = Some(next_body_block_index);
                        current_body_block_depth = Some(body_depth + 1);
                        current_body_block_text.clear();
                        current_body_block_shapes.clear();
                        anchor_complex_field = FloatingAnchorComplexField::default();
                        anchor_field_state.clear();
                        next_body_block_index += 1;
                    }
                    body_depth += 1;
                }
                if in_body && current_body_block_index.is_some() && name == b"fldSimple" {
                    let simple_field_depth = scan_depth + 1;
                    if anchor_complex_field.suppresses_result() {
                        skip_subtree(&mut r);
                        shape_field_cursor.end_element(b"fldSimple", simple_field_depth);
                        body_depth = body_depth.saturating_sub(1);
                        continue;
                    } else if let Some(instruction) = attr_local_trimmed(&e, b"instr") {
                        if is_text_form_field_instruction(&instruction) {
                            let text = computed_floating_anchor_simple_text_form_field_text(
                                &mut r,
                                &instruction,
                                &mut anchor_field_state,
                            );
                            if let Some(text) = text {
                                append_floating_anchor_text(&mut current_body_block_text, &text);
                            }
                            shape_field_cursor.end_element(b"fldSimple", simple_field_depth);
                            body_depth = body_depth.saturating_sub(1);
                            continue;
                        } else if let Some(text) = computed_floating_anchor_field_text(
                            &instruction,
                            &mut anchor_field_state,
                        ) {
                            append_floating_anchor_text(&mut current_body_block_text, &text);
                            skip_subtree(&mut r);
                            shape_field_cursor.end_element(b"fldSimple", simple_field_depth);
                            body_depth = body_depth.saturating_sub(1);
                            continue;
                        }
                    }
                }
                if in_body && current_body_block_index.is_some() && name == b"fldChar" {
                    if let Some(text) =
                        anchor_complex_field.apply_field_char(&e, &mut anchor_field_state)
                    {
                        append_floating_anchor_text(&mut current_body_block_text, &text);
                    }
                    skip_subtree(&mut r);
                    body_depth = body_depth.saturating_sub(1);
                    continue;
                }
                if name == b"instrText" {
                    let instruction = read_text(&mut r);
                    shape_field_cursor.append_instruction_text(&instruction);
                    if in_body && current_body_block_index.is_some() {
                        anchor_complex_field.append_instruction_text(&instruction);
                    }
                    if in_body {
                        body_depth = body_depth.saturating_sub(1);
                    }
                    continue;
                }
                if name == b"anchor" {
                    let index = shapes.len();
                    let shape = read_floating_shape(
                        &mut r,
                        &e,
                        index,
                        current_body_block_index,
                        cx,
                        &mut shape_field_cursor,
                    );
                    anchor_field_state = anchor_field_state.with_legacy_form_context_from(
                        cx.legacy_forms,
                        shape_field_cursor.next_legacy_form_position(),
                    );
                    shapes.push(shape);
                    if current_body_block_index.is_some() {
                        current_body_block_shapes.push(FloatingShapeAnchorCandidate {
                            shape_index: index,
                            raw_prefix: current_body_block_text.clone(),
                        });
                    }
                    if in_body {
                        body_depth = body_depth.saturating_sub(1);
                    }
                    continue;
                }
                if in_body && current_body_block_index.is_some() && name == b"t" {
                    let text = read_text(&mut r);
                    anchor_complex_field.append_result_text(&text);
                    if !anchor_complex_field.suppresses_result() {
                        append_floating_anchor_text(&mut current_body_block_text, &text);
                    }
                    body_depth = body_depth.saturating_sub(1);
                    continue;
                }
                if in_body && current_body_block_index.is_some() {
                    if let Some(marker) = inline_marker_text(&e) {
                        anchor_complex_field.append_result_text(marker);
                        if !anchor_complex_field.suppresses_result() {
                            append_floating_anchor_text(&mut current_body_block_text, marker);
                        }
                        skip_subtree(&mut r);
                        body_depth = body_depth.saturating_sub(1);
                        continue;
                    }
                }
                if in_body && current_body_block_index.is_some() && name == b"sym" {
                    if let Some(ch) = floating_run_symbol_char(&e) {
                        anchor_complex_field.append_result_char(ch);
                    }
                    if !anchor_complex_field.suppresses_result() {
                        append_floating_anchor_symbol(&mut current_body_block_text, &e);
                    }
                    skip_subtree(&mut r);
                    body_depth = body_depth.saturating_sub(1);
                    continue;
                }
                scan_depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if in_body
                    && current_body_block_index.is_none()
                    && body_block_candidate_depths.contains(&body_depth)
                    && is_body_block(name)
                {
                    next_body_block_index += 1;
                }
                if name == b"anchor" {
                    let index = shapes.len();
                    shapes.push(floating_shape_shell(index, &e, current_body_block_index));
                    if current_body_block_index.is_some() {
                        current_body_block_shapes.push(FloatingShapeAnchorCandidate {
                            shape_index: index,
                            raw_prefix: current_body_block_text.clone(),
                        });
                    }
                } else if in_body && current_body_block_index.is_some() {
                    if name == b"fldChar" {
                        if let Some(text) =
                            anchor_complex_field.apply_field_char(&e, &mut anchor_field_state)
                        {
                            append_floating_anchor_text(&mut current_body_block_text, &text);
                        }
                    } else {
                        if let Some(marker) = inline_marker_text(&e) {
                            anchor_complex_field.append_result_text(marker);
                        } else if name == b"sym" {
                            if let Some(ch) = floating_run_symbol_char(&e) {
                                anchor_complex_field.append_result_char(ch);
                            }
                        }
                        if !anchor_complex_field.suppresses_result() {
                            append_floating_anchor_empty(
                                &mut current_body_block_text,
                                &e,
                                name,
                                &mut anchor_field_state,
                            );
                        }
                    }
                }
                if name == b"fldSimple" {
                    shape_field_cursor.empty_simple_field(&e, cx);
                } else if name == b"fldChar" {
                    shape_field_cursor.apply_field_char(&e, cx);
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                shape_field_cursor.end_element(name, scan_depth);
                if name == b"body" {
                    in_body = false;
                    body_depth = 0;
                    body_block_candidate_depths.clear();
                    body_block_candidate_depths.push(0);
                    current_body_block_index = None;
                    current_body_block_depth = None;
                    current_body_block_text.clear();
                    current_body_block_shapes.clear();
                    anchor_complex_field = FloatingAnchorComplexField::default();
                    anchor_field_state.clear();
                    alternate_content_stack.clear();
                    scan_depth = scan_depth.saturating_sub(1);
                    continue;
                }
                if name == b"AlternateContent"
                    && alternate_content_stack
                        .last()
                        .is_some_and(|state| state.branch_depth == scan_depth)
                {
                    alternate_content_stack.pop();
                }
                if in_body {
                    let ending_current_body_block = current_body_block_depth == Some(body_depth);
                    if ending_current_body_block {
                        apply_floating_anchor_text_with_offsets(
                            &mut shapes,
                            &current_body_block_shapes,
                            &current_body_block_text,
                        );
                    }
                    if body_block_candidate_depths.last().copied() == Some(body_depth) {
                        body_block_candidate_depths.pop();
                    }
                    body_depth = body_depth.saturating_sub(1);
                    if ending_current_body_block || body_depth == 0 {
                        current_body_block_index = None;
                        current_body_block_depth = None;
                        current_body_block_text.clear();
                        current_body_block_shapes.clear();
                        anchor_complex_field = FloatingAnchorComplexField::default();
                        anchor_field_state.clear();
                    }
                }
                scan_depth = scan_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    shapes
}

fn append_floating_anchor_text(out: &mut String, text: &str) {
    out.push_str(text);
}

fn append_floating_anchor_symbol(out: &mut String, e: &BytesStart<'_>) {
    if let Some(ch) = floating_run_symbol_char(e) {
        out.push(ch);
    }
}

fn append_floating_anchor_empty(
    out: &mut String,
    e: &BytesStart<'_>,
    name: &[u8],
    field_state: &mut fields::ContextlessFieldState<'_>,
) {
    if name == b"fldSimple" {
        if let Some(text) = computed_floating_anchor_simple_field_text(e, field_state) {
            append_floating_anchor_text(out, &text);
        }
    } else if name == b"sym" {
        append_floating_anchor_symbol(out, e);
    } else if let Some(marker) = inline_marker_text(e) {
        append_floating_anchor_text(out, marker);
    }
}

fn append_floating_anchor_empty_marker(out: &mut String, e: &BytesStart<'_>, name: &[u8]) {
    if name == b"sym" {
        append_floating_anchor_symbol(out, e);
    } else if let Some(marker) = inline_marker_text(e) {
        append_floating_anchor_text(out, marker);
    }
}

fn computed_floating_anchor_simple_field_text(
    e: &BytesStart<'_>,
    field_state: &mut fields::ContextlessFieldState<'_>,
) -> Option<String> {
    let instruction = attr_local_trimmed(e, b"instr")?;
    computed_floating_anchor_field_text(&instruction, field_state)
}

fn computed_floating_anchor_simple_text_form_field_text(
    r: &mut Reader<&[u8]>,
    instruction: &str,
    field_state: &mut fields::ContextlessFieldState<'_>,
) -> Option<String> {
    let current_result = read_floating_anchor_simple_field_current_result(r);
    field_state
        .computed_legacy_text_form_current_result(instruction, &current_result)
        .or_else(|| (!current_result.is_empty()).then_some(current_result))
}

fn read_floating_anchor_simple_field_current_result(r: &mut Reader<&[u8]>) -> String {
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
                    append_floating_anchor_symbol(&mut result, &e);
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
                append_floating_anchor_empty_marker(&mut result, &e, name);
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

fn computed_floating_anchor_field_text(
    instruction: &str,
    field_state: &mut fields::ContextlessFieldState<'_>,
) -> Option<String> {
    fields::computed_contextless_result(instruction, field_state)
}

#[derive(Default)]
struct FloatingAnchorComplexField {
    depth: usize,
    instruction: String,
    phase: Option<FloatingAnchorComplexFieldPhase>,
    computed_result: Option<String>,
    result_text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FloatingAnchorComplexFieldPhase {
    Instruction,
    Result,
}

impl FloatingAnchorComplexField {
    fn apply_field_char(
        &mut self,
        e: &BytesStart<'_>,
        field_state: &mut fields::ContextlessFieldState<'_>,
    ) -> Option<String> {
        match field_char_type(e).as_deref() {
            Some("begin") => {
                if self.depth == 0 {
                    self.instruction.clear();
                    self.result_text.clear();
                    self.phase = Some(FloatingAnchorComplexFieldPhase::Instruction);
                    self.computed_result = None;
                }
                self.depth += 1;
                None
            }
            Some("separate")
                if self.depth == 1
                    && self.phase == Some(FloatingAnchorComplexFieldPhase::Instruction) =>
            {
                self.phase = Some(FloatingAnchorComplexFieldPhase::Result);
                if !is_text_form_field_instruction(&self.instruction) {
                    self.computed_result =
                        computed_floating_anchor_field_text(&self.instruction, field_state);
                }
                self.computed_result.clone()
            }
            Some("end") => {
                let computed_text_form = if self.depth == 1
                    && self.phase == Some(FloatingAnchorComplexFieldPhase::Result)
                    && self.computed_result.is_none()
                    && is_text_form_field_instruction(&self.instruction)
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
        if self.depth == 1 && self.phase == Some(FloatingAnchorComplexFieldPhase::Instruction) {
            self.instruction.push_str(text);
        }
    }

    fn suppresses_result(&self) -> bool {
        self.depth > 0
            && self.phase == Some(FloatingAnchorComplexFieldPhase::Result)
            && (self.computed_result.is_some() || is_text_form_field_instruction(&self.instruction))
    }

    fn append_result_text(&mut self, text: &str) {
        if self.collects_result_text() {
            self.result_text.push_str(text);
        }
    }

    fn append_result_char(&mut self, ch: char) {
        if self.collects_result_text() {
            self.result_text.push(ch);
        }
    }

    fn collects_result_text(&self) -> bool {
        self.depth > 0
            && self.phase == Some(FloatingAnchorComplexFieldPhase::Result)
            && self.computed_result.is_none()
            && is_text_form_field_instruction(&self.instruction)
    }
}

#[derive(Debug, Clone)]
struct FloatingShapeAnchorCandidate {
    shape_index: usize,
    raw_prefix: String,
}

#[derive(Debug, Clone, Copy)]
struct AlternateContentState {
    branch_depth: usize,
    took_branch: bool,
}

#[derive(Debug, Default)]
struct ShapeFieldCursor {
    next_index: usize,
    ref_index: usize,
    sequence_index: usize,
    sequence_counters: HashMap<String, i64>,
    sequence_heading_scopes: HashMap<(String, u8), u32>,
    autonum_counter: i64,
    listnum_counter: i64,
    style_ref_index: usize,
    page_index: usize,
    page_ref_index: usize,
    note_ref_index: usize,
    section_index: usize,
    formula_index: usize,
    simple_field_depth: Option<usize>,
    simple_text_form: Option<ShapeSimpleTextFormField>,
    complex_field: Option<ShapeFieldCursorField>,
}

#[derive(Debug)]
struct ShapeSimpleTextFormField {
    instruction: String,
    result_text: String,
    legacy_form_index: usize,
    depth: usize,
}

#[derive(Debug)]
struct ShapeFieldCursorField {
    instruction: String,
    phase: ShapeFieldCursorPhase,
    computed_result: Option<String>,
    ref_indexed: bool,
    legacy_form_indexed: bool,
    style_ref_indexed: bool,
    page_indexed: bool,
    page_ref_indexed: bool,
    note_ref_indexed: bool,
    section_indexed: bool,
    formula_indexed: bool,
    result_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeFieldCursorPhase {
    Instruction,
    Result,
}

impl ShapeFieldCursor {
    fn start_simple_field(&mut self, e: &BytesStart<'_>, depth: usize, cx: ShapeFieldContext<'_>) {
        if self.simple_field_depth.is_some() {
            return;
        }
        self.simple_field_depth = Some(depth);
        if let Some(instruction) = attr_local(e, b"instr") {
            self.index_simple_field_instruction(&instruction, cx);
        }
    }

    fn empty_simple_field(&mut self, e: &BytesStart<'_>, cx: ShapeFieldContext<'_>) {
        if self.simple_field_depth.is_some() {
            return;
        }
        if let Some(instruction) = attr_local(e, b"instr") {
            self.index_simple_field_instruction(&instruction, cx);
        }
    }

    /// Advance every field-family cursor past one simple-field instruction so
    /// later fields resolve to their correct source-ordered position.
    fn index_simple_field_instruction(&mut self, instruction: &str, cx: ShapeFieldContext<'_>) {
        self.computed_sequence_result(instruction, cx.sequence_headings);
        self.computed_autonum_result(instruction);
        self.computed_listnum_result(instruction);
        self.next_ref_field_context(instruction, cx.ref_positions, cx.note_refs);
        self.next_style_ref_field_position(instruction, cx.style_refs);
        self.next_page_field_position(instruction, cx.page_refs);
        self.next_page_ref_field_context(instruction, cx.page_refs);
        self.next_note_ref_field_position(instruction, cx.note_refs);
        self.next_section_field_position(instruction, cx.sections);
        self.next_table_formula_result(instruction, cx.table_formulas);
        self.next_legacy_form_index(instruction);
    }

    fn end_element(&mut self, name: &[u8], depth: usize) {
        if name == b"fldSimple" && self.simple_field_depth == Some(depth) {
            self.simple_field_depth = None;
        }
    }

    fn start_simple_text_form_field(&mut self, instruction: &str, depth: usize) -> bool {
        if self.simple_text_form.is_some() || !is_text_form_field_instruction(instruction) {
            return false;
        }
        let Some(index) = self.next_legacy_form_index(instruction) else {
            return false;
        };
        self.simple_text_form = Some(ShapeSimpleTextFormField {
            instruction: instruction.to_string(),
            result_text: String::new(),
            legacy_form_index: index,
            depth,
        });
        true
    }

    fn append_simple_text_form_result_text(&mut self, text: &str) -> bool {
        let Some(field) = self.simple_text_form.as_mut() else {
            return false;
        };
        field.result_text.push_str(text);
        true
    }

    fn end_simple_text_form_field(
        &mut self,
        name: &[u8],
        depth: usize,
        legacy_forms: &fields::LegacyFormContext,
    ) -> Option<String> {
        if name != b"fldSimple"
            || self
                .simple_text_form
                .as_ref()
                .is_none_or(|field| field.depth != depth)
        {
            return None;
        }
        let field = self.simple_text_form.take()?;
        fields::computed_legacy_form_result(
            &field.instruction,
            &field.result_text,
            legacy_forms,
            field.legacy_form_index,
        )
        .or_else(|| (!field.result_text.is_empty()).then_some(field.result_text))
    }

    fn in_simple_text_form_field(&self) -> bool {
        self.simple_text_form.is_some()
    }

    fn apply_field_char(
        &mut self,
        e: &BytesStart<'_>,
        cx: ShapeFieldContext<'_>,
    ) -> Option<String> {
        if self.simple_field_depth.is_some() {
            return None;
        }
        match field_char_type(e).as_deref() {
            Some("begin") => {
                self.complex_field = Some(ShapeFieldCursorField {
                    instruction: String::new(),
                    phase: ShapeFieldCursorPhase::Instruction,
                    computed_result: None,
                    ref_indexed: false,
                    legacy_form_indexed: false,
                    style_ref_indexed: false,
                    page_indexed: false,
                    page_ref_indexed: false,
                    note_ref_indexed: false,
                    section_indexed: false,
                    formula_indexed: false,
                    result_text: String::new(),
                });
            }
            Some("separate") => {
                if let Some(field) = self.complex_field.as_mut() {
                    field.phase = ShapeFieldCursorPhase::Result;
                }
            }
            Some("end") => {
                if let Some(field) = self.complex_field.take() {
                    if field.computed_result.is_none() {
                        self.computed_sequence_result(&field.instruction, cx.sequence_headings);
                        self.computed_autonum_result(&field.instruction);
                        self.computed_listnum_result(&field.instruction);
                        if !field.ref_indexed {
                            self.next_ref_field_context(
                                &field.instruction,
                                cx.ref_positions,
                                cx.note_refs,
                            );
                        }
                        if !field.style_ref_indexed {
                            self.next_style_ref_field_position(&field.instruction, cx.style_refs);
                        }
                        if !field.page_indexed {
                            self.next_page_field_position(&field.instruction, cx.page_refs);
                        }
                        if !field.page_ref_indexed {
                            self.next_page_ref_field_context(&field.instruction, cx.page_refs);
                        }
                        if !field.note_ref_indexed {
                            self.next_note_ref_field_position(&field.instruction, cx.note_refs);
                        }
                        if !field.section_indexed {
                            self.next_section_field_position(&field.instruction, cx.sections);
                        }
                        if !field.formula_indexed {
                            self.next_table_formula_result(&field.instruction, cx.table_formulas);
                        }
                        if !field.legacy_form_indexed {
                            self.next_legacy_form_index(&field.instruction);
                        }
                    }
                    return field.computed_result;
                }
            }
            _ => {}
        }
        None
    }

    fn append_instruction_text(&mut self, text: &str) {
        if self.simple_field_depth.is_some() {
            return;
        }
        if let Some(field) = self.complex_field.as_mut() {
            if field.phase == ShapeFieldCursorPhase::Instruction {
                field.instruction.push_str(text);
            }
        }
    }

    fn current_complex_instruction(&self) -> Option<&str> {
        let field = self.complex_field.as_ref()?;
        (field.phase == ShapeFieldCursorPhase::Result).then_some(field.instruction.as_str())
    }

    fn current_complex_style_ref_position(
        &mut self,
        style_refs: &fields::StyleRefContext,
    ) -> Option<fields::StyleRefFieldPosition> {
        let field = self.complex_field.as_mut()?;
        if field.phase != ShapeFieldCursorPhase::Result || field.style_ref_indexed {
            return None;
        }
        let instruction = field.instruction.clone();
        if !fields::is_style_ref_field_instruction(&instruction) {
            return None;
        }
        field.style_ref_indexed = true;
        self.next_style_ref_field_position(&instruction, style_refs)
    }

    fn current_complex_page_field_position(
        &mut self,
        page_refs: &fields::PageRefContext,
    ) -> Option<fields::PageRefPosition> {
        let instruction = {
            let field = self.complex_field.as_ref()?;
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.page_indexed
            {
                return None;
            }
            field.instruction.clone()
        };
        let position = self.next_page_field_position(&instruction, page_refs);
        if let Some(field) = self.complex_field.as_mut() {
            if FieldKind::from_instruction(&instruction) == FieldKind::Page {
                field.page_indexed = true;
            }
        }
        position
    }

    fn current_complex_ref_field_context(
        &mut self,
        ref_positions: &fields::RefPositionContext,
        note_refs: &fields::NoteRefContext,
    ) -> (
        Option<fields::RefFieldPosition>,
        Option<fields::NoteRefFieldPosition>,
    ) {
        let instruction = {
            let Some(field) = self.complex_field.as_ref() else {
                return (None, None);
            };
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.ref_indexed
            {
                return (None, None);
            }
            field.instruction.clone()
        };
        let context = self.next_ref_field_context(&instruction, ref_positions, note_refs);
        if let Some(field) = self.complex_field.as_mut() {
            if fields::is_ref_position_field_instruction(&instruction) {
                field.ref_indexed = true;
            }
        }
        context
    }

    fn current_complex_page_ref_field_context(
        &mut self,
        page_refs: &fields::PageRefContext,
    ) -> (Option<fields::PageRefPosition>, Option<usize>) {
        let instruction = {
            let Some(field) = self.complex_field.as_ref() else {
                return (None, None);
            };
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.page_ref_indexed
            {
                return (None, None);
            }
            field.instruction.clone()
        };
        let context = self.next_page_ref_field_context(&instruction, page_refs);
        if let Some(field) = self.complex_field.as_mut() {
            if FieldKind::from_instruction(&instruction) == FieldKind::PageRef {
                field.page_ref_indexed = true;
            }
        }
        context
    }

    fn current_complex_note_ref_field_position(
        &mut self,
        note_refs: &fields::NoteRefContext,
    ) -> Option<fields::NoteRefFieldPosition> {
        let instruction = {
            let field = self.complex_field.as_ref()?;
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.note_ref_indexed
            {
                return None;
            }
            field.instruction.clone()
        };
        let position = self.next_note_ref_field_position(&instruction, note_refs);
        if let Some(field) = self.complex_field.as_mut() {
            if FieldKind::from_instruction(&instruction) == FieldKind::NoteRef {
                field.note_ref_indexed = true;
            }
        }
        position
    }

    fn current_complex_section_field_position(
        &mut self,
        sections: &fields::SectionContext,
    ) -> Option<fields::SectionFieldPosition> {
        let instruction = {
            let field = self.complex_field.as_ref()?;
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.section_indexed
            {
                return None;
            }
            field.instruction.clone()
        };
        let position = self.next_section_field_position(&instruction, sections);
        if let Some(field) = self.complex_field.as_mut() {
            if fields::is_section_field_instruction(&instruction) {
                field.section_indexed = true;
            }
        }
        position
    }

    fn current_complex_table_formula_result(
        &mut self,
        table_formulas: &fields::TableFormulaContext,
    ) -> Option<String> {
        let instruction = {
            let field = self.complex_field.as_ref()?;
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.formula_indexed
            {
                return None;
            }
            field.instruction.clone()
        };
        let result = self.next_table_formula_result(&instruction, table_formulas);
        if let Some(field) = self.complex_field.as_mut() {
            if is_table_formula_field_instruction(&instruction) {
                field.formula_indexed = true;
            }
        }
        result
    }

    fn set_complex_result(&mut self, text: String) {
        if let Some(field) = self.complex_field.as_mut() {
            if field.phase == ShapeFieldCursorPhase::Result {
                field.computed_result = Some(text);
            }
        }
    }

    fn append_complex_result_text(&mut self, text: &str) {
        if let Some(field) = self.complex_field.as_mut() {
            if field.phase == ShapeFieldCursorPhase::Result && field.computed_result.is_none() {
                field.result_text.push_str(text);
            }
        }
    }

    fn computed_complex_source_order_result(
        &mut self,
        sequence_headings: &fields::SequenceHeadingContext,
    ) -> Option<String> {
        let field = self.complex_field.as_ref()?;
        if field.phase != ShapeFieldCursorPhase::Result || field.computed_result.is_some() {
            return None;
        }
        let instruction = field.instruction.clone();
        self.computed_sequence_result(&instruction, sequence_headings)
            .or_else(|| self.computed_autonum_result(&instruction))
            .or_else(|| self.computed_listnum_result(&instruction))
    }

    fn computed_complex_legacy_form_result(
        &mut self,
        legacy_forms: &fields::LegacyFormContext,
        include_empty_text_form: bool,
    ) -> Option<String> {
        let (instruction, current_result) = {
            let field = self.complex_field.as_ref()?;
            if field.phase != ShapeFieldCursorPhase::Result
                || field.computed_result.is_some()
                || field.legacy_form_indexed
            {
                return None;
            }
            (field.instruction.clone(), field.result_text.clone())
        };
        let is_text_form = matches!(
            FieldKind::from_instruction(&instruction),
            FieldKind::FormField(kind) if kind == "FORMTEXT"
        );
        if is_text_form && (!include_empty_text_form || !current_result.is_empty()) {
            return None;
        }
        let index = self.next_legacy_form_index(&instruction)?;
        if let Some(field) = self.complex_field.as_mut() {
            field.legacy_form_indexed = true;
        }
        fields::computed_legacy_form_result(&instruction, &current_result, legacy_forms, index)
    }

    fn suppresses_complex_result(&self) -> bool {
        self.complex_field.as_ref().is_some_and(|field| {
            field.phase == ShapeFieldCursorPhase::Result && field.computed_result.is_some()
        })
    }

    fn next_legacy_form_index(&mut self, instruction: &str) -> Option<usize> {
        if !is_legacy_form_field_instruction(instruction) {
            return None;
        }
        let index = self.next_index;
        self.next_index += 1;
        Some(index)
    }

    fn next_legacy_form_position(&self) -> usize {
        self.next_index
    }

    fn next_style_ref_field_position(
        &mut self,
        instruction: &str,
        style_refs: &fields::StyleRefContext,
    ) -> Option<fields::StyleRefFieldPosition> {
        if !fields::is_style_ref_field_instruction(instruction) {
            return None;
        }
        let index = self.style_ref_index;
        self.style_ref_index += 1;
        style_refs.field_position(index)
    }

    fn next_ref_field_context(
        &mut self,
        instruction: &str,
        ref_positions: &fields::RefPositionContext,
        note_refs: &fields::NoteRefContext,
    ) -> (
        Option<fields::RefFieldPosition>,
        Option<fields::NoteRefFieldPosition>,
    ) {
        if !fields::is_ref_position_field_instruction(instruction) {
            return (None, None);
        }
        let index = self.ref_index;
        self.ref_index += 1;
        (
            ref_positions.field_position(index),
            note_refs.ref_field_position(index),
        )
    }

    fn next_page_field_position(
        &mut self,
        instruction: &str,
        page_refs: &fields::PageRefContext,
    ) -> Option<fields::PageRefPosition> {
        if FieldKind::from_instruction(instruction) != FieldKind::Page {
            return None;
        }
        let index = self.page_index;
        self.page_index += 1;
        page_refs.page_field_position(index)
    }

    fn next_page_ref_field_context(
        &mut self,
        instruction: &str,
        page_refs: &fields::PageRefContext,
    ) -> (Option<fields::PageRefPosition>, Option<usize>) {
        if FieldKind::from_instruction(instruction) != FieldKind::PageRef {
            return (None, None);
        }
        let index = self.page_ref_index;
        self.page_ref_index += 1;
        (
            page_refs.field_position(index),
            page_refs.field_order(index),
        )
    }

    fn next_note_ref_field_position(
        &mut self,
        instruction: &str,
        note_refs: &fields::NoteRefContext,
    ) -> Option<fields::NoteRefFieldPosition> {
        if FieldKind::from_instruction(instruction) != FieldKind::NoteRef {
            return None;
        }
        let index = self.note_ref_index;
        self.note_ref_index += 1;
        note_refs.field_position(index)
    }

    fn next_section_field_position(
        &mut self,
        instruction: &str,
        sections: &fields::SectionContext,
    ) -> Option<fields::SectionFieldPosition> {
        if !fields::is_section_field_instruction(instruction) {
            return None;
        }
        let index = self.section_index;
        self.section_index += 1;
        sections.field_position(index)
    }

    fn next_table_formula_result(
        &mut self,
        instruction: &str,
        table_formulas: &fields::TableFormulaContext,
    ) -> Option<String> {
        if !is_table_formula_field_instruction(instruction) {
            return None;
        }
        let index = self.formula_index;
        self.formula_index += 1;
        table_formulas.field_result(index)
    }

    fn computed_sequence_result(
        &mut self,
        instruction: &str,
        sequence_headings: &fields::SequenceHeadingContext,
    ) -> Option<String> {
        if FieldKind::from_instruction(instruction) != FieldKind::Sequence {
            return None;
        }
        let index = self.sequence_index;
        self.sequence_index += 1;
        let heading_scope = sequence_headings.field_scope(index);
        fields::computed_sequence_result_with_heading_scope(
            instruction,
            &mut self.sequence_counters,
            heading_scope,
            &mut self.sequence_heading_scopes,
        )
    }

    fn computed_autonum_result(&mut self, instruction: &str) -> Option<String> {
        if !matches!(
            FieldKind::from_instruction(instruction),
            FieldKind::Numbering(kind)
                if kind == "AUTONUM"
                    || kind == "AUTONUMLGL"
                    || kind == "AUTONUMOUT"
                    || kind == "BIDIOUTLINE"
        ) {
            return None;
        }
        fields::computed_numbering_result(instruction, &mut self.autonum_counter)
    }

    fn computed_listnum_result(&mut self, instruction: &str) -> Option<String> {
        if !matches!(
            FieldKind::from_instruction(instruction),
            FieldKind::Numbering(kind) if kind == "LISTNUM"
        ) {
            return None;
        }
        fields::computed_listnum_result(instruction, &mut self.listnum_counter)
    }
}

fn is_legacy_form_field_instruction(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::FormField(_)
    )
}

fn is_text_form_field_instruction(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::FormField(kind) if kind == "FORMTEXT"
    )
}

fn is_table_formula_field_instruction(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Dynamic(kind) if kind == "="
    )
}

fn should_skip_redundant_alternate_branch(
    stack: &mut [AlternateContentState],
    body_depth: usize,
    name: &[u8],
) -> bool {
    if !matches!(name, b"Choice" | b"Fallback") {
        return false;
    }
    let Some(state) = stack.last_mut() else {
        return false;
    };
    if state.branch_depth != body_depth {
        return false;
    }
    if state.took_branch {
        true
    } else {
        state.took_branch = true;
        false
    }
}

fn apply_floating_anchor_text_with_offsets(
    shapes: &mut [FloatingShape],
    shape_indices: &[FloatingShapeAnchorCandidate],
    raw: &str,
) {
    if shape_indices.is_empty() {
        return;
    }
    let text = text::finalize(raw);
    if text.is_empty() {
        return;
    }
    for index in shape_indices {
        if let Some(shape) = shapes.get_mut(index.shape_index) {
            shape.anchor_text = Some(text.clone());
            shape.anchor_char_offset = normalized_anchor_char_offset(raw, &index.raw_prefix);
        }
    }
}

fn normalized_anchor_char_offset(raw: &str, raw_prefix: &str) -> Option<usize> {
    let suffix = raw.get(raw_prefix.len()..)?;
    const MARKER: char = '\u{E000}';
    if raw.contains(MARKER) {
        return None;
    }
    let mut marked = String::with_capacity(raw.len() + MARKER.len_utf8());
    marked.push_str(raw_prefix);
    marked.push(MARKER);
    marked.push_str(suffix);
    let normalized = text::finalize(&marked);
    let marker_byte = normalized.find(MARKER)?;
    Some(normalized[..marker_byte].chars().count())
}

fn read_floating_shape(
    r: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    index: usize,
    anchor_block_index: Option<usize>,
    cx: ShapeFieldContext<'_>,
    shape_field_cursor: &mut ShapeFieldCursor,
) -> FloatingShape {
    let mut shape = floating_shape_shell(index, start, anchor_block_index);
    let mut text_box_depth = 0usize;
    let mut shape_text = String::new();
    let mut field_bookmarks = HashMap::new();
    let mut outline_depth = 0usize;
    let mut solid_fill = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if text_box_depth > 0 {
                    match name {
                        b"t" => {
                            let text = read_text(r);
                            if !shape_field_cursor.append_simple_text_form_result_text(&text)
                                && !shape_field_cursor.suppresses_complex_result()
                            {
                                shape_field_cursor.append_complex_result_text(&text);
                                append_shape_text(&mut shape_text, &text);
                            }
                        }
                        b"fldSimple" => {
                            if shape_field_cursor.suppresses_complex_result() {
                                skip_subtree(r);
                            } else if shape_field_cursor.in_simple_text_form_field() {
                                text_box_depth += 1;
                            } else if append_shape_simple_field(
                                &mut shape_text,
                                &e,
                                cx,
                                &mut field_bookmarks,
                                shape_field_cursor,
                                Some(text_box_depth + 1),
                            ) {
                                skip_subtree(r);
                            } else {
                                text_box_depth += 1;
                            }
                        }
                        b"fldChar" => {
                            apply_shape_field_char(
                                &mut shape_text,
                                &e,
                                cx,
                                &mut field_bookmarks,
                                shape_field_cursor,
                            );
                            text_box_depth += 1;
                        }
                        b"instrText" => {
                            shape_field_cursor.append_instruction_text(&read_text(r));
                        }
                        b"sym" => {
                            if let Some(text) = shape_symbol_text(&e) {
                                if !shape_field_cursor.append_simple_text_form_result_text(&text)
                                    && !shape_field_cursor.suppresses_complex_result()
                                {
                                    shape_field_cursor.append_complex_result_text(&text);
                                    append_shape_text(&mut shape_text, &text);
                                }
                            }
                            skip_subtree(r);
                        }
                        b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                            if let Some(text) = shape_empty_text(&e, name) {
                                if shape_field_cursor.append_simple_text_form_result_text(&text) {
                                    skip_subtree(r);
                                    continue;
                                }
                            }
                            if !shape_field_cursor.suppresses_complex_result() {
                                append_shape_empty(&mut shape_text, &e, name);
                            }
                            skip_subtree(r);
                        }
                        _ => text_box_depth += 1,
                    }
                    continue;
                }
                enter_shape_color_context(name, &mut outline_depth, &mut solid_fill);
                match name {
                    b"positionH" => shape.horizontal_position = Some(read_shape_position(r, &e)),
                    b"positionV" => shape.vertical_position = Some(read_shape_position(r, &e)),
                    b"simplePos" => shape.simple_position = shape_point(&e),
                    b"extent" => shape.extent = shape_extent(&e),
                    b"effectExtent" => shape.effect_extent = shape_effect_extent(&e),
                    b"docPr" => apply_shape_doc_pr(&mut shape, &e),
                    b"prstGeom" => apply_shape_preset_geometry(&mut shape, &e),
                    b"srgbClr" => apply_shape_srgb_color(&mut shape, &e, solid_fill),
                    b"txbxContent" => text_box_depth = 1,
                    name if is_shape_wrapping_name(name) => {
                        shape.wrapping = Some(read_shape_wrapping(r, &e));
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if text_box_depth > 0 {
                    if name == b"fldChar" {
                        apply_shape_field_char(
                            &mut shape_text,
                            &e,
                            cx,
                            &mut field_bookmarks,
                            shape_field_cursor,
                        );
                    }
                    if let Some(text) = shape_empty_text(&e, name) {
                        if shape_field_cursor.append_simple_text_form_result_text(&text) {
                            continue;
                        }
                    }
                    if !shape_field_cursor.suppresses_complex_result()
                        && (name != b"fldSimple"
                            || !append_shape_simple_field(
                                &mut shape_text,
                                &e,
                                cx,
                                &mut field_bookmarks,
                                shape_field_cursor,
                                None,
                            ))
                    {
                        append_shape_empty(&mut shape_text, &e, name);
                    }
                    continue;
                }
                if name == b"srgbClr" {
                    apply_shape_srgb_color(&mut shape, &e, solid_fill);
                }
                match name {
                    b"positionH" => shape.horizontal_position = Some(empty_shape_position(&e)),
                    b"positionV" => shape.vertical_position = Some(empty_shape_position(&e)),
                    b"simplePos" => shape.simple_position = shape_point(&e),
                    b"extent" => shape.extent = shape_extent(&e),
                    b"effectExtent" => shape.effect_extent = shape_effect_extent(&e),
                    b"docPr" => apply_shape_doc_pr(&mut shape, &e),
                    b"prstGeom" => apply_shape_preset_geometry(&mut shape, &e),
                    name if is_shape_wrapping_name(name) => {
                        shape.wrapping = Some(shape_wrapping(&e));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"anchor" => break,
            Ok(Event::End(e)) if text_box_depth > 0 => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if let Some(text) = shape_field_cursor.end_simple_text_form_field(
                    name,
                    text_box_depth,
                    cx.legacy_forms,
                ) {
                    if !text.is_empty() {
                        append_shape_text(&mut shape_text, &text);
                    }
                }
                if name == b"p" {
                    append_shape_paragraph_break(&mut shape_text);
                }
                text_box_depth = text_box_depth.saturating_sub(1);
            }
            Ok(Event::End(_)) => {
                leave_shape_color_context(&mut outline_depth, &mut solid_fill);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    shape.text = finalized_shape_text(shape_text);
    shape
}

fn floating_shape_shell(
    index: usize,
    start: &BytesStart<'_>,
    anchor_block_index: Option<usize>,
) -> FloatingShape {
    FloatingShape {
        id: format!("docx-floating-shape-{index}"),
        name: None,
        description: None,
        text: None,
        preset_geometry: None,
        fill_color: None,
        outline_color: None,
        simple_position_enabled: attr_bool(start, b"simplePos"),
        simple_position: None,
        effect_extent: None,
        anchor_block_index,
        anchor_text: None,
        anchor_char_offset: None,
        extent: None,
        horizontal_position: None,
        vertical_position: None,
        relative_height: attr_i64(start, b"relativeHeight"),
        behind_doc: attr_bool(start, b"behindDoc"),
        layout_in_cell: attr_bool(start, b"layoutInCell"),
        locked: attr_bool(start, b"locked"),
        allow_overlap: attr_bool(start, b"allowOverlap"),
        distance: ShapeDistance {
            top_emu: attr_i64(start, b"distT"),
            bottom_emu: attr_i64(start, b"distB"),
            left_emu: attr_i64(start, b"distL"),
            right_emu: attr_i64(start, b"distR"),
        },
        wrapping: None,
    }
}

fn is_body_block(name: &[u8]) -> bool {
    matches!(name, b"p" | b"tbl")
}

fn is_transparent_body_block_container(name: &[u8]) -> bool {
    matches!(
        name,
        b"sdt"
            | b"sdtContent"
            | b"customXml"
            | b"smartTag"
            | b"ins"
            | b"moveTo"
            | b"AlternateContent"
            | b"Choice"
            | b"Fallback"
    )
}

fn is_old_revision_content(name: &[u8]) -> bool {
    matches!(name, b"del" | b"moveFrom")
}

fn append_shape_text(out: &mut String, text: &str) {
    let previous_is_space = matches!(out.chars().last(), Some(' ' | '\n' | '\t'));
    let previous_is_joiner = out.ends_with('-') || out.ends_with('\u{00ad}');
    let next_is_space = matches!(text.chars().next(), Some(' ' | '\n' | '\t'));
    if !out.is_empty() && !previous_is_space && !previous_is_joiner && !next_is_space {
        out.push(' ');
    }
    out.push_str(text);
}

fn append_shape_symbol(out: &mut String, e: &BytesStart<'_>) {
    if let Some(text) = shape_symbol_text(e) {
        append_shape_text(out, &text);
    }
}

fn shape_symbol_text(e: &BytesStart<'_>) -> Option<String> {
    let ch = floating_run_symbol_char(e)?;
    let mut buf = [0; 4];
    Some(ch.encode_utf8(&mut buf).to_string())
}

fn shape_empty_text(e: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    match name {
        b"sym" => shape_symbol_text(e),
        b"tab" => Some("\t".to_string()),
        b"br" | b"cr" => Some("\n".to_string()),
        b"noBreakHyphen" => Some("-".to_string()),
        b"softHyphen" => Some("\u{00ad}".to_string()),
        _ => None,
    }
}

fn computed_shape_ref_result(
    instruction: &str,
    cx: ShapeFieldContext<'_>,
    field_bookmarks: &HashMap<String, String>,
    ref_position: Option<fields::RefFieldPosition>,
    note_ref_position: Option<fields::NoteRefFieldPosition>,
) -> Option<String> {
    let ctx = fields::RefResultContext {
        bookmarks: cx.document_bookmarks,
        ref_positions: cx.ref_positions,
        ref_numbers: cx.ref_numbers,
        note_refs: cx.note_refs,
        field_bookmarks,
    };
    fields::computed_ref_result(instruction, &ctx, ref_position.clone(), note_ref_position).or_else(
        || {
            fields::computed_direct_bookmark_ref_result(
                instruction,
                &ctx,
                ref_position,
                note_ref_position,
            )
        },
    )
}

fn computed_shape_context_field_result(
    instruction: &str,
    cx: ShapeFieldContext<'_>,
    field_bookmarks: &mut HashMap<String, String>,
    positions: ShapeFieldPositions,
) -> Option<String> {
    let properties = cx.properties;
    let document_bookmarks = cx.document_bookmarks;
    if update_field_bookmarks_from_instruction(instruction, field_bookmarks) {
        return Some(String::new());
    }
    fields::computed_formula_result_with_bookmark_context(
        instruction,
        document_bookmarks,
        field_bookmarks,
    )
    .or_else(|| fields::computed_page_result(instruction, positions.page_position))
    .or_else(|| {
        fields::computed_page_ref_result(
            instruction,
            cx.page_refs,
            positions.page_ref_position,
            positions.page_ref_order,
        )
    })
    .or_else(|| {
        fields::computed_note_ref_result(instruction, cx.note_refs, positions.note_ref_position)
    })
    .or_else(|| fields::computed_section_result(instruction, positions.section_position))
    .or_else(|| {
        fields::computed_if_compare_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| {
        fields::computed_merge_control_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| {
        computed_shape_ref_result(
            instruction,
            cx,
            field_bookmarks,
            positions.ref_position,
            positions.ref_note_position,
        )
    })
    .or_else(|| fields::computed_dynamic_result_with_bookmarks(instruction, field_bookmarks))
    .or_else(|| fields::computed_toc_entry_result(instruction))
    .or_else(|| {
        fields::computed_document_info_result(
            instruction,
            properties.core,
            properties.custom,
            properties.variables,
            properties.extended,
            properties.file_size_bytes,
        )
    })
    .or_else(|| fields::computed_revision_number_result(instruction, properties.core))
    .or_else(|| {
        fields::computed_style_ref_result(instruction, cx.style_refs, positions.style_ref_position)
    })
    .or_else(|| fields::computed_display_result(instruction))
    .or_else(|| fields::computed_action_result(instruction))
    .or_else(|| fields::computed_reference_index_result(instruction))
    .or_else(|| fields::computed_toc_result(instruction, cx.toc_entries, cx.bookmark_names))
}

fn apply_shape_field_char(
    out: &mut String,
    e: &BytesStart<'_>,
    cx: ShapeFieldContext<'_>,
    field_bookmarks: &mut HashMap<String, String>,
    shape_field_cursor: &mut ShapeFieldCursor,
) {
    if field_char_type(e).as_deref() == Some("end") {
        let computed =
            shape_field_cursor.computed_complex_legacy_form_result(cx.legacy_forms, true);
        if let Some(text) = computed {
            shape_field_cursor.set_complex_result(text);
        }
    }
    let completed = shape_field_cursor.apply_field_char(e, cx);
    let computed = shape_field_cursor
        .current_complex_instruction()
        .map(str::to_string)
        .and_then(|instruction| {
            let page_position =
                shape_field_cursor.current_complex_page_field_position(cx.page_refs);
            let (ref_position, ref_note_position) = shape_field_cursor
                .current_complex_ref_field_context(cx.ref_positions, cx.note_refs);
            let (page_ref_position, page_ref_order) =
                shape_field_cursor.current_complex_page_ref_field_context(cx.page_refs);
            let note_ref_position =
                shape_field_cursor.current_complex_note_ref_field_position(cx.note_refs);
            let section_position =
                shape_field_cursor.current_complex_section_field_position(cx.sections);
            let style_ref_position =
                shape_field_cursor.current_complex_style_ref_position(cx.style_refs);
            shape_field_cursor
                .current_complex_table_formula_result(cx.table_formulas)
                .or_else(|| {
                    computed_shape_context_field_result(
                        &instruction,
                        cx,
                        field_bookmarks,
                        ShapeFieldPositions {
                            ref_position,
                            page_position,
                            page_ref_position,
                            page_ref_order,
                            note_ref_position,
                            ref_note_position,
                            section_position,
                            style_ref_position,
                        },
                    )
                })
        })
        .or_else(|| shape_field_cursor.computed_complex_source_order_result(cx.sequence_headings))
        .or_else(|| shape_field_cursor.computed_complex_legacy_form_result(cx.legacy_forms, false));
    if let Some(text) = computed {
        shape_field_cursor.set_complex_result(text);
    }
    if let Some(text) = completed {
        if !text.is_empty() {
            append_shape_text(out, &text);
        }
    }
}

fn append_shape_simple_field(
    out: &mut String,
    e: &BytesStart<'_>,
    cx: ShapeFieldContext<'_>,
    field_bookmarks: &mut HashMap<String, String>,
    shape_field_cursor: &mut ShapeFieldCursor,
    simple_text_form_depth: Option<usize>,
) -> bool {
    let Some(instruction) = attr_local(e, b"instr") else {
        return false;
    };
    if update_field_bookmarks_from_instruction(&instruction, field_bookmarks) {
        return true;
    }
    if let Some(depth) = simple_text_form_depth {
        if shape_field_cursor.start_simple_text_form_field(&instruction, depth) {
            return false;
        }
    }
    let page_position = shape_field_cursor.next_page_field_position(&instruction, cx.page_refs);
    let (ref_position, ref_note_position) =
        shape_field_cursor.next_ref_field_context(&instruction, cx.ref_positions, cx.note_refs);
    let (page_ref_position, page_ref_order) =
        shape_field_cursor.next_page_ref_field_context(&instruction, cx.page_refs);
    let note_ref_position =
        shape_field_cursor.next_note_ref_field_position(&instruction, cx.note_refs);
    let section_position =
        shape_field_cursor.next_section_field_position(&instruction, cx.sections);
    let style_ref_position =
        shape_field_cursor.next_style_ref_field_position(&instruction, cx.style_refs);
    let text = shape_field_cursor
        .next_table_formula_result(&instruction, cx.table_formulas)
        .or_else(|| {
            computed_shape_context_field_result(
                &instruction,
                cx,
                field_bookmarks,
                ShapeFieldPositions {
                    ref_position,
                    page_position,
                    page_ref_position,
                    page_ref_order,
                    note_ref_position,
                    ref_note_position,
                    section_position,
                    style_ref_position,
                },
            )
        })
        .or_else(|| shape_field_cursor.computed_sequence_result(&instruction, cx.sequence_headings))
        .or_else(|| shape_field_cursor.computed_autonum_result(&instruction))
        .or_else(|| shape_field_cursor.computed_listnum_result(&instruction))
        .or_else(|| {
            let index = shape_field_cursor.next_legacy_form_index(&instruction)?;
            fields::computed_legacy_form_result(&instruction, "", cx.legacy_forms, index)
        });
    let Some(text) = text else {
        return false;
    };
    if !text.is_empty() {
        append_shape_text(out, &text);
    }
    true
}

fn append_shape_empty(out: &mut String, e: &BytesStart<'_>, name: &[u8]) {
    match name {
        b"sym" => append_shape_symbol(out, e),
        b"tab" => out.push('\t'),
        b"br" | b"cr" => out.push('\n'),
        b"noBreakHyphen" => out.push('-'),
        b"softHyphen" => out.push('\u{00ad}'),
        _ => {}
    }
}

fn append_shape_paragraph_break(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn finalized_shape_text(text: String) -> Option<String> {
    let text = text.trim_matches('\n').to_string();
    (!text.trim().is_empty()).then_some(text)
}

fn floating_run_symbol_char(e: &BytesStart<'_>) -> Option<char> {
    let value = attr_local_trimmed(e, b"char")?;
    let font = attr_local_trimmed(e, b"font");
    fields::computed_run_symbol_char(font.as_deref(), &value)
}

fn empty_shape_position(start: &BytesStart<'_>) -> ShapePosition {
    ShapePosition {
        relative_from: attr_local_trimmed(start, b"relativeFrom"),
        offset_emu: None,
        align: None,
    }
}

fn read_shape_position(r: &mut Reader<&[u8]>, start: &BytesStart<'_>) -> ShapePosition {
    let mut position = empty_shape_position(start);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"posOffset" => {
                position.offset_emu = read_i64_text(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"align" => {
                position.align = Some(read_text(r));
            }
            Ok(Event::End(e))
                if matches!(local(e.name().as_ref()), b"positionH" | b"positionV") =>
            {
                break;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    position
}

fn shape_extent(e: &BytesStart<'_>) -> Option<ShapeExtent> {
    Some(ShapeExtent {
        cx_emu: attr_i64(e, b"cx")?,
        cy_emu: attr_i64(e, b"cy")?,
    })
}

fn shape_point(e: &BytesStart<'_>) -> Option<ShapePoint> {
    Some(ShapePoint {
        x_emu: attr_i64(e, b"x")?,
        y_emu: attr_i64(e, b"y")?,
    })
}

fn shape_effect_extent(e: &BytesStart<'_>) -> Option<ShapeEffectExtent> {
    Some(ShapeEffectExtent {
        left_emu: attr_i64(e, b"l")?,
        top_emu: attr_i64(e, b"t")?,
        right_emu: attr_i64(e, b"r")?,
        bottom_emu: attr_i64(e, b"b")?,
    })
}

fn is_shape_wrapping_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"wrapNone" | b"wrapSquare" | b"wrapTight" | b"wrapThrough" | b"wrapTopAndBottom"
    )
}

fn shape_wrapping(e: &BytesStart<'_>) -> ShapeWrapping {
    let kind = match local(e.name().as_ref()) {
        b"wrapNone" => "none",
        b"wrapSquare" => "square",
        b"wrapTight" => "tight",
        b"wrapThrough" => "through",
        b"wrapTopAndBottom" => "topAndBottom",
        _ => "unknown",
    };
    ShapeWrapping {
        kind: kind.to_string(),
        text: attr_local_trimmed(e, b"wrapText"),
        distance: ShapeDistance {
            top_emu: attr_i64(e, b"distT"),
            bottom_emu: attr_i64(e, b"distB"),
            left_emu: attr_i64(e, b"distL"),
            right_emu: attr_i64(e, b"distR"),
        },
        polygon: Vec::new(),
    }
}

fn read_shape_wrapping(r: &mut Reader<&[u8]>, start: &BytesStart<'_>) -> ShapeWrapping {
    let mut wrapping = shape_wrapping(start);
    let qname = start.name();
    let wrap_name = local(qname.as_ref()).to_vec();
    loop {
        match r.read_event() {
            Ok(Event::Empty(e)) if is_wrap_polygon_point(local(e.name().as_ref())) => {
                if let Some(point) = shape_point(&e) {
                    wrapping.polygon.push(point);
                }
            }
            Ok(Event::Start(e)) if is_wrap_polygon_point(local(e.name().as_ref())) => {
                if let Some(point) = shape_point(&e) {
                    wrapping.polygon.push(point);
                }
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"wrapPolygon" => {}
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == wrap_name.as_slice() => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    wrapping
}

fn is_wrap_polygon_point(name: &[u8]) -> bool {
    matches!(name, b"start" | b"lineTo")
}

fn apply_shape_doc_pr(shape: &mut FloatingShape, e: &BytesStart<'_>) {
    if let Some(id) = attr_local_trimmed(e, b"id") {
        shape.id = id;
    }
    shape.name = attr_local_trimmed(e, b"name");
    shape.description = attr_local_trimmed(e, b"descr");
}

fn apply_shape_preset_geometry(shape: &mut FloatingShape, e: &BytesStart<'_>) {
    if shape.preset_geometry.is_none() {
        shape.preset_geometry = attr_local_trimmed(e, b"prst");
    }
}

#[derive(Debug, Clone, Copy)]
enum ShapeColorTarget {
    Fill,
    Outline,
}

fn enter_shape_color_context(
    name: &[u8],
    outline_depth: &mut usize,
    solid_fill: &mut Option<(usize, ShapeColorTarget)>,
) {
    if *outline_depth > 0 || name == b"ln" {
        *outline_depth += 1;
    }
    if name == b"solidFill" {
        let target = if *outline_depth > 0 {
            ShapeColorTarget::Outline
        } else {
            ShapeColorTarget::Fill
        };
        *solid_fill = Some((1, target));
    } else if let Some((depth, _)) = solid_fill.as_mut() {
        *depth += 1;
    }
}

fn leave_shape_color_context(
    outline_depth: &mut usize,
    solid_fill: &mut Option<(usize, ShapeColorTarget)>,
) {
    if let Some((depth, _)) = solid_fill.as_mut() {
        *depth = depth.saturating_sub(1);
        if *depth == 0 {
            *solid_fill = None;
        }
    }
    *outline_depth = outline_depth.saturating_sub(1);
}

fn apply_shape_srgb_color(
    shape: &mut FloatingShape,
    e: &BytesStart<'_>,
    solid_fill: Option<(usize, ShapeColorTarget)>,
) {
    let Some((_, target)) = solid_fill else {
        return;
    };
    let Some(color) = attr_local(e, b"val").and_then(|value| parse_rgb_hex_color(&value)) else {
        return;
    };
    match target {
        ShapeColorTarget::Fill if shape.fill_color.is_none() => shape.fill_color = Some(color),
        ShapeColorTarget::Outline if shape.outline_color.is_none() => {
            shape.outline_color = Some(color);
        }
        _ => {}
    }
}

pub(crate) fn parse_rgb_hex_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if value.len() != 6 {
        return None;
    }
    let rgb = u32::from_str_radix(value, 16).ok()?;
    Some(Color {
        r: (rgb >> 16) as u8,
        g: (rgb >> 8) as u8,
        b: rgb as u8,
    })
}

pub(crate) fn attr_i64(e: &BytesStart<'_>, key: &[u8]) -> Option<i64> {
    attr_local(e, key)?.trim().parse().ok()
}

pub(crate) fn attr_i32(e: &BytesStart<'_>, key: &[u8]) -> Option<i32> {
    attr_local(e, key)?.trim().parse().ok()
}

pub(crate) fn attr_u8(e: &BytesStart<'_>, key: &[u8]) -> Option<u8> {
    attr_local(e, key)?.trim().parse().ok()
}

pub(crate) fn attr_u16(e: &BytesStart<'_>, key: &[u8]) -> Option<u16> {
    attr_local(e, key)?.trim().parse().ok()
}

pub(crate) fn attr_f32(e: &BytesStart<'_>, key: &[u8]) -> Option<f32> {
    attr_local(e, key)?.trim().parse().ok()
}

pub(crate) fn attr_u32(e: &BytesStart<'_>, key: &[u8]) -> Option<u32> {
    attr_local(e, key)?.trim().parse().ok()
}

pub(crate) fn attr_usize(e: &BytesStart<'_>, key: &[u8]) -> Option<usize> {
    attr_local(e, key)?.trim().parse().ok()
}

fn attr_bool(e: &BytesStart<'_>, key: &[u8]) -> Option<bool> {
    attr_local(e, key).map(|value| toggle_on(Some(value)))
}

fn parse_core_properties(xml: &str) -> CoreProperties {
    let mut r = Reader::from_str(xml);
    let mut props = CoreProperties::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let key = local(e.name().as_ref()).to_vec();
                if is_core_property_key(&key) {
                    set_core_property_value(&mut props, &key, read_text(&mut r));
                }
            }
            Ok(Event::Empty(e)) => {
                let key = local(e.name().as_ref()).to_vec();
                if is_core_property_key(&key) {
                    set_core_property_value(&mut props, &key, String::new());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn is_core_property_key(key: &[u8]) -> bool {
    matches!(
        key,
        b"title"
            | b"subject"
            | b"creator"
            | b"description"
            | b"keywords"
            | b"category"
            | b"contentStatus"
            | b"lastModifiedBy"
            | b"created"
            | b"modified"
            | b"lastPrinted"
            | b"revision"
            | b"version"
    )
}

fn set_core_property_value(props: &mut CoreProperties, key: &[u8], value: String) {
    match key {
        b"title" => props.title = Some(value),
        b"subject" => props.subject = Some(value),
        b"creator" => props.creator = Some(value),
        b"description" => props.description = Some(value),
        b"keywords" => props.keywords = Some(value),
        b"category" => props.category = Some(value),
        b"contentStatus" => props.content_status = Some(value),
        b"lastModifiedBy" => props.last_modified_by = Some(value),
        b"created" => props.created = Some(value),
        b"modified" => props.modified = Some(value),
        b"lastPrinted" => props.last_printed = Some(value),
        b"revision" => props.revision = Some(value),
        b"version" => props.version = Some(value),
        _ => {}
    }
}

fn parse_custom_properties(xml: &str) -> BTreeMap<String, String> {
    let mut r = Reader::from_str(xml);
    let mut props = BTreeMap::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"property" => {
                if let Some(name) = attr_local_trimmed(&e, b"name") {
                    if let Some(value) = read_custom_property_value(&mut r) {
                        props.insert(name, value);
                    }
                } else {
                    skip_subtree(&mut r);
                }
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"property" => {
                if let Some(name) = attr_local_trimmed(&e, b"name") {
                    props.insert(name, String::new());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_custom_xml_items(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Vec<CustomXmlItem> {
    let mut names = Vec::new();
    for index in 0..zip.len() {
        if let Ok(file) = zip.by_index(index) {
            let name = file.name().to_string();
            if let Some(number) = custom_xml_item_number(&name) {
                names.push((number, name));
            }
        }
    }
    names.sort_by_key(|(number, _)| *number);
    names
        .into_iter()
        .filter_map(|(number, name)| {
            let xml = part(zip, &name)?;
            let store_item_id = part(zip, &format!("customXml/itemProps{number}.xml"))
                .and_then(|props| custom_xml_item_id(&props))
                .unwrap_or_default();
            Some(CustomXmlItem { store_item_id, xml })
        })
        .collect()
}

fn custom_xml_item_number(name: &str) -> Option<usize> {
    name.strip_prefix("customXml/item")?
        .strip_suffix(".xml")?
        .parse()
        .ok()
}

fn custom_xml_item_id(xml: &str) -> Option<String> {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"datastoreItem" =>
            {
                return attr_local_trimmed(&e, b"itemID");
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

fn parse_extended_properties(xml: &str) -> HashMap<String, String> {
    let mut r = Reader::from_str(xml);
    let mut props = HashMap::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let key = local(e.name().as_ref()).to_vec();
                if is_extended_property_key(&key) {
                    if let Ok(name) = std::str::from_utf8(&key) {
                        props.insert(document_property_key(name), read_text(&mut r));
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let key = local(e.name().as_ref()).to_vec();
                if is_extended_property_key(&key) {
                    if let Ok(name) = std::str::from_utf8(&key) {
                        props.insert(document_property_key(name), String::new());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn is_extended_property_key(key: &[u8]) -> bool {
    matches!(
        key,
        b"Application"
            | b"AppVersion"
            | b"Characters"
            | b"CharactersWithSpaces"
            | b"Company"
            | b"DocSecurity"
            | b"HiddenSlides"
            | b"HyperlinkBase"
            | b"HyperlinksChanged"
            | b"Lines"
            | b"LinksUpToDate"
            | b"Manager"
            | b"MMClips"
            | b"Notes"
            | b"Pages"
            | b"Paragraphs"
            | b"PresentationFormat"
            | b"ScaleCrop"
            | b"SharedDoc"
            | b"Slides"
            | b"Template"
            | b"TotalTime"
            | b"Words"
    )
}

fn parse_document_variables(xml: &str) -> HashMap<String, String> {
    let mut r = Reader::from_str(xml);
    let mut vars = HashMap::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"docVar" => {
                if let Some(name) = attr_local_trimmed(&e, b"name") {
                    vars.insert(
                        document_property_key(&name),
                        attr_local(&e, b"val").unwrap_or_default(),
                    );
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    vars
}

fn settings_preserves_legacy_form_cache(xml: &str) -> bool {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"documentProtection" =>
            {
                let edit_forms = attr_local(&e, b"edit")
                    .as_deref()
                    .is_some_and(|edit| edit.trim().eq_ignore_ascii_case("forms"));
                if edit_forms
                    && attr_local(&e, b"enforcement").is_some_and(|value| toggle_on(Some(value)))
                {
                    return true;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    false
}

fn parse_document_id(xml: &str) -> Option<String> {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"docId" => {
                return attr_local(&e, b"val")
                    .map(|id| id.trim().to_owned())
                    .filter(|id| !id.is_empty());
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

#[cfg(any(feature = "render", test))]
fn parse_default_tab_stop_pt(xml: &str) -> Option<f32> {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"defaultTabStop" =>
            {
                return attr_local(&e, b"val")
                    .and_then(|value| value.trim().parse::<f32>().ok())
                    .filter(|twips| twips.is_finite() && *twips > 0.0)
                    .map(|twips| twips / 20.0)
                    .filter(|points| points.is_finite() && *points > 0.0);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    None
}

fn read_custom_property_value(r: &mut Reader<&[u8]>) -> Option<String> {
    let mut value = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(_)) if value.is_none() => {
                value = Some(read_text(r));
            }
            Ok(Event::Empty(_)) if value.is_none() => {
                value = Some(String::new());
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"property" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    value
}

/// `word/header1.xml` → `word/_rels/header1.xml.rels`.
fn part_rels_path(part_path: &str) -> String {
    match part_path.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part_path}.rels"),
    }
}

/// Largest accepted *decompressed* size for an XML part — orders of magnitude
/// above any real document (a 64 MiB `document.xml` is a ~50,000-page doc), but
/// bounds a zip bomb. We reject a part whose declared uncompressed size already
/// exceeds this (rather than silently truncating it), and `take` still caps the
/// actual read in case the ZIP's declared size lies.
const MAX_XML_PART: u64 = 64 << 20;
/// Largest accepted embedded media (image) entry.
const MAX_MEDIA_PART: u64 = 64 << 20;
/// Whole-archive budget for decompressed media. Per-entry caps alone don't bound a
/// hostile package with thousands of large image relationships; this caps the
/// cumulative media inflation across all entries.
const MAX_TOTAL_MEDIA: u64 = 256 << 20;

/// Read a ZIP entry to a UTF-8 string, if present — bounded to guard against a
/// zip bomb (a tiny entry that decompresses to gigabytes).
fn part(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Option<String> {
    let f = zip.by_name(name).ok()?;
    if f.size() > MAX_XML_PART {
        return None;
    }
    let mut s = String::new();
    f.take(MAX_XML_PART).read_to_string(&mut s).ok()?;
    Some(s)
}

/// Read a ZIP entry to raw bytes (for media), bounded like [`part`].
fn part_bytes(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let f = zip.by_name(name).ok()?;
    if f.size() > MAX_MEDIA_PART {
        return None;
    }
    let mut v = Vec::new();
    f.take(MAX_MEDIA_PART).read_to_end(&mut v).ok()?;
    Some(v)
}

/// Cap on relationships the lenient reader path collects from one `.rels` part — bounds
/// memory on a size-capped but record-stuffed part (the package layer caps separately).
const MAX_REL_RECORDS: usize = 1 << 16;

/// `word/_rels/document.xml.rels`: `<Relationship Id Target TargetMode?/>`.
fn parse_rels(xml: &str) -> Rels {
    let mut r = Reader::from_str(xml);
    let mut map = HashMap::new();
    loop {
        if map.len() >= MAX_REL_RECORDS {
            break; // bounded: stop collecting (lenient read path)
        }
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"Relationship" =>
            {
                if let (Some(id), Some(target)) = (
                    attr_local_trimmed(&e, b"Id"),
                    attr_local_trimmed(&e, b"Target"),
                ) {
                    let external = attr_local_trimmed(&e, b"TargetMode")
                        .is_some_and(|value| value == "External");
                    map.insert(id, (target, external));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    map
}

const MAX_MODELED_CHARTS_PER_STORY: usize = 256;

fn read_charts(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    rels: &Rels,
) -> HashMap<String, Chart> {
    let mut chart_rels = rels
        .iter()
        .filter(|(_, (_, external))| !external)
        .filter_map(|(id, (target, _))| {
            let path = normalize_part(target);
            is_authored_chart_part(&path).then_some((id.clone(), path))
        })
        .collect::<Vec<_>>();
    chart_rels.sort_unstable();
    chart_rels.truncate(MAX_MODELED_CHARTS_PER_STORY);
    let mut parsed_by_path: HashMap<String, Chart> = HashMap::new();
    let mut charts = HashMap::new();
    for (id, path) in chart_rels {
        let parsed = if let Some(chart) = parsed_by_path.get(&path) {
            Some(chart.clone())
        } else {
            let chart = part(zip, &path).and_then(|xml| chart::parse(&xml));
            if let Some(chart) = chart.as_ref() {
                parsed_by_path.insert(path, chart.clone());
            }
            chart
        };
        if let Some(chart) = parsed {
            charts.insert(id, chart);
        }
    }
    charts
}

fn is_authored_chart_part(path: &str) -> bool {
    let Some(file) = path.strip_prefix("word/charts/") else {
        return false;
    };
    let Some(stem) = file.strip_suffix(".xml") else {
        return false;
    };
    ["chart", "chartEx"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
    })
}

/// Pre-read every embedded raster image referenced by an internal relationship
/// into `rel-id → Image`. Metafiles are extracted only when they are a bounded
/// single-DIB wrapper; vector metafiles stay placeholders.
fn read_media(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    rels: &Rels,
) -> HashMap<String, Image> {
    let mut media = HashMap::new();
    // Collect first to avoid borrowing `rels` while mutably borrowing `zip`.
    let image_rels: Vec<(String, String)> = rels
        .iter()
        .filter(|(_, (target, external))| {
            !external
                && (mime_for(target).is_some()
                    || crate::metafile::format_for_part(target).is_some())
        })
        .map(|(id, (target, _))| (id.clone(), target.clone()))
        .collect();
    let mut total: u64 = 0;
    for (id, target) in image_rels {
        let path = normalize_part(&target);
        if let Some(bytes) = part_bytes(zip, &path) {
            if let Some(mime) = mime_for(&target) {
                // Stop BEFORE inserting a part that would push the in-memory media set past
                // the whole-archive budget, so the advertised cap is a hard ceiling (not
                // cap + one part). `part_bytes` already bounds each part to MAX_MEDIA_PART.
                if total.saturating_add(bytes.len() as u64) > MAX_TOTAL_MEDIA {
                    break;
                }
                total = total.saturating_add(bytes.len() as u64);
                let (width_px, height_px) = crate::image::dims(&bytes, mime).unzip();
                media.insert(
                    id,
                    Image {
                        alt: None,
                        bytes: Some(bytes),
                        mime: Some(mime.to_string()),
                        width_px,
                        height_px,
                        rotation_degrees: None,
                        floating_offset_emu: None,
                    },
                );
                continue;
            }
            let Some((kind, compressed_by_extension)) = crate::metafile::format_for_part(&target)
            else {
                continue;
            };
            let compressed = compressed_by_extension || crate::metafile::is_gzip_payload(&bytes);
            let Some(raster) = crate::metafile::extract_raster(kind, &bytes, compressed) else {
                continue;
            };
            // Stop BEFORE inserting a part that would push the in-memory media set past
            // the whole-archive budget, so the advertised cap is a hard ceiling (not
            // cap + one part). Metafile wrappers store the decoded RGBA payload.
            if total.saturating_add(raster.rgba.len() as u64) > MAX_TOTAL_MEDIA {
                break;
            }
            total = total.saturating_add(raster.rgba.len() as u64);
            media.insert(
                id,
                Image {
                    alt: None,
                    bytes: Some(raster.rgba),
                    mime: Some(crate::image::MIME_RAW_RGBA.to_string()),
                    width_px: Some(raster.width_px),
                    height_px: Some(raster.height_px),
                    rotation_degrees: None,
                    floating_offset_emu: None,
                },
            );
        }
    }
    media
}

/// A `word/document.xml.rels` relationship target → a ZIP entry name, resolving it
/// relative to the `word/` directory and normalizing `.`/`..`/leading-`/` per OPC URI
/// rules: `media/image1.png` → `word/media/image1.png`, `/word/header1.xml` →
/// `word/header1.xml`, `../customXml/item1.xml` → `customXml/item1.xml`, `./media/x.png`
/// → `word/media/x.png`. A target escaping the package root yields the joined remainder.
fn normalize_part(target: &str) -> String {
    // `/`-absolute targets are package-root relative; others are relative to `word/`.
    let base: &[&str] = if target.starts_with('/') {
        &[]
    } else {
        &["word"]
    };
    let mut segs: Vec<&str> = base.to_vec();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/")
}

/// MIME type for a media target by extension, restricted to the rasters the
/// `.doc` path also extracts. `None` ⇒ not a normal encoded raster.
fn mime_for(target: &str) -> Option<&'static str> {
    let ext = target.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Flat text of just the main body (excludes headers/footers).
fn body_text(model: &DocModel) -> String {
    blocks_text(&model.blocks)
}

pub(crate) fn blocks_text(blocks: &[Block]) -> String {
    let mut raw = String::new();
    flatten(blocks, &mut raw);
    text::finalize(&raw)
}

fn exact_fresh_conversion_notes(
    blocks: &[Block],
    notes: &[Note],
    source_entries: &[NoteSourceEntry],
    doc_xml: &str,
    ctx: &fields::FieldResolutionContext<'_>,
) -> Option<FreshConversionNotes> {
    if notes.is_empty() {
        return None;
    }
    let footnotes = body::scan_note_ref_positions(doc_xml, b"footnoteReference", ctx);
    let endnotes = body::scan_note_ref_positions(doc_xml, b"endnoteReference", ctx);
    if footnotes.block_count != blocks.len() || endnotes.block_count != blocks.len() {
        return None;
    }

    let mut note_by_key = HashMap::new();
    for note in notes {
        if note.text.is_empty()
            || note_by_key
                .insert((note_kind_key(note.kind), note.id.as_str()), note)
                .is_some()
        {
            return None;
        }
    }
    let mut source_by_key = HashMap::new();
    for entry in source_entries {
        if source_by_key
            .insert((note_kind_key(entry.kind), entry.id.as_str()), entry)
            .is_some()
        {
            return None;
        }
    }

    let mut references = footnotes
        .references
        .into_iter()
        .map(|reference| (NoteKind::Footnote, reference))
        .chain(
            endnotes
                .references
                .into_iter()
                .map(|reference| (NoteKind::Endnote, reference)),
        )
        .collect::<Vec<_>>();
    if references.len() != notes.len() {
        return None;
    }
    references.sort_by_key(|(_, reference)| (reference.block_index, reference.text_offset));

    let mut seen = HashSet::new();
    let mut notes_by_block = (0..blocks.len())
        .map(|_| Vec::<(usize, AuthoredNote, Option<crate::model::NoteWritePayload>)>::new())
        .collect::<Vec<_>>();
    for (kind, reference) in references {
        if reference.custom_mark
            || !reference.paragraph
            || !seen.insert((note_kind_key(kind), reference.id.clone()))
        {
            return None;
        }
        let note = note_by_key.get(&(note_kind_key(kind), reference.id.as_str()))?;
        let source = source_by_key.get(&(note_kind_key(kind), reference.id.as_str()))?;
        let Some(Block::Paragraph(paragraph)) = blocks.get(reference.block_index) else {
            return None;
        };
        if paragraph.text() != reference.block_text {
            return None;
        }
        notes_by_block[reference.block_index].push((
            reference.text_offset,
            AuthoredNote {
                kind,
                text: note.text.clone(),
            },
            source.payload.clone(),
        ));
    }

    let mut promoted = blocks.to_vec();
    let mut payloads = Vec::with_capacity(promoted.len());
    for (block_index, mut notes) in notes_by_block.into_iter().enumerate() {
        if notes.is_empty() {
            payloads.push(match &promoted[block_index] {
                Block::Paragraph(paragraph) => vec![None; paragraph.runs.len()],
                _ => Vec::new(),
            });
            continue;
        }
        notes.sort_by_key(|(offset, _, _)| *offset);
        let Block::Paragraph(paragraph) = &mut promoted[block_index] else {
            return None;
        };
        let authored = notes
            .iter()
            .map(|(offset, note, _)| (*offset, note.clone()))
            .collect::<Vec<_>>();
        if !crate::attach_authored_notes_to_paragraph(paragraph, authored) {
            return None;
        }
        let mut expected = notes.into_iter();
        let mut run_payloads = Vec::with_capacity(paragraph.runs.len());
        for run in &paragraph.runs {
            let Some(attached) = &run.note else {
                run_payloads.push(None);
                continue;
            };
            let (_, expected_note, payload) = expected.next()?;
            if attached != &expected_note {
                return None;
            }
            run_payloads.push(payload);
        }
        if expected.next().is_some() {
            return None;
        }
        payloads.push(run_payloads);
    }
    Some(FreshConversionNotes {
        body_blocks: promoted,
        payloads,
    })
}

fn note_kind_key(kind: NoteKind) -> u8 {
    match kind {
        NoteKind::Footnote => 0,
        NoteKind::Endnote => 1,
    }
}

fn attach_note_reference_anchors(
    notes: &mut [Note],
    doc_xml: &str,
    ctx: &fields::FieldResolutionContext<'_>,
) {
    let footnote_refs = body::scan_note_ref_anchors(doc_xml, b"footnoteReference", ctx);
    let endnote_refs = body::scan_note_ref_anchors(doc_xml, b"endnoteReference", ctx);
    for note in notes {
        let anchor_text = match note.kind {
            NoteKind::Footnote => footnote_refs.get(&note.id),
            NoteKind::Endnote => endnote_refs.get(&note.id),
        };
        if let Some(text) = anchor_text {
            note.anchor = Some(TextAnchor {
                id: note.id.clone(),
                text: text.clone(),
            });
        }
    }
}

/// Flat text of the running headers and footers only.
pub(crate) fn header_footer_text(model: &DocModel) -> String {
    let mut raw = String::new();
    flatten_header_footer_surfaces(model, &mut raw);
    text::finalize(&raw)
}

fn flatten_header_footer_surfaces(model: &DocModel, out: &mut String) {
    for block in &model.blocks {
        if let Block::SectionBreak(section) = block {
            flatten(&section.header, out);
            flatten(&section.first_header, out);
            flatten(&section.even_header, out);
            flatten(&section.footer, out);
            flatten(&section.first_footer, out);
            flatten(&section.even_footer, out);
        }
    }
    flatten(&model.setup.header, out);
    flatten(&model.setup.first_header, out);
    flatten(&model.setup.even_header, out);
    flatten(&model.setup.footer, out);
    flatten(&model.setup.first_footer, out);
    flatten(&model.setup.even_footer, out);
}

fn package_text_part(package: &crate::opc::Package, name: &str) -> Option<String> {
    String::from_utf8(package.part(name)?).ok()
}

pub(crate) fn main_text_with_revision_view(state: &DocxState, view: crate::RevisionView) -> String {
    let Some(doc_xml) = package_text_part(&state.package, "word/document.xml") else {
        return state.main_text.clone();
    };
    let numbering = package_text_part(&state.package, "word/numbering.xml")
        .map(|xml| numbering::parse(&xml))
        .unwrap_or_default();
    let core_properties = package_text_part(&state.package, "docProps/core.xml")
        .map(|xml| parse_core_properties(&xml))
        .unwrap_or_else(|| state.core_properties.clone());
    let custom_properties = package_text_part(&state.package, "docProps/custom.xml")
        .map(|xml| parse_custom_properties(&xml))
        .unwrap_or_default();
    let custom_property_fields = custom_properties
        .iter()
        .map(|(key, value)| (document_property_key(key), value.clone()))
        .collect::<HashMap<_, _>>();
    let settings_xml = package_text_part(&state.package, "word/settings.xml");
    let document_variables = settings_xml
        .as_deref()
        .map(parse_document_variables)
        .unwrap_or_default();
    let preserve_legacy_form_cache = settings_xml
        .as_deref()
        .is_some_and(settings_preserves_legacy_form_cache);
    let note_numbering = settings_xml
        .as_deref()
        .map(fields::note_numbering_from_settings)
        .unwrap_or_default();
    let extended_properties = package_text_part(&state.package, "docProps/app.xml")
        .map(|xml| parse_extended_properties(&xml))
        .unwrap_or_default();
    let properties = fields::FieldDocumentProperties {
        core: &core_properties,
        custom: &custom_property_fields,
        variables: &document_variables,
        extended: &extended_properties,
        file_size_bytes: Some(state.source_size_bytes),
    };
    let styles = package_text_part(&state.package, "word/styles.xml")
        .map(|xml| styles::parse(&xml))
        .unwrap_or_default();
    let raw_document_bookmarks =
        fields::ref_targets_with_properties(&doc_xml, properties, preserve_legacy_form_cache);
    let note_ref_context = fields::note_ref_context_with_numbering(
        &doc_xml,
        &raw_document_bookmarks,
        properties,
        preserve_legacy_form_cache,
        note_numbering,
    );
    let document_bookmarks = fields::ref_targets_with_note_context(
        &doc_xml,
        properties,
        preserve_legacy_form_cache,
        &note_ref_context,
    );
    let section_context = fields::section_context_with_properties(
        &doc_xml,
        &document_bookmarks,
        properties,
        preserve_legacy_form_cache,
    );
    let style_ref_context = fields::style_ref_context_with_properties(
        &doc_xml,
        &styles,
        &numbering,
        &fields::StyleRefResolutionSources {
            document_bookmarks: &document_bookmarks,
            note_refs: &note_ref_context,
            sections: &section_context,
        },
        properties,
        preserve_legacy_form_cache,
    );
    let toc_entries = fields::toc_entries_with_properties(
        &doc_xml,
        &styles,
        &document_bookmarks,
        &note_ref_context,
        &section_context,
        properties,
        preserve_legacy_form_cache,
    );
    let legacy_form_context = fields::legacy_form_context(&doc_xml, preserve_legacy_form_cache);
    let bookmark_names = fields::bookmark_names(&doc_xml);
    revisions::main_text_with_view(
        &doc_xml,
        view,
        Some(&fields::FieldResolutionContext {
            properties,
            document_bookmarks: &document_bookmarks,
            note_refs: &note_ref_context,
            sections: &section_context,
            style_refs: &style_ref_context,
            legacy_forms: &legacy_form_context,
            toc_entries: &toc_entries,
            bookmark_names: &bookmark_names,
        }),
    )
}

fn flatten(blocks: &[Block], out: &mut String) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                out.push_str(&p.text());
                out.push('\n');
            }
            Block::PageBreak | Block::SectionBreak(_) => out.push('\n'),
            Block::Image(_) | Block::Chart(_) => {}
            Block::Table(t) => {
                for row in &t.rows {
                    for (i, cell) in row.cells.iter().enumerate() {
                        if i > 0 {
                            out.push('\t');
                        }
                        flatten_inline(&cell.blocks, out);
                    }
                    out.push('\n');
                }
            }
        }
    }
}

/// Flatten a cell's content to a single line (paragraphs and nested-table cells
/// space-joined) so a table row stays one tab-separated line.
fn flatten_inline(blocks: &[Block], out: &mut String) {
    let mut first = true;
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                let t = p.text();
                if !t.is_empty() {
                    if !first {
                        out.push(' ');
                    }
                    out.push_str(&t);
                    first = false;
                }
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        if !first {
                            out.push(' ');
                        }
                        flatten_inline(&cell.blocks, out);
                        first = false;
                    }
                }
            }
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

// --- shared XML helpers (namespace-prefix-agnostic, like the rxls .xlsx path) ---

/// Strip a namespace prefix: `w:p` → `p`, `r:embed` → `embed`.
pub(crate) fn local(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// First attribute value whose local name equals `key` (unescaped, owned).
pub(crate) fn attr_local(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local(a.key.as_ref()) == key {
            a.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, e.decoder())
                .ok()
                .map(|v| v.into_owned())
        } else {
            None
        }
    })
}

pub(crate) fn attr_local_trimmed_preserve_empty(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    attr_local(e, key).map(|value| value.trim().to_owned())
}

pub(crate) fn attr_local_trimmed(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    attr_local(e, key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(crate) fn is_page_break_type(e: &BytesStart<'_>) -> bool {
    attr_local_trimmed(e, b"type").is_some_and(|value| value == "page")
}

pub(crate) fn is_column_break_type(e: &BytesStart<'_>) -> bool {
    attr_local_trimmed(e, b"type").is_some_and(|value| value == "column")
}

pub(crate) fn field_char_type(e: &BytesStart<'_>) -> Option<String> {
    attr_local_trimmed(e, b"fldCharType")
}

/// Resolve an OOXML on/off toggle: a present element with no `w:val` means *on*;
/// `false`/`0`/`off` mean *off*; anything else is *on*.
pub(crate) fn toggle_on(val: Option<String>) -> bool {
    match val.as_deref().map(str::trim) {
        None => true,
        Some(v) => v != "0" && !v.eq_ignore_ascii_case("false") && !v.eq_ignore_ascii_case("off"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        custom_xml_item_id, normalize_part, parse_default_tab_stop_pt, parse_document_id,
        parse_rels, toggle_on, MAX_REL_RECORDS,
    };
    use super::{running_surface_distances_by_model_section, SectionHeaderFooter};
    use crate::model::{Block, RunningSurfaceDistanceHints, SectionSetup};

    #[test]
    fn toggle_on_accepts_case_insensitive_off_values() {
        assert!(!toggle_on(Some("FALSE".to_string())));
        assert!(!toggle_on(Some(" Off ".to_string())));
        assert!(!toggle_on(Some("0".to_string())));
        assert!(toggle_on(None));
        assert!(toggle_on(Some("true".to_string())));
    }

    /// The lenient reader path bounds how many relationships it collects
    /// from one part, so a size-capped but record-stuffed `.rels` can't amplify memory.
    #[test]
    fn reader_rels_parse_is_bounded() {
        let mut s = String::from(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for i in 0..(MAX_REL_RECORDS + 1000) {
            s.push_str(&format!(r#"<Relationship Id="r{i}" Target="t{i}"/>"#));
        }
        s.push_str("</Relationships>");
        assert!(
            parse_rels(&s).len() <= MAX_REL_RECORDS,
            "reader rels not bounded"
        );
    }

    #[test]
    fn reader_rels_trims_ooxml_values() {
        let rels = parse_rels(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                <Relationship Id=" rLink " Target=" https://example.com/ " TargetMode=" External "/>
            </Relationships>"#,
        );
        assert_eq!(
            rels.get("rLink")
                .map(|(target, external)| (target.as_str(), *external)),
            Some(("https://example.com/", true))
        );
    }

    #[test]
    fn parse_document_id_trims_ooxml_value() {
        let xml = r#"<w:settings xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml">
            <w14:docId w14:val=" 6ECD4467 "/>
        </w:settings>"#;
        assert_eq!(parse_document_id(xml).as_deref(), Some("6ECD4467"));

        let alternate_prefix =
            r#"<settings><m:docId xmlns:m="urn:any" m:val=" 6ECD4467 "/></settings>"#;
        assert_eq!(
            parse_document_id(alternate_prefix).as_deref(),
            Some("6ECD4467")
        );
    }

    #[test]
    fn parse_default_tab_stop_pt_reads_positive_twips() {
        let xml = r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val=" 1440 "/></w:settings>"#;
        assert_eq!(parse_default_tab_stop_pt(xml), Some(72.0));
        assert_eq!(
            parse_default_tab_stop_pt(
                r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="0"/></w:settings>"#
            ),
            None
        );
    }

    #[test]
    fn running_surface_distances_align_break_sections_and_final_section() {
        let blocks = vec![
            Block::SectionBreak(SectionSetup::default()),
            Block::SectionBreak(SectionSetup::default()),
        ];
        let distances = |header_pt, footer_pt| SectionHeaderFooter {
            distances: RunningSurfaceDistanceHints {
                header_pt: Some(header_pt),
                footer_pt: Some(footer_pt),
            },
            ..SectionHeaderFooter::default()
        };
        let sections = vec![
            distances(10.0, 11.0),
            distances(20.0, 21.0),
            distances(30.0, 31.0),
        ];

        assert_eq!(
            running_surface_distances_by_model_section(&blocks, &sections),
            vec![
                RunningSurfaceDistanceHints {
                    header_pt: Some(10.0),
                    footer_pt: Some(11.0),
                },
                RunningSurfaceDistanceHints {
                    header_pt: Some(20.0),
                    footer_pt: Some(21.0),
                },
                RunningSurfaceDistanceHints {
                    header_pt: Some(30.0),
                    footer_pt: Some(31.0),
                },
            ]
        );
    }

    #[test]
    fn custom_xml_item_id_trims_ooxml_value() {
        let xml = r#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID=" {11111111-2222-3333-4444-555555555555} ">
            <ds:schemaRefs/>
        </ds:datastoreItem>"#;
        assert_eq!(
            custom_xml_item_id(xml).as_deref(),
            Some("{11111111-2222-3333-4444-555555555555}")
        );

        let blank = r#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID=" "/>"#;
        assert_eq!(custom_xml_item_id(blank), None);
    }

    /// Relationship targets resolve relative to `word/` with `.`/`..`/
    /// leading-`/` normalized per OPC URI rules (the reader was missing dot-segment ones).
    #[test]
    fn normalize_part_resolves_dot_segments() {
        assert_eq!(normalize_part("media/image1.png"), "word/media/image1.png");
        assert_eq!(
            normalize_part("/word/media/image1.png"),
            "word/media/image1.png"
        );
        assert_eq!(
            normalize_part("./media/image1.png"),
            "word/media/image1.png"
        );
        assert_eq!(
            normalize_part("../customXml/item1.xml"),
            "customXml/item1.xml"
        );
        assert_eq!(normalize_part("header1.xml"), "word/header1.xml");
    }
}
