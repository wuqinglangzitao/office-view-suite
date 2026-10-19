//! Annotation types extracted from Word documents.

use std::collections::HashMap;

use crate::fib::Fib;
use crate::model::Color;
use crate::util::{u16le, u32le};

const ATRD_PRE10_LEN: usize = 30;
const ATRD_INITIALS_LEN: usize = 20;
const FRD_LEN: usize = 2;
const SPA_LEN: usize = 26;
const MAX_ATN_OWNER_XSTS: usize = 0x7FFF;
const MAX_ATN_OWNER_CCH: usize = 55;

/// A text range in the main document body associated with an annotation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextAnchor {
    /// Anchor id as stored by WordprocessingML (`w:id`).
    pub id: String,
    /// Visible text covered by the anchor range.
    pub text: String,
}

/// A Word comment extracted from a `.docx` comments part or recovered from a
/// legacy `.doc` annotation subdocument.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comment {
    /// Comment id as stored by WordprocessingML (`w:id`).
    pub id: String,
    /// Comment author (`w:author`), if present.
    pub author: Option<String>,
    /// Author initials (`w:initials`), if present.
    pub initials: Option<String>,
    /// Comment timestamp (`w:date`), if present.
    pub date: Option<String>,
    /// Parent comment id for replies, from `w:parentId` or commentsExtended metadata.
    pub parent_comment_id: Option<String>,
    /// Visible text contained in the comment body.
    pub text: String,
    /// Main-document range this comment is anchored to, if rwml found one.
    pub anchor: Option<TextAnchor>,
    /// Resolved/"done" state from `commentsExtended.xml` (`w15:done`): `Some(true)`
    /// when marked done, `Some(false)` when explicitly not done, `None` when the
    /// document has no commentsExtended entry for the comment.
    pub resolved: Option<bool>,
}

/// Legacy `.doc` comment metadata recovered from an `ATRDPre10` record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LegacyDocCommentMetadata {
    /// Author initials from `xstUsrInitl`, when present and valid.
    pub(crate) initials: Option<String>,
    /// Owner-name index into `GrpXstAtnOwners`.
    pub(crate) owner_index: Option<usize>,
}

/// Parse the legacy `.doc` annotation owner-name XST array.
pub(crate) fn legacy_doc_comment_owners(fib: &Fib, table: &[u8]) -> Vec<String> {
    let Some(slice) = table_slice(table, fib.fc_grp_xst_atn_owners, fib.lcb_grp_xst_atn_owners)
    else {
        return Vec::new();
    };
    parse_xst_array(slice).unwrap_or_default()
}

/// Parse legacy `.doc` annotation reference metadata records.
pub(crate) fn legacy_doc_comment_metadata(
    fib: &Fib,
    table: &[u8],
) -> Vec<LegacyDocCommentMetadata> {
    let Some(slice) = table_slice(table, fib.fc_plcfand_ref, fib.lcb_plcfand_ref) else {
        return Vec::new();
    };
    if slice.is_empty() {
        return Vec::new();
    }
    if slice.len() < 4 || (slice.len() - 4) % (4 + ATRD_PRE10_LEN) != 0 {
        return Vec::new();
    }
    let count = (slice.len() - 4) / (4 + ATRD_PRE10_LEN);
    let data_offset = (count + 1) * 4;
    let Some(data_end) = data_offset.checked_add(count * ATRD_PRE10_LEN) else {
        return Vec::new();
    };
    if data_end != slice.len() {
        return Vec::new();
    }

    let mut metadata = Vec::with_capacity(count);
    for index in 0..count {
        let offset = data_offset + index * ATRD_PRE10_LEN;
        let Some(record) = slice.get(offset..offset + ATRD_PRE10_LEN) else {
            return Vec::new();
        };
        metadata.push(LegacyDocCommentMetadata {
            initials: read_lpx_char_buffer9(&record[..ATRD_INITIALS_LEN]),
            owner_index: u16le(record, ATRD_INITIALS_LEN).map(|ibst| ibst as usize),
        });
    }
    metadata
}

pub(crate) fn legacy_doc_note_reference_cps(
    table: &[u8],
    fc: usize,
    lcb: usize,
    ccp_text: u32,
) -> Vec<usize> {
    let Some(slice) = table_slice(table, fc, lcb) else {
        return Vec::new();
    };
    parse_plc_reference_cps(slice, FRD_LEN, ccp_text as usize).unwrap_or_default()
}

pub(crate) fn legacy_doc_shape_anchor_cps(
    table: &[u8],
    fc: usize,
    lcb: usize,
    ccp_text: u32,
) -> Vec<usize> {
    let Some(slice) = table_slice(table, fc, lcb) else {
        return Vec::new();
    };
    parse_plc_reference_cps(slice, SPA_LEN, ccp_text as usize).unwrap_or_default()
}

fn parse_plc_reference_cps(slice: &[u8], data_len: usize, ccp_text: usize) -> Option<Vec<usize>> {
    if slice.is_empty() {
        return Some(Vec::new());
    }
    if slice.len() < 4 || (slice.len() - 4) % (4 + data_len) != 0 {
        return None;
    }
    let count = (slice.len() - 4) / (4 + data_len);
    let cp_bytes = count.checked_add(1)?.checked_mul(4)?;
    let data_end = cp_bytes.checked_add(count.checked_mul(data_len)?)?;
    if data_end != slice.len() {
        return None;
    }

    let mut cps = Vec::with_capacity(count);
    for index in 0..count {
        let cp = u32le(slice, index * 4)? as usize;
        if cp >= ccp_text {
            return None;
        }
        cps.push(cp);
    }
    let _last_cp = u32le(slice, count * 4)?;
    Some(cps)
}

fn table_slice(table: &[u8], fc: usize, lcb: usize) -> Option<&[u8]> {
    if lcb == 0 {
        return Some(&[]);
    }
    table.get(fc..fc.checked_add(lcb)?)
}

fn parse_xst_array(slice: &[u8]) -> Option<Vec<String>> {
    let mut owners = Vec::new();
    let mut offset = 0usize;
    while offset < slice.len() {
        if owners.len() == MAX_ATN_OWNER_XSTS {
            return None;
        }
        let (owner, next) = read_xst(slice, offset)?;
        owners.push(owner);
        offset = next;
    }
    Some(owners)
}

fn read_xst(slice: &[u8], offset: usize) -> Option<(String, usize)> {
    let cch = u16le(slice, offset)? as usize;
    if cch > MAX_ATN_OWNER_CCH {
        return None;
    }
    let data_start = offset.checked_add(2)?;
    let data_end = data_start.checked_add(cch.checked_mul(2)?)?;
    let units = read_utf16_units(slice.get(data_start..data_end)?)?;
    let text = String::from_utf16(&units).ok()?;
    Some((text, data_end))
}

fn read_lpx_char_buffer9(slice: &[u8]) -> Option<String> {
    let cch = u16le(slice, 0)? as usize;
    if cch > 9 {
        return None;
    }
    let data_end = 2usize.checked_add(cch.checked_mul(2)?)?;
    let units = read_utf16_units(slice.get(2..data_end)?)?;
    let text = String::from_utf16(&units).ok()?;
    (!text.is_empty()).then_some(text)
}

fn read_utf16_units(bytes: &[u8]) -> Option<Vec<u16>> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Some(units)
}

/// Kind of note recovered from a Word document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// Footnote content.
    Footnote,
    /// Endnote content.
    Endnote,
}

/// A footnote or endnote recovered from a Word document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// Stable note id. For legacy `.doc` recovery this is a synthetic id because
    /// the current binary reader only exposes subdocument regions, not individual
    /// note reference ids.
    pub id: String,
    /// Footnote or endnote.
    pub kind: NoteKind,
    /// Visible note body text.
    pub text: String,
    /// Body range or reference marker this note is anchored to, if known.
    pub anchor: Option<TextAnchor>,
}

/// A text box recovered from a Word document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBox {
    /// Stable text-box id. For legacy `.doc` recovery this is a synthetic id
    /// because the current binary reader exposes text-box subdocument regions,
    /// not individual shape ids.
    pub id: String,
    /// Visible text-box content.
    pub text: String,
    /// Body range or shape anchor this text box is attached to, if known.
    pub anchor: Option<TextAnchor>,
}

/// Drawing extent for a recovered floating shape, in English Metric Units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeExtent {
    /// Width in EMUs.
    pub cx_emu: i64,
    /// Height in EMUs.
    pub cy_emu: i64,
}

/// Absolute point for a recovered floating shape, in English Metric Units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapePoint {
    /// X coordinate in EMUs.
    pub x_emu: i64,
    /// Y coordinate in EMUs.
    pub y_emu: i64,
}

/// Visual-effect extents for a recovered floating shape, in English Metric Units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeEffectExtent {
    /// Extra left extent from `wp:effectExtent/@l`.
    pub left_emu: i64,
    /// Extra top extent from `wp:effectExtent/@t`.
    pub top_emu: i64,
    /// Extra right extent from `wp:effectExtent/@r`.
    pub right_emu: i64,
    /// Extra bottom extent from `wp:effectExtent/@b`.
    pub bottom_emu: i64,
}

/// Positioning metadata for a recovered floating shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapePosition {
    /// WordprocessingML `relativeFrom` value, such as `page`, `margin`,
    /// `column`, or `paragraph`, when present.
    pub relative_from: Option<String>,
    /// Absolute position offset in EMUs, when the shape uses `wp:posOffset`.
    pub offset_emu: Option<i64>,
    /// Alignment keyword from `wp:align`, such as `left`, `center`, or `top`,
    /// when the shape is aligned rather than offset.
    pub align: Option<String>,
}

/// Distance from a floating shape to surrounding text, in English Metric Units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShapeDistance {
    /// Top distance from a `distT` attribute, when present.
    pub top_emu: Option<i64>,
    /// Bottom distance from a `distB` attribute, when present.
    pub bottom_emu: Option<i64>,
    /// Left distance from a `distL` attribute, when present.
    pub left_emu: Option<i64>,
    /// Right distance from a `distR` attribute, when present.
    pub right_emu: Option<i64>,
}

/// Text-wrapping policy declared by a floating shape anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeWrapping {
    /// WordprocessingML wrap kind, such as `none`, `square`, `tight`,
    /// `through`, or `topAndBottom`.
    pub kind: String,
    /// `wrapText` value, such as `bothSides`, `left`, or `right`, when the
    /// wrap kind exposes one.
    pub text: Option<String>,
    /// Text-distance margins declared on the `wp:wrap*` element.
    pub distance: ShapeDistance,
    /// Optional `wp:wrapPolygon` points in document order.
    pub polygon: Vec<ShapePoint>,
}

/// Floating shape geometry recovered from a Word document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatingShape {
    /// Stable shape id, normally the `wp:docPr/@id` value for `.docx`.
    pub id: String,
    /// Display name from `wp:docPr/@name`, if present.
    pub name: Option<String>,
    /// Alternative text/description from `wp:docPr/@descr`, if present.
    pub description: Option<String>,
    /// Visible text recovered from a text-bearing shape body, when present.
    pub text: Option<String>,
    /// DrawingML preset geometry name from `a:prstGeom/@prst`, when present.
    pub preset_geometry: Option<String>,
    /// Simple DrawingML solid fill color from `a:solidFill/a:srgbClr`, when present.
    pub fill_color: Option<Color>,
    /// Simple DrawingML outline color from `a:ln/a:solidFill/a:srgbClr`, when present.
    pub outline_color: Option<Color>,
    /// Whether `wp:anchor/@simplePos` asks consumers to use `wp:simplePos`.
    pub simple_position_enabled: Option<bool>,
    /// Absolute `wp:simplePos` point, when present.
    pub simple_position: Option<ShapePoint>,
    /// Visual-effect extents from `wp:effectExtent`, when present.
    pub effect_extent: Option<ShapeEffectExtent>,
    /// Best-effort zero-based top-level body block index containing the anchor,
    /// when recoverable from `.docx` body order.
    pub anchor_block_index: Option<usize>,
    /// Best-effort visible text of the containing top-level body block, excluding
    /// the floating shape body itself when recovered from `.docx`.
    pub anchor_text: Option<String>,
    /// Zero-width anchor offset inside `anchor_text`, counted in Unicode scalar
    /// values after the same text normalization used for `anchor_text`.
    pub anchor_char_offset: Option<usize>,
    /// Shape drawing extent in EMUs, if present.
    pub extent: Option<ShapeExtent>,
    /// Horizontal positioning metadata from `wp:positionH`, if present.
    pub horizontal_position: Option<ShapePosition>,
    /// Vertical positioning metadata from `wp:positionV`, if present.
    pub vertical_position: Option<ShapePosition>,
    /// WordprocessingML `relativeHeight` z-order value, when present.
    pub relative_height: Option<i64>,
    /// Whether the anchor asks Word to place the shape behind document text.
    pub behind_doc: Option<bool>,
    /// Whether the anchor is laid out inside table cells.
    pub layout_in_cell: Option<bool>,
    /// Whether the anchor is locked.
    pub locked: Option<bool>,
    /// Whether the anchor may overlap other floating objects.
    pub allow_overlap: Option<bool>,
    /// Text-distance margins declared by `wp:anchor/@dist*`.
    pub distance: ShapeDistance,
    /// Text-wrapping policy declared by the anchor, if present.
    pub wrapping: Option<ShapeWrapping>,
}

/// Kind of running header/footer region recovered from a Word document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderFooterKind {
    /// Running header content.
    Header,
    /// Running footer content.
    Footer,
    /// Even-page header content.
    EvenPageHeader,
    /// Odd-page header content.
    OddPageHeader,
    /// Even-page footer content.
    EvenPageFooter,
    /// Odd-page footer content.
    OddPageFooter,
    /// First-page header content.
    FirstPageHeader,
    /// First-page footer content.
    FirstPageFooter,
    /// Header/footer text recovered from a format surface that does not yet
    /// distinguish the exact variant.
    Unknown,
}

/// A running header/footer record recovered from a Word document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderFooter {
    /// Stable header/footer id. `.docx` records use `part#type` ids such as
    /// `word/header1.xml#default`; legacy `.doc` recovery uses synthetic ids.
    pub id: String,
    /// Header/footer variant when known.
    pub kind: HeaderFooterKind,
    /// Zero-based document section this record belongs to when it can be
    /// determined. Legacy `.doc` recovery derives it from the `PlcfHdd` story
    /// group (`(story_index - 6) / 6`); `None` for `.docx` parts (which are shared
    /// across sections) and for legacy base/unknown stories.
    pub section: Option<usize>,
    /// Visible header/footer text.
    pub text: String,
}

