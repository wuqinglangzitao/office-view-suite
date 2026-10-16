//! Machine-readable document feature reports.
//!
//! The report surface is intentionally conservative: feature counts mean rwml
//! observed format markers for a construct, not that every behavior of that
//! construct is fully modeled, editable, or renderable.

#[cfg(not(feature = "docx"))]
use crate::annotation::action_field_syntax;
#[cfg(not(feature = "docx"))]
use crate::annotation::field_name_token as diagnostic_name_token;
#[cfg(not(feature = "docx"))]
use crate::annotation::{
    accept_field_text_format_switch, accept_general_format_switch,
    field_literal_token as diagnostic_literal_token, hyperlink_field_target, instruction_parts,
    strip_ascii_switch_prefix, FieldTextFormat,
};
#[cfg(not(feature = "docx"))]
use crate::annotation::{
    advance_field_syntax, eq_enclosed_operand, eq_fraction_operands, eq_list_operands,
    eq_numeric_prefix_option, eq_parenthesized_operand, eq_prefix_switch_tail, eq_radical_operands,
    symbol_field_syntax,
};
use crate::annotation::{
    barcode_field_syntax, direct_ref_field_syntax, legacy_form_field_syntax, merge_field_syntax,
    note_ref_field_syntax, opaque_field_syntax, page_ref_field_syntax, ref_field_syntax,
    toc_field_syntax, Field, FieldKind, RefFieldSyntax,
};
#[cfg(not(feature = "docx"))]
use crate::annotation::{
    compare_field_syntax, formula_field_expression_syntax, if_field_syntax,
    merge_control_field_syntax, numbering_field_syntax, page_field_format_syntax_tail,
    prompt_field_syntax, quote_field_syntax, reference_index_category_token,
    reference_index_literal_operand, reference_index_literal_token,
    reference_index_plain_value_token, revision_number_field_text_format, sequence_field_syntax,
    set_field_syntax, style_ref_field_syntax, toc_entry_field_syntax,
};
#[cfg(not(feature = "docx"))]
use crate::annotation::{
    document_property_key, field_name_operand as diagnostic_name_operand,
    field_non_empty_non_switch_literal_token, field_quoted_literal_token, filename_field_syntax,
};
#[cfg(feature = "docx")]
use crate::docx::local;
use crate::model::{Block, FieldRole, FieldUnsupportedReason, Stats, Table};
use crate::CoreProperties;
#[cfg(feature = "docx")]
use crate::RevisionKind;
#[cfg(feature = "docx")]
use std::collections::HashMap;
use std::collections::{BTreeMap, HashSet};

/// Source document format detected by [`crate::Document::open`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    /// Legacy Word 97-2003 binary `.doc`.
    Doc,
    /// Modern OOXML WordprocessingML `.docx`.
    Docx,
}

impl DocumentFormat {
    fn as_json_str(self) -> &'static str {
        match self {
            DocumentFormat::Doc => "doc",
            DocumentFormat::Docx => "docx",
        }
    }
}

/// Reason package-preserving edits are unavailable for an opened document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditReadOnlyReason {
    /// The source was a legacy binary `.doc`; rwml can convert it to `.docx`,
    /// but cannot preserve-edit the binary package in place.
    LegacyDoc,
    /// The OPC ZIP contained one or more unreadable entries, so saving the
    /// retained package would silently drop data.
    IncompletePackage,
    /// `[Content_Types].xml`, a `.rels` part, or case-colliding package metadata
    /// parsed lossily; edits that regenerate OPC metadata are therefore refused.
    LossyOpcMetadata,
}

impl EditReadOnlyReason {
    fn as_json_str(&self) -> &'static str {
        match self {
            EditReadOnlyReason::LegacyDoc => "legacy_doc",
            EditReadOnlyReason::IncompletePackage => "incomplete_package",
            EditReadOnlyReason::LossyOpcMetadata => "lossy_opc_metadata",
        }
    }
}

/// Whether package-preserving edit APIs are available for an opened document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditCapability {
    /// `true` when preservation edit APIs such as
    /// [`crate::Document::replace_body_text`] can mutate the retained source
    /// package and later [`crate::Document::save`] it.
    pub package_preserving: bool,
    /// Machine-readable reasons edits are unavailable. Empty when
    /// [`EditCapability::package_preserving`] is `true`.
    pub read_only_reasons: Vec<EditReadOnlyReason>,
}

impl EditCapability {
    #[cfg(feature = "docx")]
    pub(crate) fn editable() -> Self {
        Self {
            package_preserving: true,
            read_only_reasons: Vec::new(),
        }
    }

    pub(crate) fn read_only(read_only_reasons: Vec<EditReadOnlyReason>) -> Self {
        Self {
            package_preserving: false,
            read_only_reasons,
        }
    }
}

/// Count of observed fields for one field kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldKindCount {
    /// Field kind.
    pub kind: FieldKind,
    /// Number of fields of this kind.
    pub count: usize,
}

/// Reason a field's computed value is not available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldEvaluationReason {
    /// rwml does not know this field instruction class yet.
    UnknownField,
    /// The field points at a bookmark/scope rwml could not resolve.
    UnresolvedBookmark,
    /// The instruction contains a switch whose value can change the result.
    UnsupportedSwitch,
    /// The instruction is supported, but the document contains no computable value.
    NoComputedResult,
}

impl FieldEvaluationReason {
    fn as_json_str(self) -> &'static str {
        match self {
            FieldEvaluationReason::UnknownField => "UnknownField",
            FieldEvaluationReason::UnresolvedBookmark => "UnresolvedBookmark",
            FieldEvaluationReason::UnsupportedSwitch => "UnsupportedSwitch",
            FieldEvaluationReason::NoComputedResult => "NoComputedResult",
        }
    }
}

/// Count of unsupported field evaluations for one reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEvaluationReasonCount {
    /// Unsupported evaluation reason.
    pub reason: FieldEvaluationReason,
    /// Number of fields with this reason.
    pub count: usize,
}

/// Metafile image container observed in a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetafileFormat {
    /// Enhanced Metafile (`.emf` or compressed `.emz`).
    Emf,
    /// Windows Metafile (`.wmf` or compressed `.wmz`).
    Wmf,
}

impl MetafileFormat {
    fn as_json_str(self) -> &'static str {
        match self {
            MetafileFormat::Emf => "EMF",
            MetafileFormat::Wmf => "WMF",
        }
    }
}

/// Best-effort metadata for a preserved WMF/EMF package part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetafileInfo {
    /// OPC part path, for example `word/media/image1.emf`.
    pub path: String,
    /// Metafile family inferred from the part extension.
    pub format: MetafileFormat,
    /// Stored part payload size in bytes.
    pub bytes: usize,
    /// `true` for compressed `.emz`/`.wmz` wrappers or gzip-marked payloads.
    pub compressed: bool,
    /// Header-derived width in pixels/preview units when recoverable.
    pub width_px: Option<u32>,
    /// Header-derived height in pixels/preview units when recoverable.
    pub height_px: Option<u32>,
}

/// Counts of Word features observed while opening a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeatureInventory {
    /// Comment anchors or comment records.
    pub comments: usize,
    /// Footnote records.
    pub footnotes: usize,
    /// Endnote records.
    pub endnotes: usize,
    /// Text-box records.
    pub text_boxes: usize,
    /// Tracked insertion markers.
    pub tracked_insertions: usize,
    /// Tracked deletion markers.
    pub tracked_deletions: usize,
    /// Tracked move markers.
    pub tracked_moves: usize,
    /// Tracked property-change markers such as `w:pPrChange` and `w:rPrChange`.
    pub tracked_property_changes: usize,
    /// Field markers or field instructions.
    pub fields: usize,
    /// Field counts grouped by normalized field kind.
    pub field_kinds: Vec<FieldKindCount>,
    /// Field counts whose computed values rwml still cannot evaluate.
    pub unsupported_field_kinds: Vec<FieldKindCount>,
    /// Unsupported field evaluation counts grouped by reason.
    pub unsupported_field_reasons: Vec<FieldEvaluationReasonCount>,
    /// Relationship-backed or field-backed hyperlinks.
    pub hyperlinks: usize,
    /// Content controls.
    pub content_controls: usize,
    /// Tables nested inside another table cell.
    pub nested_tables: usize,
    /// Floating or alternate-content shape markers.
    pub floating_shapes: usize,
    /// Chart parts or chart references.
    pub charts: usize,
    /// Chart payloads or references that are still preserved but not modeled.
    pub unsupported_charts: usize,
    /// OLE embedded object markers.
    pub ole_objects: usize,
    /// WMF/EMF image parts without an extracted single-raster fallback.
    pub unsupported_metafiles: usize,
    /// Best-effort metadata for preserved WMF/EMF image parts.
    pub metafiles: Vec<MetafileInfo>,
}

impl FeatureInventory {
    /// `true` when this inventory contains features the current renderer cannot
    /// faithfully draw beyond cached field results, placeholders, or preserved
    /// package payloads.
    pub fn has_unsupported_render_features(&self) -> bool {
        self.unsupported_field_kinds
            .iter()
            .any(|item| item.count > 0)
            || self.floating_shapes > 0
            || self.unsupported_charts > 0
            || self.ole_objects > 0
            || self.unsupported_metafiles > 0
    }
}

/// Human- and machine-readable warnings derived from [`FeatureInventory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentWarning {
    /// Field instructions whose computed value rwml does not evaluate yet were
    /// found; cached visible text is preserved instead.
    UnsupportedFieldEvaluation {
        /// Number of observed field markers/instructions.
        count: usize,
        /// Observed field kinds.
        field_kinds: Vec<FieldKindCount>,
    },
    /// Tracked-change markup was found.
    TrackedChangesPresent {
        /// Number of insertion markers.
        insertions: usize,
        /// Number of deletion markers.
        deletions: usize,
        /// Number of move markers.
        moves: usize,
    },
    /// Tracked property-change markup was found. Revision text views preserve
    /// current visible text, but do not reconstruct original formatting.
    IncompleteRevisionView {
        /// Number of tracked property-change markers.
        property_changes: usize,
    },
    /// Floating shape markers were found.
    FloatingShapePlaceholderOnly {
        /// Number of observed floating/alternate-content shape markers.
        count: usize,
    },
    /// Chart payloads or references were found.
    ChartsPreservedButNotModeled {
        /// Number of chart parts/references observed.
        count: usize,
    },
    /// OLE object markers were found.
    OleObjectsPreservedButNotModeled {
        /// Number of OLE object markers observed.
        count: usize,
    },
    /// WMF/EMF/EMZ/WMZ images were found.
    UnsupportedMetafileImages {
        /// Number of WMF/EMF parts observed.
        count: usize,
    },
    /// A legacy `.doc` contains non-body subdocuments that the current rich
    /// model still flattens into the body flow.
    LegacyDocFlattenedSubdocuments {
        /// Footnote characters reported by the FIB.
        footnotes: usize,
        /// Header/footer characters reported by the FIB.
        headers_footers: usize,
        /// Annotation/comment characters reported by the FIB.
        annotations: usize,
        /// Endnote characters reported by the FIB.
        endnotes: usize,
        /// Text-box characters reported by the FIB.
        text_boxes: usize,
    },
    /// Package-preserving edits are unavailable for this opened document.
    PackageReadOnly {
        /// Machine-readable read-only reasons.
        reasons: Vec<EditReadOnlyReason>,
    },
}

/// Summary of the opened document's format, visible stats, observed feature
/// markers, and warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentReport {
    /// Detected source format.
    pub format: DocumentFormat,
    /// Visible model statistics.
    pub stats: Stats,
    /// Core document metadata.
    pub core_properties: CoreProperties,
    /// String custom document metadata.
    pub custom_properties: BTreeMap<String, String>,
    /// Package-preserving edit availability and read-only reasons.
    pub edit: EditCapability,
    /// Package part names touched by preservation edits in the current session.
    pub edited_parts: Vec<String>,
    /// Observed feature inventory.
    pub features: FeatureInventory,
    /// Warnings derived from the feature inventory.
    pub warnings: Vec<DocumentWarning>,
}

/// Human- and machine-readable warnings derived from features the current
/// renderer cannot faithfully compute or draw yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderWarning {
    /// Field instructions whose computed value the renderer does not evaluate
    /// yet are rendered from cached visible text.
    UnsupportedFieldEvaluation {
        /// Number of observed field markers/instructions.
        count: usize,
        /// Observed field kinds.
        field_kinds: Vec<FieldKindCount>,
    },
    /// Floating shape markers were found; renderer support is placeholder-only.
    FloatingShapePlaceholderOnly {
        /// Number of observed floating/alternate-content shape markers.
        count: usize,
    },
    /// Chart payloads or references were found, but charts are not drawn.
    ChartsPreservedButNotModeled {
        /// Number of chart parts/references observed.
        count: usize,
    },
    /// OLE object markers were found, but embedded OLE payloads are not drawn.
    OleObjectsPreservedButNotModeled {
        /// Number of OLE object markers observed.
        count: usize,
    },
    /// WMF/EMF/EMZ/WMZ images were found, but the renderer does not draw them.
    UnsupportedMetafileImages {
        /// Number of WMF/EMF parts observed.
        count: usize,
    },
    /// Model image nodes had no extracted bytes, so the renderer emits a
    /// placeholder instead of drawing the image.
    MissingImageBytes {
        /// Number of model images skipped by the renderer.
        count: usize,
    },
    /// Raster image bytes were present, but the current PDF backend could not
    /// decode that image format.
    UndecodableRasterImages {
        /// Number of model images skipped by the renderer.
        count: usize,
    },
}

/// Renderer metrics and warnings for a generated PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderReport {
    /// Number of PDF pages emitted by the renderer.
    pub pages: usize,
    /// Renderer-specific warnings derived from the feature inventory.
    pub warnings: Vec<RenderWarning>,
    /// Observed features relevant to unsupported or partial rendering behavior.
    pub unsupported: FeatureInventory,
}

/// PDF bytes plus the render report produced by the same pagination pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPdf {
    /// Generated PDF bytes.
    pub pdf: Vec<u8>,
    /// Render metrics and warnings for `pdf`.
    pub report: RenderReport,
}

impl DocumentReport {
    /// Serialize this report as compact JSON without requiring a serde
    /// dependency.
    ///
    /// The schema is intentionally small and stable enough for examples, shell
    /// scripts, and future CLI output:
    /// `format`, `stats`, `core_properties`, `custom_properties`, `edit`,
    /// `edited_parts`, `features`, and `warnings`.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push('{');
        json_field_str(&mut out, "format", self.format.as_json_str());
        out.push(',');
        out.push_str("\"stats\":{");
        json_field_num(&mut out, "paragraphs", self.stats.paragraphs);
        out.push(',');
        json_field_num(&mut out, "tables", self.stats.tables);
        out.push(',');
        json_field_num(&mut out, "figures", self.stats.figures);
        out.push(',');
        json_field_num(&mut out, "text_chars", self.stats.text_chars);
        out.push('}');
        out.push(',');
        out.push_str("\"core_properties\":");
        core_properties_json(&mut out, &self.core_properties);
        out.push(',');
        out.push_str("\"custom_properties\":");
        string_map_json(&mut out, &self.custom_properties);
        out.push(',');
        out.push_str("\"edit\":");
        edit_capability_json(&mut out, &self.edit);
        out.push(',');
        out.push_str("\"edited_parts\":");
        json_string_array(&mut out, &self.edited_parts);
        out.push(',');
        out.push_str("\"features\":");
        feature_inventory_json(&mut out, &self.features);
        out.push(',');
        out.push_str("\"warnings\":[");
        for (i, warning) in self.warnings.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            warning_json(&mut out, warning);
        }
        out.push_str("]}");
        out
    }
}

impl RenderReport {
    /// Serialize this render report as compact JSON without requiring a serde
    /// dependency.
    ///
    /// The schema is intended for CLI and validation-script output:
    /// `pages`, `unsupported`, and `warnings`.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push('{');
        json_field_num(&mut out, "pages", self.pages);
        out.push(',');
        out.push_str("\"unsupported\":");
        feature_inventory_json(&mut out, &self.unsupported);
        out.push(',');
        out.push_str("\"warnings\":[");
        for (i, warning) in self.warnings.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            render_warning_json(&mut out, warning);
        }
        out.push_str("]}");
        out
    }
}

pub(crate) fn warnings_for(
    features: &FeatureInventory,
    edit: &EditCapability,
) -> Vec<DocumentWarning> {
    let mut warnings = Vec::new();
    let (unsupported_fields, unsupported_field_kinds) = unsupported_field_evaluation(features);
    if unsupported_fields > 0 {
        warnings.push(DocumentWarning::UnsupportedFieldEvaluation {
            count: unsupported_fields,
            field_kinds: unsupported_field_kinds,
        });
    }
    if features.tracked_insertions > 0
        || features.tracked_deletions > 0
        || features.tracked_moves > 0
    {
        warnings.push(DocumentWarning::TrackedChangesPresent {
            insertions: features.tracked_insertions,
            deletions: features.tracked_deletions,
            moves: features.tracked_moves,
        });
    }
    if features.tracked_property_changes > 0 {
        warnings.push(DocumentWarning::IncompleteRevisionView {
            property_changes: features.tracked_property_changes,
        });
    }
    if features.floating_shapes > 0 {
        warnings.push(DocumentWarning::FloatingShapePlaceholderOnly {
            count: features.floating_shapes,
        });
    }
    if features.unsupported_charts > 0 {
        warnings.push(DocumentWarning::ChartsPreservedButNotModeled {
            count: features.unsupported_charts,
        });
    }
    if features.ole_objects > 0 {
        warnings.push(DocumentWarning::OleObjectsPreservedButNotModeled {
            count: features.ole_objects,
        });
    }
    if features.unsupported_metafiles > 0 {
        warnings.push(DocumentWarning::UnsupportedMetafileImages {
            count: features.unsupported_metafiles,
        });
    }
    if !edit.package_preserving {
        warnings.push(DocumentWarning::PackageReadOnly {
            reasons: edit.read_only_reasons.clone(),
        });
    }
    warnings
}