/// Known Word field instruction classes rwml distinguishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// `HYPERLINK`.
    Hyperlink,
    /// Current-page field. Trusted structural/rendered `.docx` body contexts
    /// compute a current page value; broader layout-derived cases preserve
    /// cached display text.
    Page,
    /// `TOC`.
    Toc,
    /// `FILENAME`.
    Filename,
    /// `MERGEFIELD`.
    MergeField,
    /// `REF`.
    Ref,
    /// `PAGEREF`.
    PageRef,
    /// `NOTEREF` (and legacy `FTNREF`).
    NoteRef,
    /// `TC` table-of-contents entry markers. Supported literal marker fields
    /// compute as hidden output and feed matching `TOC \f` fields; unsupported
    /// marker syntax preserves cached text with no computed result.
    TocEntry,
    /// `SEQ`. Deterministic source-order forms compute; unsupported sequence
    /// syntax preserves cached text without mutating later counters.
    Sequence,
    /// Cached display fields for document information, date/time, and document
    /// statistics such as `DATE`, `AUTHOR`, `DOCPROPERTY`, or `NUMPAGES`.
    DocumentInfo(String),
    /// Named dynamic/control fields, including deterministic literal
    /// expression fields (`=`, `IF`, `QUOTE`, `COMPARE`), explicit-default
    /// prompt fields (`FILLIN`, `ASK`), literal bookmark assignment (`SET`),
    /// and merge-control fields (`NEXT`, `NEXTIF`, `SKIPIF`) where rwml can
    /// compute a side-effect-free result.
    Dynamic(String),
    /// Fields that insert or link external/package/application content that rwml
    /// does not evaluate yet, such as `INCLUDETEXT`, `INCLUDEPICTURE`, `LINK`,
    /// `EMBED`, `DATABASE`, `DDE`, `DDEAUTO`, `IMPORT`, `INCLUDE`, `AUTOTEXT`,
    /// or `AUTOTEXTLIST`.
    InsertedContent(String),
    /// Mail-merge helper fields beyond `MERGEFIELD`, such as `ADDRESSBLOCK`,
    /// `GREETINGLINE`, `MERGEREC`, or `MERGESEQ`.
    MailMerge(String),
    /// Bibliography, citation, index, reference-document, and table-of-authorities
    /// fields. Simple literal `RD`, `TA`, and `XE` marker fields, including
    /// quoted or switch-bounded unquoted `TA`/`XE` marker operands, compute as
    /// hidden output; generated bibliography, index, and table-of-authorities
    /// fields preserve cached display text until native generation is broader.
    ReferenceIndex(String),
    /// Automatic numbering and list-number fields. Plain `AUTONUM` computes
    /// source-order values with common number-format switches and the
    /// documented separator switch, including unquoted or quoted one-character
    /// separators; standalone plain/neutral `AUTONUMLGL`, `AUTONUMOUT`, and
    /// `BIDIOUTLINE` compute on the same source-order counter; level-1 bare
    /// `LISTNUM` and quoted or unquoted `LISTNUM NumberDefault`/`LegalDefault`
    /// compute source-order values with common
    /// number-format switches, neutral field-format switches, and starts/resets;
    /// richer outline/list fields preserve cached display text until native
    /// automatic-numbering evaluation is broader.
    Numbering(String),
    /// Document-structure fields such as `REVNUM`, `SECTION`, `SECTIONPAGES`,
    /// or `STYLEREF`. Deterministic structural/style-reference subsets compute;
    /// broader layout-derived cases preserve cached display text.
    DocumentStructure(String),
    /// Display and layout fields such as `ADVANCE`, `EQ`, or `SYMBOL`.
    /// Deterministic hidden-output or plain-text subsets compute; remaining
    /// visual/layout cases preserve cached display text.
    Display(String),
    /// Action and automation fields such as `GOTOBUTTON`, `MACROBUTTON`, or
    /// `PRINT`. Display-text or hidden-output subsets compute without executing
    /// navigation, macros, printer, or PostScript side effects.
    Action(String),
    /// Opaque compatibility/private fields whose cached text is preserved but
    /// whose payload is not interpreted, such as `PRIVATE`, `ADDIN`, `DATA`,
    /// `GLOSSARY`, or `HTMLACTIVEX`.
    Compatibility(String),
    /// Barcode-generation fields whose cached visible text is preserved but
    /// whose barcode image generation is not evaluated, such as
    /// `BARCODE`, `DISPLAYBARCODE`, or `MERGEBARCODE`.
    Barcode(String),
    /// Legacy Word form fields such as `FORMTEXT`, `FORMCHECKBOX`, or
    /// `FORMDROPDOWN`. Deterministic `w:ffData` current/default values compute;
    /// broader protected-form behavior preserves cached display text.
    FormField(String),
    /// Any field instruction whose first token is not one of rwml's named
    /// classes yet.
    Unknown(String),
}

impl FieldKind {
    /// Classify a normalized or raw Word field instruction by its first token.
    pub fn from_instruction(instruction: &str) -> Self {
        let token = instruction
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches('"')
            .to_ascii_uppercase();
        match token.as_str() {
            "HYPERLINK" => FieldKind::Hyperlink,
            "PAGE" => FieldKind::Page,
            "TOC" => FieldKind::Toc,
            "FILENAME" => FieldKind::Filename,
            "MERGEFIELD" => FieldKind::MergeField,
            "REF" => FieldKind::Ref,
            "PAGEREF" => FieldKind::PageRef,
            "NOTEREF" | "FTNREF" => FieldKind::NoteRef,
            "TC" => FieldKind::TocEntry,
            "SEQ" => FieldKind::Sequence,
            _ if is_document_info_field(&token) => FieldKind::DocumentInfo(token),
            _ if is_dynamic_field(&token) => FieldKind::Dynamic(token),
            _ if is_inserted_content_field(&token) => FieldKind::InsertedContent(token),
            _ if is_mail_merge_field(&token) => FieldKind::MailMerge(token),
            _ if is_reference_index_field(&token) => FieldKind::ReferenceIndex(token),
            _ if is_numbering_field(&token) => FieldKind::Numbering(token),
            _ if is_document_structure_field(&token) => FieldKind::DocumentStructure(token),
            _ if is_display_field(&token) => FieldKind::Display(token),
            _ if is_action_field(&token) => FieldKind::Action(token),
            _ if is_compatibility_field(&token) => FieldKind::Compatibility(token),
            _ if is_barcode_field(&token) => FieldKind::Barcode(token),
            _ if is_legacy_form_field(&token) => FieldKind::FormField(token),
            _ => FieldKind::Unknown(token),
        }
    }

    /// Canonical field instruction name.
    pub fn as_str(&self) -> &str {
        match self {
            FieldKind::Hyperlink => "HYPERLINK",
            FieldKind::Page => "PAGE",
            FieldKind::Toc => "TOC",
            FieldKind::Filename => "FILENAME",
            FieldKind::MergeField => "MERGEFIELD",
            FieldKind::Ref => "REF",
            FieldKind::PageRef => "PAGEREF",
            FieldKind::NoteRef => "NOTEREF",
            FieldKind::TocEntry => "TC",
            FieldKind::Sequence => "SEQ",
            FieldKind::DocumentInfo(kind) => kind,
            FieldKind::Dynamic(kind) => kind,
            FieldKind::InsertedContent(kind) => kind,
            FieldKind::MailMerge(kind) => kind,
            FieldKind::ReferenceIndex(kind) => kind,
            FieldKind::Numbering(kind) => kind,
            FieldKind::DocumentStructure(kind) => kind,
            FieldKind::Display(kind) => kind,
            FieldKind::Action(kind) => kind,
            FieldKind::Compatibility(kind) => kind,
            FieldKind::Barcode(kind) => kind,
            FieldKind::FormField(kind) => kind,
            FieldKind::Unknown(kind) => kind,
        }
    }
}

pub(crate) fn is_neutral_field_format_switch(part: &str) -> bool {
    part.eq_ignore_ascii_case("MERGEFORMAT")
        || part.eq_ignore_ascii_case("MERGEFORMATINET")
        || part.eq_ignore_ascii_case("CHARFORMAT")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldTextFormat {
    Upper,
    Lower,
    Caps,
    FirstCap,
}

pub(crate) fn field_text_format_switch(part: &str) -> Option<FieldTextFormat> {
    if part.eq_ignore_ascii_case("Upper") {
        Some(FieldTextFormat::Upper)
    } else if part.eq_ignore_ascii_case("Lower") {
        Some(FieldTextFormat::Lower)
    } else if part.eq_ignore_ascii_case("Caps") {
        Some(FieldTextFormat::Caps)
    } else if part.eq_ignore_ascii_case("FirstCap") {
        Some(FieldTextFormat::FirstCap)
    } else {
        None
    }
}

pub(crate) fn accept_field_text_format_switch(
    part: &str,
    text_format: &mut Option<FieldTextFormat>,
) -> bool {
    if is_neutral_field_format_switch(part) {
        return true;
    }
    field_text_format_switch(part).is_some_and(|format| text_format.replace(format).is_none())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldNumberFormat {
    Arabic,
    ArabicDash,
    AlphabeticLower,
    AlphabeticUpper,
    RomanLower,
    RomanUpper,
    Ordinal,
    CardText,
    OrdText,
    Hex,
    DollarText,
}

pub(crate) fn field_number_format_switch(part: &str) -> Option<FieldNumberFormat> {
    match part {
        _ if part.eq_ignore_ascii_case("Arabic") => Some(FieldNumberFormat::Arabic),
        "alphabetic" => Some(FieldNumberFormat::AlphabeticLower),
        "ALPHABETIC" => Some(FieldNumberFormat::AlphabeticUpper),
        "roman" => Some(FieldNumberFormat::RomanLower),
        "ROMAN" => Some(FieldNumberFormat::RomanUpper),
        _ if part.eq_ignore_ascii_case("Ordinal") => Some(FieldNumberFormat::Ordinal),
        _ if part.eq_ignore_ascii_case("CardText") => Some(FieldNumberFormat::CardText),
        _ if part.eq_ignore_ascii_case("OrdText") => Some(FieldNumberFormat::OrdText),
        _ if part.eq_ignore_ascii_case("ArabicDash") => Some(FieldNumberFormat::ArabicDash),
        _ if part.eq_ignore_ascii_case("Hex") => Some(FieldNumberFormat::Hex),
        _ if part.eq_ignore_ascii_case("DollarText") => Some(FieldNumberFormat::DollarText),
        _ => None,
    }
}

pub(crate) fn accept_field_number_format_switch(
    part: &str,
    number_format: &mut Option<FieldNumberFormat>,
) -> bool {
    if is_neutral_field_format_switch(part) {
        return true;
    }
    field_number_format_switch(part).is_some_and(|format| number_format.replace(format).is_none())
}

pub(crate) fn accept_general_format_switch<'a, I>(
    part: &'a str,
    parts: &mut I,
    accept: impl FnOnce(&str) -> bool,
) -> Option<bool>
where
    I: Iterator<Item = &'a str>,
{
    if part == "\\*" {
        return accept(parts.next()?).then_some(true);
    }
    if let Some(format) = part.strip_prefix("\\*") {
        return accept(format).then_some(true);
    }
    Some(false)
}

pub(crate) fn is_toc_value_neutral_switch(part: &str) -> bool {
    part.eq_ignore_ascii_case("\\h")
        || part.eq_ignore_ascii_case("\\z")
        || part.eq_ignore_ascii_case("\\w")
        || part.eq_ignore_ascii_case("\\x")
}

pub(crate) fn is_note_ref_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("NOTEREF") || kind.eq_ignore_ascii_case("FTNREF")
}

pub(crate) fn strip_ascii_switch_prefix<'a>(part: &'a str, switch: &str) -> Option<&'a str> {
    let prefix = part.get(..switch.len())?;
    prefix
        .eq_ignore_ascii_case(switch)
        .then_some(&part[switch.len()..])
}

pub(crate) fn field_name_token(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = match (value.starts_with('"'), value.ends_with('"')) {
        (true, true) if value.len() >= 2 => &value[1..value.len() - 1],
        (true, _) | (_, true) => return None,
        (false, false) => value,
    }
    .trim();
    if value.is_empty() || value.starts_with('\\') || value.contains('"') {
        return None;
    }
    Some(value)
}

pub(crate) fn field_identifier_token(value: &str) -> Option<&str> {
    let value = field_name_token(value)?;
    (!value.chars().any(char::is_whitespace)).then_some(value)
}

pub(crate) fn field_name_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    if first.starts_with('\\') {
        return None;
    }
    let first_name = field_name_token(first)?;
    if field_quoted_literal_token(first).is_some() {
        return Some(first_name.to_string());
    }

    let mut name = first_name.to_string();
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        let part = parts.next()?;
        if part.contains('"') {
            return None;
        }
        let part = field_name_token(part)?;
        name.push(' ');
        name.push_str(part);
    }
    Some(name)
}

pub(crate) fn field_literal_token(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = match (value.starts_with('"'), value.ends_with('"')) {
        (true, true) if value.len() >= 2 => &value[1..value.len() - 1],
        (true, _) | (_, true) => return None,
        (false, false) => value,
    };
    (!value.contains('"')).then_some(value)
}

pub(crate) fn field_non_empty_literal_token(value: &str) -> Option<&str> {
    let value = field_literal_token(value)?;
    (!value.is_empty()).then_some(value)
}

pub(crate) fn field_quoted_literal_token(value: &str) -> Option<&str> {
    let value = value.trim();
    value.starts_with('"').then(|| field_literal_token(value))?
}

pub(crate) fn field_non_empty_quoted_literal_token(value: &str) -> Option<&str> {
    let value = field_quoted_literal_token(value)?;
    (!value.is_empty()).then_some(value)
}

pub(crate) fn field_non_switch_literal_token(value: &str) -> Option<&str> {
    let value = field_literal_token(value)?;
    (!value.starts_with('\\')).then_some(value)
}

pub(crate) fn field_non_empty_non_switch_literal_token(value: &str) -> Option<&str> {
    let value = field_non_switch_literal_token(value)?;
    (!value.is_empty()).then_some(value)
}

pub(crate) fn hyperlink_field_target(instruction: &str) -> Option<String> {
    let text = instruction.trim_start();
    let field_name_len = "HYPERLINK".len();
    let field_name = text.get(..field_name_len)?;
    if !field_name.eq_ignore_ascii_case("HYPERLINK") {
        return None;
    }
    let after_name = text.get(field_name_len..)?;
    if matches!(after_name.chars().next(), Some(ch) if !ch.is_whitespace()) {
        return None;
    }
    let after_name = after_name.trim_start();
    if after_name.starts_with('\\')
        && !after_name
            .get(..2)
            .is_some_and(|switch| switch.eq_ignore_ascii_case("\\l"))
    {
        return None;
    }
    let target_start = after_name.find('"')?;
    let rest = &after_name[target_start + 1..];
    let target_end = rest.find('"')?;
    let tail = &rest[target_end + 1..];
    if !hyperlink_tail_syntax(tail) {
        return None;
    }
    let target = rest[..target_end].trim();
    (!target.is_empty()).then(|| target.to_string())
}

fn hyperlink_tail_syntax(tail: &str) -> bool {
    let tokens = instruction_parts(tail);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    while let Some(part) = parts.next() {
        if !part.starts_with('\\') || part.contains('"') {
            return false;
        }
        let lower = part.to_ascii_lowercase();
        if matches!(lower.as_str(), "\\m" | "\\n") {
            continue;
        }
        if part.len() > 2 && matches!(lower.get(..2), Some("\\l") | Some("\\o") | Some("\\t")) {
            if field_non_empty_non_switch_literal_token(&part[2..]).is_none() {
                return false;
            }
            continue;
        }
        if matches!(lower.as_str(), "\\l" | "\\o" | "\\t") {
            if !hyperlink_tail_value_syntax(&mut parts) {
                return false;
            }
            continue;
        }
        return false;
    }
    true
}

fn hyperlink_tail_value_syntax<'a>(
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> bool {
    let Some(value) = parts.next() else {
        return false;
    };
    if field_non_empty_non_switch_literal_token(value).is_none() {
        return false;
    }
    if field_quoted_literal_token(value).is_some() {
        return true;
    }
    while let Some(value) = parts.peek().copied() {
        if value.starts_with('\\') {
            break;
        }
        let Some(value) = parts.next() else {
            return false;
        };
        if field_quoted_literal_token(value).is_some()
            || field_non_empty_non_switch_literal_token(value).is_none()
        {
            return false;
        }
    }
    true
}

// Used only by the `docx` field-context evaluator; unused in the dependency-light
// legacy-`.doc` build.
#[cfg_attr(not(feature = "docx"), allow(dead_code))]
pub(crate) fn merge_field_name(instruction: &str) -> Option<String> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("MERGEFIELD") {
        return None;
    }
    let first = parts.next()?;
    field_name_operand(first, &mut parts)
}

pub(crate) fn merge_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("MERGEFIELD") {
        return false;
    }
    let Some(first) = parts.next() else {
        return false;
    };
    if field_name_operand(first, &mut parts).is_none() {
        return false;
    }
    merge_field_tail_syntax(&mut parts)
}

fn merge_field_tail_syntax<'a, I>(parts: &mut std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = &'a str>,
{
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_field_diagnostic_format_switch_syntax(part, parts) else {
            return false;
        };
        if accepted {
            continue;
        }
        if part.eq_ignore_ascii_case("\\b") || part.eq_ignore_ascii_case("\\f") {
            if !merge_field_text_switch_operand_syntax(parts) {
                return false;
            }
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\b")
            .or_else(|| strip_ascii_switch_prefix(part, "\\f"))
        {
            if value.is_empty() || field_non_empty_non_switch_literal_token(value).is_none() {
                return false;
            }
            continue;
        }
        return false;
    }
    true
}

fn merge_field_text_switch_operand_syntax<'a, I>(parts: &mut std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = &'a str>,
{
    let Some(first) = parts.next() else {
        return false;
    };
    if field_non_empty_quoted_literal_token(first).is_some() {
        return true;
    }
    if field_non_empty_non_switch_literal_token(first).is_none() {
        return false;
    }
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        if field_quoted_literal_token(part).is_some()
            || field_non_empty_non_switch_literal_token(part).is_none()
        {
            return false;
        }
        parts.next();
    }
    true
}

pub(crate) fn field_comparison_syntax<'a>(
    first: &str,
    parts: &mut impl Iterator<Item = &'a str>,
) -> bool {
    if let Some((left, operator, right)) = compact_field_comparison_syntax(first) {
        return field_comparison_operand_syntax(left)
            && field_comparison_operator_syntax(operator)
            && field_comparison_operand_syntax(right);
    }
    let Some(operator) = parts.next() else {
        return false;
    };
    let Some(right) = parts.next() else {
        return false;
    };
    field_comparison_operand_syntax(first)
        && field_comparison_operator_syntax(operator)
        && field_comparison_operand_syntax(right)
}

pub(crate) fn compare_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("COMPARE") {
        return false;
    }
    let Some(first) = parts.next() else {
        return false;
    };
    field_comparison_syntax(first, &mut parts) && field_text_format_tail_syntax(&mut parts)
}

pub(crate) fn if_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("IF") {
        return false;
    }
    let Some(first) = parts.next() else {
        return false;
    };
    if !field_comparison_syntax(first, &mut parts) {
        return false;
    }
    let Some(true_text) = parts.next() else {
        return false;
    };
    if if_result_text_syntax(true_text).is_none() {
        return false;
    }
    let mut text_format = None;
    if let Some(part) = parts.next() {
        let Some(accepted) = accept_field_text_format_syntax(part, &mut parts, &mut text_format)
        else {
            return false;
        };
        if !accepted && if_result_text_syntax(part).is_none() {
            return false;
        }
    }
    field_text_format_tail_with(&mut parts, text_format)
}

pub(crate) fn merge_control_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if kind.eq_ignore_ascii_case("NEXT") {
        return field_text_format_tail_syntax(&mut parts);
    }
    if kind.eq_ignore_ascii_case("NEXTIF") || kind.eq_ignore_ascii_case("SKIPIF") {
        let Some(first) = parts.next() else {
            return false;
        };
        return field_comparison_syntax(first, &mut parts)
            && field_text_format_tail_syntax(&mut parts);
    }
    false
}

pub(crate) fn opaque_field_syntax(instruction: &str, is_kind: fn(&str) -> bool) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !is_kind(kind) {
        return false;
    }
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_field_diagnostic_format_switch_syntax(part, &mut parts) else {
            return false;
        };
        if accepted {
            continue;
        }
        if !field_diagnostic_token_syntax(part) {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarcodeFieldKind {
    Legacy,
    Display,
    Merge,
}

pub(crate) fn barcode_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    let Some(kind) = barcode_field_kind(kind) else {
        return false;
    };
    let mut value_tokens = 0usize;
    let mut positional_values = true;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_field_diagnostic_format_switch_syntax(part, &mut parts) else {
            return false;
        };
        if accepted {
            continue;
        }
        if part.starts_with('\\') {
            if !barcode_field_switch_syntax(part, &mut parts, kind) {
                return false;
            }
            positional_values = false;
            continue;
        }
        let Some(value) = field_non_empty_literal_token(part) else {
            return false;
        };
        if positional_values {
            value_tokens += 1;
            if value_tokens == 2 && kind != BarcodeFieldKind::Legacy && !barcode_type_token(value) {
                return false;
            }
        }
    }
    match kind {
        BarcodeFieldKind::Legacy => value_tokens >= 1,
        BarcodeFieldKind::Display | BarcodeFieldKind::Merge => value_tokens >= 2,
    }
}

fn barcode_field_kind(kind: &str) -> Option<BarcodeFieldKind> {
    if kind.eq_ignore_ascii_case("BARCODE") {
        Some(BarcodeFieldKind::Legacy)
    } else if kind.eq_ignore_ascii_case("DISPLAYBARCODE") {
        Some(BarcodeFieldKind::Display)
    } else if kind.eq_ignore_ascii_case("MERGEBARCODE") {
        Some(BarcodeFieldKind::Merge)
    } else {
        None
    }
}

fn barcode_type_token(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "UPCA"
            | "UPCE"
            | "JAN13"
            | "JAN8"
            | "EAN13"
            | "EAN8"
            | "CASE"
            | "ITF14"
            | "NW7"
            | "CODE39"
            | "CODE128"
            | "JPPOST"
            | "QR"
    )
}

fn barcode_field_switch_syntax<'a, I>(part: &'a str, parts: &mut I, kind: BarcodeFieldKind) -> bool
where
    I: Iterator<Item = &'a str>,
{
    if part.eq_ignore_ascii_case("\\x")
        || part.eq_ignore_ascii_case("\\d")
        || part.eq_ignore_ascii_case("\\t")
    {
        return true;
    }
    if kind == BarcodeFieldKind::Merge && part.eq_ignore_ascii_case("\\a") {
        return true;
    }
    for (switch, accept) in [
        ("\\h", barcode_unsigned_integer_token as fn(&str) -> bool),
        ("\\s", barcode_scale_token),
        ("\\q", barcode_qr_error_correction_token),
        ("\\p", barcode_pos_style_token),
        ("\\c", barcode_case_style_token),
        ("\\r", barcode_rotation_token),
        ("\\f", barcode_color_token),
        ("\\b", barcode_color_token),
    ] {
        if let Some(accepted) = barcode_argument_switch_syntax(part, parts, switch, accept) {
            return accepted;
        }
    }
    false
}

fn barcode_argument_switch_syntax<'a, I>(
    part: &'a str,
    parts: &mut I,
    switch: &str,
    accept: fn(&str) -> bool,
) -> Option<bool>
where
    I: Iterator<Item = &'a str>,
{
    let rest = strip_ascii_switch_prefix(part, switch)?;
    let value = if rest.is_empty() {
        match parts.next() {
            Some(value) => value,
            None => return Some(false),
        }
    } else {
        rest
    };
    let Some(value) = field_non_empty_non_switch_literal_token(value) else {
        return Some(false);
    };
    Some(accept(value))
}

fn barcode_unsigned_integer_token(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn barcode_scale_token(value: &str) -> bool {
    barcode_decimal_u32(value).is_some_and(|value| (10..=1000).contains(&value))
}

fn barcode_qr_error_correction_token(value: &str) -> bool {
    matches!(value.to_ascii_uppercase().as_str(), "L" | "M" | "Q" | "H")
        || barcode_decimal_u32(value).is_some_and(|value| value <= 3)
}

fn barcode_pos_style_token(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "STD" | "SUP2" | "SUP5" | "CASE"
    )
}

fn barcode_case_style_token(value: &str) -> bool {
    matches!(value.to_ascii_uppercase().as_str(), "STD" | "EXT" | "ADD")
}

fn barcode_rotation_token(value: &str) -> bool {
    barcode_decimal_u32(value).is_some_and(|value| value <= 3)
}

fn barcode_color_token(value: &str) -> bool {
    barcode_integer_token(value).is_some_and(|value| value <= 0xFF_FFFF)
}

fn barcode_decimal_u32(value: &str) -> Option<u32> {
    (!value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn barcode_integer_token(value: &str) -> Option<u32> {
    let value = value.trim();
    let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    else {
        if value
            .chars()
            .any(|ch| ch.is_ascii_hexdigit() && ch.is_ascii_alphabetic())
        {
            return u32::from_str_radix(value, 16).ok();
        }
        return barcode_decimal_u32(value);
    };
    (!hex.is_empty() && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyFormFieldSyntax {
    pub(crate) kind: String,
    pub(crate) text_format: Option<FieldTextFormat>,
}

pub(crate) fn legacy_form_field_syntax(instruction: &str) -> Option<LegacyFormFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = legacy_form_field_kind(parts.next()?)?;
    let text_format = field_text_format_tail(&mut parts)?;
    Some(LegacyFormFieldSyntax { kind, text_format })
}

fn legacy_form_field_kind(kind: &str) -> Option<String> {
    let kind = kind.to_ascii_uppercase();
    matches!(kind.as_str(), "FORMCHECKBOX" | "FORMDROPDOWN" | "FORMTEXT").then_some(kind)
}

fn field_text_format_tail<'a, I>(parts: &mut I) -> Option<Option<FieldTextFormat>>
where
    I: Iterator<Item = &'a str>,
{
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if accept_field_text_format_syntax(part, parts, &mut text_format)? {
            continue;
        }
        return None;
    }
    Some(text_format)
}

fn field_text_format_tail_syntax<'a, I>(parts: &mut I) -> bool
where
    I: Iterator<Item = &'a str>,
{
    field_text_format_tail_with(parts, None)
}

fn field_text_format_tail_with<'a, I>(
    parts: &mut I,
    mut text_format: Option<FieldTextFormat>,
) -> bool
where
    I: Iterator<Item = &'a str>,
{
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_field_text_format_syntax(part, parts, &mut text_format) else {
            return false;
        };
        if !accepted {
            return false;
        }
    }
    true
}

fn accept_field_text_format_syntax<'a, I>(
    part: &'a str,
    parts: &mut I,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<bool>
where
    I: Iterator<Item = &'a str>,
{
    accept_general_format_switch(part, parts, |format| {
        accept_field_text_format_switch(format, text_format)
    })
}

fn if_result_text_syntax(token: &str) -> Option<&str> {
    (!token.starts_with('\\'))
        .then(|| field_literal_token(token))
        .flatten()
}

fn accept_field_diagnostic_format_switch_syntax<'a, I>(part: &'a str, parts: &mut I) -> Option<bool>
where
    I: Iterator<Item = &'a str>,
{
    accept_general_format_switch(part, parts, |format| {
        field_diagnostic_format_switch_syntax(format)
    })
}

fn field_diagnostic_format_switch_syntax(part: &str) -> bool {
    let mut text_format = None;
    let mut number_format = None;
    accept_field_text_format_switch(part.trim(), &mut text_format)
        || accept_field_number_format_switch(part.trim(), &mut number_format)
}

fn field_diagnostic_token_syntax(part: &str) -> bool {
    if part.starts_with('\\') {
        !part.contains('"')
    } else {
        field_literal_token(part).is_some()
    }
}

fn compact_field_comparison_syntax(token: &str) -> Option<(&str, &str, &str)> {
    for operator in [">=", "<=", "<>", "=", ">", "<"] {
        let Some(index) = find_unquoted_field_operator(token, operator) else {
            continue;
        };
        let (left, right_with_operator) = token.split_at(index);
        let right = &right_with_operator[operator.len()..];
        if left.is_empty() || right.is_empty() {
            return None;
        }
        return Some((left, operator, right));
    }
    None
}

fn find_unquoted_field_operator(token: &str, operator: &str) -> Option<usize> {
    let mut in_quotes = false;
    for (index, ch) in token.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && token[index..].starts_with(operator) {
            return Some(index);
        }
    }
    None
}

fn field_comparison_operator_syntax(token: &str) -> bool {
    matches!(token, "=" | "<>" | ">" | "<" | ">=" | "<=")
}

fn field_comparison_operand_syntax(token: &str) -> bool {
    if field_quoted_literal_token(token).is_some() {
        return true;
    }
    match token.parse::<f64>() {
        Ok(value) => value.is_finite(),
        Err(_) => field_non_empty_non_switch_literal_token(token).is_some(),
    }
}

pub(crate) fn formula_field_expression_syntax(instruction: &str) -> bool {
    formula_field_expression(instruction)
        .as_deref()
        .is_some_and(formula_expression_shape_syntax)
}

fn formula_field_expression(instruction: &str) -> Option<String> {
    let body = instruction.trim().strip_prefix('=')?;
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let tokens = instruction_parts(body);
    let Some(format_switch) = formula_field_number_format_switch(&tokens) else {
        if let Some(tail_index) = tokens.iter().position(|part| is_field_format_start(part)) {
            if tail_index == 0 {
                return None;
            }
            let mut tail = tokens[tail_index..].iter().map(String::as_str);
            return formula_field_general_format_tail(&mut tail)
                .then(|| tokens[..tail_index].join(" "));
        }
        return Some(body.to_string());
    };
    let (format_index, tail_start) = match format_switch {
        FormulaFieldNumberFormatSwitch::Separate(format_index) => {
            let tail_start =
                formula_field_number_format_picture_operand(&tokens, format_index + 1)?;
            (format_index, tail_start)
        }
        FormulaFieldNumberFormatSwitch::Compact { index, picture } => {
            if !formula_field_number_format_picture(&picture) {
                return None;
            }
            (index, index + 1)
        }
    };
    if format_index == 0 {
        return None;
    }
    let mut tail = tokens[tail_start..].iter().map(String::as_str);
    formula_field_format_tail(&mut tail).then(|| tokens[..format_index].join(" "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormulaParenKind {
    Group,
    Function,
}

#[derive(Debug, Clone, Copy)]
struct FormulaParenFrame {
    kind: FormulaParenKind,
    saw_operand: bool,
}

fn formula_expression_shape_syntax(expression: &str) -> bool {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut expecting_operand = true;
    let mut frames = Vec::<FormulaParenFrame>::new();

    while index < chars.len() {
        if chars[index].is_whitespace() {
            index += 1;
            continue;
        }
        let ch = chars[index];
        if expecting_operand {
            match ch {
                '+' | '-' => {
                    index += 1;
                }
                '(' => {
                    frames.push(FormulaParenFrame {
                        kind: FormulaParenKind::Group,
                        saw_operand: false,
                    });
                    index += 1;
                }
                ')' => {
                    if !close_formula_expression_frame(&mut frames, true, &mut expecting_operand) {
                        return false;
                    }
                    index += 1;
                }
                _ if ch.is_ascii_digit() || ch == '.' => {
                    let Some(next) = formula_expression_number_end(&chars, index) else {
                        return false;
                    };
                    index = next;
                    formula_expression_mark_operand(&mut frames, &mut expecting_operand);
                }
                _ if is_formula_shape_identifier_start(ch) => {
                    let (next, identifier_only) = formula_expression_operand_end(&chars, index);
                    index = skip_formula_expression_ws(&chars, next);
                    if identifier_only && chars.get(index) == Some(&'(') {
                        frames.push(FormulaParenFrame {
                            kind: FormulaParenKind::Function,
                            saw_operand: false,
                        });
                        index += 1;
                        expecting_operand = true;
                    } else {
                        formula_expression_mark_operand(&mut frames, &mut expecting_operand);
                    }
                }
                _ => return false,
            }
            continue;
        }

        match ch {
            '+' | '-' | '*' | '/' | '^' => {
                expecting_operand = true;
                index += 1;
            }
            '<' | '>' | '=' => {
                expecting_operand = true;
                index += formula_expression_comparison_operator_len(&chars, index);
            }
            ',' | ';' => {
                if !matches!(
                    frames.last(),
                    Some(FormulaParenFrame {
                        kind: FormulaParenKind::Function,
                        ..
                    })
                ) {
                    return false;
                }
                expecting_operand = true;
                index += 1;
            }
            ')' => {
                if !close_formula_expression_frame(&mut frames, false, &mut expecting_operand) {
                    return false;
                }
                index += 1;
            }
            _ => return false,
        }
    }

    !expecting_operand && frames.is_empty()
}

fn formula_expression_mark_operand(frames: &mut [FormulaParenFrame], expecting_operand: &mut bool) {
    if let Some(frame) = frames.last_mut() {
        frame.saw_operand = true;
    }
    *expecting_operand = false;
}

fn close_formula_expression_frame(
    frames: &mut Vec<FormulaParenFrame>,
    empty_operand: bool,
    expecting_operand: &mut bool,
) -> bool {
    let Some(frame) = frames.pop() else {
        return false;
    };
    if empty_operand && (frame.kind != FormulaParenKind::Function || frame.saw_operand) {
        return false;
    }
    formula_expression_mark_operand(frames, expecting_operand);
    true
}

fn formula_expression_number_end(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start;
    let mut saw_digit = false;
    let mut saw_dot = false;
    while let Some(ch) = chars.get(index).copied() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            index += 1;
        } else if ch == '.' && !saw_dot {
            saw_dot = true;
            index += 1;
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }
    if chars.get(index).is_some_and(|ch| matches!(ch, 'e' | 'E')) {
        index += 1;
        if chars.get(index).is_some_and(|ch| matches!(ch, '+' | '-')) {
            index += 1;
        }
        let exponent_start = index;
        while chars.get(index).is_some_and(|ch| ch.is_ascii_digit()) {
            index += 1;
        }
        if index == exponent_start {
            return None;
        }
    }
    Some(index)
}

fn formula_expression_operand_end(chars: &[char], start: usize) -> (usize, bool) {
    let mut index = start;
    let mut identifier_only = true;
    while let Some(ch) = chars.get(index).copied() {
        if ch.is_whitespace()
            || matches!(
                ch,
                '+' | '-' | '*' | '/' | '^' | '<' | '>' | '=' | '(' | ')' | ',' | ';'
            )
        {
            break;
        }
        if !is_formula_shape_identifier_continue(ch) {
            identifier_only = false;
        }
        index += 1;
    }
    (index, identifier_only)
}

fn skip_formula_expression_ws(chars: &[char], mut index: usize) -> usize {
    while chars.get(index).is_some_and(|ch| ch.is_whitespace()) {
        index += 1;
    }
    index
}

fn formula_expression_comparison_operator_len(chars: &[char], index: usize) -> usize {
    match (chars.get(index).copied(), chars.get(index + 1).copied()) {
        (Some('<'), Some('=' | '>')) | (Some('>'), Some('=')) => 2,
        _ => 1,
    }
}

fn is_formula_shape_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_formula_shape_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

enum FormulaFieldNumberFormatSwitch {
    Separate(usize),
    Compact { index: usize, picture: String },
}

fn formula_field_number_format_switch(tokens: &[String]) -> Option<FormulaFieldNumberFormatSwitch> {
    tokens.iter().enumerate().find_map(|(index, part)| {
        if part == "\\#" {
            return Some(FormulaFieldNumberFormatSwitch::Separate(index));
        }
        let picture = strip_ascii_switch_prefix(part, "\\#")?;
        (!picture.is_empty()).then(|| FormulaFieldNumberFormatSwitch::Compact {
            index,
            picture: picture.to_string(),
        })
    })
}

fn formula_field_number_format_picture(token: &str) -> bool {
    field_quoted_literal_token(token).is_some() || field_non_switch_literal_token(token).is_some()
}

fn formula_field_number_format_picture_operand(
    tokens: &[String],
    picture_index: usize,
) -> Option<usize> {
    let first = tokens.get(picture_index)?;
    if field_quoted_literal_token(first).is_some() {
        return Some(picture_index + 1);
    }
    field_non_switch_literal_token(first)?;
    let mut index = picture_index + 1;
    while let Some(part) = tokens.get(index) {
        if part.starts_with('\\') {
            break;
        }
        field_non_switch_literal_token(part)?;
        index += 1;
    }
    Some(index)
}

fn formula_field_format_tail<'a>(parts: &mut impl Iterator<Item = &'a str>) -> bool {
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_general_format_switch(part, parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        }) else {
            return false;
        };
        if !accepted {
            return false;
        }
    }
    true
}