pub(crate) fn legacy_doc_flattened_subdocuments_warning(
    footnotes: usize,
    headers_footers: usize,
    annotations: usize,
    endnotes: usize,
    text_boxes: usize,
) -> Option<DocumentWarning> {
    let total = footnotes
        .saturating_add(headers_footers)
        .saturating_add(annotations)
        .saturating_add(endnotes)
        .saturating_add(text_boxes);
    (total > 0).then_some(DocumentWarning::LegacyDocFlattenedSubdocuments {
        footnotes,
        headers_footers,
        annotations,
        endnotes,
        text_boxes,
    })
}

#[cfg(feature = "render")]
pub(crate) fn render_warnings_for(features: &FeatureInventory) -> Vec<RenderWarning> {
    let mut warnings = Vec::new();
    let (unsupported_fields, unsupported_field_kinds) = unsupported_field_evaluation(features);
    if unsupported_fields > 0 {
        warnings.push(RenderWarning::UnsupportedFieldEvaluation {
            count: unsupported_fields,
            field_kinds: unsupported_field_kinds,
        });
    }
    if features.floating_shapes > 0 {
        warnings.push(RenderWarning::FloatingShapePlaceholderOnly {
            count: features.floating_shapes,
        });
    }
    if features.unsupported_charts > 0 {
        warnings.push(RenderWarning::ChartsPreservedButNotModeled {
            count: features.unsupported_charts,
        });
    }
    if features.ole_objects > 0 {
        warnings.push(RenderWarning::OleObjectsPreservedButNotModeled {
            count: features.ole_objects,
        });
    }
    if features.unsupported_metafiles > 0 {
        warnings.push(RenderWarning::UnsupportedMetafileImages {
            count: features.unsupported_metafiles,
        });
    }
    warnings
}

#[cfg(feature = "render")]
pub(crate) fn render_unsupported_features(features: &FeatureInventory) -> FeatureInventory {
    let mut unsupported = features.clone();
    unsupported.field_kinds = unsupported.unsupported_field_kinds.clone();
    unsupported.fields = unsupported
        .unsupported_field_kinds
        .iter()
        .map(|item| item.count)
        .sum();
    unsupported.charts = unsupported.unsupported_charts;
    unsupported
}

fn unsupported_field_evaluation(features: &FeatureInventory) -> (usize, Vec<FieldKindCount>) {
    let count = features
        .unsupported_field_kinds
        .iter()
        .map(|item| item.count)
        .sum();
    (count, features.unsupported_field_kinds.clone())
}

fn supports_field_kind_evaluation(kind: &FieldKind) -> bool {
    matches!(kind, FieldKind::DocumentInfo(_))
}

fn supports_field_evaluation(field: &Field) -> bool {
    if field.computed_result.is_some() {
        return true;
    }
    if supports_owned_computed_field_evaluation(field) {
        return true;
    }
    if field.kind == FieldKind::Hyperlink {
        return supports_hyperlink_field_evaluation(field);
    }
    if matches!(&field.kind, FieldKind::DocumentInfo(_)) {
        return supports_document_info_field_evaluation(field);
    }
    if field.kind == FieldKind::MergeField {
        return supports_merge_field_evaluation(field);
    }
    if field.kind == FieldKind::Filename {
        return supported_filename_syntax(&field.instruction);
    }
    if let FieldKind::ReferenceIndex(kind) = &field.kind {
        if is_reference_index_marker_kind(kind) {
            return supports_reference_index_marker_field_evaluation(field);
        }
    }
    supports_field_kind_evaluation(&field.kind)
}

fn supports_owned_computed_field_evaluation(field: &Field) -> bool {
    #[cfg(feature = "docx")]
    {
        match &field.kind {
            FieldKind::Dynamic(kind) if kind == "QUOTE" => {
                crate::docx::supports_quote_field_syntax(&field.instruction)
            }
            FieldKind::Dynamic(kind) if kind == "IF" || kind == "COMPARE" => {
                crate::docx::supports_context_free_if_compare_field_syntax(&field.instruction)
            }
            FieldKind::Dynamic(kind) if kind == "=" => {
                crate::docx::supports_context_free_formula_field_syntax(&field.instruction)
            }
            FieldKind::Dynamic(kind) if kind == "FILLIN" => {
                crate::docx::supports_context_free_fill_in_field_syntax(&field.instruction)
            }
            FieldKind::Display(kind) if kind == "EQ" || kind == "ADVANCE" => {
                crate::docx::supports_context_free_display_field_syntax(&field.instruction)
            }
            FieldKind::Display(kind) if kind == "SYMBOL" => {
                crate::docx::supports_computed_symbol_field_syntax(&field.instruction)
            }
            FieldKind::DocumentInfo(_) => {
                crate::docx::supports_preserved_document_info_field_syntax(&field.instruction)
            }
            FieldKind::DocumentStructure(kind) if kind == "REVNUM" => {
                crate::docx::supports_revision_number_field_syntax(&field.instruction)
            }
            _ => false,
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        let _ = field;
        false
    }
}

fn supports_hyperlink_field_evaluation(field: &Field) -> bool {
    #[cfg(feature = "docx")]
    {
        crate::docx::supports_hyperlink_field_syntax(&field.instruction)
    }
    #[cfg(not(feature = "docx"))]
    {
        supported_hyperlink_syntax_for_report(&field.instruction)
    }
}

#[cfg(not(feature = "docx"))]
fn supported_hyperlink_syntax_for_report(instruction: &str) -> bool {
    hyperlink_field_target(instruction).is_some()
}

fn supports_merge_field_evaluation(field: &Field) -> bool {
    #[cfg(feature = "docx")]
    {
        merge_field_syntax(&field.instruction)
    }
    #[cfg(not(feature = "docx"))]
    {
        supported_merge_field_syntax_for_report(&field.instruction)
    }
}

#[cfg(not(feature = "docx"))]
fn supported_merge_field_syntax_for_report(instruction: &str) -> bool {
    merge_field_syntax(instruction)
}

fn supports_document_info_field_evaluation(field: &Field) -> bool {
    #[cfg(feature = "docx")]
    {
        crate::docx::supports_document_info_field_syntax(&field.instruction)
    }
    #[cfg(not(feature = "docx"))]
    {
        let _ = field;
        false
    }
}

fn supports_reference_index_marker_field_evaluation(field: &Field) -> bool {
    #[cfg(feature = "docx")]
    {
        crate::docx::supports_reference_index_marker_syntax(&field.instruction)
    }
    #[cfg(not(feature = "docx"))]
    {
        supported_reference_index_marker_syntax_for_report(&field.instruction)
    }
}

#[cfg(feature = "render")]
fn supports_render_model_field_evaluation(field: &Field) -> bool {
    (field.kind == FieldKind::Page && supported_page_syntax(&field.instruction))
        || supports_field_evaluation(field)
}

fn supports_model_field_entry_evaluation(entry: &ModelFieldEntry) -> bool {
    entry.unsupported_reason.is_none() && supports_field_evaluation(&entry.field)
}

#[cfg(feature = "render")]
fn supports_render_model_field_entry_evaluation(entry: &ModelFieldEntry) -> bool {
    entry.unsupported_reason.is_none() && supports_render_model_field_evaluation(&entry.field)
}

pub(crate) fn fields_for_model(blocks: &[Block]) -> Vec<Field> {
    field_entries_for_model(blocks)
        .into_iter()
        .map(|entry| entry.field)
        .collect()
}

pub(crate) fn feature_inventory_for_model(blocks: &[Block]) -> FeatureInventory {
    let mut inventory = FeatureInventory::default();
    let entries = field_entries_for_model(blocks);
    let reason_context = model_field_reason_context(blocks);
    inventory.fields = entries.len();
    inventory.field_kinds = count_model_field_kinds(&entries);
    inventory.unsupported_field_kinds = count_model_unsupported_field_kinds(&entries);
    inventory.unsupported_field_reasons =
        count_model_unsupported_field_reasons(&entries, &reason_context);
    inventory.hyperlinks = entries
        .iter()
        .filter(|entry| entry.field.kind == FieldKind::Hyperlink)
        .count();
    count_nested_model_tables(blocks, 0, &mut inventory, true);
    inventory
}

#[cfg(feature = "render")]
pub(crate) fn render_inventory_for_model(blocks: &[Block]) -> FeatureInventory {
    let mut inventory = FeatureInventory::default();
    let entries = field_entries_for_model(blocks);
    let reason_context = model_field_reason_context(blocks);
    inventory.fields = entries.len();
    inventory.field_kinds = count_model_field_kinds(&entries);
    inventory.hyperlinks = entries
        .iter()
        .filter(|entry| entry.field.kind == FieldKind::Hyperlink)
        .count();
    for entry in &entries {
        let field = &entry.field;
        if supports_render_model_field_entry_evaluation(entry) {
            continue;
        }
        if let Some(existing) = inventory
            .unsupported_field_kinds
            .iter_mut()
            .find(|item| item.kind == field.kind)
        {
            existing.count += 1;
        } else {
            inventory.unsupported_field_kinds.push(FieldKindCount {
                kind: field.kind.clone(),
                count: 1,
            });
        }
        if let Some(reason) = render_model_unsupported_field_reason(entry, &reason_context) {
            increment_field_evaluation_reason_count(
                &mut inventory.unsupported_field_reasons,
                reason,
            );
        }
    }
    count_nested_model_tables(blocks, 0, &mut inventory, false);
    inventory
}

struct ModelFieldEntry {
    field: Field,
    unsupported_reason: Option<FieldUnsupportedReason>,
}

#[derive(Default)]
struct ModelFieldReasonContext {
    bookmark_names: HashSet<String>,
    note_ref_target_names: HashSet<String>,
}

fn model_field_reason_context(blocks: &[Block]) -> ModelFieldReasonContext {
    let mut context = ModelFieldReasonContext::default();
    collect_model_field_reason_context(blocks, &mut context);
    context
}

fn collect_model_field_reason_context(blocks: &[Block], context: &mut ModelFieldReasonContext) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                for run in &paragraph.runs {
                    if let Some(name) = &run.bookmark {
                        context.bookmark_names.insert(name.clone());
                        if run.note.is_some() {
                            context.note_ref_target_names.insert(name.clone());
                        }
                    }
                }
            }
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_model_field_reason_context(&cell.blocks, context);
                    }
                }
            }
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

#[cfg(feature = "render")]
fn render_model_unsupported_field_reason(
    entry: &ModelFieldEntry,
    context: &ModelFieldReasonContext,
) -> Option<FieldEvaluationReason> {
    let field = &entry.field;
    if let Some(reason) = entry.unsupported_reason {
        return Some(model_field_unsupported_reason(reason));
    }
    if !supports_render_model_field_evaluation(field) && field.kind == FieldKind::PageRef {
        return Some(page_ref_uncomputed_reason(
            &field.instruction,
            Some(&context.bookmark_names),
        ));
    }
    if !supports_render_model_field_evaluation(field) && field.kind == FieldKind::Ref {
        return Some(ref_uncomputed_reason(
            &field.instruction,
            Some(&context.bookmark_names),
        ));
    }
    #[cfg(feature = "docx")]
    {
        if !supports_render_model_field_evaluation(field) && field.kind == FieldKind::NoteRef {
            return Some(note_ref_uncomputed_reason(
                &field.instruction,
                Some(&context.bookmark_names),
                Some(&context.note_ref_target_names),
            ));
        }
    }
    if !supports_render_model_field_evaluation(field) && field.kind == FieldKind::Toc {
        return Some(toc_uncomputed_reason(
            &field.instruction,
            Some(&context.bookmark_names),
        ));
    }
    unsupported_field_reason(field)
}

fn model_field_unsupported_reason(reason: FieldUnsupportedReason) -> FieldEvaluationReason {
    match reason {
        FieldUnsupportedReason::UnresolvedBookmark => FieldEvaluationReason::UnresolvedBookmark,
        FieldUnsupportedReason::UnsupportedSwitch => FieldEvaluationReason::UnsupportedSwitch,
        FieldUnsupportedReason::NoComputedResult => FieldEvaluationReason::NoComputedResult,
    }
}

fn count_nested_model_tables(
    blocks: &[Block],
    depth: usize,
    inventory: &mut FeatureInventory,
    count_charts: bool,
) {
    for block in blocks {
        match block {
            Block::Table(table) => {
                if depth > 0 {
                    inventory.nested_tables += 1;
                }
                for row in &table.rows {
                    for cell in &row.cells {
                        count_nested_model_tables(&cell.blocks, depth + 1, inventory, count_charts);
                    }
                }
            }
            Block::Chart(_) if count_charts => inventory.charts += 1,
            Block::Chart(_) => {}
            Block::Paragraph(_) | Block::Image(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

fn field_entries_for_model(blocks: &[Block]) -> Vec<ModelFieldEntry> {
    let mut fields = Vec::new();
    let bookmark_names = model_bookmark_names(blocks);
    collect_model_field_entries(blocks, &bookmark_names, &mut fields);
    fields
}

fn collect_model_field_entries(
    blocks: &[Block],
    bookmark_names: &HashSet<String>,
    out: &mut Vec<ModelFieldEntry>,
) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                let mut current: Option<ModelFieldEntry> = None;
                for run in &paragraph.runs {
                    let field = field_from_role(
                        &run.field,
                        &run.text,
                        bookmark_names,
                        run.field_unsupported_reason,
                    );
                    match field {
                        Some(field) => {
                            if let Some(active) = &mut current {
                                if active.field.kind == field.field.kind
                                    && active.field.instruction == field.field.instruction
                                    && active.unsupported_reason == field.unsupported_reason
                                {
                                    active.field.result.push_str(&field.field.result);
                                    continue;
                                }
                                out.push(current.take().expect("checked above"));
                            }
                            current = Some(field);
                        }
                        None => {
                            if let Some(done) = current.take() {
                                out.push(done);
                            }
                        }
                    }
                }
                if let Some(done) = current {
                    out.push(done);
                }
            }
            Block::Table(table) => collect_model_table_field_entries(table, bookmark_names, out),
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

fn collect_model_table_field_entries(
    table: &Table,
    bookmark_names: &HashSet<String>,
    out: &mut Vec<ModelFieldEntry>,
) {
    for row in &table.rows {
        for cell in &row.cells {
            collect_model_field_entries(&cell.blocks, bookmark_names, out);
        }
    }
}

fn field_from_role(
    role: &FieldRole,
    result: &str,
    bookmark_names: &HashSet<String>,
    unsupported_reason: Option<FieldUnsupportedReason>,
) -> Option<ModelFieldEntry> {
    match role {
        FieldRole::Simple { instruction } => {
            let instruction = normalize_model_field_instruction(instruction);
            if instruction.is_empty() {
                None
            } else {
                let kind = model_field_kind(&instruction, bookmark_names, unsupported_reason);
                Some(ModelFieldEntry {
                    field: Field {
                        kind,
                        instruction,
                        result: result.to_string(),
                        computed_result: None,
                    },
                    unsupported_reason,
                })
            }
        }
        FieldRole::Hyperlink { url } => Some(ModelFieldEntry {
            field: Field {
                kind: FieldKind::Hyperlink,
                instruction: format!("HYPERLINK \"{url}\""),
                result: result.to_string(),
                computed_result: None,
            },
            unsupported_reason,
        }),
        FieldRole::None | FieldRole::Other => None,
    }
}

fn model_field_kind(
    instruction: &str,
    bookmark_names: &HashSet<String>,
    unsupported_reason: Option<FieldUnsupportedReason>,
) -> FieldKind {
    let kind = FieldKind::from_instruction(instruction);
    if matches!(kind, FieldKind::Unknown(_))
        && direct_ref_field_syntax(instruction).is_some_and(|syntax| {
            bookmark_names.contains(&syntax.target)
                || unsupported_reason == Some(FieldUnsupportedReason::NoComputedResult)
        })
    {
        FieldKind::Ref
    } else {
        kind
    }
}

fn model_bookmark_names(blocks: &[Block]) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_model_bookmark_names(blocks, &mut names);
    names
}

fn collect_model_bookmark_names(blocks: &[Block], names: &mut HashSet<String>) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                for run in &paragraph.runs {
                    if let Some(name) = &run.bookmark {
                        names.insert(name.clone());
                    }
                }
            }
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_model_bookmark_names(&cell.blocks, names);
                    }
                }
            }
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