fn formula_field_general_format_tail<'a>(parts: &mut impl Iterator<Item = &'a str>) -> bool {
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_general_format_switch(part, parts, |format| {
            accept_field_number_format_switch(format, &mut number_format)
                || accept_field_text_format_switch(format, &mut text_format)
        }) else {
            return false;
        };
        if !accepted {
            return false;
        }
    }
    true
}

fn is_field_format_start(part: &str) -> bool {
    part == "\\*" || part.starts_with("\\*")
}

pub(crate) fn sequence_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("SEQ")
        || field_identifier_token(parts.next().unwrap_or("")).is_none()
    {
        return false;
    }
    let mut action_seen = false;
    let mut heading_reset_seen = false;
    let mut hidden = false;
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_number_format_switch(format, &mut number_format)
                || accept_field_text_format_switch(format, &mut text_format)
        }) else {
            return false;
        };
        if accepted {
            continue;
        }
        if part.eq_ignore_ascii_case("\\n") || part.eq_ignore_ascii_case("\\c") {
            if action_seen {
                return false;
            }
            action_seen = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\h") {
            if hidden {
                return false;
            }
            hidden = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\s") {
            let Some(level) = parts.next() else {
                return false;
            };
            if !accept_sequence_heading_reset_syntax(level, &mut heading_reset_seen) {
                return false;
            }
            continue;
        }
        if let Some(level) = strip_ascii_switch_prefix(part, "\\s") {
            if level.is_empty()
                || !accept_sequence_heading_reset_syntax(level, &mut heading_reset_seen)
            {
                return false;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\r") {
            let Some(reset) = parts.next() else {
                return false;
            };
            if !accept_sequence_reset_syntax(reset, &mut action_seen) {
                return false;
            }
            continue;
        }
        if let Some(reset) = strip_ascii_switch_prefix(part, "\\r") {
            if reset.is_empty() || !accept_sequence_reset_syntax(reset, &mut action_seen) {
                return false;
            }
            continue;
        }
        return false;
    }
    true
}

fn accept_sequence_heading_reset_syntax(part: &str, heading_reset_seen: &mut bool) -> bool {
    if *heading_reset_seen {
        return false;
    }
    let Some(level) = field_name_token(part).and_then(|part| part.parse::<u8>().ok()) else {
        return false;
    };
    if !(1..=9).contains(&level) {
        return false;
    }
    *heading_reset_seen = true;
    true
}

fn accept_sequence_reset_syntax(part: &str, action_seen: &mut bool) -> bool {
    if *action_seen {
        return false;
    }
    let Some(value) = field_name_token(part).and_then(|part| part.parse::<i64>().ok()) else {
        return false;
    };
    if value < 0 {
        return false;
    }
    *action_seen = true;
    true
}

pub(crate) fn numbering_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if kind.eq_ignore_ascii_case("AUTONUM")
        || kind.eq_ignore_ascii_case("AUTONUMLGL")
        || kind.eq_ignore_ascii_case("AUTONUMOUT")
    {
        return autonum_field_syntax(kind, parts);
    }
    if kind.eq_ignore_ascii_case("LISTNUM") {
        return listnum_field_syntax(parts);
    }
    if kind.eq_ignore_ascii_case("BIDIOUTLINE") {
        return bidi_outline_field_syntax(parts);
    }
    false
}

fn autonum_field_syntax<'a>(kind: &str, mut parts: impl Iterator<Item = &'a str>) -> bool {
    let accepts_separator = kind.eq_ignore_ascii_case("AUTONUM");
    let mut number_format = None;
    let mut text_format = None;
    let mut separator = false;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_page_number_text_format_syntax(
            part,
            &mut parts,
            &mut number_format,
            &mut text_format,
        ) else {
            return false;
        };
        if accepted {
            continue;
        }
        if accepts_separator && part.eq_ignore_ascii_case("\\s") {
            let Some(value) = parts.next() else {
                return false;
            };
            if !accept_autonum_separator_syntax(value, &mut separator) {
                return false;
            }
            continue;
        }
        if accepts_separator {
            if let Some(value) = strip_ascii_switch_prefix(part, "\\s") {
                if !accept_autonum_separator_syntax(value, &mut separator) {
                    return false;
                }
                continue;
            }
        }
        return false;
    }
    true
}

fn listnum_field_syntax<'a>(mut parts: impl Iterator<Item = &'a str>) -> bool {
    let mut list_name_seen = false;
    let mut level_seen = false;
    let mut reset_seen = false;
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_page_number_text_format_syntax(
            part,
            &mut parts,
            &mut number_format,
            &mut text_format,
        ) else {
            return false;
        };
        if accepted {
            continue;
        }
        if part.eq_ignore_ascii_case("\\l") {
            let Some(level) = parts.next() else {
                return false;
            };
            if !accept_listnum_level_syntax(level, &mut level_seen) {
                return false;
            }
            continue;
        }
        if let Some(level) = strip_ascii_switch_prefix(part, "\\l") {
            if level.is_empty() || !accept_listnum_level_syntax(level, &mut level_seen) {
                return false;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\s") {
            let Some(start) = parts.next() else {
                return false;
            };
            if !accept_listnum_start_syntax(start, &mut reset_seen) {
                return false;
            }
            continue;
        }
        if let Some(start) = strip_ascii_switch_prefix(part, "\\s") {
            if start.is_empty() || !accept_listnum_start_syntax(start, &mut reset_seen) {
                return false;
            }
            continue;
        }
        if part.starts_with('\\') || list_name_seen || field_name_token(part).is_none() {
            return false;
        }
        list_name_seen = true;
    }
    true
}

fn bidi_outline_field_syntax<'a>(mut parts: impl Iterator<Item = &'a str>) -> bool {
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_page_number_text_format_syntax(
            part,
            &mut parts,
            &mut number_format,
            &mut text_format,
        ) else {
            return false;
        };
        if !accepted {
            return false;
        }
    }
    true
}

fn accept_page_number_text_format_syntax<'a>(
    part: &'a str,
    parts: &mut impl Iterator<Item = &'a str>,
    number_format: &mut Option<FieldNumberFormat>,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<bool> {
    accept_general_format_switch(part, parts, |format| {
        accept_field_number_format_switch(format, number_format)
            || accept_field_text_format_switch(format, text_format)
    })
}

fn accept_autonum_separator_syntax(part: &str, separator: &mut bool) -> bool {
    if *separator {
        return false;
    }
    let Some(value) = field_literal_token(part) else {
        return false;
    };
    let mut chars = value.chars();
    let Some(_) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }
    *separator = true;
    true
}

fn accept_listnum_level_syntax(part: &str, level_seen: &mut bool) -> bool {
    if *level_seen {
        return false;
    }
    let Some(level) = field_name_token(part).and_then(|part| part.parse::<u8>().ok()) else {
        return false;
    };
    if level == 0 {
        return false;
    }
    *level_seen = true;
    true
}

fn accept_listnum_start_syntax(part: &str, reset_seen: &mut bool) -> bool {
    if *reset_seen {
        return false;
    }
    let Some(start) = field_name_token(part).and_then(|part| part.parse::<i64>().ok()) else {
        return false;
    };
    if start < 0 {
        return false;
    }
    *reset_seen = true;
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TocEntryFieldSyntax {
    pub(crate) text: String,
    pub(crate) entry_type: Option<String>,
    pub(crate) level: u8,
    pub(crate) text_format: Option<FieldTextFormat>,
}

pub(crate) fn toc_entry_field_syntax(instruction: &str) -> Option<TocEntryFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("TC") {
        return None;
    }
    let text = toc_entry_text_operand(parts.next()?, &mut parts)?;
    if text.is_empty() {
        return None;
    }
    let mut entry_type = None;
    let mut level = 1u8;
    let mut saw_level = false;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\f") {
            let value = parts.next_if(|next| !next.starts_with('\\'))?;
            set_toc_entry_type(&mut entry_type, value)?;
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\f") {
            if value.is_empty() || set_toc_entry_type(&mut entry_type, value).is_none() {
                return None;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\l") {
            let value = parts.next_if(|next| !next.starts_with('\\'))?;
            if saw_level {
                return None;
            }
            level = field_level_token(value)?;
            saw_level = true;
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\l") {
            if value.is_empty() || saw_level {
                return None;
            }
            level = field_level_token(value)?;
            saw_level = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\n") {
            continue;
        }
        if accept_field_text_format_syntax(part, &mut parts, &mut text_format)? {
            continue;
        }
        return None;
    }
    Some(TocEntryFieldSyntax {
        text,
        entry_type,
        level,
        text_format,
    })
}

fn toc_entry_text_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    let quoted = first.trim().starts_with('"');
    let mut values = vec![field_name_token(first)?];
    if quoted {
        return Some(values.join(" "));
    }
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        if part.trim().starts_with('"') || part.trim().ends_with('"') {
            return None;
        }
        values.push(field_name_token(parts.next()?)?);
    }
    Some(values.join(" "))
}

fn set_toc_entry_type(slot: &mut Option<String>, value: &str) -> Option<()> {
    let value = field_identifier_token(value)?.to_string();
    if slot.replace(value).is_some() {
        return None;
    }
    Some(())
}

pub(crate) fn filename_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("FILENAME") {
        return false;
    }
    let mut path = false;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\p") {
            if path {
                return false;
            }
            path = true;
            continue;
        }
        let Some(accepted) = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        }) else {
            return false;
        };
        if !accepted {
            return false;
        }
    }
    true
}

pub(crate) fn revision_number_field_text_format(
    instruction: &str,
) -> Option<Option<FieldTextFormat>> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("REVNUM") {
        return None;
    }
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let accepted = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })?;
        if !accepted {
            return None;
        }
    }
    Some(text_format)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleRefFieldSyntax {
    pub(crate) style_identifier: String,
    pub(crate) text_format: Option<FieldTextFormat>,
    pub(crate) result: StyleRefResult,
    pub(crate) suppress_non_numeric: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StyleRefResult {
    Text,
    ParagraphNumber,
    RelativeContextNumber,
    FullContextNumber,
    RelativePosition,
}

pub(crate) fn style_ref_field_syntax(instruction: &str) -> Option<StyleRefFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("STYLEREF") {
        return None;
    }
    let mut text_format = None;
    let mut result = StyleRefResult::Text;
    let mut suppress_non_numeric = false;
    let mut style_identifier = None;
    while let Some(part) = parts.next() {
        if style_identifier.is_some() {
            let accepted = accept_general_format_switch(part, &mut parts, |format| {
                accept_field_text_format_switch(format, &mut text_format)
            })?;
            if accepted {
                continue;
            }
        }
        if part.starts_with('\\') {
            if let Some(switches) = compact_style_ref_flag_switches(part) {
                for switch in switches {
                    apply_style_ref_flag_switch(switch, &mut result, &mut suppress_non_numeric)?;
                }
                continue;
            }
            return None;
        }
        if style_identifier.is_none() {
            style_identifier = Some(style_ref_identifier_operand(part, &mut parts)?);
            continue;
        }
        return None;
    }
    let style_identifier = style_identifier?;
    if suppress_non_numeric
        && !matches!(
            result,
            StyleRefResult::ParagraphNumber
                | StyleRefResult::RelativeContextNumber
                | StyleRefResult::FullContextNumber
        )
    {
        return None;
    }
    Some(StyleRefFieldSyntax {
        style_identifier,
        text_format,
        result,
        suppress_non_numeric,
    })
}

fn style_ref_identifier_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    let quoted = first.trim().starts_with('"');
    let mut values = vec![field_name_token(first)?];
    if quoted {
        return Some(values.join(" "));
    }
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        values.push(field_name_token(parts.next()?)?);
    }
    Some(values.join(" "))
}

fn compact_style_ref_flag_switches(part: &str) -> Option<Vec<char>> {
    compact_flag_switches(part, |switch| matches!(switch, 't' | 'n' | 'r' | 'w' | 'p'))
}

fn apply_style_ref_flag_switch(
    switch: char,
    result: &mut StyleRefResult,
    suppress_non_numeric: &mut bool,
) -> Option<()> {
    match switch {
        't' => {
            if *suppress_non_numeric {
                return None;
            }
            *suppress_non_numeric = true;
            Some(())
        }
        'n' | 'r' | 'w' | 'p' => {
            if *result != StyleRefResult::Text {
                return None;
            }
            *result = match switch {
                'n' => StyleRefResult::ParagraphNumber,
                'r' => StyleRefResult::RelativeContextNumber,
                'w' => StyleRefResult::FullContextNumber,
                'p' => StyleRefResult::RelativePosition,
                _ => return None,
            };
            Some(())
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageFieldFormatSyntax {
    pub(crate) number_format: Option<FieldNumberFormat>,
    pub(crate) text_format: Option<FieldTextFormat>,
}

pub(crate) fn page_field_format_syntax_tail<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
) -> Option<PageFieldFormatSyntax> {
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let accepted = accept_general_format_switch(part, parts, |format| {
            accept_page_field_format_switch(format, &mut number_format, &mut text_format)
        })?;
        if !accepted {
            return None;
        }
    }
    Some(PageFieldFormatSyntax {
        number_format,
        text_format,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageRefFieldSyntax {
    pub(crate) target: String,
    pub(crate) number_format: Option<FieldNumberFormat>,
    pub(crate) text_format: Option<FieldTextFormat>,
    pub(crate) relative: bool,
}

pub(crate) fn page_ref_field_syntax(instruction: &str) -> Option<PageRefFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("PAGEREF") {
        return None;
    }
    let target = field_identifier_token(parts.next()?)?.to_string();
    let mut number_format = None;
    let mut text_format = None;
    let mut relative = false;
    while let Some(part) = parts.next() {
        if accept_general_format_switch(part, &mut parts, |format| {
            accept_page_field_format_switch(format, &mut number_format, &mut text_format)
        })? {
            continue;
        }
        if part.starts_with('\\') {
            if let Some(switches) = compact_page_ref_flag_switches(part) {
                for switch in switches {
                    apply_page_ref_flag_switch(switch, &mut relative)?;
                }
                continue;
            }
            if part.eq_ignore_ascii_case("\\h") {
                continue;
            }
            if part.eq_ignore_ascii_case("\\p") {
                if relative {
                    return None;
                }
                relative = true;
                continue;
            }
            return None;
        }
        return None;
    }
    Some(PageRefFieldSyntax {
        target,
        number_format,
        text_format,
        relative,
    })
}

fn compact_page_ref_flag_switches(part: &str) -> Option<Vec<char>> {
    compact_flag_switches(part, |switch| matches!(switch, 'h' | 'p'))
}

fn apply_page_ref_flag_switch(switch: char, relative: &mut bool) -> Option<()> {
    match switch {
        'h' => Some(()),
        'p' => {
            if *relative {
                return None;
            }
            *relative = true;
            Some(())
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteRefFieldSyntax {
    pub(crate) target: String,
    pub(crate) number_format: Option<FieldNumberFormat>,
    pub(crate) text_format: Option<FieldTextFormat>,
    pub(crate) relative: bool,
}

pub(crate) fn note_ref_field_syntax(instruction: &str) -> Option<NoteRefFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !is_note_ref_kind(kind) {
        return None;
    }
    let target = field_identifier_token(parts.next()?)?.to_string();
    let mut number_format = None;
    let mut text_format = None;
    let mut relative = false;
    let mut formatted = false;
    while let Some(part) = parts.next() {
        if accept_general_format_switch(part, &mut parts, |format| {
            accept_page_field_format_switch(format, &mut number_format, &mut text_format)
        })? {
            continue;
        }
        if part.starts_with('\\') {
            if let Some(switches) = compact_note_ref_flag_switches(part) {
                for switch in switches {
                    apply_note_ref_flag_switch(switch, &mut relative, &mut formatted)?;
                }
                continue;
            }
            if part.eq_ignore_ascii_case("\\h") {
                continue;
            }
            if part.eq_ignore_ascii_case("\\f") {
                if formatted {
                    return None;
                }
                formatted = true;
                continue;
            }
            if part.eq_ignore_ascii_case("\\p") {
                if relative {
                    return None;
                }
                relative = true;
                continue;
            }
            return None;
        }
        return None;
    }
    if relative && number_format.is_some() {
        return None;
    }
    Some(NoteRefFieldSyntax {
        target,
        number_format,
        text_format,
        relative,
    })
}

fn compact_note_ref_flag_switches(part: &str) -> Option<Vec<char>> {
    compact_flag_switches(part, |switch| matches!(switch, 'h' | 'f' | 'p'))
}

fn apply_note_ref_flag_switch(
    switch: char,
    relative: &mut bool,
    formatted: &mut bool,
) -> Option<()> {
    match switch {
        'h' => Some(()),
        'f' => {
            if *formatted {
                return None;
            }
            *formatted = true;
            Some(())
        }
        'p' => {
            if *relative {
                return None;
            }
            *relative = true;
            Some(())
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefFieldSyntax {
    pub(crate) target: String,
    pub(crate) number_format: Option<FieldNumberFormat>,
    pub(crate) number_picture: Option<String>,
    pub(crate) text_format: Option<FieldTextFormat>,
    pub(crate) note_reference: bool,
    pub(crate) sequence_separator: bool,
    pub(crate) sequence_separator_value: Option<String>,
    pub(crate) relative: bool,
    pub(crate) paragraph_number: bool,
    pub(crate) full_context_number: bool,
    pub(crate) relative_context_number: bool,
    pub(crate) suppress_non_numeric: bool,
}

pub(crate) fn ref_field_syntax(instruction: &str) -> Option<RefFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("REF") {
        return None;
    }
    ref_field_syntax_parts(parts)
}

pub(crate) fn direct_ref_field_syntax(instruction: &str) -> Option<RefFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let first = tokens.first()?;
    if first.eq_ignore_ascii_case("REF") {
        return None;
    }
    ref_field_syntax_parts(tokens.iter().map(String::as_str))
}

fn ref_field_syntax_parts<'a>(mut parts: impl Iterator<Item = &'a str>) -> Option<RefFieldSyntax> {
    let target = field_identifier_token(parts.next()?)?.to_string();
    let mut number_format = None;
    let mut number_picture = None;
    let mut text_format = None;
    let mut note_reference = false;
    let mut sequence_separator = false;
    let mut sequence_separator_value = None;
    let mut relative = false;
    let mut paragraph_number = false;
    let mut full_context_number = false;
    let mut relative_context_number = false;
    let mut suppress_non_numeric = false;
    while let Some(part) = parts.next() {
        if accept_general_format_switch(part, &mut parts, |format| {
            accept_page_field_format_switch(format, &mut number_format, &mut text_format)
        })? {
            continue;
        }
        if part.starts_with('\\') {
            if let Some(switches) = compact_ref_flag_switches(part) {
                for switch in switches {
                    apply_ref_flag_switch(
                        switch,
                        &mut note_reference,
                        &mut relative,
                        &mut paragraph_number,
                        &mut full_context_number,
                        &mut relative_context_number,
                        &mut suppress_non_numeric,
                    )?;
                }
                continue;
            }
            if part.eq_ignore_ascii_case("\\!") {
                // Lock-result switch: prevents field update but does not change
                // the displayed value. Accept as neutral.
                continue;
            }
            if part.eq_ignore_ascii_case("\\#") {
                if number_picture.is_some() {
                    return None;
                }
                let picture = field_literal_token(parts.next()?)?;
                if picture.is_empty() || picture.starts_with('\\') {
                    return None;
                }
                number_picture = Some(picture.to_string());
                continue;
            }
            if let Some(picture) = strip_ascii_switch_prefix(part, "\\#") {
                if number_picture.is_some() {
                    return None;
                }
                let picture = field_literal_token(picture)?;
                if picture.is_empty() || picture.starts_with('\\') {
                    return None;
                }
                number_picture = Some(picture.to_string());
                continue;
            }
            if part.eq_ignore_ascii_case("\\d") {
                if sequence_separator {
                    return None;
                }
                let separator = field_literal_token(parts.next()?)?;
                if separator.is_empty() || separator.starts_with('\\') {
                    return None;
                }
                sequence_separator = true;
                sequence_separator_value = Some(separator.to_string());
                continue;
            }
            if let Some(separator) = strip_ascii_switch_prefix(part, "\\d") {
                if sequence_separator {
                    return None;
                }
                let separator = field_literal_token(separator)?;
                if separator.is_empty() || separator.starts_with('\\') {
                    return None;
                }
                sequence_separator = true;
                sequence_separator_value = Some(separator.to_string());
                continue;
            }
            return None;
        }
        return None;
    }
    if suppress_non_numeric && !(paragraph_number || full_context_number || relative_context_number)
    {
        return None;
    }
    if note_reference
        && (relative
            || paragraph_number
            || full_context_number
            || relative_context_number
            || suppress_non_numeric
            || sequence_separator)
    {
        return None;
    }
    if number_format.is_some()
        && !note_reference
        && (relative
            || paragraph_number
            || full_context_number
            || relative_context_number
            || suppress_non_numeric
            || sequence_separator)
    {
        return None;
    }
    if number_picture.is_some()
        && (number_format.is_some()
            || note_reference
            || relative
            || paragraph_number
            || full_context_number
            || relative_context_number
            || suppress_non_numeric
            || sequence_separator)
    {
        return None;
    }
    Some(RefFieldSyntax {
        target,
        number_format,
        number_picture,
        text_format,
        note_reference,
        sequence_separator,
        sequence_separator_value,
        relative,
        paragraph_number,
        full_context_number,
        relative_context_number,
        suppress_non_numeric,
    })
}

fn compact_ref_flag_switches(part: &str) -> Option<Vec<char>> {
    compact_flag_switches(part, |switch| {
        matches!(switch, 't' | 'f' | 'n' | 'w' | 'r' | 'p' | 'h')
    })
}

fn compact_flag_switches(part: &str, mut allowed: impl FnMut(char) -> bool) -> Option<Vec<char>> {
    let mut rest = part;
    let mut switches = Vec::new();
    while !rest.is_empty() {
        let mut chars = rest.chars();
        if chars.next()? != '\\' {
            return None;
        }
        let switch = chars.next()?.to_ascii_lowercase();
        if !allowed(switch) {
            return None;
        }
        switches.push(switch);
        rest = chars.as_str();
    }
    (!switches.is_empty()).then_some(switches)
}

fn apply_ref_flag_switch(
    switch: char,
    note_reference: &mut bool,
    relative: &mut bool,
    paragraph_number: &mut bool,
    full_context_number: &mut bool,
    relative_context_number: &mut bool,
    suppress_non_numeric: &mut bool,
) -> Option<()> {
    match switch {
        'h' => Some(()),
        't' => {
            if *suppress_non_numeric {
                return None;
            }
            *suppress_non_numeric = true;
            Some(())
        }
        'f' => {
            if *note_reference {
                return None;
            }
            *note_reference = true;
            Some(())
        }
        'n' => {
            if *paragraph_number || *full_context_number || *relative_context_number {
                return None;
            }
            *paragraph_number = true;
            Some(())
        }
        'w' => {
            if *full_context_number || *paragraph_number || *relative_context_number {
                return None;
            }
            *full_context_number = true;
            Some(())
        }
        'r' => {
            if *relative_context_number || *paragraph_number || *full_context_number {
                return None;
            }
            *relative_context_number = true;
            Some(())
        }
        'p' => {
            if *relative {
                return None;
            }
            *relative = true;
            Some(())
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TocFieldSyntax {
    pub(crate) start: u8,
    pub(crate) end: u8,
    pub(crate) outline_only: bool,
    pub(crate) include_standard: bool,
    pub(crate) custom_styles: Vec<TocStyleSpec>,
    pub(crate) tc_filter: Option<TocTcFilter>,
    pub(crate) tc_level_range: Option<(u8, u8)>,
    pub(crate) sequence_filter: Option<TocSequenceFilter>,
    pub(crate) bookmark: Option<String>,
    pub(crate) text_format: Option<FieldTextFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TocStyleSpec {
    pub(crate) name: String,
    pub(crate) level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TocTcFilter {
    All,
    EntryType(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TocSequenceFilter {
    FullCaption(String),
    CaptionText(String),
}

impl TocSequenceFilter {
    #[cfg(feature = "docx")]
    pub(crate) fn identifier(&self) -> &str {
        match self {
            Self::FullCaption(identifier) | Self::CaptionText(identifier) => identifier,
        }
    }
}

pub(crate) fn toc_field_syntax(instruction: &str) -> Option<TocFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("TOC") {
        return None;
    }
    let mut saw_switch = false;
    let mut outline_range = None;
    let mut saw_outline_switch = false;
    let mut bookmark = None;
    let mut custom_styles = Vec::new();
    let mut tc_filter = None;
    let mut tc_level_range = None;
    let mut sequence_filter = None;
    let mut text_format = None;
    let mut saw_page_number_sequence_prefix = false;
    let mut saw_default_toc_neutral_switch = false;
    while let Some(part) = parts.next() {
        saw_switch = true;
        if accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })? {
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if is_toc_value_neutral_switch(part) {
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\f") {
            let filter = match parts.next_if(|next| !next.starts_with('\\')) {
                Some(value) => TocTcFilter::EntryType(field_identifier_token(value)?.to_string()),
                None => TocTcFilter::All,
            };
            if !accept_toc_tc_filter(&mut tc_filter, filter) {
                return None;
            }
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\f") {
            let filter = if value.is_empty() {
                TocTcFilter::All
            } else {
                TocTcFilter::EntryType(field_identifier_token(value)?.to_string())
            };
            if !accept_toc_tc_filter(&mut tc_filter, filter) {
                return None;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\a") {
            let value = parts.next_if(|next| !next.starts_with('\\'))?;
            if !accept_toc_sequence_filter(
                &mut sequence_filter,
                value,
                TocSequenceFilter::CaptionText,
            ) {
                return None;
            }
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\a") {
            if value.is_empty()
                || !accept_toc_sequence_filter(
                    &mut sequence_filter,
                    value,
                    TocSequenceFilter::CaptionText,
                )
            {
                return None;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\c") {
            let value = parts.next_if(|next| !next.starts_with('\\'))?;
            if !accept_toc_sequence_filter(
                &mut sequence_filter,
                value,
                TocSequenceFilter::FullCaption,
            ) {
                return None;
            }
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\c") {
            if value.is_empty()
                || !accept_toc_sequence_filter(
                    &mut sequence_filter,
                    value,
                    TocSequenceFilter::FullCaption,
                )
            {
                return None;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\l") {
            let range = parts.next_if(|next| !next.starts_with('\\'))?;
            if tc_level_range
                .replace(field_level_range_token(range)?)
                .is_some()
            {
                return None;
            }
            continue;
        }
        if let Some(range) = strip_ascii_switch_prefix(part, "\\l") {
            if range.is_empty()
                || tc_level_range
                    .replace(field_level_range_token(range)?)
                    .is_some()
            {
                return None;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\u") {
            saw_outline_switch = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\n") {
            if let Some(range) = parts.next_if(|next| !next.starts_with('\\')) {
                field_level_range_token(range)?;
            }
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if let Some(range) = strip_ascii_switch_prefix(part, "\\n") {
            if range.is_empty() {
                return None;
            }
            field_level_range_token(range)?;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\p") {
            field_literal_token(parts.next_if(|next| !next.starts_with('\\'))?)?;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if let Some(separator) = strip_ascii_switch_prefix(part, "\\p") {
            field_literal_token(separator)?;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\d") {
            field_literal_token(parts.next_if(|next| !next.starts_with('\\'))?)?;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if let Some(separator) = strip_ascii_switch_prefix(part, "\\d") {
            field_literal_token(separator)?;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\s") {
            let identifier = parts.next_if(|next| !next.starts_with('\\'))?;
            if saw_page_number_sequence_prefix || toc_sequence_identifier(identifier).is_none() {
                return None;
            }
            saw_page_number_sequence_prefix = true;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if let Some(identifier) = strip_ascii_switch_prefix(part, "\\s") {
            if saw_page_number_sequence_prefix || toc_sequence_identifier(identifier).is_none() {
                return None;
            }
            saw_page_number_sequence_prefix = true;
            saw_default_toc_neutral_switch = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\b") {
            let target = parts.next_if(|next| !next.starts_with('\\'))?;
            let target = field_identifier_token(target)?;
            if bookmark.replace(target.to_string()).is_some() {
                return None;
            }
            continue;
        }
        if let Some(target) = strip_ascii_switch_prefix(part, "\\b") {
            let target = field_identifier_token(target)?;
            if bookmark.replace(target.to_string()).is_some() {
                return None;
            }
            continue;
        }
        if part.eq_ignore_ascii_case("\\t") {
            custom_styles.extend(
                toc_style_specs(&toc_style_specs_operand(
                    parts.next_if(|next| !next.starts_with('\\'))?,
                    &mut parts,
                )?)?
                .into_iter()
                .map(|(name, level)| TocStyleSpec {
                    name: name.to_string(),
                    level,
                }),
            );
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\t") {
            if value.is_empty() {
                return None;
            }
            custom_styles.extend(toc_style_specs(value)?.into_iter().map(|(name, level)| {
                TocStyleSpec {
                    name: name.to_string(),
                    level,
                }
            }));
            continue;
        }
        let range = if part.eq_ignore_ascii_case("\\o") {
            match parts.next_if(|next| !next.starts_with('\\')) {
                Some(range) => range,
                None => {
                    if outline_range.replace((1, 9)).is_some() {
                        return None;
                    }
                    continue;
                }
            }
        } else {
            strip_ascii_switch_prefix(part, "\\o")?
        };
        if outline_range
            .replace(field_level_range_token(range)?)
            .is_some()
        {
            return None;
        }
    }
    let (start, end, outline_only, include_standard) = if saw_switch {
        match outline_range {
            Some((start, end)) => (start, end, false, true),
            None if saw_outline_switch => (1, 9, true, true),
            None if !custom_styles.is_empty() => (1, 9, false, false),
            None if tc_filter.is_some() || tc_level_range.is_some() => {
                let (start, end) = tc_level_range.unwrap_or((1, 9));
                (start, end, false, false)
            }
            None if sequence_filter.is_some() => (1, 9, false, false),
            None if bookmark.is_some() => (1, 3, false, true),
            None if saw_default_toc_neutral_switch => (1, 3, false, true),
            None => return None,
        }
    } else {
        (1, 3, false, true)
    };
    Some(TocFieldSyntax {
        start,
        end,
        outline_only,
        include_standard,
        custom_styles,
        tc_filter,
        tc_level_range,
        sequence_filter,
        bookmark,
        text_format,
    })
}

fn accept_toc_tc_filter(slot: &mut Option<TocTcFilter>, filter: TocTcFilter) -> bool {
    slot.replace(filter).is_none()
}

fn accept_toc_sequence_filter(
    slot: &mut Option<TocSequenceFilter>,
    value: &str,
    filter: fn(String) -> TocSequenceFilter,
) -> bool {
    let Some(value) = toc_sequence_identifier(value) else {
        return false;
    };
    if slot.replace(filter(value.to_string())).is_some() {
        return false;
    }
    true
}

fn toc_sequence_identifier(value: &str) -> Option<&str> {
    field_identifier_token(value)
}

fn accept_page_field_format_switch(
    part: &str,
    number_format: &mut Option<FieldNumberFormat>,
    text_format: &mut Option<FieldTextFormat>,
) -> bool {
    accept_field_number_format_switch(part, number_format)
        || accept_field_text_format_switch(part, text_format)
}

pub(crate) fn set_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("SET") {
        return false;
    }
    if field_identifier_token(parts.next().unwrap_or("")).is_none() {
        return false;
    }
    let Some(value) = parts.next() else {
        return false;
    };
    if field_quoted_literal_token(value).is_some() {
        let mut text_format = None;
        while let Some(part) = parts.next() {
            let Some(accepted) = accept_general_format_switch(part, &mut parts, |format| {
                accept_field_text_format_switch(format, &mut text_format)
            }) else {
                return false;
            };
            if !accepted {
                return false;
            }
        }
        return true;
    }
    if value.is_empty() || value.starts_with('\\') || value.contains('"') {
        return false;
    }
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        }) else {
            return false;
        };
        if accepted {
            continue;
        }
        if part.starts_with('\\') || part.contains('"') {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptFieldSyntax {
    FillIn {
        default: Option<String>,
        text_format: Option<FieldTextFormat>,
    },
    Ask {
        bookmark: String,
        default: Option<String>,
    },
}

pub(crate) fn prompt_field_syntax(instruction: &str) -> Option<PromptFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if kind.eq_ignore_ascii_case("FILLIN") {
        return fill_in_field_syntax(parts);
    }
    if kind.eq_ignore_ascii_case("ASK") {
        return ask_field_syntax(parts);
    }
    None
}

fn fill_in_field_syntax<'a>(parts: impl Iterator<Item = &'a str>) -> Option<PromptFieldSyntax> {
    let mut parts = parts.peekable();
    let mut default = None;
    let mut text_format = None;
    let mut ask_once = false;
    let mut prompt_seen = false;
    while let Some(part) = parts.next() {
        if prompt_default_switch(part, &mut parts, &mut default) {
            continue;
        }
        if part.eq_ignore_ascii_case("\\o") {
            if ask_once {
                return None;
            }
            ask_once = true;
            continue;
        }
        let accepted = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })?;
        if accepted {
            continue;
        }
        if prompt_seen {
            return None;
        }
        prompt_text_operand(part, &mut parts)?;
        prompt_seen = true;
    }
    Some(PromptFieldSyntax::FillIn {
        default,
        text_format,
    })
}

fn ask_field_syntax<'a>(parts: impl Iterator<Item = &'a str>) -> Option<PromptFieldSyntax> {
    let mut parts = parts.peekable();
    let bookmark = field_identifier_token(parts.next()?)?.to_string();
    prompt_text_operand(parts.next()?, &mut parts)?;
    let mut default = None;
    let mut text_format = None;
    let mut ask_once = false;
    while let Some(part) = parts.next() {
        if prompt_default_switch(part, &mut parts, &mut default) {
            continue;
        }
        if part.eq_ignore_ascii_case("\\o") {
            if ask_once {
                return None;
            }
            ask_once = true;
            continue;
        }
        let accepted = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })?;
        if !accepted {
            return None;
        }
    }
    Some(PromptFieldSyntax::Ask { bookmark, default })
}

fn prompt_text_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<()> {
    if field_non_empty_quoted_literal_token(first).is_some() {
        return Some(());
    }
    field_non_empty_non_switch_literal_token(first)?;
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            return Some(());
        }
        if part.trim().starts_with('"') || part.trim().ends_with('"') {
            return None;
        }
        field_non_empty_non_switch_literal_token(parts.next()?)?;
    }
    Some(())
}

fn prompt_default_switch<'a>(
    part: &str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
    default: &mut Option<String>,
) -> bool {
    let value = if part.eq_ignore_ascii_case("\\d") {
        prompt_default_literal(parts)
    } else {
        strip_ascii_switch_prefix(part, "\\d")
            .and_then(field_non_switch_literal_token)
            .map(str::to_string)
    };
    let Some(value) = value else {
        return false;
    };
    default.replace(value).is_none()
}

fn prompt_default_literal<'a>(
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    let first = parts.next()?;
    if let Some(value) = field_quoted_literal_token(first) {
        return Some(value.to_string());
    }
    let mut values = vec![field_non_switch_literal_token(first)?];
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        values.push(field_non_switch_literal_token(parts.next()?)?);
    }
    Some(values.join(" "))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuoteFieldSyntax {
    pub(crate) text: String,
    pub(crate) text_format: Option<FieldTextFormat>,
}

pub(crate) fn quote_field_syntax(instruction: &str) -> Option<QuoteFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("QUOTE") {
        return None;
    }
    let mut text_parts = Vec::new();
    let mut text_format = None;
    let mut saw_format = false;
    while let Some(part) = parts.next() {
        let accepted = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })?;
        if accepted {
            saw_format = true;
            continue;
        }
        if saw_format || part.starts_with('\\') {
            return None;
        }
        text_parts.push(part);
    }
    let text = text_parts.join(" ");
    let text = field_literal_token(&text)?;
    if text.is_empty() {
        return None;
    }
    Some(QuoteFieldSyntax {
        text: text.to_string(),
        text_format,
    })
}

pub(crate) fn advance_field_syntax(instruction: &str) -> bool {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let Some(kind) = parts.next() else {
        return false;
    };
    if !kind.eq_ignore_ascii_case("ADVANCE") {
        return false;
    }
    let mut text_format = None;
    while let Some(part) = parts.next() {
        let Some(accepted) = accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        }) else {
            return false;
        };
        if accepted {
            continue;
        }
        if accept_advance_switch(part, &mut parts).is_none() {
            return false;
        }
    }
    true
}

fn accept_advance_switch<'a>(part: &str, parts: &mut impl Iterator<Item = &'a str>) -> Option<()> {
    for switch in ["\\d", "\\u", "\\l", "\\r", "\\x", "\\y"] {
        if part.eq_ignore_ascii_case(switch) {
            field_points_token(parts.next()?)?;
            return Some(());
        }
        if let Some(value) = strip_ascii_switch_prefix(part, switch) {
            if value.is_empty() {
                return None;
            }
            field_points_token(value)?;
            return Some(());
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolFieldSyntax {
    pub(crate) code: u32,
    pub(crate) unicode: bool,
    pub(crate) font: Option<String>,
    pub(crate) text_format: Option<FieldTextFormat>,
}

pub(crate) fn symbol_field_syntax(instruction: &str) -> Option<SymbolFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("SYMBOL") {
        return None;
    }
    let code = field_symbol_code_token(parts.next()?)?;
    let mut unicode = false;
    let mut font = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\a") || part.eq_ignore_ascii_case("\\h") {
            continue;
        }
        if part.eq_ignore_ascii_case("\\u") {
            unicode = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\j") {
            return None;
        }
        if part.eq_ignore_ascii_case("\\f") {
            font = Some(symbol_font_operand(parts.next()?, &mut parts)?);
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\f") {
            if value.is_empty() {
                return None;
            }
            font = Some(symbol_font_operand(value, &mut parts)?);
            continue;
        }
        if part.eq_ignore_ascii_case("\\s") {
            field_positive_points_token(parts.next()?)?;
            continue;
        }
        if let Some(size) = strip_ascii_switch_prefix(part, "\\s") {
            if size.is_empty() {
                return None;
            }
            field_positive_points_token(size)?;
            continue;
        }
        if accept_general_format_switch(part, &mut parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })? {
            continue;
        }
        return None;
    }
    Some(SymbolFieldSyntax {
        code,
        unicode,
        font,
        text_format,
    })
}