fn normalize_model_field_instruction(instruction: &str) -> String {
    instruction.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn doc_edit_capability() -> EditCapability {
    EditCapability::read_only(vec![EditReadOnlyReason::LegacyDoc])
}

fn json_field_str(out: &mut String, name: &str, value: &str) {
    push_json_string(out, name);
    out.push(':');
    push_json_string(out, value);
}

fn json_field_opt_str(out: &mut String, name: &str, value: Option<&str>) {
    push_json_string(out, name);
    out.push(':');
    match value {
        Some(value) => push_json_string(out, value),
        None => out.push_str("null"),
    }
}

fn json_field_num<T: std::fmt::Display>(out: &mut String, name: &str, value: T) {
    push_json_string(out, name);
    out.push(':');
    out.push_str(&value.to_string());
}

fn json_field_bool(out: &mut String, name: &str, value: bool) {
    push_json_string(out, name);
    out.push(':');
    out.push_str(if value { "true" } else { "false" });
}

fn json_field_opt_num<T: std::fmt::Display>(out: &mut String, name: &str, value: Option<T>) {
    push_json_string(out, name);
    out.push(':');
    match value {
        Some(value) => out.push_str(&value.to_string()),
        None => out.push_str("null"),
    }
}

fn edit_capability_json(out: &mut String, edit: &EditCapability) {
    out.push('{');
    json_field_bool(out, "package_preserving", edit.package_preserving);
    out.push(',');
    out.push_str("\"read_only_reasons\":");
    let reasons: Vec<&str> = edit
        .read_only_reasons
        .iter()
        .map(EditReadOnlyReason::as_json_str)
        .collect();
    json_str_array(out, &reasons);
    out.push('}');
}

fn json_string_array(out: &mut String, values: &[String]) {
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    json_str_array(out, &refs);
}

fn core_properties_json(out: &mut String, props: &CoreProperties) {
    out.push('{');
    json_field_opt_str(out, "title", props.title.as_deref());
    out.push(',');
    json_field_opt_str(out, "subject", props.subject.as_deref());
    out.push(',');
    json_field_opt_str(out, "creator", props.creator.as_deref());
    out.push(',');
    json_field_opt_str(out, "description", props.description.as_deref());
    out.push(',');
    json_field_opt_str(out, "keywords", props.keywords.as_deref());
    out.push(',');
    json_field_opt_str(out, "category", props.category.as_deref());
    out.push(',');
    json_field_opt_str(out, "content_status", props.content_status.as_deref());
    out.push(',');
    json_field_opt_str(out, "last_modified_by", props.last_modified_by.as_deref());
    out.push(',');
    json_field_opt_str(out, "created", props.created.as_deref());
    out.push(',');
    json_field_opt_str(out, "modified", props.modified.as_deref());
    out.push(',');
    json_field_opt_str(out, "last_printed", props.last_printed.as_deref());
    out.push(',');
    json_field_opt_str(out, "revision", props.revision.as_deref());
    out.push(',');
    json_field_opt_str(out, "version", props.version.as_deref());
    out.push('}');
}

fn string_map_json(out: &mut String, values: &BTreeMap<String, String>) {
    out.push('{');
    for (i, (key, value)) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_string(out, key);
        out.push(':');
        push_json_string(out, value);
    }
    out.push('}');
}

fn json_str_array(out: &mut String, values: &[&str]) {
    out.push('[');
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_string(out, value);
    }
    out.push(']');
}

fn feature_inventory_json(out: &mut String, features: &FeatureInventory) {
    out.push('{');
    json_field_num(out, "comments", features.comments);
    out.push(',');
    json_field_num(out, "footnotes", features.footnotes);
    out.push(',');
    json_field_num(out, "endnotes", features.endnotes);
    out.push(',');
    json_field_num(out, "text_boxes", features.text_boxes);
    out.push(',');
    json_field_num(out, "tracked_insertions", features.tracked_insertions);
    out.push(',');
    json_field_num(out, "tracked_deletions", features.tracked_deletions);
    out.push(',');
    json_field_num(out, "tracked_moves", features.tracked_moves);
    out.push(',');
    json_field_num(
        out,
        "tracked_property_changes",
        features.tracked_property_changes,
    );
    out.push(',');
    json_field_num(out, "fields", features.fields);
    out.push(',');
    out.push_str("\"field_kinds\":[");
    for (i, item) in features.field_kinds.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        field_kind_count_json(out, item);
    }
    out.push(']');
    out.push(',');
    out.push_str("\"unsupported_field_kinds\":[");
    for (i, item) in features.unsupported_field_kinds.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        field_kind_count_json(out, item);
    }
    out.push(']');
    out.push(',');
    out.push_str("\"unsupported_field_reasons\":[");
    for (i, item) in features.unsupported_field_reasons.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        field_evaluation_reason_count_json(out, item);
    }
    out.push(']');
    out.push(',');
    json_field_num(out, "hyperlinks", features.hyperlinks);
    out.push(',');
    json_field_num(out, "content_controls", features.content_controls);
    out.push(',');
    json_field_num(out, "nested_tables", features.nested_tables);
    out.push(',');
    json_field_num(out, "floating_shapes", features.floating_shapes);
    out.push(',');
    json_field_num(out, "charts", features.charts);
    out.push(',');
    json_field_num(out, "unsupported_charts", features.unsupported_charts);
    out.push(',');
    json_field_num(out, "ole_objects", features.ole_objects);
    out.push(',');
    json_field_num(out, "unsupported_metafiles", features.unsupported_metafiles);
    out.push(',');
    out.push_str("\"metafiles\":[");
    for (i, item) in features.metafiles.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        metafile_info_json(out, item);
    }
    out.push(']');
    out.push('}');
}

fn metafile_info_json(out: &mut String, item: &MetafileInfo) {
    out.push('{');
    json_field_str(out, "path", &item.path);
    out.push(',');
    json_field_str(out, "format", item.format.as_json_str());
    out.push(',');
    json_field_num(out, "bytes", item.bytes);
    out.push(',');
    json_field_bool(out, "compressed", item.compressed);
    out.push(',');
    json_field_opt_num(out, "width_px", item.width_px);
    out.push(',');
    json_field_opt_num(out, "height_px", item.height_px);
    out.push('}');
}

fn warning_json(out: &mut String, warning: &DocumentWarning) {
    out.push('{');
    match warning {
        DocumentWarning::UnsupportedFieldEvaluation { count, field_kinds } => {
            json_field_str(out, "kind", "UnsupportedFieldEvaluation");
            out.push(',');
            json_field_num(out, "count", count);
            out.push(',');
            out.push_str("\"field_kinds\":[");
            for (i, item) in field_kinds.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                field_kind_count_json(out, item);
            }
            out.push(']');
        }
        DocumentWarning::TrackedChangesPresent {
            insertions,
            deletions,
            moves,
        } => {
            json_field_str(out, "kind", "TrackedChangesPresent");
            out.push(',');
            json_field_num(out, "insertions", insertions);
            out.push(',');
            json_field_num(out, "deletions", deletions);
            out.push(',');
            json_field_num(out, "moves", moves);
        }
        DocumentWarning::IncompleteRevisionView { property_changes } => {
            json_field_str(out, "kind", "IncompleteRevisionView");
            out.push(',');
            json_field_num(out, "property_changes", property_changes);
        }
        DocumentWarning::FloatingShapePlaceholderOnly { count } => {
            json_field_str(out, "kind", "FloatingShapePlaceholderOnly");
            out.push(',');
            json_field_num(out, "count", count);
        }
        DocumentWarning::ChartsPreservedButNotModeled { count } => {
            json_field_str(out, "kind", "ChartsPreservedButNotModeled");
            out.push(',');
            json_field_num(out, "count", count);
        }
        DocumentWarning::OleObjectsPreservedButNotModeled { count } => {
            json_field_str(out, "kind", "OleObjectsPreservedButNotModeled");
            out.push(',');
            json_field_num(out, "count", count);
        }
        DocumentWarning::UnsupportedMetafileImages { count } => {
            json_field_str(out, "kind", "UnsupportedMetafileImages");
            out.push(',');
            json_field_num(out, "count", count);
        }
        DocumentWarning::LegacyDocFlattenedSubdocuments {
            footnotes,
            headers_footers,
            annotations,
            endnotes,
            text_boxes,
        } => {
            json_field_str(out, "kind", "LegacyDocFlattenedSubdocuments");
            out.push(',');
            json_field_num(out, "footnotes", footnotes);
            out.push(',');
            json_field_num(out, "headers_footers", headers_footers);
            out.push(',');
            json_field_num(out, "annotations", annotations);
            out.push(',');
            json_field_num(out, "endnotes", endnotes);
            out.push(',');
            json_field_num(out, "text_boxes", text_boxes);
        }
        DocumentWarning::PackageReadOnly { reasons } => {
            json_field_str(out, "kind", "PackageReadOnly");
            out.push(',');
            out.push_str("\"reasons\":[");
            for (i, reason) in reasons.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_json_string(out, reason.as_json_str());
            }
            out.push(']');
        }
    }
    out.push('}');
}

fn render_warning_json(out: &mut String, warning: &RenderWarning) {
    out.push('{');
    match warning {
        RenderWarning::UnsupportedFieldEvaluation { count, field_kinds } => {
            json_field_str(out, "kind", "UnsupportedFieldEvaluation");
            out.push(',');
            json_field_num(out, "count", count);
            out.push(',');
            out.push_str("\"field_kinds\":[");
            for (i, item) in field_kinds.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                field_kind_count_json(out, item);
            }
            out.push(']');
        }
        RenderWarning::FloatingShapePlaceholderOnly { count } => {
            json_field_str(out, "kind", "FloatingShapePlaceholderOnly");
            out.push(',');
            json_field_num(out, "count", count);
        }
        RenderWarning::ChartsPreservedButNotModeled { count } => {
            json_field_str(out, "kind", "ChartsPreservedButNotModeled");
            out.push(',');
            json_field_num(out, "count", count);
        }
        RenderWarning::OleObjectsPreservedButNotModeled { count } => {
            json_field_str(out, "kind", "OleObjectsPreservedButNotModeled");
            out.push(',');
            json_field_num(out, "count", count);
        }
        RenderWarning::UnsupportedMetafileImages { count } => {
            json_field_str(out, "kind", "UnsupportedMetafileImages");
            out.push(',');
            json_field_num(out, "count", count);
        }
        RenderWarning::MissingImageBytes { count } => {
            json_field_str(out, "kind", "MissingImageBytes");
            out.push(',');
            json_field_num(out, "count", count);
        }
        RenderWarning::UndecodableRasterImages { count } => {
            json_field_str(out, "kind", "UndecodableRasterImages");
            out.push(',');
            json_field_num(out, "count", count);
        }
    }
    out.push('}');
}

fn field_kind_count_json(out: &mut String, item: &FieldKindCount) {
    out.push('{');
    json_field_str(out, "kind", item.kind.as_str());
    out.push(',');
    json_field_num(out, "count", item.count);
    out.push('}');
}

fn field_evaluation_reason_count_json(out: &mut String, item: &FieldEvaluationReasonCount) {
    out.push('{');
    json_field_str(out, "reason", item.reason.as_json_str());
    out.push(',');
    json_field_num(out, "count", item.count);
    out.push('}');
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{1F}' => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(feature = "docx")]
pub(crate) fn docx_features(docx: &crate::docx::DocxState) -> FeatureInventory {
    let mut features = FeatureInventory::default();
    let document_xml = docx.package.part("word/document.xml");
    if let Some(xml) = document_xml.as_ref() {
        scan_docx_xml(&String::from_utf8_lossy(xml), &mut features);
    }
    for part in docx_non_body_story_feature_parts(&docx.package) {
        if let Some(xml) = docx.package.part(&part) {
            scan_docx_story_structure_markers(&String::from_utf8_lossy(&xml), &mut features);
        }
    }
    if let Some(xml) = docx.package.part("word/comments.xml") {
        features.comments = features
            .comments
            .max(count_elements(&String::from_utf8_lossy(&xml), b"comment"));
    }
    features.comments = features.comments.max(docx.comments.len());
    features.footnotes = docx
        .note_records
        .iter()
        .filter(|note| note.kind == crate::NoteKind::Footnote)
        .count();
    features.endnotes = docx
        .note_records
        .iter()
        .filter(|note| note.kind == crate::NoteKind::Endnote)
        .count();
    features.text_boxes = docx.text_boxes.len();
    features.fields = features.fields.max(docx.fields.len());
    features.field_kinds = count_field_kinds(&docx.fields);
    features.unsupported_field_kinds = count_unsupported_field_kinds(&docx.fields);
    features.unsupported_field_reasons =
        count_docx_unsupported_field_reasons(&docx.fields, document_xml.as_deref());
    features.hyperlinks += docx
        .fields
        .iter()
        .filter(|field| field.kind == FieldKind::Hyperlink)
        .count();
    features.floating_shapes = features.floating_shapes.max(docx.floating_shapes.len());
    let mut parsed_insertions = 0;
    let mut parsed_deletions = 0;
    let mut parsed_moves = 0;
    for rev in &docx.revisions {
        match rev.kind {
            RevisionKind::Insertion => parsed_insertions += 1,
            RevisionKind::Deletion => parsed_deletions += 1,
            RevisionKind::MoveFrom | RevisionKind::MoveTo => parsed_moves += 1,
        }
    }
    features.tracked_insertions = features.tracked_insertions.max(parsed_insertions);
    features.tracked_deletions = features.tracked_deletions.max(parsed_deletions);
    features.tracked_moves = features.tracked_moves.max(parsed_moves);
    let chart_parts = docx
        .package
        .part_names()
        .filter(|name| is_docx_chart_payload_part(name))
        .count();
    features.charts = features.charts.max(chart_parts);
    let unmodeled_chart_parts = docx_unmodeled_chart_count(docx);
    let modeled_chart_parts = chart_parts.saturating_sub(unmodeled_chart_parts);
    features.unsupported_charts =
        unmodeled_chart_parts.max(features.charts.saturating_sub(modeled_chart_parts));
    let metafile_scan = metafile_scan(docx);
    features.unsupported_metafiles += metafile_scan.unsupported;
    features.metafiles = metafile_scan.infos;
    features
}

#[cfg(feature = "docx")]
pub(crate) fn docx_unmodeled_chart_count(docx: &crate::docx::DocxState) -> usize {
    docx.package
        .part_names()
        .filter(|name| is_docx_chart_payload_part(name))
        .filter(|name| match docx.package.part(name) {
            Some(bytes) => !is_modeled_docx_chart_payload(name, &bytes),
            None => true,
        })
        .count()
}

#[cfg(feature = "docx")]
fn docx_non_body_story_feature_parts(package: &crate::opc::Package) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::new();
    for part in ["word/footnotes.xml", "word/endnotes.xml"] {
        if package.has_part(part) && seen.insert(part.to_ascii_lowercase()) {
            parts.push(part.to_string());
        }
    }
    for rel in package.rels_for("word/document.xml") {
        if rel.external {
            continue;
        }
        match rel.rel_type.as_str() {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header"
            | "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" => {}
            _ => continue,
        }
        let part = crate::opc::resolve_rel_target("word/document.xml", &rel.target);
        if package.has_part(&part) && seen.insert(part.to_ascii_lowercase()) {
            parts.push(part);
        }
    }
    parts
}

#[cfg(feature = "docx")]
fn scan_docx_story_structure_markers(xml: &str, features: &mut FeatureInventory) {
    let mut story = FeatureInventory::default();
    scan_docx_xml(xml, &mut story);
    features.comments += story.comments;
    features.content_controls += story.content_controls;
    features.fields += story.fields;
    features.nested_tables += story.nested_tables;
    features.ole_objects += story.ole_objects;
    features.charts += story.charts;
    features.floating_shapes += story.floating_shapes;
    features.tracked_property_changes += story.tracked_property_changes;
}

#[cfg(feature = "docx")]
fn is_docx_chart_payload_part(name: &str) -> bool {
    let Some(file) = name.strip_prefix("word/charts/") else {
        return false;
    };
    let Some(stem) = file.strip_suffix(".xml") else {
        return false;
    };
    has_numbered_suffix(stem, "chart") || has_numbered_suffix(stem, "chartEx")
}

#[cfg(feature = "docx")]
fn is_modeled_docx_chart_payload(name: &str, bytes: &[u8]) -> bool {
    let Some(file) = name.strip_prefix("word/charts/") else {
        return false;
    };
    let Some(stem) = file.strip_suffix(".xml") else {
        return false;
    };
    if !has_numbered_suffix(stem, "chart") && !has_numbered_suffix(stem, "chartEx") {
        return false;
    }
    std::str::from_utf8(bytes)
        .ok()
        .and_then(crate::docx::chart::parse)
        .is_some()
}

#[cfg(feature = "docx")]
fn has_numbered_suffix(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[cfg(feature = "docx")]
struct MetafileScan {
    infos: Vec<MetafileInfo>,
    unsupported: usize,
}

#[cfg(feature = "docx")]
fn metafile_scan(docx: &crate::docx::DocxState) -> MetafileScan {
    let mut unsupported = 0usize;
    let mut infos: Vec<MetafileInfo> = docx
        .package
        .part_names()
        .filter_map(|name| {
            let (kind, compressed_by_extension) = crate::metafile::format_for_part(name)?;
            let bytes = docx.package.part(name).unwrap_or_default();
            let compressed = compressed_by_extension || crate::metafile::is_gzip_payload(&bytes);
            let (width_px, height_px) = crate::metafile::dimensions(kind, &bytes, compressed);
            if crate::metafile::extract_raster(kind, &bytes, compressed).is_none() {
                unsupported += 1;
            }
            Some(MetafileInfo {
                path: name.to_string(),
                format: report_metafile_format(kind),
                bytes: bytes.len(),
                compressed,
                width_px,
                height_px,
            })
        })
        .collect();
    infos.sort_by(|a, b| a.path.cmp(&b.path));
    MetafileScan { infos, unsupported }
}

#[cfg(feature = "docx")]
fn report_metafile_format(kind: crate::metafile::MetafileKind) -> MetafileFormat {
    match kind {
        crate::metafile::MetafileKind::Emf => MetafileFormat::Emf,
        crate::metafile::MetafileKind::Wmf => MetafileFormat::Wmf,
    }
}

#[cfg(feature = "docx")]
pub(crate) fn docx_edit_capability(docx: &crate::docx::DocxState) -> EditCapability {
    let mut reasons = Vec::new();
    if !docx.package.is_complete() {
        reasons.push(EditReadOnlyReason::IncompletePackage);
    }
    if docx.package.is_meta_lossy() {
        reasons.push(EditReadOnlyReason::LossyOpcMetadata);
    }
    if reasons.is_empty() {
        EditCapability::editable()
    } else {
        EditCapability::read_only(reasons)
    }
}

#[cfg(feature = "docx")]
fn count_field_kinds(fields: &[Field]) -> Vec<FieldKindCount> {
    let mut counts: Vec<FieldKindCount> = Vec::new();
    for field in fields {
        if let Some(existing) = counts.iter_mut().find(|item| item.kind == field.kind) {
            existing.count += 1;
        } else {
            counts.push(FieldKindCount {
                kind: field.kind.clone(),
                count: 1,
            });
        }
    }
    counts
}

#[cfg(feature = "docx")]
fn count_unsupported_field_kinds(fields: &[Field]) -> Vec<FieldKindCount> {
    let mut counts: Vec<FieldKindCount> = Vec::new();
    for field in fields {
        if supports_field_evaluation(field) {
            continue;
        }
        if let Some(existing) = counts.iter_mut().find(|item| item.kind == field.kind) {
            existing.count += 1;
        } else {
            counts.push(FieldKindCount {
                kind: field.kind.clone(),
                count: 1,
            });
        }
    }
    counts
}

fn count_model_field_kinds(entries: &[ModelFieldEntry]) -> Vec<FieldKindCount> {
    let mut counts: Vec<FieldKindCount> = Vec::new();
    for entry in entries {
        let kind = &entry.field.kind;
        if let Some(existing) = counts.iter_mut().find(|item| item.kind == *kind) {
            existing.count += 1;
        } else {
            counts.push(FieldKindCount {
                kind: kind.clone(),
                count: 1,
            });
        }
    }
    counts
}

fn count_model_unsupported_field_kinds(entries: &[ModelFieldEntry]) -> Vec<FieldKindCount> {
    let mut counts: Vec<FieldKindCount> = Vec::new();
    for entry in entries {
        let field = &entry.field;
        if supports_model_field_entry_evaluation(entry) {
            continue;
        }
        if let Some(existing) = counts.iter_mut().find(|item| item.kind == field.kind) {
            existing.count += 1;
        } else {
            counts.push(FieldKindCount {
                kind: field.kind.clone(),
                count: 1,
            });
        }
    }
    counts
}

fn count_model_unsupported_field_reasons(
    entries: &[ModelFieldEntry],
    context: &ModelFieldReasonContext,
) -> Vec<FieldEvaluationReasonCount> {
    let mut counts: Vec<FieldEvaluationReasonCount> = Vec::new();
    for entry in entries {
        if let Some(reason) = model_unsupported_field_reason(entry, context) {
            increment_field_evaluation_reason_count(&mut counts, reason);
        }
    }
    counts
}

#[cfg(feature = "docx")]
fn count_docx_unsupported_field_reasons(
    fields: &[Field],
    document_xml: Option<&[u8]>,
) -> Vec<FieldEvaluationReasonCount> {
    let document_xml = document_xml.map(String::from_utf8_lossy);
    let bookmark_names = document_xml.as_deref().map(docx_bookmark_names);
    let note_ref_target_names = document_xml
        .as_deref()
        .map(crate::docx::note_ref_target_names);
    let unsupported_page_ref_section_format_targets = document_xml
        .as_deref()
        .map(docx_page_ref_unsupported_section_format_targets);
    let page_field_unsupported_display_formats = document_xml
        .as_deref()
        .map(crate::docx::page_field_unsupported_display_formats);
    let mut counts: Vec<FieldEvaluationReasonCount> = Vec::new();
    let mut page_field_index = 0usize;
    let mut field_bookmarks = HashMap::new();
    for field in fields {
        let page_field_unsupported_display_format = if field.kind == FieldKind::Page {
            let unsupported = page_field_unsupported_display_formats
                .as_ref()
                .and_then(|formats| formats.get(page_field_index))
                .copied()
                .unwrap_or(false);
            page_field_index += 1;
            unsupported
        } else {
            false
        };
        let merged_ref_bookmark_names;
        let bookmark_names_for_field =
            if field.kind == FieldKind::Ref && !field_bookmarks.is_empty() {
                let mut names = bookmark_names.as_ref().cloned().unwrap_or_default();
                names.extend(field_bookmarks.keys().cloned());
                merged_ref_bookmark_names = names;
                Some(&merged_ref_bookmark_names)
            } else {
                bookmark_names.as_ref()
            };
        if let Some(reason) = unsupported_docx_field_reason(
            field,
            bookmark_names_for_field,
            note_ref_target_names.as_ref(),
            unsupported_page_ref_section_format_targets.as_ref(),
            page_field_unsupported_display_format,
        ) {
            increment_field_evaluation_reason_count(&mut counts, reason);
        }
        crate::docx::update_field_bookmarks_from_instruction(
            &field.instruction,
            &mut field_bookmarks,
        );
    }
    counts
}

fn increment_field_evaluation_reason_count(
    counts: &mut Vec<FieldEvaluationReasonCount>,
    reason: FieldEvaluationReason,
) {
    if let Some(existing) = counts.iter_mut().find(|item| item.reason == reason) {
        existing.count += 1;
    } else {
        counts.push(FieldEvaluationReasonCount { reason, count: 1 });
    }
}

#[cfg(feature = "docx")]
fn unsupported_docx_field_reason(
    field: &Field,
    bookmark_names: Option<&HashSet<String>>,
    note_ref_target_names: Option<&HashSet<String>>,
    unsupported_page_ref_section_format_targets: Option<&HashSet<String>>,
    page_field_unsupported_display_format: bool,
) -> Option<FieldEvaluationReason> {
    if supports_field_evaluation(field) {
        return None;
    }
    if field.kind == FieldKind::Page && page_field_unsupported_display_format {
        return Some(FieldEvaluationReason::UnsupportedSwitch);
    }
    if field.kind == FieldKind::PageRef {
        return Some(docx_page_ref_uncomputed_reason(
            &field.instruction,
            bookmark_names,
            unsupported_page_ref_section_format_targets,
        ));
    }
    if field.kind == FieldKind::Ref {
        return Some(ref_uncomputed_reason(&field.instruction, bookmark_names));
    }
    if field.kind == FieldKind::NoteRef {
        return Some(note_ref_uncomputed_reason(
            &field.instruction,
            bookmark_names,
            note_ref_target_names,
        ));
    }
    if field.kind == FieldKind::Toc {
        return Some(toc_uncomputed_reason(&field.instruction, bookmark_names));
    }
    unsupported_field_reason(field)
}

fn model_unsupported_field_reason(
    entry: &ModelFieldEntry,
    context: &ModelFieldReasonContext,
) -> Option<FieldEvaluationReason> {
    let field = &entry.field;
    if let Some(reason) = entry.unsupported_reason {
        return Some(model_field_unsupported_reason(reason));
    }
    if supports_field_evaluation(field) {
        return None;
    }
    if field.kind == FieldKind::PageRef {
        return Some(page_ref_uncomputed_reason(
            &field.instruction,
            Some(&context.bookmark_names),
        ));
    }
    if field.kind == FieldKind::Ref {
        return Some(ref_uncomputed_reason(
            &field.instruction,
            Some(&context.bookmark_names),
        ));
    }
    #[cfg(feature = "docx")]
    {
        if field.kind == FieldKind::NoteRef {
            return Some(note_ref_uncomputed_reason(
                &field.instruction,
                Some(&context.bookmark_names),
                Some(&context.note_ref_target_names),
            ));
        }
    }
    if field.kind == FieldKind::Toc {
        return Some(toc_uncomputed_reason(
            &field.instruction,
            Some(&context.bookmark_names),
        ));
    }
    unsupported_field_reason(field)
}

fn unsupported_field_reason(field: &Field) -> Option<FieldEvaluationReason> {
    if supports_field_evaluation(field) {
        return None;
    }
    match field.kind {
        FieldKind::Unknown(_) => Some(FieldEvaluationReason::UnknownField),
        FieldKind::Page => Some(page_uncomputed_reason(&field.instruction)),
        FieldKind::Ref => Some(ref_uncomputed_reason(&field.instruction, None)),
        FieldKind::Toc => Some(toc_uncomputed_reason(&field.instruction, None)),
        FieldKind::PageRef => Some(page_ref_uncomputed_reason(&field.instruction, None)),
        FieldKind::NoteRef => {
            if supported_note_ref_target(&field.instruction).is_some() {
                Some(FieldEvaluationReason::UnresolvedBookmark)
            } else {
                Some(FieldEvaluationReason::UnsupportedSwitch)
            }
        }
        FieldKind::Dynamic(ref kind) if kind == "=" => {
            Some(formula_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(ref kind) if kind.eq_ignore_ascii_case("COMPARE") => {
            Some(compare_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(ref kind) if kind.eq_ignore_ascii_case("IF") => {
            Some(if_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(ref kind) if kind.eq_ignore_ascii_case("QUOTE") => {
            Some(quote_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(ref kind)
            if kind.eq_ignore_ascii_case("FILLIN") || kind.eq_ignore_ascii_case("ASK") =>
        {
            Some(prompt_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(ref kind) if kind.eq_ignore_ascii_case("SET") => {
            Some(set_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(ref kind)
            if kind.eq_ignore_ascii_case("NEXT")
                || kind.eq_ignore_ascii_case("NEXTIF")
                || kind.eq_ignore_ascii_case("SKIPIF") =>
        {
            Some(merge_control_uncomputed_reason(&field.instruction))
        }
        FieldKind::Dynamic(_) => Some(FieldEvaluationReason::NoComputedResult),
        FieldKind::InsertedContent(_) => {
            Some(inserted_content_uncomputed_reason(&field.instruction))
        }
        FieldKind::MailMerge(_) => Some(mail_merge_uncomputed_reason(&field.instruction)),
        FieldKind::ReferenceIndex(ref kind) if is_reference_index_marker_kind(kind.as_str()) => {
            Some(reference_index_marker_uncomputed_reason(&field.instruction))
        }
        FieldKind::ReferenceIndex(_) => Some(reference_index_uncomputed_reason(&field.instruction)),
        FieldKind::Numbering(_) => Some(numbering_uncomputed_reason(&field.instruction)),
        FieldKind::DocumentStructure(ref kind)
            if is_section_document_structure_kind(kind.as_str()) =>
        {
            Some(section_document_structure_uncomputed_reason(
                &field.instruction,
            ))
        }
        FieldKind::DocumentStructure(ref kind) if kind.eq_ignore_ascii_case("REVNUM") => {
            Some(revision_number_uncomputed_reason(&field.instruction))
        }
        FieldKind::DocumentStructure(ref kind) if kind.eq_ignore_ascii_case("STYLEREF") => {
            Some(style_ref_uncomputed_reason(&field.instruction))
        }
        FieldKind::DocumentStructure(_) => Some(FieldEvaluationReason::NoComputedResult),
        FieldKind::Display(_) => Some(display_uncomputed_reason(&field.instruction)),
        FieldKind::Action(_) => Some(action_uncomputed_reason(&field.instruction)),
        FieldKind::Compatibility(_) => Some(compatibility_uncomputed_reason(&field.instruction)),
        FieldKind::Barcode(_) => Some(barcode_uncomputed_reason(&field.instruction)),
        FieldKind::FormField(_) => Some(form_field_uncomputed_reason(&field.instruction)),
        FieldKind::Filename => Some(filename_uncomputed_reason(&field.instruction)),
        FieldKind::Hyperlink => Some(FieldEvaluationReason::UnsupportedSwitch),
        FieldKind::MergeField => Some(FieldEvaluationReason::UnsupportedSwitch),
        FieldKind::DocumentInfo(_) => Some(document_info_uncomputed_reason(&field.instruction)),
        FieldKind::Sequence => Some(sequence_uncomputed_reason(&field.instruction)),
        FieldKind::TocEntry => Some(toc_entry_uncomputed_reason(&field.instruction)),
    }
}

fn document_info_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_document_info_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        if supported_document_info_syntax_for_report(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    FieldEvaluationReason::UnsupportedSwitch
}

#[cfg(not(feature = "docx"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum DocumentInfoSyntaxProperty {
    FileSize,
    UserInfo,
    Other,
}

#[cfg(not(feature = "docx"))]
fn supported_document_info_syntax_for_report(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let Some(kind) = parts.next() else {
        return false;
    };
    let Some(property) = document_info_syntax_property_for_report(kind, &mut parts) else {
        return false;
    };
    let mut text_format = false;
    let mut date_format = false;
    let mut file_size_unit = false;
    let mut user_override = false;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_field_format_for_report(part, &mut parts, &mut text_format)
        else {
            return false;
        };
        if accepted {
            continue;
        }
        if part.eq_ignore_ascii_case("\\@") {
            if date_format
                || document_info_date_format_for_report(parts.next(), &mut parts).is_none()
            {
                return false;
            }
            date_format = true;
            continue;
        }
        if let Some(format) = strip_ascii_switch_prefix(part, "\\@") {
            if date_format
                || format.is_empty()
                || document_info_date_format_for_report(Some(format), &mut parts).is_none()
            {
                return false;
            }
            date_format = true;
            continue;
        }
        if document_info_file_size_unit_switch_for_report(part) {
            if property != DocumentInfoSyntaxProperty::FileSize || file_size_unit {
                return false;
            }
            file_size_unit = true;
            continue;
        }
        if property == DocumentInfoSyntaxProperty::UserInfo && !part.starts_with('\\') {
            if user_override || document_info_literal_operand_for_report(part, &mut parts).is_none()
            {
                return false;
            }
            user_override = true;
            continue;
        }
        return false;
    }
    true
}

#[cfg(not(feature = "docx"))]
fn document_info_syntax_property_for_report<'a>(
    kind: &str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<DocumentInfoSyntaxProperty> {
    if kind.eq_ignore_ascii_case("DOCPROPERTY") {
        let name = diagnostic_name_operand(parts.next()?, parts)?;
        return Some(
            document_info_property_kind_for_report(&name)
                .unwrap_or(DocumentInfoSyntaxProperty::Other),
        );
    }
    if kind.eq_ignore_ascii_case("DOCVARIABLE") {
        diagnostic_name_operand(parts.next()?, parts)?;
        return Some(DocumentInfoSyntaxProperty::Other);
    }
    if kind.eq_ignore_ascii_case("INFO") {
        diagnostic_name_token(parts.next()?)?;
        return Some(DocumentInfoSyntaxProperty::Other);
    }
    if is_user_info_property_for_report(kind) {
        return Some(DocumentInfoSyntaxProperty::UserInfo);
    }
    if kind.eq_ignore_ascii_case("DATE") || kind.eq_ignore_ascii_case("TIME") {
        return Some(DocumentInfoSyntaxProperty::Other);
    }
    document_info_property_kind_for_report(kind)
}

#[cfg(not(feature = "docx"))]
fn document_info_property_kind_for_report(value: &str) -> Option<DocumentInfoSyntaxProperty> {
    let key = document_property_key(value);
    if key == "FILESIZE" {
        return Some(DocumentInfoSyntaxProperty::FileSize);
    }
    if matches!(
        key.as_str(),
        "TITLE"
            | "SUBJECT"
            | "AUTHOR"
            | "CREATOR"
            | "COMMENTS"
            | "COMMENT"
            | "DESCRIPTION"
            | "KEYWORDS"
            | "KEYWORD"
            | "CATEGORY"
            | "CONTENTSTATUS"
            | "LASTSAVEDBY"
            | "LASTMODIFIEDBY"
            | "CREATEDATE"
            | "SAVEDATE"
            | "PRINTDATE"
            | "VERSION"
            | "APPLICATION"
            | "APPVERSION"
            | "COMPANY"
            | "DOCSECURITY"
            | "HIDDENSLIDES"
            | "HYPERLINKBASE"
            | "HYPERLINKSCHANGED"
            | "LINES"
            | "LINKSUPTODATE"
            | "MANAGER"
            | "MMCLIPS"
            | "NOTES"
            | "PAGES"
            | "NUMPAGES"
            | "PARAGRAPHS"
            | "PRESENTATIONFORMAT"
            | "SCALECROP"
            | "SHAREDDOC"
            | "SLIDES"
            | "WORDS"
            | "NUMWORDS"
            | "CHARACTERS"
            | "NUMCHARS"
            | "CHARACTERSWITHSPACES"
            | "TOTALTIME"
            | "EDITTIME"
            | "TEMPLATE"
    ) {
        return Some(DocumentInfoSyntaxProperty::Other);
    }
    None
}

#[cfg(not(feature = "docx"))]
fn is_user_info_property_for_report(value: &str) -> bool {
    let key = document_property_key(value);
    matches!(key.as_str(), "USERNAME" | "USERINITIALS" | "USERADDRESS")
}

#[cfg(not(feature = "docx"))]
fn document_info_date_format_for_report<'a>(
    first: Option<&'a str>,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<()> {
    let first = first?;
    if let Some(format) = field_quoted_literal_token(first) {
        return (!format.is_empty()).then_some(());
    }
    field_non_empty_non_switch_literal_token(first)?;
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        field_non_empty_non_switch_literal_token(parts.next()?)?;
    }
    Some(())
}

#[cfg(not(feature = "docx"))]
fn document_info_file_size_unit_switch_for_report(part: &str) -> bool {
    part.eq_ignore_ascii_case("\\k") || part.eq_ignore_ascii_case("\\m")
}

#[cfg(not(feature = "docx"))]
fn document_info_literal_operand_for_report<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<()> {
    if let Some(value) = field_quoted_literal_token(first) {
        return (!value.is_empty()).then_some(());
    }
    field_non_empty_non_switch_literal_token(first)?;
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        field_non_empty_non_switch_literal_token(parts.next()?)?;
    }
    Some(())
}

fn filename_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_filename_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

#[cfg(feature = "docx")]
fn supported_filename_syntax(instruction: &str) -> bool {
    crate::docx::supports_filename_field_syntax(instruction)
}

#[cfg(not(feature = "docx"))]
fn supported_filename_syntax(instruction: &str) -> bool {
    filename_field_syntax(instruction)
}

fn ref_uncomputed_reason(
    instruction: &str,
    bookmark_names: Option<&HashSet<String>>,
) -> FieldEvaluationReason {
    let Some(syntax) =
        supported_ref_syntax(instruction).or_else(|| supported_direct_ref_syntax(instruction))
    else {
        return FieldEvaluationReason::UnsupportedSwitch;
    };
    let target_exists = bookmark_names.is_some_and(|names| names.contains(&syntax.target));
    if bookmark_names.is_some_and(|names| !names.contains(&syntax.target)) {
        return FieldEvaluationReason::UnresolvedBookmark;
    }
    if syntax.sequence_separator {
        FieldEvaluationReason::NoComputedResult
    } else if syntax.note_reference {
        if target_exists {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    } else if target_exists {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnresolvedBookmark
    }
}

fn supported_ref_syntax(instruction: &str) -> Option<RefFieldSyntax> {
    ref_field_syntax(instruction)
}

fn supported_direct_ref_syntax(instruction: &str) -> Option<RefFieldSyntax> {
    direct_ref_field_syntax(instruction)
}

fn page_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_page_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

#[cfg(feature = "docx")]
fn supported_page_syntax(instruction: &str) -> bool {
    crate::docx::supports_page_field_syntax(instruction)
}

#[cfg(not(feature = "docx"))]
fn supported_page_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("PAGE") {
        return false;
    }
    page_field_format_syntax_tail(&mut parts).is_some()
}

fn is_section_document_structure_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("SECTION") || kind.eq_ignore_ascii_case("SECTIONPAGES")
}

fn section_document_structure_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_section_document_structure_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

#[cfg(feature = "docx")]
fn supported_section_document_structure_syntax(instruction: &str) -> bool {
    crate::docx::supports_section_field_syntax(instruction)
}

#[cfg(not(feature = "docx"))]
fn supported_section_document_structure_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !is_section_document_structure_kind(kind) {
        return false;
    }
    page_field_format_syntax_tail(&mut parts).is_some()
}

fn revision_number_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_revision_number_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        if supported_revision_number_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

#[cfg(not(feature = "docx"))]
fn supported_revision_number_syntax(instruction: &str) -> bool {
    revision_number_field_text_format(instruction).is_some()
}

fn style_ref_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_style_ref_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        if supported_style_ref_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

#[cfg(not(feature = "docx"))]
fn supported_style_ref_syntax(instruction: &str) -> bool {
    style_ref_field_syntax(instruction).is_some()
}

fn display_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_display_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        if supported_advance_syntax(instruction)
            || supported_symbol_syntax(instruction)
            || supported_eq_displacement_syntax(instruction)
            || supported_eq_script_syntax(instruction)
            || supported_eq_fraction_syntax(instruction)
            || supported_eq_radical_syntax(instruction)
            || supported_eq_list_syntax(instruction)
            || supported_eq_array_syntax(instruction)
            || supported_eq_integral_syntax(instruction)
            || supported_eq_overstrike_syntax(instruction)
            || supported_eq_box_syntax(instruction)
            || supported_eq_bracket_syntax(instruction)
        {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    FieldEvaluationReason::UnsupportedSwitch
}

#[cfg(not(feature = "docx"))]
fn supported_advance_syntax(instruction: &str) -> bool {
    advance_field_syntax(instruction)
}

#[cfg(not(feature = "docx"))]
fn supported_symbol_syntax(instruction: &str) -> bool {
    symbol_field_syntax(instruction).is_some()
}

#[cfg(not(feature = "docx"))]
fn supported_eq_displacement_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(mut body) = strip_ascii_switch_prefix(expression.trim_start(), "\\d") else {
        return false;
    };
    body = body.trim_start();
    let mut has_option = false;
    loop {
        if let Some(rest) = eq_numeric_prefix_tail_for_report(body, "\\fo")
            .or_else(|| eq_numeric_prefix_tail_for_report(body, "\\ba"))
        {
            has_option = true;
            body = rest.trim_start();
            continue;
        }
        if let Some(rest) = eq_prefix_switch_tail(body, "\\li") {
            has_option = true;
            body = rest.trim_start();
            continue;
        }
        break;
    }
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    has_option && (inner.trim().is_empty() || eq_operand_for_report(inner))
}

#[cfg(not(feature = "docx"))]
fn eq_numeric_prefix_tail_for_report<'a>(value: &'a str, option: &str) -> Option<&'a str> {
    eq_numeric_prefix_option(value, option).map(|(_, rest)| rest)
}

#[cfg(not(feature = "docx"))]
fn supported_eq_script_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let mut body = expression.trim_start();
    loop {
        let Some(rest) = strip_ascii_switch_prefix(body, "\\s") else {
            return false;
        };
        let Some(remaining) = eq_script_syntax_segment_for_report(rest.trim_start()) else {
            return false;
        };
        body = remaining.trim_start();
        if body.is_empty() {
            return true;
        }
    }
}

#[cfg(not(feature = "docx"))]
fn eq_expression_for_report(instruction: &str) -> Option<String> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("EQ") {
        return None;
    }
    let mut expression_parts = Vec::new();
    let mut text_format = false;
    while let Some(part) = parts.next() {
        if accept_field_format_for_report(part, &mut parts, &mut text_format)? {
            continue;
        }
        expression_parts.push(part);
    }
    (!expression_parts.is_empty()).then(|| expression_parts.join(" "))
}

#[cfg(not(feature = "docx"))]
fn eq_script_syntax_segment_for_report(mut body: &str) -> Option<&str> {
    let mut saw_option = false;
    loop {
        if body.is_empty() || eq_prefix_switch_tail(body, "\\s").is_some() {
            return saw_option.then_some(body);
        }
        if let Some(rest) = consume_eq_script_option_for_report(body, "\\up", false)
            .or_else(|| consume_eq_script_option_for_report(body, "\\do", false))
            .or_else(|| consume_eq_script_option_for_report(body, "\\ai", true))
            .or_else(|| consume_eq_script_option_for_report(body, "\\di", true))
        {
            body = rest.trim_start();
            saw_option = true;
            continue;
        }
        return None;
    }
}

#[cfg(not(feature = "docx"))]
fn consume_eq_script_option_for_report<'a>(
    value: &'a str,
    option: &str,
    allow_empty: bool,
) -> Option<&'a str> {
    let rest = eq_numeric_prefix_tail_for_report(value, option)?;
    let (operand, rest) = eq_parenthesized_operand(rest)?;
    if operand.trim().is_empty() {
        return allow_empty.then_some(rest);
    }
    eq_operand_for_report(operand).then_some(rest)
}