fn symbol_font_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    if let Some(font) = field_quoted_literal_token(first) {
        return (!font.is_empty()).then(|| font.to_string());
    }
    let mut values = vec![field_name_token(first)?];
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        values.push(field_name_token(parts.next()?)?);
    }
    Some(values.join(" "))
}

pub(crate) fn eq_prefix_switch_tail<'a>(value: &'a str, switch: &str) -> Option<&'a str> {
    let rest = strip_ascii_switch_prefix(value, switch)?;
    if matches!(
        rest.chars().next(),
        Some(ch) if ch.is_ascii_alphabetic()
    ) {
        return None;
    }
    Some(rest)
}

pub(crate) fn eq_numeric_prefix_option<'a>(value: &'a str, option: &str) -> Option<(f32, &'a str)> {
    let rest = strip_ascii_switch_prefix(value, option)?;
    if matches!(
        rest.chars().next(),
        Some(ch) if ch.is_ascii_alphabetic()
    ) {
        return None;
    }
    let rest = rest.trim_start();
    let mut end = 0usize;
    for (index, ch) in rest.char_indices() {
        if index == 0 && (ch == '-' || ch == '+') {
            end = ch.len_utf8();
            continue;
        }
        if !ch.is_ascii_digit() && ch != '.' && ch != 'e' && ch != 'E' && ch != '-' && ch != '+' {
            break;
        }
        end = index + ch.len_utf8();
    }
    if end == 0 || matches!(rest.get(..end), Some("+") | Some("-")) {
        return None;
    }
    let value = field_points_token(&rest[..end])?;
    Some((value, &rest[end..]))
}

pub(crate) fn eq_parenthesized_operand(value: &str) -> Option<(&str, &str)> {
    let value = value.trim_start();
    if !value.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes && depth == 0 => {
                return Some((&value[1..index], &value[index + 1..]));
            }
            ')' if !in_quotes => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

pub(crate) fn eq_enclosed_operand(value: &str) -> Option<&str> {
    let (inner, rest) = eq_parenthesized_operand(value)?;
    rest.trim().is_empty().then_some(inner)
}

pub(crate) fn eq_fraction_operands(inner: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut separator = None;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes => depth = depth.checked_sub(1)?,
            ',' | ';' if !in_quotes && depth == 0 && separator.replace(index).is_some() => {
                return None;
            }
            _ => {}
        }
    }
    if in_quotes || escaped || depth != 0 {
        return None;
    }
    let index = separator?;
    Some((&inner[..index], &inner[index + 1..]))
}

pub(crate) fn eq_radical_operands(inner: &str) -> Option<(&str, Option<&str>)> {
    let mut depth = 0usize;
    let mut separator = None;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes => depth = depth.checked_sub(1)?,
            ',' | ';' if !in_quotes && depth == 0 && separator.replace(index).is_some() => {
                return None;
            }
            _ => {}
        }
    }
    if in_quotes || escaped || depth != 0 {
        return None;
    }
    match separator {
        Some(index) => Some((&inner[..index], Some(&inner[index + 1..]))),
        None => Some((inner, None)),
    }
}

pub(crate) fn eq_list_operands(inner: &str) -> Option<Vec<&str>> {
    let mut depth = 0usize;
    let mut operands = Vec::new();
    let mut start = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes => depth = depth.checked_sub(1)?,
            ',' | ';' if !in_quotes && depth == 0 => {
                let operand = &inner[start..index];
                if operand.trim().is_empty() {
                    return None;
                }
                operands.push(operand);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if in_quotes || escaped || depth != 0 {
        return None;
    }
    let operand = &inner[start..];
    if operand.trim().is_empty() {
        return None;
    }
    operands.push(operand);
    Some(operands)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionFieldSyntax {
    pub(crate) computed_text: Option<String>,
    pub(crate) text_format: Option<FieldTextFormat>,
}

pub(crate) fn action_field_syntax(instruction: &str) -> Option<ActionFieldSyntax> {
    print_action_field_syntax(instruction).or_else(|| button_action_field_syntax(instruction))
}

fn print_action_field_syntax(instruction: &str) -> Option<ActionFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("PRINT") {
        return None;
    }
    let first = parts.next()?;
    if first.eq_ignore_ascii_case("\\p") {
        field_identifier_token(parts.next()?)?;
        print_group_code_operand(parts.next()?, &mut parts)?;
        action_field_format_tail(&mut parts)?;
    } else if let Some(group) = strip_ascii_switch_prefix(first, "\\p") {
        field_identifier_token(group)?;
        print_group_code_operand(parts.next()?, &mut parts)?;
        action_field_format_tail(&mut parts)?;
    } else {
        field_non_empty_non_switch_literal_token(first)?;
        let mut text_format = None;
        let mut saw_format = false;
        while let Some(part) = parts.next() {
            if action_field_format_switch(part, &mut parts, &mut text_format)? {
                saw_format = true;
                continue;
            }
            if saw_format {
                return None;
            }
            field_non_empty_non_switch_literal_token(part)?;
        }
    }
    Some(ActionFieldSyntax {
        computed_text: Some(String::new()),
        text_format: None,
    })
}

fn print_group_code_operand<'a, I>(first: &'a str, parts: &mut std::iter::Peekable<I>) -> Option<()>
where
    I: Iterator<Item = &'a str>,
{
    if field_non_empty_quoted_literal_token(first).is_some() {
        return Some(());
    }
    field_non_empty_non_switch_literal_token(first)?;
    while let Some(part) = parts.peek().copied() {
        if is_field_format_start(part) {
            break;
        }
        field_non_empty_non_switch_literal_token(part)?;
        parts.next();
    }
    Some(())
}

fn button_action_field_syntax(instruction: &str) -> Option<ActionFieldSyntax> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("GOTOBUTTON") && !kind.eq_ignore_ascii_case("MACROBUTTON") {
        return None;
    }
    field_identifier_token(parts.next()?)?;
    let mut display_parts = Vec::new();
    let mut text_format = None;
    let mut saw_format = false;
    while let Some(part) = parts.next() {
        if action_field_format_switch(part, &mut parts, &mut text_format)? {
            saw_format = true;
            continue;
        }
        if saw_format || part.starts_with('\\') {
            return None;
        }
        display_parts.push(part);
    }
    if display_parts.is_empty() {
        return Some(ActionFieldSyntax {
            computed_text: None,
            text_format: None,
        });
    }
    let display_text = display_parts.join(" ");
    let display_text = field_literal_token(&display_text)?;
    if display_text.is_empty() {
        return None;
    }
    Some(ActionFieldSyntax {
        computed_text: Some(display_text.to_string()),
        text_format,
    })
}