#[cfg(not(feature = "docx"))]
fn supported_eq_fraction_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(body) = strip_ascii_switch_prefix(expression.trim_start(), "\\f") else {
        return false;
    };
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    let Some((numerator, denominator)) = eq_fraction_operands(inner) else {
        return false;
    };
    eq_operand_for_report(numerator) && eq_operand_for_report(denominator)
}

#[cfg(not(feature = "docx"))]
fn eq_operand_for_report(operand: &str) -> bool {
    let operand = operand.trim();
    if operand.is_empty() {
        return false;
    }
    let Some(text) = diagnostic_literal_token(operand) else {
        return false;
    };
    if supported_eq_operand_expression_for_report(text) {
        return true;
    }
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            continue;
        }
        if !matches!(chars.next(), Some(',' | ';' | '(' | ')' | '\\')) {
            return false;
        }
    }
    true
}

#[cfg(not(feature = "docx"))]
fn supported_eq_operand_expression_for_report(expression: &str) -> bool {
    let instruction = format!("EQ {expression}");
    supported_eq_displacement_syntax(&instruction)
        || supported_eq_script_syntax(&instruction)
        || supported_eq_fraction_syntax(&instruction)
        || supported_eq_radical_syntax(&instruction)
        || supported_eq_list_syntax(&instruction)
        || supported_eq_array_syntax(&instruction)
        || supported_eq_integral_syntax(&instruction)
        || supported_eq_overstrike_syntax(&instruction)
        || supported_eq_box_syntax(&instruction)
        || supported_eq_bracket_syntax(&instruction)
}

#[cfg(not(feature = "docx"))]
fn supported_eq_radical_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(body) = strip_ascii_switch_prefix(expression.trim_start(), "\\r") else {
        return false;
    };
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    let Some((degree, radicand)) = eq_radical_operands(inner) else {
        return false;
    };
    eq_operand_for_report(degree)
        && match radicand {
            Some(radicand) => eq_operand_for_report(radicand),
            None => true,
        }
}

#[cfg(not(feature = "docx"))]
fn supported_eq_list_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(body) = strip_ascii_switch_prefix(expression.trim_start(), "\\l") else {
        return false;
    };
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    eq_list_operands(inner).is_some_and(|operands| {
        operands
            .iter()
            .all(|operand| eq_operand_for_report(operand))
    })
}

#[cfg(not(feature = "docx"))]
fn supported_eq_array_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(mut body) = strip_ascii_switch_prefix(expression.trim_start(), "\\a") else {
        return false;
    };
    body = body.trim_start();
    loop {
        if let Some(rest) = eq_prefix_switch_tail(body, "\\al")
            .or_else(|| eq_prefix_switch_tail(body, "\\ac"))
            .or_else(|| eq_prefix_switch_tail(body, "\\ar"))
        {
            body = rest.trim_start();
            continue;
        }
        if let Some((columns, rest)) = eq_numeric_prefix_option(body, "\\co") {
            if columns.fract() != 0.0 || columns < 1.0 {
                return false;
            }
            body = rest.trim_start();
            continue;
        }
        if let Some(rest) = eq_numeric_prefix_tail_for_report(body, "\\vs")
            .or_else(|| eq_numeric_prefix_tail_for_report(body, "\\hs"))
        {
            body = rest.trim_start();
            continue;
        }
        break;
    }
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    eq_list_operands(inner).is_some_and(|operands| {
        operands
            .iter()
            .all(|operand| eq_operand_for_report(operand))
    })
}

#[cfg(not(feature = "docx"))]
fn supported_eq_integral_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(mut body) = strip_ascii_switch_prefix(expression.trim_start(), "\\i") else {
        return false;
    };
    body = body.trim_start();
    loop {
        if let Some(rest) = eq_prefix_switch_tail(body, "\\su")
            .or_else(|| eq_prefix_switch_tail(body, "\\pr"))
            .or_else(|| eq_prefix_switch_tail(body, "\\in"))
        {
            body = rest.trim_start();
            continue;
        }
        if let Some((_, rest)) = consume_eq_bracket_option_for_report(body, "\\fc")
            .or_else(|| consume_eq_bracket_option_for_report(body, "\\vc"))
        {
            body = rest.trim_start();
            continue;
        }
        break;
    }
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    let Some(operands) = eq_list_operands(inner) else {
        return false;
    };
    operands.len() == 3
        && operands
            .iter()
            .all(|operand| eq_operand_for_report(operand))
}

#[cfg(not(feature = "docx"))]
fn supported_eq_overstrike_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(mut body) = strip_ascii_switch_prefix(expression.trim_start(), "\\o") else {
        return false;
    };
    body = body.trim_start();
    while let Some(rest) = eq_prefix_switch_tail(body, "\\al")
        .or_else(|| eq_prefix_switch_tail(body, "\\ac"))
        .or_else(|| eq_prefix_switch_tail(body, "\\ar"))
    {
        body = rest.trim_start();
    }
    let Some(inner) = body
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    else {
        return false;
    };
    eq_list_operands(inner).is_some_and(|operands| {
        operands
            .iter()
            .all(|operand| eq_operand_for_report(operand))
    })
}

#[cfg(not(feature = "docx"))]
fn supported_eq_box_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(inner) = eq_enclosed_operand_with_prefixes_for_report(
        expression.trim_start(),
        "\\x",
        &["\\to", "\\bo", "\\le", "\\ri"],
    ) else {
        return false;
    };
    eq_operand_for_report(inner)
}

#[cfg(not(feature = "docx"))]
fn eq_enclosed_operand_with_prefixes_for_report<'a>(
    expression: &'a str,
    switch: &str,
    options: &[&str],
) -> Option<&'a str> {
    let mut body = strip_ascii_switch_prefix(expression, switch)?.trim_start();
    loop {
        let mut consumed = false;
        for option in options {
            if let Some(rest) = eq_prefix_switch_tail(body, option) {
                body = rest.trim_start();
                consumed = true;
                break;
            }
        }
        if !consumed {
            break;
        }
    }
    eq_enclosed_operand(body)
}

#[cfg(not(feature = "docx"))]
fn supported_eq_bracket_syntax(instruction: &str) -> bool {
    let Some(expression) = eq_expression_for_report(instruction) else {
        return false;
    };
    let Some(mut body) = strip_ascii_switch_prefix(expression.trim_start(), "\\b") else {
        return false;
    };
    body = body.trim_start();
    loop {
        if let Some((_, rest)) = consume_eq_bracket_option_for_report(body, "\\bc")
            .or_else(|| consume_eq_bracket_option_for_report(body, "\\lc"))
            .or_else(|| consume_eq_bracket_option_for_report(body, "\\rc"))
        {
            body = rest.trim_start();
            continue;
        }
        break;
    }
    let (inner, rest) = match eq_parenthesized_operand(body.trim_start()) {
        Some(value) => value,
        None => return false,
    };
    rest.trim().is_empty() && eq_operand_for_report(inner)
}

#[cfg(not(feature = "docx"))]
fn consume_eq_bracket_option_for_report<'a>(
    value: &'a str,
    option: &str,
) -> Option<(char, &'a str)> {
    let rest = strip_ascii_switch_prefix(value, option)?;
    consume_eq_bracket_char_for_report(rest)
}

#[cfg(not(feature = "docx"))]
fn consume_eq_bracket_char_for_report(value: &str) -> Option<(char, &str)> {
    let rest = value.trim_start();
    let rest = rest.strip_prefix('\\').unwrap_or(rest);
    let ch = rest.chars().next()?;
    if ch.is_whitespace() {
        return None;
    }
    Some((ch, &rest[ch.len_utf8()..]))
}

fn action_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_action_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        if supported_action_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    FieldEvaluationReason::UnsupportedSwitch
}

#[cfg(not(feature = "docx"))]
fn supported_action_syntax(instruction: &str) -> bool {
    action_field_syntax(instruction).is_some()
}

fn inserted_content_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_inserted_content_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

fn supported_inserted_content_syntax(instruction: &str) -> bool {
    opaque_field_syntax(instruction, is_inserted_content_kind)
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

fn mail_merge_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_mail_merge_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

fn supported_mail_merge_syntax(instruction: &str) -> bool {
    opaque_field_syntax(instruction, is_mail_merge_kind)
}

fn is_mail_merge_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "ADDRESSBLOCK" | "GREETINGLINE" | "MERGEREC" | "MERGESEQ"
    )
}

fn reference_index_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_reference_index_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

fn supported_reference_index_syntax(instruction: &str) -> bool {
    opaque_field_syntax(instruction, is_generated_reference_index_kind)
}

fn is_generated_reference_index_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "BIBLIOGRAPHY" | "CITATION" | "INDEX" | "TOA"
    )
}

fn compatibility_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_compatibility_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

fn supported_compatibility_syntax(instruction: &str) -> bool {
    opaque_field_syntax(instruction, is_compatibility_kind)
}

fn is_compatibility_kind(kind: &str) -> bool {
    matches!(
        kind.to_ascii_uppercase().as_str(),
        "ADDIN" | "DATA" | "GLOSSARY" | "HTMLACTIVEX" | "PRIVATE"
    )
}

fn barcode_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if barcode_field_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

fn form_field_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    if supported_form_field_syntax(instruction) {
        FieldEvaluationReason::NoComputedResult
    } else {
        FieldEvaluationReason::UnsupportedSwitch
    }
}

fn supported_form_field_syntax(instruction: &str) -> bool {
    legacy_form_field_syntax(instruction).is_some()
}

fn is_reference_index_marker_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("RD")
        || kind.eq_ignore_ascii_case("TA")
        || kind.eq_ignore_ascii_case("XE")
}

fn reference_index_marker_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_reference_index_marker_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    #[cfg(not(feature = "docx"))]
    {
        if supported_reference_index_marker_syntax_for_report(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
    }
    FieldEvaluationReason::UnsupportedSwitch
}

#[cfg(not(feature = "docx"))]
fn supported_reference_index_marker_syntax_for_report(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if kind.eq_ignore_ascii_case("RD") {
        return supported_reference_index_rd_syntax_for_report(parts);
    }
    if kind.eq_ignore_ascii_case("TA") {
        return supported_reference_index_ta_syntax_for_report(parts);
    }
    if kind.eq_ignore_ascii_case("XE") {
        return supported_reference_index_xe_syntax_for_report(parts);
    }
    false
}

#[cfg(not(feature = "docx"))]
fn supported_reference_index_rd_syntax_for_report<'a>(
    mut parts: impl Iterator<Item = &'a str>,
) -> bool {
    if parts
        .next()
        .and_then(reference_index_literal_token)
        .is_none()
    {
        return false;
    }
    let mut text_format = false;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\f") {
            continue;
        }
        if accept_field_format_for_report(part, &mut parts, &mut text_format)
            .is_some_and(|accepted| accepted)
        {
            continue;
        }
        return false;
    }
    true
}

#[cfg(not(feature = "docx"))]
fn supported_reference_index_ta_syntax_for_report<'a>(
    parts: impl Iterator<Item = &'a str>,
) -> bool {
    let mut parts = parts.peekable();
    let mut has_entry_text = false;
    let mut text_format = false;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\l") || part.eq_ignore_ascii_case("\\s") {
            let Some(first) = parts.next() else {
                return false;
            };
            if reference_index_literal_operand(first, &mut parts).is_none() {
                return false;
            }
            has_entry_text = true;
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\l")
            .or_else(|| strip_ascii_switch_prefix(part, "\\s"))
        {
            if value.is_empty() || reference_index_literal_token(value).is_none() {
                return false;
            }
            has_entry_text = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\c") {
            if parts
                .next()
                .and_then(reference_index_category_token)
                .is_none()
            {
                return false;
            }
            continue;
        }
        if let Some(category) = strip_ascii_switch_prefix(part, "\\c") {
            if category.is_empty() || reference_index_category_token(category).is_none() {
                return false;
            }
            continue;
        }
        if accept_field_format_for_report(part, &mut parts, &mut text_format)
            .is_some_and(|accepted| accepted)
        {
            continue;
        }
        return false;
    }
    has_entry_text
}