fn action_field_format_tail<'a, I>(parts: &mut I) -> Option<()>
where
    I: Iterator<Item = &'a str>,
{
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if action_field_format_switch(part, parts, &mut text_format)? {
            continue;
        }
        return None;
    }
    Some(())
}

fn action_field_format_switch<'a, I>(
    part: &'a str,
    parts: &mut I,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<bool>
where
    I: Iterator<Item = &'a str>,
{
    accept_general_format_switch(part, parts, |format| {
        accept_field_text_format_switch(format, text_format)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        action_field_syntax, advance_field_syntax, barcode_field_syntax, compare_field_syntax,
        direct_ref_field_syntax, eq_enclosed_operand, eq_fraction_operands, eq_list_operands,
        eq_numeric_prefix_option, eq_parenthesized_operand, eq_prefix_switch_tail,
        eq_radical_operands, hyperlink_field_target, if_field_syntax, legacy_form_field_syntax,
        merge_control_field_syntax, merge_field_name, merge_field_syntax,
        normalized_field_instruction, note_ref_field_syntax, opaque_field_syntax,
        page_ref_field_syntax, ref_field_syntax, style_ref_field_syntax, symbol_field_syntax,
        toc_field_syntax, FieldNumberFormat, FieldTextFormat, StyleRefResult, TocSequenceFilter,
        TocTcFilter,
    };

    #[test]
    fn action_field_syntax_accepts_computed_and_target_only_forms() {
        assert!(action_field_syntax(r#"GOTOBUTTON TargetBookmark "Jump Now""#).is_some());
        assert!(action_field_syntax(r#"PRINT \p ReportBox "0 0 moveto""#).is_some());
        assert!(action_field_syntax(r#"MACROBUTTON RunReport \* MERGEFORMAT"#).is_some());
        assert!(action_field_syntax(r#"MACROBUTTON RunReport Run \* Upper Again"#).is_none());
    }

    #[test]
    fn dynamic_control_wrapper_syntax_accepts_shared_comparison_forms() {
        assert!(compare_field_syntax(
            r#"COMPARE CustomerTier="Gold" \* MERGEFORMAT"#
        ));
        assert!(if_field_syntax(r#"IF 1 = 1 "ship" "hold" \* Upper"#));
        assert!(merge_control_field_syntax(
            r#"NEXTIF City = "Tokyo" \* CHARFORMAT"#
        ));
        assert!(merge_control_field_syntax(r#"NEXT \* MERGEFORMAT"#));
        assert!(!compare_field_syntax(r#"COMPARE \o = "Gold""#));
        assert!(!if_field_syntax(r#"IF 1 = 1 "ship" \* Upper Again"#));
        assert!(!merge_control_field_syntax(r#"NEXTIF 1e309 = 1"#));
    }

    #[test]
    fn opaque_field_syntax_accepts_literal_payloads_and_format_tails() {
        fn inserted_kind(kind: &str) -> bool {
            kind.eq_ignore_ascii_case("INCLUDETEXT")
        }

        assert!(opaque_field_syntax(
            r#"INCLUDETEXT "chapter.docx" \* Upper"#,
            inserted_kind
        ));
        assert!(opaque_field_syntax(
            r#"INCLUDETEXT "chapter.docx" \* MERGEFORMAT"#,
            inserted_kind
        ));
        assert!(!opaque_field_syntax(
            r#"INCLUDETEXT "chapter.docx" \* BadFormat"#,
            inserted_kind
        ));
        assert!(!opaque_field_syntax(
            r#"INCLUDETEXT "chapter.docx"#,
            inserted_kind
        ));
        assert!(!opaque_field_syntax(
            r#"LINK "chapter.docx""#,
            inserted_kind
        ));
    }

    #[test]
    fn legacy_form_field_syntax_accepts_text_format_tails() {
        let form = legacy_form_field_syntax(r#"FORMDROPDOWN \* Upper"#).unwrap();
        assert_eq!(form.kind, "FORMDROPDOWN");
        assert_eq!(form.text_format, Some(FieldTextFormat::Upper));
        let compact = legacy_form_field_syntax(r#"FORMTEXT \*Upper"#).unwrap();
        assert_eq!(compact.kind, "FORMTEXT");
        assert_eq!(compact.text_format, Some(FieldTextFormat::Upper));
        assert!(legacy_form_field_syntax(r#"FORMTEXT \* MERGEFORMAT"#).is_some());
        assert!(legacy_form_field_syntax(r#"FORMCHECKBOX"#).is_some());
        assert!(legacy_form_field_syntax(r#"FORMTEXT \x"#).is_none());
        assert!(legacy_form_field_syntax(r#"FORMTEXT \* BadFormat"#).is_none());
        assert!(legacy_form_field_syntax("FORMFIELD").is_none());
    }

    #[test]
    fn style_ref_field_syntax_accepts_compact_number_switches() {
        let compact = style_ref_field_syntax(r#"STYLEREF NumberedTarget \n\t"#)
            .expect("valid compact numbered style ref syntax");
        assert_eq!(compact.style_identifier, "NumberedTarget");
        assert_eq!(compact.result, StyleRefResult::ParagraphNumber);
        assert!(compact.suppress_non_numeric);

        let compact_context = style_ref_field_syntax(r#"STYLEREF NumberedTarget \t\w"#)
            .expect("valid compact full-context style ref syntax");
        assert_eq!(compact_context.result, StyleRefResult::FullContextNumber);
        assert!(compact_context.suppress_non_numeric);

        let switch_first = style_ref_field_syntax(r#"STYLEREF \p "Heading 1""#)
            .expect("valid switch-first relative style ref syntax");
        assert_eq!(switch_first.style_identifier, "Heading 1");
        assert_eq!(switch_first.result, StyleRefResult::RelativePosition);
        assert!(!switch_first.suppress_non_numeric);

        assert!(style_ref_field_syntax(r#"STYLEREF NumberedTarget \n\r"#).is_none());
        assert!(style_ref_field_syntax(r#"STYLEREF NumberedTarget \t\t"#).is_none());
        assert!(style_ref_field_syntax(r#"STYLEREF NumberedTarget \p\t"#).is_none());
        assert!(style_ref_field_syntax(r#"STYLEREF NumberedTarget \x"#).is_none());
    }

    #[test]
    fn page_ref_field_syntax_accepts_target_relative_and_format_tail() {
        let page_ref = page_ref_field_syntax(r#"PAGEREF Figure1 \h \p \* Arabic \* Upper"#)
            .expect("valid page ref syntax");
        assert_eq!(page_ref.target, "Figure1");
        assert_eq!(page_ref.number_format, Some(FieldNumberFormat::Arabic));
        assert_eq!(page_ref.text_format, Some(FieldTextFormat::Upper));
        assert!(page_ref.relative);
        let compact = page_ref_field_syntax(r#"PAGEREF Figure1 \h\p"#)
            .expect("valid compact page ref syntax");
        assert_eq!(compact.target, "Figure1");
        assert!(compact.relative);
        assert!(page_ref_field_syntax(r#"PAGEREF Figure1 \* ArAbIc"#).is_some());
        assert!(page_ref_field_syntax(r#"PAGEREF Figure1 \p \p"#).is_none());
        assert!(page_ref_field_syntax(r#"PAGEREF Figure1 \p\p"#).is_none());
        assert!(page_ref_field_syntax(r#"PAGEREF \p Figure1"#).is_none());
        assert!(page_ref_field_syntax(r#"PAGEREF "Figure List""#).is_none());
        assert!(page_ref_field_syntax(r#"PAGEREF Figure1 \x"#).is_none());
    }

    #[test]
    fn note_ref_field_syntax_accepts_target_relative_and_format_tail() {
        let note_ref = note_ref_field_syntax(r#"NOTEREF FootOne \f \* OrdText \* Upper"#)
            .expect("valid note ref syntax");
        assert_eq!(note_ref.target, "FootOne");
        assert_eq!(note_ref.number_format, Some(FieldNumberFormat::OrdText));
        assert_eq!(note_ref.text_format, Some(FieldTextFormat::Upper));
        assert!(!note_ref.relative);

        let relative = note_ref_field_syntax(r#"NOTEREF LaterNote \p \* Upper"#)
            .expect("valid relative note ref syntax");
        assert_eq!(relative.target, "LaterNote");
        assert_eq!(relative.text_format, Some(FieldTextFormat::Upper));
        assert!(relative.relative);

        let compact = note_ref_field_syntax(r#"NOTEREF FootOne \h\f"#)
            .expect("valid compact note ref syntax");
        assert_eq!(compact.target, "FootOne");
        assert!(!compact.relative);
        let compact_relative = note_ref_field_syntax(r#"NOTEREF LaterNote \h\p"#)
            .expect("valid compact relative note ref syntax");
        assert_eq!(compact_relative.target, "LaterNote");
        assert!(compact_relative.relative);

        assert!(note_ref_field_syntax(r#"FTNREF FootOne \h"#).is_some());
        assert!(note_ref_field_syntax(r#"NOTEREF FootOne \p \p"#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF FootOne \p\p"#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF FootOne \f \f"#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF FootOne \f\f"#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF LaterNote \p \* roman"#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF \p FootOne"#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF "Foot One""#).is_none());
        assert!(note_ref_field_syntax(r#"NOTEREF FootOne \x"#).is_none());
    }

    #[test]
    fn ref_field_syntax_accepts_explicit_and_direct_bookmark_forms() {
        let note_ref =
            ref_field_syntax(r#"REF FootOne \f \* roman \* Upper"#).expect("valid note REF syntax");
        assert_eq!(note_ref.target, "FootOne");
        assert_eq!(note_ref.number_format, Some(FieldNumberFormat::RomanLower));
        assert_eq!(note_ref.text_format, Some(FieldTextFormat::Upper));
        assert!(note_ref.note_reference);
        assert!(!note_ref.sequence_separator);

        let direct = direct_ref_field_syntax(r#"Figure1 \p \* FirstCap"#)
            .expect("valid direct bookmark REF syntax");
        assert_eq!(direct.target, "Figure1");
        assert_eq!(direct.text_format, Some(FieldTextFormat::FirstCap));
        assert!(direct.relative);

        let compact_number =
            ref_field_syntax(r#"REF Figure1 \n\p"#).expect("valid compact numbered REF syntax");
        assert!(compact_number.paragraph_number);
        assert!(compact_number.relative);

        let compact_direct = direct_ref_field_syntax(r#"Figure1 \n\t"#)
            .expect("valid compact direct numbered REF syntax");
        assert!(compact_direct.paragraph_number);
        assert!(compact_direct.suppress_non_numeric);

        let sequence =
            ref_field_syntax(r#"REF Figure1 \d-"#).expect("valid compact sequence separator");
        assert_eq!(sequence.target, "Figure1");
        assert!(sequence.sequence_separator);
        assert_eq!(sequence.sequence_separator_value.as_deref(), Some("-"));

        let quoted_sequence =
            ref_field_syntax(r#"REF Figure1 \d "-""#).expect("valid quoted sequence separator");
        assert!(quoted_sequence.sequence_separator);
        assert_eq!(
            quoted_sequence.sequence_separator_value.as_deref(),
            Some("-")
        );

        assert!(direct_ref_field_syntax("REF Figure1").is_none());
        let numeric =
            ref_field_syntax(r#"REF Figure1 \* roman"#).expect("valid numeric REF format syntax");
        assert_eq!(numeric.number_format, Some(FieldNumberFormat::RomanLower));
        assert!(ref_field_syntax(r#"REF Figure1 \p \* roman"#).is_none());
        assert!(ref_field_syntax(r#"REF FootOne \f \p"#).is_none());
        assert!(ref_field_syntax(r#"REF Figure1 \t"#).is_none());
        assert!(ref_field_syntax(r#"REF \h Figure1"#).is_none());
        assert!(direct_ref_field_syntax(r#"\h Figure1"#).is_none());
        assert!(direct_ref_field_syntax(r#"Figure1 \d"-"#).is_none());
    }

    #[test]
    fn toc_field_syntax_accepts_table_of_contents_forms() {
        let scoped =
            toc_field_syntax(r#"TOC \b ChapterList \o "1-2" \t "Custom Heading,3" \* Upper"#)
                .expect("valid scoped TOC syntax");
        assert_eq!(scoped.bookmark.as_deref(), Some("ChapterList"));
        assert_eq!((scoped.start, scoped.end), (1, 2));
        assert_eq!(scoped.custom_styles[0].name, "Custom Heading");
        assert_eq!(scoped.custom_styles[0].level, 3);
        assert_eq!(scoped.text_format, Some(FieldTextFormat::Upper));

        let tc = toc_field_syntax(r#"TOC \f A \l 2-3"#).expect("valid TC TOC syntax");
        assert_eq!(tc.tc_filter, Some(TocTcFilter::EntryType("A".to_string())));
        assert_eq!(tc.tc_level_range, Some((2, 3)));
        assert!(!tc.include_standard);

        let sequence =
            toc_field_syntax(r#"TOC \c Figure \p """#).expect("valid sequence TOC syntax");
        assert_eq!(
            sequence.sequence_filter,
            Some(TocSequenceFilter::FullCaption("Figure".to_string()))
        );

        assert!(toc_field_syntax(r#"TOC \f A \f B"#).is_none());
        assert!(toc_field_syntax(r#"TOC \o "1-2"#).is_none());
        assert!(toc_field_syntax(r#"TOC \b "Chapter List""#).is_none());
    }

    #[test]
    fn display_field_syntax_accepts_advance_and_symbol_forms() {
        assert!(advance_field_syntax(r#"ADVANCE \r 2 \d4 \* MERGEFORMAT"#));
        assert!(advance_field_syntax(r#"ADVANCE \r"2" \* Upper"#));
        assert!(!advance_field_syntax(r#"ADVANCE \z 2"#));
        assert!(!advance_field_syntax(r#"ADVANCE \r "2"#));

        let symbol = symbol_field_syntax(r#"SYMBOL 0x0063 \u \f "Symbol" \s12 \* Upper"#)
            .expect("valid symbol syntax");
        assert_eq!(symbol.code, 0x0063);
        assert!(symbol.unicode);
        assert_eq!(symbol.font.as_deref(), Some("Symbol"));
        assert_eq!(symbol.text_format, Some(FieldTextFormat::Upper));

        assert!(symbol_field_syntax(r#"SYMBOL "65""#).is_some());
        assert!(symbol_field_syntax(r#"SYMBOL 65 \f"Symbol"#).is_none());
        assert!(symbol_field_syntax(r#"SYMBOL 65 \s "12"#).is_none());
        assert!(symbol_field_syntax(r#"SYMBOL 65 \j"#).is_none());
    }

    #[test]
    fn eq_scanners_handle_nested_quotes_and_prefix_options() {
        assert_eq!(
            eq_prefix_switch_tail(r"\li(Title)", r"\li"),
            Some("(Title)")
        );
        assert_eq!(eq_prefix_switch_tail(r"\left", r"\li"), None);

        let (points, rest) = eq_numeric_prefix_option(r"\fo10.5(A)", r"\fo").unwrap();
        assert_eq!(points, 10.5);
        assert_eq!(rest, "(A)");
        assert!(eq_numeric_prefix_option(r"\foe10(A)", r"\fo").is_none());

        assert_eq!(
            eq_parenthesized_operand(r#"(A,\f(1,2),"B,C")tail"#),
            Some((r#"A,\f(1,2),"B,C""#, "tail"))
        );
        assert_eq!(
            eq_enclosed_operand(r#"(A,\f(1,2),"B,C")"#),
            Some(r#"A,\f(1,2),"B,C""#)
        );
        assert!(eq_enclosed_operand(r#"(A) tail"#).is_none());

        assert_eq!(
            eq_fraction_operands(r#"A,\f(1,2)"#),
            Some(("A", r"\f(1,2)"))
        );
        assert_eq!(eq_radical_operands(r#"3,27"#), Some(("3", Some("27"))));
        assert_eq!(eq_radical_operands("27"), Some(("27", None)));
        assert_eq!(eq_list_operands(r#"A,"B,C",\f(1,2)"#).unwrap().len(), 3);
        assert!(eq_list_operands(r#"A,"B,C"#).is_none());
    }

    #[test]
    fn barcode_field_syntax_accepts_known_switches_and_rejects_malformed_forms() {
        assert!(barcode_field_syntax(r#"DISPLAYBARCODE "12345" QR \q H"#));
        assert!(barcode_field_syntax(r#"DISPLAYBARCODE "12345" QR \qH"#));
        assert!(barcode_field_syntax(
            r#"MERGEBARCODE Zip JPPOST \h1440 \s100 \r1 \f0x000000 \bFFFFFF \t \a"#
        ));
        assert!(barcode_field_syntax(r#"BARCODE "9781234567890""#));
        assert!(!barcode_field_syntax(r#"DISPLAYBARCODE "12345" QR \h"#));
        assert!(!barcode_field_syntax(r#"DISPLAYBARCODE "12345" QR \z"#));
        assert!(!barcode_field_syntax(r#"DISPLAYBARCODE "12345" BADTYPE"#));
        assert!(!barcode_field_syntax(r#"DISPLAYBARCODE "12345" QR \s 1"#));
    }

    #[test]
    fn hyperlink_field_target_accepts_target_and_anchor_forms() {
        assert_eq!(
            hyperlink_field_target(r#" HYPERLINK "https://example.com" \o "tip" "#).as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            hyperlink_field_target(
                r#"HYPERLINK "https://example.com" \o Client portal tooltip \t NewWindow"#
            )
            .as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            hyperlink_field_target(
                r#"HYPERLINK "https://example.com" \m \n \o "tip" \t NewWindow"#
            )
            .as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            hyperlink_field_target(r#"HYPERLINK \l "AnchorName""#).as_deref(),
            Some("AnchorName")
        );
        assert!(hyperlink_field_target(r#"HYPERLINK "https://example.com" extra"#).is_none());
        assert!(hyperlink_field_target(r#"HYPERLINK "https://example.com" \x bad"#).is_none());
        assert!(hyperlink_field_target(r#"HYPERLINK "https://example.com" \n "extra""#).is_none());
        assert!(hyperlink_field_target(r#"HYPERLINK \o "tip""#).is_none());
        assert!(hyperlink_field_target(r#"HYPERLINKBASE "https://example.com""#).is_none());
    }

    #[test]
    fn merge_field_name_accepts_quoted_and_unquoted_names() {
        assert_eq!(
            merge_field_name(r#"MERGEFIELD ClientName \* MERGEFORMAT"#).as_deref(),
            Some("ClientName")
        );
        assert_eq!(
            merge_field_name(r#"MERGEFIELD "Client Name" \* MERGEFORMAT"#).as_deref(),
            Some("Client Name")
        );
        assert_eq!(
            merge_field_name(r#"MERGEFIELD Client Name \* MERGEFORMAT"#).as_deref(),
            Some("Client Name")
        );
        assert_eq!(
            merge_field_name(r#"MERGEFIELD Client Name"#).as_deref(),
            Some("Client Name")
        );
        assert!(merge_field_name(r#"MERGEFIELD \* MERGEFORMAT ClientName"#).is_none());
        assert!(merge_field_name(r#"MERGEFIELD "Client Name"#).is_none());
        assert!(merge_field_name(r#"MERGEFIELD "Client"Name""#).is_none());
    }

    #[test]
    fn merge_field_syntax_validates_common_switch_tails() {
        assert!(merge_field_syntax(
            r#"MERGEFIELD ClientName \b "Before " \f " After" \* MERGEFORMAT"#
        ));
        assert!(merge_field_syntax(
            r#"MERGEFIELD Client Name \f suffix text"#
        ));
        assert!(merge_field_syntax(
            r#"MERGEFIELD ClientName \bPrefix \fSuffix"#
        ));
        assert!(!merge_field_syntax(r#"MERGEFIELD ClientName \b"#));
        assert!(!merge_field_syntax(r#"MERGEFIELD ClientName \x"#));
        assert!(!merge_field_syntax(
            r#"MERGEFIELD ClientName \b "Before " trailing"#
        ));
    }

    #[test]
    fn normalized_field_instruction_preserves_quoted_whitespace() {
        assert_eq!(
            normalized_field_instruction(r#" DOCPROPERTY   "Client Name"   \*  MERGEFORMAT "#),
            r#"DOCPROPERTY "Client Name" \* MERGEFORMAT"#
        );
        assert_eq!(
            normalized_field_instruction(r#"TITLE   "Unclosed value   tail"#),
            r#"TITLE "Unclosed value   tail"#
        );
    }
}

pub(crate) fn document_property_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch != '_' && *ch != '-' && !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

pub(crate) fn reference_index_literal_token(value: &str) -> Option<&str> {
    let token = value.trim();
    let quoted = token.starts_with('"') && token.ends_with('"') && token.len() >= 2;
    let text = field_literal_token(token)?;
    if text.is_empty() || (!quoted && text.starts_with('\\')) {
        return None;
    }
    Some(text)
}

pub(crate) fn reference_index_literal_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    let quoted = first.trim().starts_with('"');
    let mut values = vec![reference_index_literal_token(first)?];
    if quoted {
        return Some(values.join(" "));
    }
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        values.push(reference_index_literal_token(parts.next()?)?);
    }
    Some(values.join(" "))
}

pub(crate) fn reference_index_plain_value_token(value: &str) -> Option<&str> {
    (!value.is_empty() && !value.starts_with('\\') && !value.contains('"')).then_some(value)
}

pub(crate) fn reference_index_category_token(value: &str) -> Option<u8> {
    let value = field_name_token(value)?.parse::<u8>().ok()?;
    (1..=16).contains(&value).then_some(value)
}

pub(crate) fn field_level_token(value: &str) -> Option<u8> {
    let level = field_name_token(value)?.parse::<u8>().ok()?;
    (1..=9).contains(&level).then_some(level)
}

pub(crate) fn field_level_range_token(value: &str) -> Option<(u8, u8)> {
    let value = field_name_token(value)?;
    let (start, end) = value.split_once('-')?;
    let start = start.parse::<u8>().ok()?;
    let end = end.parse::<u8>().ok()?;
    ((1..=9).contains(&start) && start <= end && end <= 9).then_some((start, end))
}

pub(crate) fn field_points_token(value: &str) -> Option<f32> {
    field_name_token(value)?
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

pub(crate) fn field_positive_points_token(value: &str) -> Option<f32> {
    field_points_token(value).filter(|value| *value > 0.0)
}

pub(crate) fn field_symbol_code_token(value: &str) -> Option<u32> {
    let value = field_name_token(value)?;
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16).ok();
    }
    if let Ok(code) = value.parse::<u32>() {
        return Some(code);
    }
    let mut chars = value.chars();
    let ch = chars.next()?;
    chars.next().is_none().then_some(ch as u32)
}

pub(crate) fn toc_style_specs(value: &str) -> Option<Vec<(&str, u8)>> {
    let value = value.trim();
    let value = match (value.starts_with('"'), value.ends_with('"')) {
        (true, true) if value.len() >= 2 => &value[1..value.len() - 1],
        (true, _) | (_, true) => return None,
        (false, false) => value,
    };
    let parts: Vec<_> = value.split(',').map(str::trim).collect();
    if parts.is_empty() || parts.len() % 2 != 0 {
        return None;
    }
    let mut specs = Vec::new();
    for pair in parts.chunks_exact(2) {
        let name = pair[0];
        let level = pair[1];
        if name.is_empty() || name.starts_with('\\') || name.contains('"') || level.contains('"') {
            return None;
        }
        let level = level.parse::<u8>().ok()?;
        if !(1..=9).contains(&level) {
            return None;
        }
        specs.push((name, level));
    }
    Some(specs)
}

fn toc_style_specs_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    if first.starts_with('"') {
        return Some(first.to_string());
    }
    let mut values = vec![first];
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        values.push(parts.next()?);
    }
    Some(values.join(" "))
}

pub(crate) fn instruction_parts(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in s.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            current.push(ch);
        } else if ch.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg_attr(not(feature = "docx"), allow(dead_code))]
pub(crate) fn normalized_field_instruction(instruction: &str) -> String {
    instruction_parts(instruction).join(" ")
}

fn is_document_info_field(token: &str) -> bool {
    matches!(
        token,
        "APPLICATION"
            | "APPVERSION"
            | "AUTHOR"
            | "CATEGORY"
            | "CHARACTERS"
            | "CHARACTERSWITHSPACES"
            | "COMMENT"
            | "COMMENTS"
            | "COMPANY"
            | "CONTENTSTATUS"
            | "CREATOR"
            | "CREATEDATE"
            | "DATE"
            | "DESCRIPTION"
            | "DOCPROPERTY"
            | "DOCVARIABLE"
            | "DOCSECURITY"
            | "EDITTIME"
            | "FILESIZE"
            | "HIDDENSLIDES"
            | "HYPERLINKBASE"
            | "HYPERLINKSCHANGED"
            | "INFO"
            | "KEYWORD"
            | "KEYWORDS"
            | "LASTMODIFIEDBY"
            | "LASTSAVEDBY"
            | "LINES"
            | "LINKSUPTODATE"
            | "MANAGER"
            | "MMCLIPS"
            | "NOTES"
            | "NUMCHARS"
            | "NUMPAGES"
            | "NUMWORDS"
            | "PAGES"
            | "PARAGRAPHS"
            | "PRINTDATE"
            | "PRESENTATIONFORMAT"
            | "SCALECROP"
            | "SAVEDATE"
            | "SHAREDDOC"
            | "SLIDES"
            | "SUBJECT"
            | "TEMPLATE"
            | "TIME"
            | "TITLE"
            | "TOTALTIME"
            | "USERADDRESS"
            | "USERINITIALS"
            | "USERNAME"
            | "VERSION"
            | "WORDS"
    )
}

fn is_dynamic_field(token: &str) -> bool {
    matches!(
        token,
        "=" | "ASK" | "COMPARE" | "FILLIN" | "IF" | "NEXT" | "NEXTIF" | "QUOTE" | "SET" | "SKIPIF"
    )
}

fn is_inserted_content_field(token: &str) -> bool {
    matches!(
        token,
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

fn is_mail_merge_field(token: &str) -> bool {
    matches!(
        token,
        "ADDRESSBLOCK" | "GREETINGLINE" | "MERGEREC" | "MERGESEQ"
    )
}

fn is_reference_index_field(token: &str) -> bool {
    matches!(
        token,
        "BIBLIOGRAPHY" | "CITATION" | "INDEX" | "RD" | "TA" | "TOA" | "XE"
    )
}

fn is_numbering_field(token: &str) -> bool {
    matches!(
        token,
        "AUTONUM" | "AUTONUMLGL" | "AUTONUMOUT" | "BIDIOUTLINE" | "LISTNUM"
    )
}

fn is_document_structure_field(token: &str) -> bool {
    matches!(token, "REVNUM" | "SECTION" | "SECTIONPAGES" | "STYLEREF")
}

fn is_display_field(token: &str) -> bool {
    matches!(token, "ADVANCE" | "EQ" | "SYMBOL")
}

fn is_action_field(token: &str) -> bool {
    matches!(token, "GOTOBUTTON" | "MACROBUTTON" | "PRINT")
}

fn is_compatibility_field(token: &str) -> bool {
    matches!(
        token,
        "ADDIN" | "DATA" | "GLOSSARY" | "HTMLACTIVEX" | "PRIVATE"
    )
}

fn is_barcode_field(token: &str) -> bool {
    matches!(token, "BARCODE" | "DISPLAYBARCODE" | "MERGEBARCODE")
}

fn is_legacy_form_field(token: &str) -> bool {
    matches!(token, "FORMTEXT" | "FORMCHECKBOX" | "FORMDROPDOWN")
}

/// Caller-supplied evaluation context for volatile or external-data fields.
///
/// rwml keeps fields whose value depends on runtime state (current date/time,
/// user identity, merge data, or included text) cached-only by default so
/// results stay deterministic. A `FieldContext` turns those into
/// deterministic-given-inputs computations: the context values are inputs, so
/// the same document plus the same context always yields the same results.
/// Fields the context does not cover keep their default behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldContext {
    pub(crate) now: Option<String>,
    pub(crate) user_name: Option<String>,
    pub(crate) user_initials: Option<String>,
    pub(crate) user_address: Option<String>,
    pub(crate) merge_values: HashMap<String, String>,
    pub(crate) include_text: HashMap<String, String>,
}

impl FieldContext {
    /// Empty context: `fields_with_context` behaves like [`Document::fields`].
    ///
    /// [`Document::fields`]: crate::Document::fields
    pub fn new() -> Self {
        Self::default()
    }

    /// Current timestamp for `DATE`/`TIME` fields with an explicit `\@`
    /// picture, in core-properties form (`yyyy-MM-ddTHH:mm:ss`, optional
    /// trailing zone). Pictureless `DATE`/`TIME` stay cached because they
    /// would need a locale-default picture.
    pub fn now(mut self, now: impl Into<String>) -> Self {
        self.now = Some(now.into());
        self
    }

    /// User name for `USERNAME` fields without an explicit literal override.
    pub fn user_name(mut self, name: impl Into<String>) -> Self {
        self.user_name = Some(name.into());
        self
    }

    /// User initials for `USERINITIALS` fields without an explicit override.
    pub fn user_initials(mut self, initials: impl Into<String>) -> Self {
        self.user_initials = Some(initials.into());
        self
    }

    /// User address for `USERADDRESS` fields without an explicit override.
    pub fn user_address(mut self, address: impl Into<String>) -> Self {
        self.user_address = Some(address.into());
        self
    }

    /// Merge data for `MERGEFIELD` fields. Names use the same case-insensitive
    /// folding as document property names, so `Client Name`, `client-name`, and
    /// `client_name` address the same deterministic-given-inputs value.
    pub fn merge_value(mut self, name: impl AsRef<str>, value: impl Into<String>) -> Self {
        self.merge_values
            .insert(document_property_key(name.as_ref()), value.into());
        self
    }

    /// Included text for `INCLUDETEXT` fields, keyed by the first quoted or bare
    /// source operand from the instruction. The supplied text is treated as the
    /// deterministic-given-inputs replacement for that external source.
    pub fn include_text(mut self, source: impl Into<String>, text: impl Into<String>) -> Self {
        self.include_text.insert(source.into(), text.into());
        self
    }
}

/// A Word field observed in a document story.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    /// Field kind derived from the first instruction token.
    pub kind: FieldKind,
    /// Normalized field instruction text.
    pub instruction: String,
    /// Cached visible result text stored in the document.
    pub result: String,
    /// Value rwml computed from document semantics, when evaluation is supported
    /// and unambiguous. Cached result text remains available in [`Field::result`].
    pub computed_result: Option<String>,
}

impl Default for FieldKind {
    fn default() -> Self {
        FieldKind::Unknown(String::new())
    }
}

/// Kind of tracked revision marker observed in a `.docx` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionKind {
    /// `w:ins`.
    Insertion,
    /// `w:del`.
    Deletion,
    /// `w:moveFrom`.
    MoveFrom,
    /// `w:moveTo`.
    MoveTo,
}

/// Flat text view policy for tracked revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionView {
    /// Current accepted view: include insertions and move destinations; exclude
    /// deletions and move sources.
    Accepted,
    /// Original view: include deletions and move sources; exclude insertions and
    /// move destinations.
    Original,
    /// Annotated view: keep both sides with compact textual markers.
    Annotated,
}

/// A tracked revision extracted from a `.docx` body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    /// Revision kind.
    pub kind: RevisionKind,
    /// Revision id (`w:id`), if present.
    pub id: Option<String>,
    /// Revision author (`w:author`), if present.
    pub author: Option<String>,
    /// Revision timestamp (`w:date`), if present.
    pub date: Option<String>,
    /// Visible text contained in the revision subtree.
    pub text: String,
}