#[cfg(not(feature = "docx"))]
fn supported_reference_index_xe_syntax_for_report<'a>(
    parts: impl Iterator<Item = &'a str>,
) -> bool {
    let mut parts = parts.peekable();
    if parts
        .next()
        .and_then(reference_index_literal_token)
        .is_none()
    {
        return false;
    }
    let mut text_format = false;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\b") || part.eq_ignore_ascii_case("\\i") {
            continue;
        }
        if part.eq_ignore_ascii_case("\\f") || part.eq_ignore_ascii_case("\\r") {
            if parts
                .next()
                .and_then(reference_index_plain_value_token)
                .is_none()
            {
                return false;
            }
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\f")
            .or_else(|| strip_ascii_switch_prefix(part, "\\r"))
        {
            if value.is_empty() || reference_index_plain_value_token(value).is_none() {
                return false;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\t") {
            let Some(first) = parts.next() else {
                return false;
            };
            if reference_index_literal_operand(first, &mut parts).is_none() {
                return false;
            }
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\t") {
            if value.is_empty() || reference_index_literal_token(value).is_none() {
                return false;
            }
            continue;
        }
        if accept_field_format_for_report(part, &mut parts, &mut text_format)
            .is_some_and(|accepted| accepted)
        {
            continue;
        }
        return false;
    }
    true
}

fn toc_entry_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_toc_entry_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if toc_entry_field_syntax(instruction).is_some() {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn numbering_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_numbering_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if numbering_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn compare_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_compare_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if compare_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn formula_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_formula_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if formula_field_expression_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn sequence_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_sequence_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if sequence_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn if_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_if_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if if_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn quote_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_quote_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if quote_field_syntax(instruction).is_some() {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn prompt_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_prompt_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if prompt_field_syntax(instruction).is_some() {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn set_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_set_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if set_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

fn merge_control_uncomputed_reason(instruction: &str) -> FieldEvaluationReason {
    #[cfg(feature = "docx")]
    {
        if crate::docx::supports_merge_control_field_syntax(instruction) {
            return FieldEvaluationReason::NoComputedResult;
        }
        FieldEvaluationReason::UnsupportedSwitch
    }
    #[cfg(not(feature = "docx"))]
    {
        if merge_control_field_syntax(instruction) {
            FieldEvaluationReason::NoComputedResult
        } else {
            FieldEvaluationReason::UnsupportedSwitch
        }
    }
}

struct PageRefDiagnosticSyntax {
    target: String,
    #[cfg(feature = "docx")]
    uses_target_section_number_format: bool,
}

fn supported_page_ref_syntax(instruction: &str) -> Option<PageRefDiagnosticSyntax> {
    let syntax = page_ref_field_syntax(instruction)?;
    Some(PageRefDiagnosticSyntax {
        target: syntax.target,
        #[cfg(feature = "docx")]
        uses_target_section_number_format: syntax.number_format.is_none(),
    })
}

fn page_ref_uncomputed_reason(
    instruction: &str,
    bookmark_names: Option<&HashSet<String>>,
) -> FieldEvaluationReason {
    let Some(syntax) = supported_page_ref_syntax(instruction) else {
        return FieldEvaluationReason::UnsupportedSwitch;
    };
    if bookmark_names.is_some_and(|names| !names.contains(&syntax.target)) {
        return FieldEvaluationReason::UnresolvedBookmark;
    }
    FieldEvaluationReason::NoComputedResult
}

#[cfg(feature = "docx")]
fn docx_page_ref_uncomputed_reason(
    instruction: &str,
    bookmark_names: Option<&HashSet<String>>,
    unsupported_section_format_targets: Option<&HashSet<String>>,
) -> FieldEvaluationReason {
    let Some(syntax) = supported_page_ref_syntax(instruction) else {
        return FieldEvaluationReason::UnsupportedSwitch;
    };
    if bookmark_names.is_some_and(|names| !names.contains(&syntax.target)) {
        return FieldEvaluationReason::UnresolvedBookmark;
    }
    if syntax.uses_target_section_number_format
        && unsupported_section_format_targets
            .is_some_and(|targets| targets.contains(&syntax.target))
    {
        return FieldEvaluationReason::UnsupportedSwitch;
    }
    FieldEvaluationReason::NoComputedResult
}

#[cfg(feature = "docx")]
fn note_ref_uncomputed_reason(
    instruction: &str,
    bookmark_names: Option<&HashSet<String>>,
    note_ref_target_names: Option<&HashSet<String>>,
) -> FieldEvaluationReason {
    let Some(target) = supported_note_ref_target(instruction) else {
        return FieldEvaluationReason::UnsupportedSwitch;
    };
    if note_ref_target_names.is_some_and(|names| names.contains(&target)) {
        return FieldEvaluationReason::NoComputedResult;
    }
    match bookmark_names {
        Some(names) if names.contains(&target) => FieldEvaluationReason::NoComputedResult,
        Some(_) => FieldEvaluationReason::UnresolvedBookmark,
        None => FieldEvaluationReason::UnresolvedBookmark,
    }
}

fn supported_note_ref_target(instruction: &str) -> Option<String> {
    note_ref_field_syntax(instruction).map(|syntax| syntax.target)
}

#[cfg(not(feature = "docx"))]
fn accept_field_format_switch(part: &str, text_format: &mut bool) -> bool {
    let mut format = text_format.then_some(FieldTextFormat::Upper);
    let accepted = accept_field_text_format_switch(part, &mut format);
    if accepted {
        *text_format = format.is_some();
    }
    accepted
}

#[cfg(not(feature = "docx"))]
fn accept_field_format_for_report<'a>(
    part: &'a str,
    parts: &mut impl Iterator<Item = &'a str>,
    text_format: &mut bool,
) -> Option<bool> {
    accept_general_format_switch(part, parts, |format| {
        accept_field_format_switch(format, text_format)
    })
}

fn supported_toc_bookmark_scope(instruction: &str) -> Option<Option<String>> {
    toc_field_syntax(instruction).map(|syntax| syntax.bookmark)
}

fn toc_uncomputed_reason(
    instruction: &str,
    bookmark_names: Option<&HashSet<String>>,
) -> FieldEvaluationReason {
    match supported_toc_bookmark_scope(instruction) {
        Some(Some(target)) => match bookmark_names {
            Some(names) if names.contains(&target) => FieldEvaluationReason::NoComputedResult,
            Some(_) => FieldEvaluationReason::UnresolvedBookmark,
            None => FieldEvaluationReason::UnresolvedBookmark,
        },
        Some(None) => FieldEvaluationReason::NoComputedResult,
        None => FieldEvaluationReason::UnsupportedSwitch,
    }
}

#[cfg(feature = "docx")]
fn scan_docx_xml(xml: &str, features: &mut FeatureInventory) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut object_depth = 0usize;
    let mut object_has_ole = false;
    let mut table_depth = 0usize;
    let mut xml_depth = 0usize;
    let mut old_revision_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_report_alternate_branch(
                    &mut alternate_content_stack,
                    xml_depth,
                    name,
                ) {
                    crate::docx::skip_xml_subtree(&mut reader);
                    continue;
                }
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(ReportAlternateContentState {
                            branch_depth: xml_depth + 1,
                            anchor_seen: false,
                            shape_marker_seen: false,
                            took_branch: false,
                        });
                    }
                    b"tbl" => {
                        if table_depth > 0 && old_revision_depth == 0 {
                            features.nested_tables += 1;
                        }
                        table_depth = table_depth.saturating_add(1);
                    }
                    b"commentReference" if old_revision_depth == 0 => features.comments += 1,
                    b"commentReference" => {}
                    b"ins" => features.tracked_insertions += 1,
                    b"del" => {
                        features.tracked_deletions += 1;
                        old_revision_depth = old_revision_depth.saturating_add(1);
                    }
                    b"moveFrom" => {
                        features.tracked_moves += 1;
                        old_revision_depth = old_revision_depth.saturating_add(1);
                    }
                    b"moveTo" => features.tracked_moves += 1,
                    name if is_revision_property_change(name) => {
                        features.tracked_property_changes += 1;
                    }
                    b"fldSimple" if old_revision_depth == 0 => features.fields += 1,
                    b"fldSimple" => {}
                    b"fldChar" if old_revision_depth == 0 && is_complex_field_begin(&e) => {
                        features.fields += 1;
                    }
                    b"fldChar" => {}
                    b"hyperlink" if old_revision_depth == 0 => features.hyperlinks += 1,
                    b"hyperlink" => {}
                    b"sdt" if old_revision_depth == 0 => features.content_controls += 1,
                    b"sdt" => {}
                    b"anchor" if old_revision_depth == 0 => {
                        features.floating_shapes += 1;
                        mark_report_alternate_anchor_seen(&mut alternate_content_stack);
                    }
                    b"anchor" => {}
                    name if is_alternate_content_shape_marker(name) && old_revision_depth == 0 => {
                        mark_report_alternate_shape_marker_seen(&mut alternate_content_stack);
                    }
                    b"chart" if old_revision_depth == 0 => features.charts += 1,
                    b"chart" => {}
                    b"object" if old_revision_depth == 0 => {
                        object_depth = object_depth.saturating_add(1);
                        object_has_ole = false;
                    }
                    b"object" => {}
                    b"oleObject" if old_revision_depth == 0 => {
                        if object_depth > 0 {
                            if !object_has_ole {
                                features.ole_objects += 1;
                                object_has_ole = true;
                            }
                        } else {
                            features.ole_objects += 1;
                        }
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_add(1);
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_report_alternate_branch(
                    &mut alternate_content_stack,
                    xml_depth,
                    name,
                ) {
                    continue;
                }
                match name {
                    b"AlternateContent" if old_revision_depth == 0 => features.floating_shapes += 1,
                    b"AlternateContent" => {}
                    b"tbl" if table_depth > 0 && old_revision_depth == 0 => {
                        features.nested_tables += 1;
                    }
                    b"tbl" => {}
                    b"commentReference" if old_revision_depth == 0 => features.comments += 1,
                    b"commentReference" => {}
                    b"ins" => features.tracked_insertions += 1,
                    b"del" => features.tracked_deletions += 1,
                    b"moveFrom" | b"moveTo" => features.tracked_moves += 1,
                    name if is_revision_property_change(name) => {
                        features.tracked_property_changes += 1;
                    }
                    b"fldSimple" if old_revision_depth == 0 => features.fields += 1,
                    b"fldSimple" => {}
                    b"fldChar" if old_revision_depth == 0 && is_complex_field_begin(&e) => {
                        features.fields += 1;
                    }
                    b"fldChar" => {}
                    b"hyperlink" if old_revision_depth == 0 => features.hyperlinks += 1,
                    b"hyperlink" => {}
                    b"sdt" if old_revision_depth == 0 => features.content_controls += 1,
                    b"sdt" => {}
                    b"anchor" if old_revision_depth == 0 => {
                        features.floating_shapes += 1;
                        mark_report_alternate_anchor_seen(&mut alternate_content_stack);
                    }
                    b"anchor" => {}
                    name if is_alternate_content_shape_marker(name) && old_revision_depth == 0 => {
                        mark_report_alternate_shape_marker_seen(&mut alternate_content_stack);
                    }
                    b"chart" if old_revision_depth == 0 => features.charts += 1,
                    b"chart" => {}
                    b"object" if old_revision_depth == 0 => features.ole_objects += 1,
                    b"object" => {}
                    b"oleObject" if old_revision_depth == 0 => {
                        if object_depth > 0 {
                            if !object_has_ole {
                                features.ole_objects += 1;
                                object_has_ole = true;
                            }
                        } else {
                            features.ole_objects += 1;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"object" && object_depth > 0 => {
                if !object_has_ole {
                    features.ole_objects += 1;
                }
                object_depth -= 1;
                object_has_ole = false;
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tbl" && table_depth > 0 => {
                table_depth -= 1;
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"AlternateContent" {
                    if let Some(state) = alternate_content_stack.pop() {
                        if !state.anchor_seen && state.shape_marker_seen && old_revision_depth == 0
                        {
                            features.floating_shapes += 1;
                        }
                    }
                }
                if is_old_revision_content(name) {
                    old_revision_depth = old_revision_depth.saturating_sub(1);
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

#[cfg(feature = "docx")]
fn is_complex_field_begin(e: &quick_xml::events::BytesStart<'_>) -> bool {
    crate::docx::field_char_type(e).as_deref() == Some("begin")
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Copy)]
struct ReportAlternateContentState {
    branch_depth: usize,
    anchor_seen: bool,
    shape_marker_seen: bool,
    took_branch: bool,
}

#[cfg(feature = "docx")]
fn should_skip_report_alternate_branch(
    stack: &mut [ReportAlternateContentState],
    xml_depth: usize,
    name: &[u8],
) -> bool {
    if !matches!(name, b"Choice" | b"Fallback") {
        return false;
    }
    let Some(state) = stack.last_mut() else {
        return false;
    };
    if state.branch_depth != xml_depth {
        return false;
    }
    if state.took_branch {
        true
    } else {
        state.took_branch = true;
        false
    }
}

#[cfg(feature = "docx")]
fn mark_report_alternate_anchor_seen(stack: &mut [ReportAlternateContentState]) {
    if let Some(state) = stack.last_mut() {
        state.anchor_seen = true;
    }
}

#[cfg(feature = "docx")]
fn mark_report_alternate_shape_marker_seen(stack: &mut [ReportAlternateContentState]) {
    if let Some(state) = stack.last_mut() {
        state.shape_marker_seen = true;
    }
}

#[cfg(feature = "docx")]
fn is_revision_property_change(name: &[u8]) -> bool {
    matches!(
        name,
        b"rPrChange"
            | b"pPrChange"
            | b"tblPrChange"
            | b"tblGridChange"
            | b"trPrChange"
            | b"tcPrChange"
            | b"sectPrChange"
            | b"numPrChange"
            | b"cellIns"
            | b"cellDel"
            | b"cellMerge"
    )
}

#[cfg(feature = "docx")]
fn is_old_revision_content(name: &[u8]) -> bool {
    matches!(name, b"del" | b"moveFrom")
}

#[cfg(feature = "docx")]
fn is_alternate_content_shape_marker(name: &[u8]) -> bool {
    matches!(name, b"drawing" | b"pict" | b"shape")
}

#[cfg(feature = "docx")]
fn count_elements(xml: &str, needle: &[u8]) -> usize {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut count = 0;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == needle => {
                count += 1;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    count
}

#[cfg(feature = "docx")]
fn docx_bookmark_names(xml: &str) -> HashSet<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut names = HashSet::new();
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_report_alternate_branch(
                    &mut alternate_content_stack,
                    xml_depth,
                    name,
                ) {
                    crate::docx::skip_xml_subtree(&mut reader);
                    continue;
                }
                match name {
                    b"del" | b"moveFrom" => {
                        crate::docx::skip_xml_subtree(&mut reader);
                        continue;
                    }
                    b"AlternateContent" => {
                        alternate_content_stack.push(ReportAlternateContentState {
                            branch_depth: xml_depth + 1,
                            anchor_seen: false,
                            shape_marker_seen: false,
                            took_branch: false,
                        });
                    }
                    b"bookmarkStart" => {
                        if let Some(name) = crate::docx::attr_local_trimmed(&e, b"name") {
                            names.insert(name);
                        }
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_add(1);
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_report_alternate_branch(
                    &mut alternate_content_stack,
                    xml_depth,
                    name,
                ) {
                    continue;
                }
                if name == b"bookmarkStart" {
                    if let Some(name) = crate::docx::attr_local_trimmed(&e, b"name") {
                        names.insert(name);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if local(e.name().as_ref()) == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    names
}

#[cfg(feature = "docx")]
fn docx_page_ref_unsupported_section_format_targets(xml: &str) -> HashSet<String> {
    use quick_xml::events::{BytesStart, Event};
    use quick_xml::Reader;

    fn section_page_number_format_unsupported(e: &BytesStart<'_>) -> Option<bool> {
        let format = crate::docx::attr_local_trimmed(e, b"fmt")?;
        Some(!matches!(
            format.as_str(),
            "" | "decimal"
                | "decimalZero"
                | "numberInDash"
                | "decimalFullWidth"
                | "decimalHalfWidth"
                | "decimalFullWidth2"
                | "decimalEnclosedCircle"
                | "decimalEnclosedFullstop"
                | "decimalEnclosedParen"
                | "ganada"
                | "chosung"
                | "koreanDigital"
                | "koreanCounting"
                | "koreanLegal"
                | "koreanDigital2"
                | "lowerLetter"
                | "upperLetter"
                | "lowerRoman"
                | "upperRoman"
                | "ordinal"
                | "cardinalText"
                | "ordinalText"
        ))
    }

    fn record_bookmark(
        e: &BytesStart<'_>,
        current_section_unsupported: bool,
        current_section_bookmarks: &mut Vec<String>,
        unsupported_targets: &mut HashSet<String>,
    ) {
        if let Some(name) = crate::docx::attr_local_trimmed(e, b"name") {
            if current_section_unsupported {
                unsupported_targets.insert(name.clone());
            }
            current_section_bookmarks.push(name);
        }
    }

    fn apply_section_format(
        is_paragraph_break: bool,
        page_format_unsupported: Option<bool>,
        current_section_unsupported: &mut bool,
        current_section_bookmarks: &mut Vec<String>,
        unsupported_targets: &mut HashSet<String>,
    ) {
        if is_paragraph_break {
            if let Some(is_unsupported) = page_format_unsupported {
                *current_section_unsupported = is_unsupported;
            }
            current_section_bookmarks.clear();
        } else if let Some(is_unsupported) = page_format_unsupported {
            *current_section_unsupported = is_unsupported;
            for name in current_section_bookmarks.iter() {
                if is_unsupported {
                    unsupported_targets.insert(name.clone());
                } else {
                    unsupported_targets.remove(name);
                }
            }
        }
    }

    let mut reader = Reader::from_str(xml);
    let mut unsupported_targets = HashSet::new();
    let mut current_section_bookmarks = Vec::new();
    let mut current_section_unsupported = false;
    let mut paragraph_depth = 0usize;
    let mut paragraph_properties_depth = 0usize;
    let mut section_properties_depth = 0usize;
    let mut section_is_paragraph_break = false;
    let mut section_page_format_unsupported = None;
    let mut paragraph_section_format_pending = false;
    let mut paragraph_section_page_format_unsupported = None;
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_report_alternate_branch(
                    &mut alternate_content_stack,
                    xml_depth,
                    name,
                ) {
                    crate::docx::skip_xml_subtree(&mut reader);
                    continue;
                }
                match name {
                    b"del" | b"moveFrom" => {
                        crate::docx::skip_xml_subtree(&mut reader);
                        continue;
                    }
                    b"AlternateContent" => {
                        alternate_content_stack.push(ReportAlternateContentState {
                            branch_depth: xml_depth + 1,
                            anchor_seen: false,
                            shape_marker_seen: false,
                            took_branch: false,
                        });
                    }
                    b"p" => paragraph_depth += 1,
                    b"pPr" => paragraph_properties_depth += 1,
                    b"sectPr" => {
                        section_properties_depth += 1;
                        if section_properties_depth == 1 {
                            section_is_paragraph_break = paragraph_properties_depth > 0;
                            section_page_format_unsupported = None;
                        }
                    }
                    b"pgNumType"
                        if section_properties_depth > 0
                            && section_page_format_unsupported.is_none() =>
                    {
                        section_page_format_unsupported =
                            section_page_number_format_unsupported(&e);
                    }
                    b"bookmarkStart" => record_bookmark(
                        &e,
                        current_section_unsupported,
                        &mut current_section_bookmarks,
                        &mut unsupported_targets,
                    ),
                    _ => {}
                }
                xml_depth = xml_depth.saturating_add(1);
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_report_alternate_branch(
                    &mut alternate_content_stack,
                    xml_depth,
                    name,
                ) {
                    continue;
                }
                match name {
                    b"sectPr" => {
                        if paragraph_properties_depth > 0 {
                            paragraph_section_format_pending = true;
                            paragraph_section_page_format_unsupported = None;
                        } else {
                            apply_section_format(
                                false,
                                None,
                                &mut current_section_unsupported,
                                &mut current_section_bookmarks,
                                &mut unsupported_targets,
                            );
                        }
                    }
                    b"pgNumType"
                        if section_properties_depth > 0
                            && section_page_format_unsupported.is_none() =>
                    {
                        section_page_format_unsupported =
                            section_page_number_format_unsupported(&e);
                    }
                    b"bookmarkStart" => record_bookmark(
                        &e,
                        current_section_unsupported,
                        &mut current_section_bookmarks,
                        &mut unsupported_targets,
                    ),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                match local(qname.as_ref()) {
                    b"sectPr" => {
                        if section_properties_depth == 1 {
                            if section_is_paragraph_break {
                                paragraph_section_format_pending = true;
                                paragraph_section_page_format_unsupported =
                                    section_page_format_unsupported;
                            } else {
                                apply_section_format(
                                    false,
                                    section_page_format_unsupported,
                                    &mut current_section_unsupported,
                                    &mut current_section_bookmarks,
                                    &mut unsupported_targets,
                                );
                            }
                            section_is_paragraph_break = false;
                            section_page_format_unsupported = None;
                        }
                        section_properties_depth = section_properties_depth.saturating_sub(1);
                    }
                    b"p" => {
                        if paragraph_depth == 1 && paragraph_section_format_pending {
                            apply_section_format(
                                true,
                                paragraph_section_page_format_unsupported.take(),
                                &mut current_section_unsupported,
                                &mut current_section_bookmarks,
                                &mut unsupported_targets,
                            );
                            paragraph_section_format_pending = false;
                        }
                        paragraph_depth = paragraph_depth.saturating_sub(1);
                    }
                    b"pPr" => {
                        paragraph_properties_depth = paragraph_properties_depth.saturating_sub(1);
                    }
                    b"AlternateContent" => {
                        alternate_content_stack.pop();
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    unsupported_targets
}

#[cfg(test)]
mod tests {
    use super::fields_for_model;
    use crate::annotation::{Field, FieldKind};
    use crate::model::{Block, FieldRole, FieldUnsupportedReason, Paragraph, Run};

    #[test]
    fn supported_page_ref_syntax_accepts_mixed_case_arabic() {
        assert!(super::supported_page_ref_syntax(r"PAGEREF Figure1 \* ArAbIc").is_some());
    }

    #[test]
    fn reference_diagnostics_reject_switch_first_names() {
        assert!(super::supported_ref_syntax(r"REF \h Figure1").is_none());
        assert!(super::supported_direct_ref_syntax(r"\h Figure1").is_none());
        assert!(super::supported_page_ref_syntax(r"PAGEREF \p Figure1").is_none());
        assert!(super::supported_note_ref_target(r"NOTEREF \p FootOne").is_none());
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_revision_number_diagnostics_reject_malformed_tails() {
        assert_eq!(
            super::revision_number_uncomputed_reason(r"REVNUM \* MERGEFORMAT"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::revision_number_uncomputed_reason(r"REVNUM \x"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_style_ref_diagnostics_reject_malformed_tails() {
        assert_eq!(
            super::style_ref_uncomputed_reason(r"STYLEREF Heading1 \n \t"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::style_ref_uncomputed_reason(r"STYLEREF Heading1 \t"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn supported_doc_structure_accepts_unquoted_multi_token_style_ref_name() {
        assert_eq!(
            super::style_ref_uncomputed_reason(r"STYLEREF Heading 1 \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_page_section_diagnostics_reject_malformed_tails() {
        assert_eq!(
            super::page_uncomputed_reason(r"PAGE \* ROMAN \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::page_uncomputed_reason(r"PAGE \x"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::section_document_structure_uncomputed_reason(r"SECTIONPAGES \* CardText"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::section_document_structure_uncomputed_reason(r"SECTION \* ROMAN \* Arabic"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_document_info_diagnostics_reject_malformed_syntax() {
        assert_eq!(
            super::document_info_uncomputed_reason(r#"DOCPROPERTY "Client Name""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"CREATEDATE \@"yyyy-MM-dd""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"CREATEDATE \@ MMMM d, yyyy \* Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"USERNAME "Casey Reviewer" \*Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"USERNAME Casey Reviewer \*Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"DOCPROPERTY "Client Name" \*Caps"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"DOCPROPERTY Client Name \*Caps"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"DOCVARIABLE ClientCode \*Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"DOCVARIABLE Client Code \*Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"DOCPROPERTY "Client Name"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::document_info_uncomputed_reason(r#"CREATEDATE \@ "yyyy-MM-dd"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );

        let valid = Field {
            kind: FieldKind::DocumentInfo("DOCPROPERTY".to_string()),
            instruction: r#"DOCPROPERTY "Client Name""#.to_string(),
            ..Field::default()
        };
        assert_eq!(
            super::unsupported_field_reason(&valid),
            Some(super::FieldEvaluationReason::NoComputedResult)
        );
        let malformed = Field {
            instruction: r#"DOCPROPERTY "Client Name"#.to_string(),
            ..valid
        };
        assert_eq!(
            super::unsupported_field_reason(&malformed),
            Some(super::FieldEvaluationReason::UnsupportedSwitch)
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_hyperlink_diagnostics_reject_malformed_syntax() {
        let valid = Field {
            kind: FieldKind::Hyperlink,
            instruction: r#"HYPERLINK "https://example.com" \o "tip""#.to_string(),
            ..Field::default()
        };
        assert_eq!(super::unsupported_field_reason(&valid), None);
        let unquoted_tip = Field {
            instruction: r#"HYPERLINK "https://example.com" \o Client portal tooltip \t NewWindow"#
                .to_string(),
            ..valid.clone()
        };
        assert_eq!(super::unsupported_field_reason(&unquoted_tip), None);
        let malformed = Field {
            instruction: r#"HYPERLINK "https://example.com" extra"#.to_string(),
            ..valid
        };
        assert_eq!(
            super::unsupported_field_reason(&malformed),
            Some(super::FieldEvaluationReason::UnsupportedSwitch)
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_merge_field_diagnostics_reject_malformed_names() {
        let valid = Field {
            kind: FieldKind::MergeField,
            instruction: r#"MERGEFIELD "Client Name" \* MERGEFORMAT"#.to_string(),
            ..Field::default()
        };
        assert_eq!(super::unsupported_field_reason(&valid), None);
        let valid_prefix_suffix = Field {
            instruction: r#"MERGEFIELD "Client Name" \b "Before " \f suffix text"#.to_string(),
            ..valid.clone()
        };
        assert_eq!(super::unsupported_field_reason(&valid_prefix_suffix), None);
        let malformed = Field {
            instruction: r#"MERGEFIELD \* MERGEFORMAT"#.to_string(),
            ..valid.clone()
        };
        assert_eq!(
            super::unsupported_field_reason(&malformed),
            Some(super::FieldEvaluationReason::UnsupportedSwitch)
        );
        let malformed_tail = Field {
            instruction: r#"MERGEFIELD "Client Name" \b"#.to_string(),
            ..valid
        };
        assert_eq!(
            super::unsupported_field_reason(&malformed_tail),
            Some(super::FieldEvaluationReason::UnsupportedSwitch)
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_toc_entry_diagnostics_reject_malformed_tails() {
        assert_eq!(
            super::toc_entry_uncomputed_reason(r#"TC "Entry" \f A \l 2"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::toc_entry_uncomputed_reason(r#"TC "Entry" \l "2"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_toc_entry_diagnostics_accept_unquoted_multi_token_marker_text() {
        assert_eq!(
            super::toc_entry_uncomputed_reason(r#"TC Manual Appendix Entry \f m \l 2"#),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_reference_index_marker_diagnostics_reject_malformed_syntax() {
        assert_eq!(
            super::reference_index_marker_uncomputed_reason(r#"RD "chapter2.docx" \* Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::reference_index_marker_uncomputed_reason(r#"TA \l "Case" \c "1""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::reference_index_marker_uncomputed_reason(
                r#"XE "Mercury" \t "See planets" \* FirstCap"#
            ),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::reference_index_marker_uncomputed_reason(r#"TA \l "Case" \c"1"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_reference_index_marker_diagnostics_accept_multi_token_switch_operands() {
        assert_eq!(
            super::reference_index_marker_uncomputed_reason(r#"TA \l Case v. Example \c 1"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::reference_index_marker_uncomputed_reason(
                r#"XE "Mercury" \t See planets \* FirstCap"#
            ),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_numbering_diagnostics_accept_text_format_switches() {
        assert_eq!(
            super::numbering_uncomputed_reason(r"AUTONUM \* CardText \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"AUTONUMLGL \* OrdText"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"AUTONUMOUT \* roman \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"LISTNUM NumberDefault \* CardText \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"LISTNUM LegalDefault \* OrdText"),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_numbering_diagnostics_reject_malformed_bidi_outline() {
        assert_eq!(
            super::numbering_uncomputed_reason("BIDIOUTLINE"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"BIDIOUTLINE \* MERGEFORMAT"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"BIDIOUTLINE \* roman \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::numbering_uncomputed_reason(r"BIDIOUTLINE \x"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_action_diagnostics_reject_malformed_format_tails() {
        assert_eq!(
            super::action_uncomputed_reason(r#"PRINT \p ReportBox "0 0 moveto""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::action_uncomputed_reason(r#"PRINT \p ReportBox "\p literal code""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::action_uncomputed_reason(r#"MACROBUTTON RunReport \* MERGEFORMAT"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::action_uncomputed_reason(r#"GOTOBUTTON TargetBookmark Jump Now \*Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::action_uncomputed_reason(r#"MACROBUTTON RunReport \*MERGEFORMAT"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::action_uncomputed_reason(r#"MACROBUTTON RunReport Run \* Upper Again"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_action_diagnostics_accept_unquoted_multi_token_print_group_code() {
        assert_eq!(
            super::action_uncomputed_reason(r#"PRINT \p ReportBox 0 0 moveto \* MERGEFORMAT"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::action_uncomputed_reason(r#"PRINT \pReportBox compact moveto"#),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_advance() {
        assert_eq!(
            super::display_uncomputed_reason(r"ADVANCE \r 2 \d4 \* MERGEFORMAT"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"ADVANCE \z 2"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_reject_malformed_symbol() {
        assert_eq!(
            super::display_uncomputed_reason(r"SYMBOL 65 \f Wingdings"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"SYMBOL 0x03BB \u \f Times New Roman \* Upper"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r#"SYMBOL 65 \f "Wingdings"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_displacement() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \d \fo10 \li(Title)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \d \fo10 \li(Title"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \d \fo10(\q)"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \d \ba2(\f(1,2))"),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_script() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \s\ai4(Above)\di3(Below)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \s\ai4(Above"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \s\up8(\q)"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \s\up8(\f(1,2))"),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_fraction() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \f(1,2)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \f(1)"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \f(1,\f(2,3))"),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_radical() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \r(3,27)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \r()"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_list() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \l(A,B)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \l()"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_overstrike() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \o \ac(A,/)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \o()"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_box() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \x \to(A)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \x()"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \x \to(\f(5,8))"),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_bracket() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \b(Chapter)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \b()"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_bracket_options() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \b \bc\{ (Range)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \b \bc (Range)"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_array() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \a \al \co2 \vs3 \hs3(A,B,C,D)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \a \co0(A,B)"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_display_diagnostics_accept_valid_eq_integral() {
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \i \su \fcS(0,1,x)"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::display_uncomputed_reason(r"EQ \i(0,1)"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_compare_diagnostics_reject_malformed_operands() {
        assert_eq!(
            super::compare_uncomputed_reason(r#"COMPARE "98512" = "985*""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::compare_uncomputed_reason(r#"COMPARE "" = """#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::compare_uncomputed_reason(r#"COMPARE 1e309 > 0"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::compare_uncomputed_reason(r#"COMPARE \o = "Gold""#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_formula_diagnostics_reject_malformed_switches() {
        assert_eq!(
            super::formula_uncomputed_reason(r#"= CustomerTotal \# "0.00""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::formula_uncomputed_reason(r#"= 10.25 \* DollarText"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::formula_uncomputed_reason(r#"= 5 \# 0 units \* MERGEFORMAT"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::formula_uncomputed_reason(r#"= 1 \# "0.00"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_formula_diagnostics_reject_malformed_bodies() {
        assert_eq!(
            super::formula_uncomputed_reason(r#"= CustomerTotal \# "0.00""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::formula_uncomputed_reason("= CustomerTotal + TaxTotal"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::formula_uncomputed_reason("= 1 +"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::formula_uncomputed_reason("= (1 + 2"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::formula_uncomputed_reason("= 1e+"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_if_diagnostics_reject_malformed_operands() {
        assert_eq!(
            super::if_uncomputed_reason(r#"IF 1 = 1 "yes" "no""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::if_uncomputed_reason(r#"IF "" = "" "yes" "no""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::if_uncomputed_reason(r#"IF 1e309 = 1 "yes" "no""#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::if_uncomputed_reason(r#"IF \o = "Gold" "ship" "hold""#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_merge_control_diagnostics_reject_malformed_comparisons() {
        assert_eq!(
            super::merge_control_uncomputed_reason(r"NEXT \* MERGEFORMAT"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::merge_control_uncomputed_reason(r#"NEXTIF City = "Tokyo""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::merge_control_uncomputed_reason(r#"NEXTIF "" = """#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::merge_control_uncomputed_reason(r#"NEXTIF 1e309 = 1"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::merge_control_uncomputed_reason(r#"NEXTIF \o = "Tokyo""#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_quote_diagnostics_reject_malformed_tails() {
        assert_eq!(
            super::quote_uncomputed_reason(r#"QUOTE "Ready" \* Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::quote_uncomputed_reason(r#"QUOTE "Ready" \z"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_set_diagnostics_reject_malformed_names() {
        assert_eq!(
            super::set_uncomputed_reason(r#"SET ClientName "Acme" \* Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::set_uncomputed_reason(r#"SET ClientName Client 42"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::set_uncomputed_reason(r#"SET \r "Acme""#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::set_uncomputed_reason(r#"SET ClientName "Acme"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_sequence_diagnostics_reject_malformed_tails() {
        assert_eq!(
            super::sequence_uncomputed_reason(r"SEQ Figure \r -1"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::sequence_uncomputed_reason(r"SEQ Figure \s 1"),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::sequence_uncomputed_reason(r"SEQ Figure \x"),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_prompt_diagnostics_reject_malformed_syntax() {
        assert_eq!(
            super::prompt_uncomputed_reason(r#"FILLIN "Client?" \d Acme"#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::prompt_uncomputed_reason(r#"ASK ClientCode "Client code?" \d "ac-42""#),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::prompt_uncomputed_reason(r#"FILLIN \z \d Acme"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
        assert_eq!(
            super::prompt_uncomputed_reason(r#"ASK ClientCode \o \d ac-42"#),
            super::FieldEvaluationReason::UnsupportedSwitch
        );
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn no_default_prompt_diagnostics_accept_unquoted_multi_token_prompt_text() {
        assert_eq!(
            super::prompt_uncomputed_reason(
                r#"FILLIN Client display prompt \d Acme Corp \* Upper"#
            ),
            super::FieldEvaluationReason::NoComputedResult
        );
        assert_eq!(
            super::prompt_uncomputed_reason(r#"ASK ClientName Client name prompt \d Acme Corp"#),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[test]
    fn opaque_field_diagnostics_reject_malformed_quoted_syntax() {
        for (kind, valid, valid_format, malformed, malformed_format, bad_format) in [
            (
                FieldKind::InsertedContent("INCLUDETEXT".to_string()),
                r#"INCLUDETEXT "chapter.docx""#,
                r#"INCLUDETEXT "chapter.docx" \* Upper"#,
                r#"INCLUDETEXT "chapter.docx"#,
                r#"INCLUDETEXT "chapter.docx" \*"#,
                r#"INCLUDETEXT "chapter.docx" \* BadFormat"#,
            ),
            (
                FieldKind::MailMerge("ADDRESSBLOCK".to_string()),
                r#"ADDRESSBLOCK \f "Dear""#,
                r#"ADDRESSBLOCK \* MERGEFORMAT"#,
                r#"ADDRESSBLOCK \f "Dear"#,
                r#"ADDRESSBLOCK \* \x"#,
                r#"ADDRESSBLOCK \* BadFormat"#,
            ),
            (
                FieldKind::ReferenceIndex("INDEX".to_string()),
                r#"INDEX \c "2""#,
                r#"INDEX \c "2" \* Lower"#,
                r#"INDEX \c "2"#,
                r#"INDEX \c "2" \*"#,
                r#"INDEX \c "2" \* BadFormat"#,
            ),
            (
                FieldKind::Compatibility("PRIVATE".to_string()),
                r#"PRIVATE "payload""#,
                r#"PRIVATE payload \* CHARFORMAT"#,
                r#"PRIVATE "payload"#,
                r#"PRIVATE payload \* "Upper""#,
                r#"PRIVATE payload \* BadFormat"#,
            ),
        ] {
            let valid_field = Field {
                kind: kind.clone(),
                instruction: valid.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&valid_field),
                Some(super::FieldEvaluationReason::NoComputedResult)
            );
            let valid_format_field = Field {
                kind: kind.clone(),
                instruction: valid_format.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&valid_format_field),
                Some(super::FieldEvaluationReason::NoComputedResult)
            );
            let malformed_field = Field {
                kind: kind.clone(),
                instruction: malformed.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&malformed_field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch)
            );
            let malformed_format_field = Field {
                kind: kind.clone(),
                instruction: malformed_format.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&malformed_format_field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch)
            );
            let bad_format_field = Field {
                kind,
                instruction: bad_format.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&bad_format_field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch)
            );
        }
    }

    #[test]
    fn barcode_and_form_field_diagnostics_reject_malformed_syntax() {
        for (kind, valid, valid_format, malformed, malformed_format, bad_format) in [
            (
                FieldKind::Barcode("DISPLAYBARCODE".to_string()),
                r#"DISPLAYBARCODE "12345" QR \q 3"#,
                r#"DISPLAYBARCODE "12345" QR \* Upper"#,
                r#"DISPLAYBARCODE "12345 QR"#,
                r#"DISPLAYBARCODE "12345" QR \*"#,
                r#"DISPLAYBARCODE "12345" QR \* BadFormat"#,
            ),
            (
                FieldKind::FormField("FORMTEXT".to_string()),
                "FORMTEXT",
                r#"FORMTEXT \* MERGEFORMAT"#,
                r#"FORMTEXT \x"#,
                r#"FORMTEXT \*"#,
                r#"FORMTEXT \* BadFormat"#,
            ),
        ] {
            let valid_field = Field {
                kind: kind.clone(),
                instruction: valid.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&valid_field),
                Some(super::FieldEvaluationReason::NoComputedResult)
            );
            let valid_format_field = Field {
                kind: kind.clone(),
                instruction: valid_format.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&valid_format_field),
                Some(super::FieldEvaluationReason::NoComputedResult)
            );
            let malformed_field = Field {
                kind: kind.clone(),
                instruction: malformed.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&malformed_field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch)
            );
            let malformed_format_field = Field {
                kind: kind.clone(),
                instruction: malformed_format.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&malformed_format_field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch)
            );
            let bad_format_field = Field {
                kind,
                instruction: bad_format.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&bad_format_field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch)
            );
        }
        assert_eq!(
            super::form_field_uncomputed_reason(r#"FORMTEXT \*Upper"#),
            super::FieldEvaluationReason::NoComputedResult
        );

        let missing_quality_operand = Field {
            kind: FieldKind::Barcode("DISPLAYBARCODE".to_string()),
            instruction: r#"DISPLAYBARCODE "12345" QR \q"#.to_string(),
            ..Field::default()
        };
        assert_eq!(
            super::unsupported_field_reason(&missing_quality_operand),
            Some(super::FieldEvaluationReason::UnsupportedSwitch)
        );

        for instruction in [
            r#"DISPLAYBARCODE "12345" QR \h"#,
            r#"DISPLAYBARCODE "12345" QR \z"#,
            r#"DISPLAYBARCODE "12345" BADTYPE"#,
        ] {
            let field = Field {
                kind: FieldKind::Barcode("DISPLAYBARCODE".to_string()),
                instruction: instruction.to_string(),
                ..Field::default()
            };
            assert_eq!(
                super::unsupported_field_reason(&field),
                Some(super::FieldEvaluationReason::UnsupportedSwitch),
                "{instruction}"
            );
        }

        let valid_switches = Field {
            kind: FieldKind::Barcode("MERGEBARCODE".to_string()),
            instruction:
                r#"MERGEBARCODE Zip JPPOST \h 1440 \s 100 \r 1 \f 0x000000 \b FFFFFF \t \a"#
                    .to_string(),
            ..Field::default()
        };
        assert_eq!(
            super::unsupported_field_reason(&valid_switches),
            Some(super::FieldEvaluationReason::NoComputedResult)
        );
        assert_eq!(
            super::barcode_uncomputed_reason(
                r#"MERGEBARCODE Zip JPPOST \h1440 \s100 \r1 \f0x000000 \bFFFFFF \t \a"#
            ),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[test]
    fn supported_toc_bookmark_scope_rejects_duplicate_tc_filter() {
        assert!(super::supported_toc_bookmark_scope(r"TOC \f m \f x").is_none());
    }

    #[test]
    fn supported_toc_bookmark_scope_rejects_unbalanced_outline_range_quotes() {
        assert!(super::supported_toc_bookmark_scope(r#"TOC \o "1-2"#).is_none());
        assert!(super::supported_toc_bookmark_scope(r#"TOC \o "1-2" \l "2-3"#).is_none());
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn supported_toc_bookmark_scope_accepts_unquoted_multi_token_custom_style_switch() {
        assert_eq!(
            super::toc_uncomputed_reason(r#"TOC \o "1-1" \t Custom Heading,2 \* Upper"#, None),
            super::FieldEvaluationReason::NoComputedResult
        );
    }

    #[test]
    fn model_fields_coalesce_contiguous_result_runs() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "7".to_string(),
                    field: FieldRole::Simple {
                        instruction: " PAGE ".to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: " of ".to_string(),
                    field: FieldRole::Simple {
                        instruction: "PAGE".to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "tail".to_string(),
                    ..Run::default()
                },
                Run {
                    text: "site".to_string(),
                    field: FieldRole::Hyperlink {
                        url: "https://example.com".to_string(),
                    },
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        })];

        let fields = fields_for_model(&blocks);

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].kind, FieldKind::Page);
        assert_eq!(fields[0].instruction, "PAGE");
        assert_eq!(fields[0].result, "7 of ");
        assert_eq!(fields[1].kind, FieldKind::Hyperlink);
        assert_eq!(fields[1].instruction, r#"HYPERLINK "https://example.com""#);
        assert_eq!(fields[1].result, "site");

        let inventory = super::feature_inventory_for_model(&blocks);
        assert_eq!(inventory.fields, 2);
        assert_eq!(inventory.hyperlinks, 1);
        assert_eq!(
            inventory.field_kinds,
            vec![
                super::FieldKindCount {
                    kind: FieldKind::Page,
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Hyperlink,
                    count: 1,
                },
            ]
        );
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Page,
                count: 1,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![super::FieldEvaluationReasonCount {
                reason: super::FieldEvaluationReason::NoComputedResult,
                count: 1,
            }]
        );
    }

    #[test]
    fn model_inventory_preserves_explicit_unsupported_reason_for_supported_syntax() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![Run {
                text: "site".to_string(),
                field: FieldRole::Hyperlink {
                    url: "https://example.com".to_string(),
                },
                field_unsupported_reason: Some(FieldUnsupportedReason::UnsupportedSwitch),
                ..Run::default()
            }],
            ..Paragraph::default()
        })];

        let inventory = super::feature_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 1);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Hyperlink,
                count: 1,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![super::FieldEvaluationReasonCount {
                reason: super::FieldEvaluationReason::UnsupportedSwitch,
                count: 1,
            }]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn model_inventory_recognizes_owned_context_free_computed_fields() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "Ready".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"QUOTE "Ready""#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "\u{2022}".to_string(),
                    field: FieldRole::Simple {
                        instruction: r"SYMBOL 183 \f Symbol".to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "Ready".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"IF 2 > 1 "Ready" "Held""#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "1".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"COMPARE "A*" = "AB""#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "2.50".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"= 10 / 4 \# "0.00""#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "Acme".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"FILLIN "Client?" \d "Acme""#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "1/2".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"EQ \f(1,2)"#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: String::new(),
                    field: FieldRole::Simple {
                        instruction: r#"ADVANCE \r2"#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "cached quote".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"QUOTE "broken"#.to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::UnsupportedSwitch),
                    ..Run::default()
                },
                Run {
                    text: "cached symbol".to_string(),
                    field: FieldRole::Simple {
                        instruction: r"SYMBOL 66 \f Wingdings".to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::UnsupportedSwitch),
                    ..Run::default()
                },
                Run {
                    text: "cached bookmark IF".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"IF Gate = "Ready" "yes" "no""#.to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                    ..Run::default()
                },
                Run {
                    text: "cached nonfinite compare".to_string(),
                    field: FieldRole::Simple {
                        instruction: "COMPARE 1e309 > 0".to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::UnsupportedSwitch),
                    ..Run::default()
                },
                Run {
                    text: "cached bookmark formula".to_string(),
                    field: FieldRole::Simple {
                        instruction: "= Amount + 1".to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                    ..Run::default()
                },
                Run {
                    text: "cached DEFINED formula".to_string(),
                    field: FieldRole::Simple {
                        instruction: "= DEFINED(Known)".to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                    ..Run::default()
                },
                Run {
                    text: "cached default-less FILLIN".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"FILLIN "Client?""#.to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                    ..Run::default()
                },
                Run {
                    text: "cached stateful ASK".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"ASK ClientCode "Client code?" \d "ac-42""#.to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                    ..Run::default()
                },
                Run {
                    text: "cached malformed EQ".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"EQ \f(1,"#.to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::UnsupportedSwitch),
                    ..Run::default()
                },
                Run {
                    text: "cached unsupported ADVANCE".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"ADVANCE \z 2"#.to_string(),
                    },
                    field_unsupported_reason: Some(FieldUnsupportedReason::UnsupportedSwitch),
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        })];

        let inventory = super::feature_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 18);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![
                super::FieldKindCount {
                    kind: FieldKind::Dynamic("QUOTE".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Display("SYMBOL".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Dynamic("IF".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Dynamic("COMPARE".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Dynamic("=".to_string()),
                    count: 2,
                },
                super::FieldKindCount {
                    kind: FieldKind::Dynamic("FILLIN".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Dynamic("ASK".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Display("EQ".to_string()),
                    count: 1,
                },
                super::FieldKindCount {
                    kind: FieldKind::Display("ADVANCE".to_string()),
                    count: 1,
                },
            ]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![
                super::FieldEvaluationReasonCount {
                    reason: super::FieldEvaluationReason::UnsupportedSwitch,
                    count: 5,
                },
                super::FieldEvaluationReasonCount {
                    reason: super::FieldEvaluationReason::NoComputedResult,
                    count: 5,
                },
            ]
        );

        #[cfg(feature = "render")]
        {
            let render_inventory = super::render_inventory_for_model(&blocks);
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

    #[cfg(feature = "docx")]
    #[test]
    fn owned_document_info_fields_use_the_note_writer_syntax_boundary() {
        let field = |instruction: &str| Field {
            kind: FieldKind::DocumentInfo("DOCPROPERTY".to_string()),
            instruction: instruction.to_string(),
            ..Field::default()
        };

        for instruction in [
            r#"DOCPROPERTY Subject \* Upper"#,
            r#"DOCPROPERTY "Client Name" \* Caps"#,
            r#"TITLE \* Upper"#,
            r#"CREATEDATE \@ "yyyy-MM-dd""#,
        ] {
            assert!(
                super::supports_owned_computed_field_evaluation(&field(instruction)),
                "{instruction}"
            );
        }
        for instruction in [
            r#"NUMPAGES \* ROMAN"#,
            "DOCPROPERTY Pages",
            r#"DOCVARIABLE ClientCode \* Upper"#,
            "FILESIZE",
            r#"DATE \@ "yyyy-MM-dd""#,
            "USERNAME",
            "REVNUM",
            r#"DOCPROPERTY "Broken Name"#,
        ] {
            assert!(
                !super::supports_owned_computed_field_evaluation(&field(instruction)),
                "{instruction}"
            );
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn owned_revision_number_fields_use_the_note_writer_syntax_boundary() {
        let field = |instruction: &str| Field {
            kind: FieldKind::DocumentStructure("REVNUM".to_string()),
            instruction: instruction.to_string(),
            ..Field::default()
        };

        for instruction in ["REVNUM", r#"REVNUM \* Upper"#, r#"REVNUM \* Caps"#] {
            assert!(
                super::supports_owned_computed_field_evaluation(&field(instruction)),
                "{instruction}"
            );
        }
        for instruction in [r#"REVNUM \x"#, r#"REVNUM "broken"#, "SECTION"] {
            assert!(
                !super::supports_owned_computed_field_evaluation(&field(instruction)),
                "{instruction}"
            );
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_model_page_support_is_syntax_gated() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "7".to_string(),
                    field: FieldRole::Simple {
                        instruction: " PAGE ".to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "bad".to_string(),
                    field: FieldRole::Simple {
                        instruction: "PAGE \\x".to_string(),
                    },
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        })];

        let inventory = super::render_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 2);
        assert_eq!(
            inventory.field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Page,
                count: 2,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Page,
                count: 1,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![super::FieldEvaluationReasonCount {
                reason: super::FieldEvaluationReason::UnsupportedSwitch,
                count: 1,
            }]
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_model_inventory_preserves_explicit_unsupported_reason_for_supported_syntax() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![Run {
                text: "7".to_string(),
                field: FieldRole::Simple {
                    instruction: "PAGE".to_string(),
                },
                field_unsupported_reason: Some(FieldUnsupportedReason::NoComputedResult),
                ..Run::default()
            }],
            ..Paragraph::default()
        })];

        let inventory = super::render_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 1);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Page,
                count: 1,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![super::FieldEvaluationReasonCount {
                reason: super::FieldEvaluationReason::NoComputedResult,
                count: 1,
            }]
        );
    }

    #[test]
    fn model_toc_scope_reasons_use_model_bookmarks() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "Scoped target".to_string(),
                    bookmark: Some("ExistingScope".to_string()),
                    ..Run::default()
                },
                Run {
                    text: "cached existing scope toc".to_string(),
                    field: FieldRole::Simple {
                        instruction: r"TOC \b ExistingScope".to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "cached missing scope toc".to_string(),
                    field: FieldRole::Simple {
                        instruction: r"TOC \b MissingScope".to_string(),
                    },
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        })];

        let inventory = super::feature_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 2);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Toc,
                count: 2,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![
                super::FieldEvaluationReasonCount {
                    reason: super::FieldEvaluationReason::NoComputedResult,
                    count: 1,
                },
                super::FieldEvaluationReasonCount {
                    reason: super::FieldEvaluationReason::UnresolvedBookmark,
                    count: 1,
                },
            ]
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_model_fields_coalesce_contiguous_result_runs() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "A".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#" TOC \o "1-3" "#.to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "B".to_string(),
                    field: FieldRole::Simple {
                        instruction: r#"TOC \o "1-3""#.to_string(),
                    },
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        })];

        let inventory = super::render_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 1);
        assert_eq!(
            inventory.field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            }]
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_model_toc_scope_reasons_use_model_bookmarks() {
        let blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "Scoped target".to_string(),
                    bookmark: Some("ExistingScope".to_string()),
                    ..Run::default()
                },
                Run {
                    text: "cached existing scope toc".to_string(),
                    field: FieldRole::Simple {
                        instruction: r"TOC \b ExistingScope".to_string(),
                    },
                    ..Run::default()
                },
                Run {
                    text: "cached missing scope toc".to_string(),
                    field: FieldRole::Simple {
                        instruction: r"TOC \b MissingScope".to_string(),
                    },
                    ..Run::default()
                },
            ],
            ..Paragraph::default()
        })];

        let inventory = super::render_inventory_for_model(&blocks);

        assert_eq!(inventory.fields, 2);
        assert_eq!(
            inventory.unsupported_field_kinds,
            vec![super::FieldKindCount {
                kind: FieldKind::Toc,
                count: 2,
            }]
        );
        assert_eq!(
            inventory.unsupported_field_reasons,
            vec![
                super::FieldEvaluationReasonCount {
                    reason: super::FieldEvaluationReason::NoComputedResult,
                    count: 1,
                },
                super::FieldEvaluationReasonCount {
                    reason: super::FieldEvaluationReason::UnresolvedBookmark,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn unsupported_render_features_include_unsupported_field_evaluation() {
        assert!(!super::FeatureInventory::default().has_unsupported_render_features());
        assert!(super::FeatureInventory {
            unsupported_field_kinds: vec![super::FieldKindCount {
                kind: FieldKind::Toc,
                count: 1,
            }],
            ..super::FeatureInventory::default()
        }
        .has_unsupported_render_features());
    }

    #[test]
    fn flattened_legacy_doc_warning_requires_non_body_counts() {
        assert_eq!(
            super::legacy_doc_flattened_subdocuments_warning(0, 0, 0, 0, 0),
            None
        );

        assert_eq!(
            super::legacy_doc_flattened_subdocuments_warning(2, 3, 4, 5, 6),
            Some(super::DocumentWarning::LegacyDocFlattenedSubdocuments {
                footnotes: 2,
                headers_footers: 3,
                annotations: 4,
                endnotes: 5,
                text_boxes: 6,
            })
        );
    }
}
