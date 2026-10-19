//! The rich document model — a typed intermediate representation built lazily on
//! top of the piece table, character/paragraph properties, the style sheet, and
//! the table structure.
//!
//! The flat [`crate::Document::text`] path is untouched and stays fast; the model
//! is only assembled when a caller asks for [`crate::Document::model`],
//! [`crate::Document::to_markdown`], or [`crate::Document::to_html`].
//!
//! The shape mirrors the de-facto Rust/Pandoc document IRs (a document is a list
//! of block-level nodes; paragraphs hold inline runs) so the Markdown/HTML
//! exporters are simple folds.

use std::collections::BTreeMap;

use crate::annotation::{NoteKind, RevisionKind};

/// A whole `.doc` document as an ordered list of block-level nodes plus
/// document-level metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocModel {
    /// Block-level content, in reading order.
    pub blocks: Vec<Block>,
    /// Source-region spans for content that came from distinct Word
    /// subdocuments. Empty for authored models and sources that do not expose a
    /// region map yet.
    pub regions: Vec<SourceRegion>,
    /// Document-level metadata (codepage, language, counts).
    pub meta: DocMeta,
    /// Authored string custom document properties.
    pub custom_properties: BTreeMap<String, String>,
    /// Authored custom XML data-store items.
    pub custom_xml_items: Vec<CustomXmlItem>,
    /// Document-level layout for authoring/rendering (page size, header/footer,
    /// metadata). Defaults to A4 portrait with no running header/footer.
    pub setup: DocSetup,
}

/// A generated custom XML data-store item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomXmlItem {
    /// Custom XML store item ID.
    pub store_item_id: String,
    /// Raw XML payload for `customXml/itemN.xml`.
    pub xml: String,
}

/// A generated Office web-extension task pane package entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebExtensionTaskPane {
    /// Web extension instance id.
    pub extension_id: String,
    /// Add-in reference id from the deployment catalog or manifest.
    pub reference_id: String,
    /// Add-in version string.
    pub version: String,
    /// Store or catalog pointer.
    pub store: String,
    /// Store or catalog kind, such as `EXCatalog`, `FileSystem`, or `OMEX`.
    pub store_type: String,
    /// Web-extension custom property bag.
    pub properties: BTreeMap<String, String>,
    /// Last docked task-pane location.
    pub dock_state: String,
    /// Whether the task pane is visible by default.
    pub visible: bool,
    /// Default task-pane width.
    pub width: u32,
    /// Task-pane row index among panes docked in the same location.
    pub row: u32,
    /// Whether the task pane is locked in the UI.
    pub locked: bool,
}

/// A coarse source subdocument region from the original Word file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceRegionKind {
    /// Main body text.
    Main,
    /// Legacy footnote subdocument.
    Footnote,
    /// Header/footer subdocument.
    HeaderFooter,
    /// Legacy annotation/comment subdocument.
    Annotation,
    /// Legacy endnote subdocument.
    Endnote,
    /// Text-box subdocument.
    TextBox,
}

/// A span of [`DocModel::blocks`] that came from one source subdocument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRegion {
    /// Region kind.
    pub kind: SourceRegionKind,
    /// Source-specific story index within the subdocument, when a format table
    /// exposes one. For legacy `.doc` header/footer regions this is the
    /// `PlcfHdd` story index.
    pub source_story_index: Option<usize>,
    /// Inclusive start block index in [`DocModel::blocks`].
    pub block_start: usize,
    /// Exclusive end block index in [`DocModel::blocks`].
    pub block_end: usize,
    /// UTF-16 CP start in the original Word source stream.
    pub source_start_cp: usize,
    /// UTF-16 CP length reported by the source metadata.
    pub source_len_cp: usize,
    /// Visible character start within the flattened model text.
    pub text_start: usize,
    /// Visible character length contributed by this region.
    pub text_len: usize,
}

impl DocModel {
    /// Iterate source-region spans of the requested kind.
    pub fn source_regions(&self, kind: SourceRegionKind) -> impl Iterator<Item = &SourceRegion> {
        self.regions
            .iter()
            .filter(move |region| region.kind == kind)
    }

    /// Return the block slice covered by a source-region span.
    ///
    /// The range is clamped to this model, so passing a stale region from another
    /// model returns an empty or shortened slice instead of panicking.
    pub fn source_region_blocks(&self, region: &SourceRegion) -> &[Block] {
        let start = region.block_start.min(self.blocks.len());
        let end = region.block_end.min(self.blocks.len());
        if start <= end {
            &self.blocks[start..end]
        } else {
            &self.blocks[0..0]
        }
    }

    /// Concatenate visible text from the blocks covered by a source-region span.
    pub fn source_region_text(&self, region: &SourceRegion) -> String {
        let mut out = String::new();
        append_blocks_text(self.source_region_blocks(region), &mut out);
        out
    }

    /// Concatenate visible text from all source regions of one kind, in model
    /// order. Returns an empty string when the model has no matching regions.
    pub fn source_region_kind_text(&self, kind: SourceRegionKind) -> String {
        let mut out = String::new();
        for region in self.source_regions(kind) {
            append_blocks_text(self.source_region_blocks(region), &mut out);
        }
        out
    }
}

fn append_blocks_text(blocks: &[Block], out: &mut String) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                for run in &paragraph.runs {
                    out.push_str(&run.text);
                }
            }
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        append_blocks_text(&cell.blocks, out);
                    }
                }
            }
            Block::Image(_) | Block::Chart(_) | Block::SectionBreak(_) => {}
            Block::PageBreak => out.push('\n'),
        }
    }
}

/// Document-level metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocMeta {
    /// ANSI codepage of 8-bit pieces (e.g. 949 for Korean).
    pub codepage: u16,
    /// FIB language id (`lid`).
    pub lid: u16,
    /// Aggregate counts (paragraphs, tables, figures, characters).
    pub stats: Stats,
}

/// Aggregate document statistics, mirroring the project-wide `DocStats` contract.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Number of paragraphs (including headings, list items, and cell paragraphs).
    pub paragraphs: u32,
    /// Number of tables.
    pub tables: u16,
    /// Number of images / figures.
    pub figures: u16,
    /// Total visible character count.
    pub text_chars: usize,
}

/// A block-level node.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A paragraph — also carries headings and list items via [`ParaProps`].
    Paragraph(Paragraph),
    /// A table (rows → cells → blocks).
    Table(Table),
    /// A block-level image (an image-only paragraph).
    Image(Image),
    /// A block-level chart with literal category/value data.
    Chart(Chart),
    /// An explicit page break between block-level items.
    PageBreak,
    /// A section boundary carrying layout for the section that just ended.
    SectionBreak(SectionSetup),
}

/// Section-break start behavior for a WordprocessingML section boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionBreakKind {
    /// Start the following section on the next page (`nextPage`).
    NextPage,
    /// Start the following section on the next even page (`evenPage`).
    EvenPage,
    /// Start the following section on the next odd page (`oddPage`).
    OddPage,
}

#[cfg(feature = "docx")]
impl SectionBreakKind {
    pub(crate) fn wml_value(self) -> &'static str {
        match self {
            SectionBreakKind::NextPage => "nextPage",
            SectionBreakKind::EvenPage => "evenPage",
            SectionBreakKind::OddPage => "oddPage",
        }
    }

    pub(crate) fn from_wml_value(value: &str) -> Option<Self> {
        let value = value.trim();
        match value {
            "nextPage" => Some(SectionBreakKind::NextPage),
            "evenPage" => Some(SectionBreakKind::EvenPage),
            "oddPage" => Some(SectionBreakKind::OddPage),
            _ => None,
        }
    }
}

/// A paragraph: inline runs plus paragraph-level properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paragraph {
    /// Paragraph-level properties (style, heading, alignment, list membership).
    pub props: ParaProps,
    /// Inline runs, in reading order.
    pub runs: Vec<Run>,
}

impl Paragraph {
    /// The concatenated visible text of all runs (hidden runs included — they are
    /// part of the indexed text; the HTML exporter may choose to omit them).
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// `true` if the paragraph has no visible text and no image in any run.
    pub fn is_blank(&self) -> bool {
        self.runs
            .iter()
            .all(|r| r.text.trim().is_empty() && r.image.is_none())
    }
}

/// Paragraph spacing in points; `None` = unset (renderer/writer uses defaults).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spacing {
    /// Space above the paragraph.
    pub before_pt: Option<f32>,
    /// Space below the paragraph.
    pub after_pt: Option<f32>,
    /// Line height as a multiple of the font size (e.g. `1.5`).
    pub line_pct: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LineSpacingHint {
    Exact(f32),
    AtLeast(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SectionColumnHint {
    pub(crate) width_pt: f32,
    pub(crate) space_after_pt: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SectionColumnLayoutHints {
    pub(crate) columns: Vec<SectionColumnHint>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct RunningSurfaceDistanceHints {
    pub(crate) header_pt: Option<f32>,
    pub(crate) footer_pt: Option<f32>,
}

/// Paragraph indentation in points; `None` = unset.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Indent {
    /// Left indent.
    pub left_pt: Option<f32>,
    /// Right indent.
    pub right_pt: Option<f32>,
    /// First-line additional indent.
    pub first_line_pt: Option<f32>,
    /// Hanging indent (first line outdented by this much).
    pub hanging_pt: Option<f32>,
}

/// Paragraph-level properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParaProps {
    /// Raw Word paragraph style id (`w:pStyle/@w:val`), if known or authored.
    pub style_id: Option<String>,
    /// Resolved style name (e.g. `Heading 1`, `제목 1`), if the style sheet was read.
    pub style_name: Option<String>,
    /// Heading level `1..=6`, from the outline level or a recognized style name.
    pub heading_level: Option<u8>,
    /// Paragraph alignment.
    pub align: Align,
    /// Raw outline level `0..=8` (9/None = body text).
    pub outline_level: Option<u8>,
    /// List membership — `Some` makes this paragraph a list item.
    pub list: Option<ListInfo>,
    /// Spacing (before/after/line).
    pub spacing: Spacing,
    /// Indentation.
    pub indent: Indent,
    /// Paragraph background shading, if any.
    pub shading: Option<Color>,
    /// Force this paragraph to begin on a new page (`w:pageBreakBefore`).
    pub page_break_before: bool,
    /// Right-to-left paragraph layout (`w:bidi`).
    pub bidi: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PaginationHint {
    pub(crate) keep_next: bool,
    pub(crate) keep_lines: bool,
    pub(crate) widow_control: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TableRowPaginationHint {
    pub(crate) cant_split: bool,
}

pub(crate) type TableCellPaginationHints = Vec<Vec<Vec<Option<PaginationHint>>>>;

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningBlockPaginationHints {
    pub(crate) paragraphs: Vec<PaginationHint>,
    pub(crate) table_rows: Vec<Vec<TableRowPaginationHint>>,
    pub(crate) table_cells: Vec<TableCellPaginationHints>,
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningSurfacePaginationHints {
    pub(crate) header: RunningBlockPaginationHints,
    pub(crate) first_header: RunningBlockPaginationHints,
    pub(crate) even_header: RunningBlockPaginationHints,
    pub(crate) footer: RunningBlockPaginationHints,
    pub(crate) first_footer: RunningBlockPaginationHints,
    pub(crate) even_footer: RunningBlockPaginationHints,
}

pub(crate) type TableCellLineSpacingHints = Vec<Vec<Vec<Option<LineSpacingHint>>>>;

#[cfg(any(feature = "docx", feature = "render"))]
pub(crate) type TableCellTabStopHints = Vec<Vec<Vec<Vec<TabStop>>>>;

#[cfg(feature = "docx")]
pub(crate) type TableCellColumnBreakHints = Vec<Vec<Vec<Vec<usize>>>>;

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TablePaginationHints {
    pub(crate) rows: Vec<TableRowPaginationHint>,
    pub(crate) cells: TableCellPaginationHints,
    pub(crate) cell_line_spacing: TableCellLineSpacingHints,
    #[cfg(feature = "docx")]
    pub(crate) cell_column_breaks: TableCellColumnBreakHints,
    pub(crate) nested: TableCellNestedPaginationHints,
    pub(crate) cell_tabs: TableCellTabStopHints,
}

#[cfg(any(feature = "docx", feature = "render"))]
pub(crate) type TableCellNestedPaginationHints = Vec<Vec<Vec<Option<TablePaginationHints>>>>;

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabAlignment {
    Left,
    Center,
    Right,
    Decimal,
    Bar,
    Clear,
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum TabLeader {
    #[default]
    None,
    Dot,
    Hyphen,
    Underscore,
    Heavy,
    MiddleDot,
    #[cfg(feature = "render")]
    Bar,
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabStop {
    pub(crate) position_pt: f32,
    pub(crate) alignment: TabAlignment,
    pub(crate) leader: TabLeader,
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningSurfaceLineSpacingHints {
    pub(crate) header: Vec<Option<LineSpacingHint>>,
    pub(crate) header_table_cells: Vec<TableCellLineSpacingHints>,
    pub(crate) first_header: Vec<Option<LineSpacingHint>>,
    pub(crate) first_header_table_cells: Vec<TableCellLineSpacingHints>,
    pub(crate) even_header: Vec<Option<LineSpacingHint>>,
    pub(crate) even_header_table_cells: Vec<TableCellLineSpacingHints>,
    pub(crate) footer: Vec<Option<LineSpacingHint>>,
    pub(crate) footer_table_cells: Vec<TableCellLineSpacingHints>,
    pub(crate) first_footer: Vec<Option<LineSpacingHint>>,
    pub(crate) first_footer_table_cells: Vec<TableCellLineSpacingHints>,
    pub(crate) even_footer: Vec<Option<LineSpacingHint>>,
    pub(crate) even_footer_table_cells: Vec<TableCellLineSpacingHints>,
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningSurfaceTabStopHints {
    pub(crate) header: Vec<Vec<TabStop>>,
    pub(crate) first_header: Vec<Vec<TabStop>>,
    pub(crate) even_header: Vec<Vec<TabStop>>,
    pub(crate) footer: Vec<Vec<TabStop>>,
    pub(crate) first_footer: Vec<Vec<TabStop>>,
    pub(crate) even_footer: Vec<Vec<TabStop>>,
}

#[cfg(any(feature = "docx", feature = "render"))]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningSurfaceTableCellTabStopHints {
    pub(crate) header: Vec<TableCellTabStopHints>,
    pub(crate) first_header: Vec<TableCellTabStopHints>,
    pub(crate) even_header: Vec<TableCellTabStopHints>,
    pub(crate) footer: Vec<TableCellTabStopHints>,
    pub(crate) first_footer: Vec<TableCellTabStopHints>,
    pub(crate) even_footer: Vec<TableCellTabStopHints>,
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningSurfaceColumnBreakHints {
    pub(crate) header: Vec<Vec<usize>>,
    pub(crate) first_header: Vec<Vec<usize>>,
    pub(crate) even_header: Vec<Vec<usize>>,
    pub(crate) footer: Vec<Vec<usize>>,
    pub(crate) first_footer: Vec<Vec<usize>>,
    pub(crate) even_footer: Vec<Vec<usize>>,
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NoteWritePayload {
    pub(crate) kind: NoteKind,
    pub(crate) text: String,
    pub(crate) blocks: Vec<Block>,
    pub(crate) pagination: Vec<PaginationHint>,
    pub(crate) line_spacing: Vec<Option<LineSpacingHint>>,
    pub(crate) tab_stops: Vec<Vec<TabStop>>,
    pub(crate) column_break_offsets: Vec<Vec<usize>>,
    pub(crate) table_pagination: Vec<Option<TablePaginationHints>>,
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningTableLayoutHints {
    pub(crate) cell_column_breaks: Vec<TableCellColumnBreakHints>,
    pub(crate) nested_tables: Vec<TableCellNestedPaginationHints>,
}

#[cfg(feature = "docx")]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunningSurfaceTableLayoutHints {
    pub(crate) header: RunningTableLayoutHints,
    pub(crate) first_header: RunningTableLayoutHints,
    pub(crate) even_header: RunningTableLayoutHints,
    pub(crate) footer: RunningTableLayoutHints,
    pub(crate) first_footer: RunningTableLayoutHints,
    pub(crate) even_footer: RunningTableLayoutHints,
}

#[cfg(any(feature = "docx", feature = "render"))]
pub(crate) const MAX_TAB_STOPS: usize = 256;

/// A paragraph style definition for generated `.docx` output.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphStyle {
    /// Stable Word style id (`w:styleId` / paragraph `w:pStyle` value).
    pub id: String,
    /// Human-readable style name.
    pub name: String,
    /// Base style id, if any.
    pub based_on: Option<String>,
    /// Next paragraph style id, if any.
    pub next: Option<String>,
    /// Whether the style should appear in Word's quick style gallery.
    pub q_format: bool,
    /// Optional heading level `1..=9`, emitted as style `outlineLvl`.
    pub heading_level: Option<u8>,
    /// Default paragraph alignment.
    pub align: Align,
    /// Default paragraph spacing.
    pub spacing: Spacing,
    /// Default paragraph indentation.
    pub indent: Indent,
    /// Default paragraph background shading.
    pub shading: Option<Color>,
    /// Default character properties for runs using this style.
    pub run: CharProps,
}

/// A comment to author on a generated run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredComment {
    /// Comment body text.
    pub text: String,
    /// Comment author, if any.
    pub author: Option<String>,
    /// Author initials, if any.
    pub initials: Option<String>,
    /// Comment timestamp, if any.
    pub date: Option<String>,
    /// Parent comment id for authored replies, if any.
    pub parent_comment_id: Option<String>,
}

/// Tracked revision metadata to author on a generated run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredRevision {
    /// Revision kind. Generated `.docx` currently supports insertion/deletion.
    pub kind: RevisionKind,
    /// Revision author, if any.
    pub author: Option<String>,
    /// Revision timestamp, if any.
    pub date: Option<String>,
}

/// Plain text content-control metadata to author on a generated run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredContentControl {
    /// Human-readable content-control title/alias, if any.
    pub alias: Option<String>,
    /// Machine-readable content-control tag, if any.
    pub tag: Option<String>,
    /// XPath for a data-bound content control, if any.
    pub data_binding_xpath: Option<String>,
    /// Custom XML store item ID for a data-bound content control, if any.
    pub data_binding_store_item_id: Option<String>,
}

/// Footnote or endnote metadata to author after a generated run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredNote {
    /// Footnote or endnote.
    pub kind: NoteKind,
    /// Note body text.
    pub text: String,
}

impl Default for AuthoredRevision {
    fn default() -> Self {
        Self {
            kind: RevisionKind::Insertion,
            author: None,
            date: None,
        }
    }
}

/// List membership of a paragraph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListInfo {
    /// 0-based nesting level.
    pub level: u8,
    /// `true` = numbered list, `false` = bullet list.
    pub ordered: bool,
    /// The reader-captured rendered autonumber label (e.g. `1.`, `가.`). PDF
    /// preview prefers a nonempty label and synthesizes a deterministic fallback
    /// when empty. Markdown/HTML emit native list syntax; the DOCX writer
    /// reconstructs numbering from [`Self::level`] and [`Self::ordered`].
    pub label: String,
    /// Font declared for the list marker, such as Wingdings for symbol bullets.
    pub marker_font: Option<String>,
}

/// An inline run of text with uniform character properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    /// The run's text.
    pub text: String,
    /// Character formatting.
    pub props: CharProps,
    /// Field role (a hyperlink result carries its URL).
    pub field: FieldRole,
    /// Mark generated simple fields dirty so Word can refresh them.
    pub field_dirty: bool,
    /// Best-effort reason an imported field result remains cached, when known.
    pub field_unsupported_reason: Option<FieldUnsupportedReason>,
    /// An inline picture (the run's text is empty when this is set).
    pub image: Option<Image>,
    /// Authored comment anchored to this run.
    pub comment: Option<AuthoredComment>,
    /// Authored tracked insertion/deletion metadata for this run.
    pub revision: Option<AuthoredRevision>,
    /// Authored plain text content-control metadata for this run.
    pub content_control: Option<AuthoredContentControl>,
    /// Authored bookmark name wrapping this run.
    pub bookmark: Option<String>,
    /// Authored footnote/endnote anchored after this run.
    pub note: Option<AuthoredNote>,
}

/// Best-effort reason an imported field remains cached in the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldUnsupportedReason {
    /// The field points at a bookmark/scope rwml could not resolve.
    UnresolvedBookmark,
    /// The instruction contains a switch whose value can change the result.
    UnsupportedSwitch,
    /// The instruction is supported, but the document contains no computable value.
    NoComputedResult,
}

pub(crate) fn referenceable_bookmark_name(name: &str) -> bool {
    !name.trim().is_empty()
        && !name.starts_with('\\')
        && !name.contains('"')
        && !name.chars().any(char::is_whitespace)
}

pub(crate) fn normalize_field_instruction(instruction: &str) -> String {
    instruction.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Color {
    /// Construct from components.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }
}

/// A physical side of a table border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableBorderSide {
    /// Top border.
    Top,
    /// Left border.
    Left,
    /// Bottom border.
    Bottom,
    /// Right border.
    Right,
    /// Horizontal inside borders.
    InsideHorizontal,
    /// Vertical inside borders.
    InsideVertical,
}

/// Uniform table border line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableBorderStyle {
    /// Single solid line (`single`).
    Single,
    /// Dotted line (`dotted`).
    Dotted,
    /// Dashed line (`dashed`).
    Dashed,
    /// Double solid line (`double`).
    Double,
}

#[cfg(feature = "docx")]
impl TableBorderStyle {
    pub(crate) fn wml_value(self) -> &'static str {
        match self {
            TableBorderStyle::Single => "single",
            TableBorderStyle::Dotted => "dotted",
            TableBorderStyle::Dashed => "dashed",
            TableBorderStyle::Double => "double",
        }
    }

    pub(crate) fn from_wml_value(value: &str) -> Option<Self> {
        let value = value.trim();
        match value {
            "single" => Some(TableBorderStyle::Single),
            "dotted" => Some(TableBorderStyle::Dotted),
            "dashed" => Some(TableBorderStyle::Dashed),
            "double" => Some(TableBorderStyle::Double),
            _ => None,
        }
    }
}

/// Optional table border colors by physical side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableBorderColors {
    /// Top border color.
    pub top: Option<Color>,
    /// Left border color.
    pub left: Option<Color>,
    /// Bottom border color.
    pub bottom: Option<Color>,
    /// Right border color.
    pub right: Option<Color>,
    /// Horizontal inside border color.
    pub inside_h: Option<Color>,
    /// Vertical inside border color.
    pub inside_v: Option<Color>,
}

impl TableBorderColors {
    /// Return the color for a side, if one was set.
    pub fn get(&self, side: TableBorderSide) -> Option<Color> {
        match side {
            TableBorderSide::Top => self.top,
            TableBorderSide::Left => self.left,
            TableBorderSide::Bottom => self.bottom,
            TableBorderSide::Right => self.right,
            TableBorderSide::InsideHorizontal => self.inside_h,
            TableBorderSide::InsideVertical => self.inside_v,
        }
    }

    /// Set the color for a side.
    pub fn set(&mut self, side: TableBorderSide, color: Color) {
        match side {
            TableBorderSide::Top => self.top = Some(color),
            TableBorderSide::Left => self.left = Some(color),
            TableBorderSide::Bottom => self.bottom = Some(color),
            TableBorderSide::Right => self.right = Some(color),
            TableBorderSide::InsideHorizontal => self.inside_h = Some(color),
            TableBorderSide::InsideVertical => self.inside_v = Some(color),
        }
    }
}

/// Optional table border widths by physical side, in eighths of a point.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableBorderSizes {
    /// Top border width.
    pub top: Option<u16>,
    /// Left border width.
    pub left: Option<u16>,
    /// Bottom border width.
    pub bottom: Option<u16>,
    /// Right border width.
    pub right: Option<u16>,
    /// Horizontal inside border width.
    pub inside_h: Option<u16>,
    /// Vertical inside border width.
    pub inside_v: Option<u16>,
}

impl TableBorderSizes {
    /// Return the width for a side, if one was set.
    pub fn get(&self, side: TableBorderSide) -> Option<u16> {
        match side {
            TableBorderSide::Top => self.top,
            TableBorderSide::Left => self.left,
            TableBorderSide::Bottom => self.bottom,
            TableBorderSide::Right => self.right,
            TableBorderSide::InsideHorizontal => self.inside_h,
            TableBorderSide::InsideVertical => self.inside_v,
        }
    }

    /// Set the width for a side.
    pub fn set(&mut self, side: TableBorderSide, size: u16) {
        let size = size.max(1);
        match side {
            TableBorderSide::Top => self.top = Some(size),
            TableBorderSide::Left => self.left = Some(size),
            TableBorderSide::Bottom => self.bottom = Some(size),
            TableBorderSide::Right => self.right = Some(size),
            TableBorderSide::InsideHorizontal => self.inside_h = Some(size),
            TableBorderSide::InsideVertical => self.inside_v = Some(size),
        }
    }
}

/// Optional table border line styles by physical side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableBorderStyles {
    /// Top border style.
    pub top: Option<TableBorderStyle>,
    /// Left border style.
    pub left: Option<TableBorderStyle>,
    /// Bottom border style.
    pub bottom: Option<TableBorderStyle>,
    /// Right border style.
    pub right: Option<TableBorderStyle>,
    /// Horizontal inside border style.
    pub inside_h: Option<TableBorderStyle>,
    /// Vertical inside border style.
    pub inside_v: Option<TableBorderStyle>,
}

impl TableBorderStyles {
    /// Return the style for a side, if one was set.
    pub fn get(&self, side: TableBorderSide) -> Option<TableBorderStyle> {
        match side {
            TableBorderSide::Top => self.top,
            TableBorderSide::Left => self.left,
            TableBorderSide::Bottom => self.bottom,
            TableBorderSide::Right => self.right,
            TableBorderSide::InsideHorizontal => self.inside_h,
            TableBorderSide::InsideVertical => self.inside_v,
        }
    }

    /// Set the style for a side.
    pub fn set(&mut self, side: TableBorderSide, style: TableBorderStyle) {
        match side {
            TableBorderSide::Top => self.top = Some(style),
            TableBorderSide::Left => self.left = Some(style),
            TableBorderSide::Bottom => self.bottom = Some(style),
            TableBorderSide::Right => self.right = Some(style),
            TableBorderSide::InsideHorizontal => self.inside_h = Some(style),
            TableBorderSide::InsideVertical => self.inside_v = Some(style),
        }
    }
}

/// Vertical alignment of a run (super/subscript).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VertAlign {
    /// On the baseline (normal).
    #[default]
    Baseline,
    /// Superscript.
    Super,
    /// Subscript.
    Sub,
}

/// Character-level formatting that affects rendering.
///
/// `font`/`color`/`highlight` make this non-`Copy`; clone where a `CharProps` is
/// needed by value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharProps {
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Struck through.
    pub strike: bool,
    /// Hidden text (`fVanish`) — kept in the flat/Markdown text for index recall,
    /// but the HTML exporter omits it.
    pub hidden: bool,
    /// Font family name (emitted to `w:rFonts` ascii+eastAsia), if known.
    pub font: Option<String>,
    /// Font size in half-points (Word unit; `24` = 12pt), if known.
    pub size_half_pt: Option<u16>,
    /// Text color, if known.
    pub color: Option<Color>,
    /// Highlight color name (`w:highlight`, e.g. `"yellow"`), if any.
    pub highlight: Option<String>,
    /// Super/subscript.
    pub vert_align: VertAlign,
    /// Small caps (`w:smallCaps`): lowercase letters render as small capitals.
    pub small_caps: bool,
    /// All caps (`w:caps`): text renders uppercased regardless of stored case.
    pub caps: bool,
    /// Right-to-left run text (`w:rtl`).
    pub rtl: bool,
}

/// Whether a run is plain text or the cached result of a field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FieldRole {
    /// Plain text (not part of a field).
    #[default]
    None,
    /// A hyperlink — `url` from the `HYPERLINK` field instruction.
    Hyperlink {
        /// The link target.
        url: String,
    },
    /// A simple non-hyperlink field whose instruction is preserved.
    Simple {
        /// Normalized field instruction such as `PAGE`, `FILENAME \p`, or `REF Figure1`.
        instruction: String,
    },
    /// Any other field (PAGE, REF, …) whose result text is kept verbatim.
    Other,
}

/// Paragraph alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    /// Left-aligned (the default).
    #[default]
    Left,
    /// Centered.
    Center,
    /// Right-aligned.
    Right,
    /// Justified.
    Justify,
}

/// A table: rows of cells.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    /// The table's rows, top to bottom.
    pub rows: Vec<Row>,
    /// Number of leading header rows (repeated `sprmTTableHeader` rows).
    pub header_rows: usize,
    /// Column widths as fractions of the table width; empty = even split.
    pub col_widths_pct: Vec<f32>,
    /// Table width as a fraction of the available width, if set (`None` = auto).
    pub width_pct: Option<f32>,
    /// Use Word's fixed table layout algorithm instead of autofit.
    pub fixed_layout: bool,
    /// Table indentation in twips, if explicitly set.
    pub indent_twips: Option<i32>,
    /// Table alignment, if explicitly set.
    pub align: Option<Align>,
    /// Visual right-to-left table ordering (`w:bidiVisual`).
    pub bidi_visual: bool,
    /// Uniform table border color, if explicitly set.
    pub border_color: Option<Color>,
    /// Side-specific table border colors, overriding `border_color` per side.
    pub border_colors: TableBorderColors,
    /// Uniform table border width in eighths of a point, if explicitly set.
    pub border_size_eighths: Option<u16>,
    /// Side-specific table border widths, overriding `border_size_eighths` per side.
    pub border_sizes: TableBorderSizes,
    /// Uniform table border style, if explicitly set.
    pub border_style: Option<TableBorderStyle>,
    /// Side-specific table border styles, overriding `border_style` per side.
    pub border_styles: TableBorderStyles,
}

impl Table {
    /// `true` if every cell holds exactly one single-line paragraph and no cell
    /// spans more than one row or column — i.e. a clean GFM pipe table is lossless.
    pub fn is_simple_grid(&self) -> bool {
        self.rows.iter().all(|row| {
            row.cells.iter().all(|c| {
                c.row_span <= 1
                    && c.col_span <= 1
                    && c.blocks.len() <= 1
                    && c.blocks.iter().all(|b| match b {
                        Block::Paragraph(p) => !p.text().contains('\n'),
                        _ => false,
                    })
            })
        })
    }
}

/// A table row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    /// The row's cells, left to right.
    pub cells: Vec<Cell>,
}

/// Vertical alignment of cell content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VCell {
    /// Top (the default).
    #[default]
    Top,
    /// Vertically centered.
    Center,
    /// Bottom.
    Bottom,
}

/// Table-cell margins in twips (1/20 point), ordered by physical side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellMargins {
    /// Top margin in twips.
    pub top: u32,
    /// Right margin in twips.
    pub right: u32,
    /// Bottom margin in twips.
    pub bottom: u32,
    /// Left margin in twips.
    pub left: u32,
}

/// A table cell — may hold block content and span rows/columns.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Block-level content of the cell.
    pub blocks: Vec<Block>,
    /// Number of rows this cell spans (1 = no vertical merge).
    pub row_span: u16,
    /// Number of columns this cell spans (1 = no horizontal merge).
    pub col_span: u16,
    /// Whether this is a header cell (`<th>`).
    pub is_header: bool,
    /// Cell background shading, if any.
    pub shading: Option<Color>,
    /// Vertical alignment of the cell content.
    pub valign: VCell,
    /// Cell width as a fraction of the table width, if set (`None` = auto).
    pub width_pct: Option<f32>,
    /// Effective physical cell margins in twips, if modeled from table defaults
    /// or direct cell properties.
    pub margins: Option<CellMargins>,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            blocks: Vec::new(),
            row_span: 1,
            col_span: 1,
            is_header: false,
            shading: None,
            valign: VCell::Top,
            width_pct: None,
            margins: None,
        }
    }
}

impl Cell {
    /// The cell's text, paragraphs joined by newlines.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some(p.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// An embedded image. `bytes`/`mime` are present only when the picture was
/// extracted; otherwise the node is a placeholder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Image {
    /// Alt text, if any.
    pub alt: Option<String>,
    /// Raw image bytes, when extracted.
    pub bytes: Option<Vec<u8>>,
    /// MIME type of `bytes` (e.g. `image/png`), when known.
    pub mime: Option<String>,
    /// Intrinsic width in pixels, parsed from the image header, when known.
    pub width_px: Option<u32>,
    /// Intrinsic height in pixels, parsed from the image header, when known.
    pub height_px: Option<u32>,
    /// Clockwise rotation in whole degrees.
    pub rotation_degrees: Option<i32>,
    /// Page-relative floating anchor offset in EMUs, when authoring `wp:anchor`.
    pub floating_offset_emu: Option<(i64, i64)>,
}

/// Supported chart layouts for authored `.docx` output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChartKind {
    /// A clustered horizontal bar chart.
    #[default]
    Bar,
    /// A stacked horizontal bar chart.
    StackedBar,
    /// A 100% stacked horizontal bar chart.
    PercentStackedBar,
    /// A clustered 3-D horizontal bar chart.
    Bar3D,
    /// A stacked 3-D horizontal bar chart.
    StackedBar3D,
    /// A 100% stacked 3-D horizontal bar chart.
    PercentStackedBar3D,
    /// A clustered vertical column chart.
    Column,
    /// A stacked vertical column chart.
    StackedColumn,
    /// A 100% stacked vertical column chart.
    PercentStackedColumn,
    /// A clustered 3-D vertical column chart.
    Column3D,
    /// A stacked 3-D vertical column chart.
    StackedColumn3D,
    /// A 100% stacked 3-D vertical column chart.
    PercentStackedColumn3D,
    /// A line chart with category labels on the horizontal axis.
    Line,
    /// A line chart without point markers.
    LineNoMarkers,
    /// A smoothed line chart with category labels on the horizontal axis.
    SmoothLine,
    /// A stacked line chart with category labels on the horizontal axis.
    StackedLine,
    /// A 100% stacked line chart with category labels on the horizontal axis.
    PercentStackedLine,
    /// A 3-D line chart with category labels on the horizontal axis.
    Line3D,
    /// An area chart with category labels on the horizontal axis.
    Area,
    /// A stacked area chart with category labels on the horizontal axis.
    StackedArea,
    /// A 100% stacked area chart with category labels on the horizontal axis.
    PercentStackedArea,
    /// A 3-D area chart with category labels on the horizontal axis.
    Area3D,
    /// A stacked 3-D area chart with category labels on the horizontal axis.
    StackedArea3D,
    /// A 100% stacked 3-D area chart with category labels on the horizontal axis.
    PercentStackedArea3D,
    /// A radar chart with category labels around a radial axis.
    Radar,
    /// A radar chart with explicit point markers.
    RadarWithMarkers,
    /// A filled radar chart with category labels around a radial axis.
    FilledRadar,
    /// A scatter chart with numeric horizontal and vertical values.
    Scatter,
    /// A marker-only scatter chart with numeric horizontal and vertical values.
    ScatterMarkers,
    /// A straight-line scatter chart without point markers.
    ScatterLines,
    /// A smoothed scatter chart with point markers.
    ScatterSmooth,
    /// A smoothed scatter chart without point markers.
    ScatterSmoothNoMarkers,
    /// A bubble chart with numeric horizontal values, vertical values, and sizes.
    Bubble,
    /// A 3-D bubble chart with numeric horizontal values, vertical values, and sizes.
    Bubble3D,
    /// A pie chart using the first series as slice values.
    Pie,
    /// An exploded pie chart using the first series as slice values.
    ExplodedPie,
    /// A 3-D pie chart using the first series as slice values.
    Pie3D,
    /// An exploded 3-D pie chart using the first series as slice values.
    ExplodedPie3D,
    /// A pie-of-pie chart using the first series as slice values.
    PieOfPie,
    /// A bar-of-pie chart using the first series as slice values.
    BarOfPie,
    /// A doughnut chart using the first series as slice values.
    Doughnut,
    /// An exploded doughnut chart using the first series as slice values.
    ExplodedDoughnut,
    /// A surface chart using category columns and series rows as a value grid.
    Surface,
    /// A 3-D surface chart using category columns and series rows as a value grid.
    Surface3D,
    /// A high-low-close stock chart using date/category labels.
    StockHighLowClose,
    /// A stock chart using date/category labels and open/high/low/close-style series.
    Stock,
    /// A waterfall chart using the first series as category deltas.
    Waterfall,
    /// A treemap chart using the first series as rectangle sizes.
    Treemap,
    /// A sunburst chart using the first series as radial slice sizes.
    Sunburst,
    /// A histogram chart using category bins and the first series as bin counts.
    Histogram,
    /// A box-and-whisker chart using the first series as distribution values.
    BoxWhisker,
    /// A funnel chart using the first series as staged values.
    Funnel,
}

/// Supported shape styles for authored 3-D bar/column charts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChartShape {
    /// Rectangular 3-D boxes.
    #[default]
    Box,
    /// Cylindrical 3-D bars or columns.
    Cylinder,
    /// Cone-shaped 3-D bars or columns.
    Cone,
    /// Cone-shaped 3-D bars or columns scaled to the maximum value.
    ConeToMax,
    /// Pyramid-shaped 3-D bars or columns.
    Pyramid,
    /// Pyramid-shaped 3-D bars or columns scaled to the maximum value.
    PyramidToMax,
}

/// One named chart series with literal numeric values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartSeries {
    /// Series display name.
    pub name: String,
    /// Values aligned with [`Chart::categories`].
    pub values: Vec<f64>,
    /// Optional bubble sizes aligned with [`ChartSeries::values`].
    pub bubble_sizes: Vec<f64>,
}

/// A block-level chart with literal category and numeric caches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chart {
    /// Chart layout.
    pub kind: ChartKind,
    /// Optional display title.
    pub title: Option<String>,
    /// Category labels shared by all series.
    pub categories: Vec<String>,
    /// Named numeric series.
    pub series: Vec<ChartSeries>,
    /// Drawing width in pixels, interpreted at 96 dpi.
    pub width_px: Option<u32>,
    /// Drawing height in pixels, interpreted at 96 dpi.
    pub height_px: Option<u32>,
    /// Alternate text for the chart drawing.
    pub alt: Option<String>,
    /// Render surface-family charts as wireframes instead of filled surfaces.
    pub wireframe: bool,
    /// Shape style for 3-D bar and 3-D column charts.
    pub shape: ChartShape,
}

/// Page geometry, in points. Default is A4 portrait with 1-inch margins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    /// Page width.
    pub width_pt: f32,
    /// Page height.
    pub height_pt: f32,
    /// Uniform margin fallback (used for any side without an explicit override).
    pub margin_pt: f32,
    /// Left margin override (points); falls back to `margin_pt`. Per-side overrides
    /// let asymmetric layouts (a wide left sidebar, a binding gutter) render with
    /// the correct content box instead of a forced-uniform margin.
    pub margin_left_pt: Option<f32>,
    /// Right margin override (points); falls back to `margin_pt`.
    pub margin_right_pt: Option<f32>,
    /// Top margin override (points); falls back to `margin_pt`.
    pub margin_top_pt: Option<f32>,
    /// Bottom margin override (points); falls back to `margin_pt`.
    pub margin_bottom_pt: Option<f32>,
    /// Landscape orientation (swaps width/height semantics on emit).
    pub landscape: bool,
}

impl PageSetup {
    /// Left margin (override or uniform fallback).
    pub fn left(&self) -> f32 {
        self.margin_left_pt.unwrap_or(self.margin_pt)
    }
    /// Right margin (override or uniform fallback).
    pub fn right(&self) -> f32 {
        self.margin_right_pt.unwrap_or(self.margin_pt)
    }
    /// Top margin (override or uniform fallback).
    pub fn top(&self) -> f32 {
        self.margin_top_pt.unwrap_or(self.margin_pt)
    }
    /// Bottom margin (override or uniform fallback).
    pub fn bottom(&self) -> f32 {
        self.margin_bottom_pt.unwrap_or(self.margin_pt)
    }
}

impl Default for PageSetup {
    fn default() -> Self {
        // A4 = 210×297mm = 595.3×841.9pt; 1in = 72pt margins.
        PageSetup {
            width_pt: 595.3,
            height_pt: 841.9,
            margin_pt: 72.0,
            margin_left_pt: None,
            margin_right_pt: None,
            margin_top_pt: None,
            margin_bottom_pt: None,
            landscape: false,
        }
    }
}

/// Display format for generated section page numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageNumberFormat {
    /// Decimal numbers (`1`, `2`, `3`).
    Decimal,
    /// Zero-padded decimal numbers (`01`, `02`, `03`).
    DecimalZero,
    /// Decimal numbers surrounded by dashes (`- 1 -`, `- 2 -`, `- 3 -`).
    NumberInDash,
    /// Full-width decimal digits (`１`, `２`, `３`).
    DecimalFullWidth,
    /// Half-width decimal digits (`1`, `2`, `3`).
    DecimalHalfWidth,
    /// Alternate full-width decimal digits.
    DecimalFullWidth2,
    /// Circled decimal digits for one through twenty, then decimal fallback.
    DecimalEnclosedCircle,
    /// Decimal digits followed by full-stop glyphs for one through twenty.
    DecimalEnclosedFullstop,
    /// Parenthesized decimal digits for one through twenty.
    DecimalEnclosedParen,
    /// Korean Ganada sequence.
    Ganada,
    /// Korean Chosung sequence.
    Chosung,
    /// Korean digital numerals.
    KoreanDigital,
    /// Native Korean counting words.
    KoreanCounting,
    /// Korean legal numerals.
    KoreanLegal,
    /// Alternate Korean digital numerals.
    KoreanDigital2,
    /// Lowercase letters (`a`, `b`, `c`).
    LowerLetter,
    /// Uppercase letters (`A`, `B`, `C`).
    UpperLetter,
    /// Lowercase Roman numerals (`i`, `ii`, `iii`).
    LowerRoman,
    /// Uppercase Roman numerals (`I`, `II`, `III`).
    UpperRoman,
    /// Ordinal decimal numbers (`1st`, `2nd`, `3rd`).
    Ordinal,
    /// Cardinal text (`one`, `two`, `three`).
    CardinalText,
    /// Ordinal text (`first`, `second`, `third`).
    OrdinalText,
}

#[cfg(feature = "docx")]
impl PageNumberFormat {
    pub(crate) fn wml_value(self) -> &'static str {
        match self {
            PageNumberFormat::Decimal => "decimal",
            PageNumberFormat::DecimalZero => "decimalZero",
            PageNumberFormat::NumberInDash => "numberInDash",
            PageNumberFormat::DecimalFullWidth => "decimalFullWidth",
            PageNumberFormat::DecimalHalfWidth => "decimalHalfWidth",
            PageNumberFormat::DecimalFullWidth2 => "decimalFullWidth2",
            PageNumberFormat::DecimalEnclosedCircle => "decimalEnclosedCircle",
            PageNumberFormat::DecimalEnclosedFullstop => "decimalEnclosedFullstop",
            PageNumberFormat::DecimalEnclosedParen => "decimalEnclosedParen",
            PageNumberFormat::Ganada => "ganada",
            PageNumberFormat::Chosung => "chosung",
            PageNumberFormat::KoreanDigital => "koreanDigital",
            PageNumberFormat::KoreanCounting => "koreanCounting",
            PageNumberFormat::KoreanLegal => "koreanLegal",
            PageNumberFormat::KoreanDigital2 => "koreanDigital2",
            PageNumberFormat::LowerLetter => "lowerLetter",
            PageNumberFormat::UpperLetter => "upperLetter",
            PageNumberFormat::LowerRoman => "lowerRoman",
            PageNumberFormat::UpperRoman => "upperRoman",
            PageNumberFormat::Ordinal => "ordinal",
            PageNumberFormat::CardinalText => "cardinalText",
            PageNumberFormat::OrdinalText => "ordinalText",
        }
    }

    pub(crate) fn from_wml_value(value: &str) -> Option<Self> {
        let value = value.trim();
        match value {
            "decimal" => Some(PageNumberFormat::Decimal),
            "decimalZero" => Some(PageNumberFormat::DecimalZero),
            "numberInDash" => Some(PageNumberFormat::NumberInDash),
            "decimalFullWidth" => Some(PageNumberFormat::DecimalFullWidth),
            "decimalHalfWidth" => Some(PageNumberFormat::DecimalHalfWidth),
            "decimalFullWidth2" => Some(PageNumberFormat::DecimalFullWidth2),
            "decimalEnclosedCircle" => Some(PageNumberFormat::DecimalEnclosedCircle),
            "decimalEnclosedFullstop" => Some(PageNumberFormat::DecimalEnclosedFullstop),
            "decimalEnclosedParen" => Some(PageNumberFormat::DecimalEnclosedParen),
            "ganada" => Some(PageNumberFormat::Ganada),
            "chosung" => Some(PageNumberFormat::Chosung),
            "koreanDigital" => Some(PageNumberFormat::KoreanDigital),
            "koreanCounting" => Some(PageNumberFormat::KoreanCounting),
            "koreanLegal" => Some(PageNumberFormat::KoreanLegal),
            "koreanDigital2" => Some(PageNumberFormat::KoreanDigital2),
            "lowerLetter" => Some(PageNumberFormat::LowerLetter),
            "upperLetter" => Some(PageNumberFormat::UpperLetter),
            "lowerRoman" => Some(PageNumberFormat::LowerRoman),
            "upperRoman" => Some(PageNumberFormat::UpperRoman),
            "ordinal" => Some(PageNumberFormat::Ordinal),
            "cardinalText" => Some(PageNumberFormat::CardinalText),
            "ordinalText" => Some(PageNumberFormat::OrdinalText),
            _ => None,
        }
    }
}

/// Text flow direction for generated `.docx` section properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    /// Left-to-right lines flowing from top to bottom (`lrTb`).
    LeftToRightTopToBottom,
    /// Top-to-bottom columns flowing from right to left (`tbRl`).
    TopToBottomRightToLeft,
    /// Bottom-to-top columns flowing from left to right (`btLr`).
    BottomToTopLeftToRight,
    /// Vertically oriented left-to-right text flowing from top to bottom (`lrTbV`).
    LeftToRightTopToBottomVertical,
    /// Vertically oriented top-to-bottom text flowing from right to left (`tbRlV`).
    TopToBottomRightToLeftVertical,
    /// Vertically oriented top-to-bottom text flowing from left to right (`tbLrV`).
    TopToBottomLeftToRightVertical,
}

#[cfg(feature = "docx")]
impl TextDirection {
    pub(crate) fn wml_value(self) -> &'static str {
        match self {
            TextDirection::LeftToRightTopToBottom => "lrTb",
            TextDirection::TopToBottomRightToLeft => "tbRl",
            TextDirection::BottomToTopLeftToRight => "btLr",
            TextDirection::LeftToRightTopToBottomVertical => "lrTbV",
            TextDirection::TopToBottomRightToLeftVertical => "tbRlV",
            TextDirection::TopToBottomLeftToRightVertical => "tbLrV",
        }
    }

    pub(crate) fn from_wml_value(value: &str) -> Option<Self> {
        let value = value.trim();
        match value {
            "lrTb" => Some(TextDirection::LeftToRightTopToBottom),
            "tbRl" => Some(TextDirection::TopToBottomRightToLeft),
            "btLr" => Some(TextDirection::BottomToTopLeftToRight),
            "lrTbV" => Some(TextDirection::LeftToRightTopToBottomVertical),
            "tbRlV" => Some(TextDirection::TopToBottomRightToLeftVertical),
            "tbLrV" => Some(TextDirection::TopToBottomLeftToRightVertical),
            _ => None,
        }
    }
}

/// Document grid behavior for a WordprocessingML section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocGridType {
    /// No document grid (`default`).
    Default,
    /// Line grid only (`lines`).
    Lines,
    /// Line and character grid (`linesAndChars`).
    LinesAndChars,
    /// Character grid only (`snapToChars`).
    SnapToChars,
}

#[cfg(feature = "docx")]
impl DocGridType {
    pub(crate) fn wml_value(self) -> &'static str {
        match self {
            DocGridType::Default => "default",
            DocGridType::Lines => "lines",
            DocGridType::LinesAndChars => "linesAndChars",
            DocGridType::SnapToChars => "snapToChars",
        }
    }

    pub(crate) fn from_wml_value(value: &str) -> Option<Self> {
        let value = value.trim();
        match value {
            "default" => Some(DocGridType::Default),
            "lines" => Some(DocGridType::Lines),
            "linesAndChars" => Some(DocGridType::LinesAndChars),
            "snapToChars" => Some(DocGridType::SnapToChars),
            _ => None,
        }
    }
}

/// Document grid settings for a WordprocessingML section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocGrid {
    /// Grid behavior.
    pub grid_type: DocGridType,
    /// Grid line pitch (`w:linePitch`) in twentieths of a point, if set.
    pub line_pitch: Option<u32>,
    /// Grid character pitch adjustment (`w:charSpace`), if set.
    pub character_space: Option<u32>,
}

/// Section-level layout recovered from or generated into `.docx` section
/// properties.
///
/// In WordprocessingML a section property block closes the section that came
/// before it. `Block::SectionBreak(setup)` follows that convention: the stored
/// setup describes the section ending at that block, while `DocModel::setup`
/// describes the final section.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SectionSetup {
    /// Section-break start behavior, if this setup belongs to a section boundary.
    pub section_break: Option<SectionBreakKind>,
    /// Page geometry.
    pub page: PageSetup,
    /// Running header content (empty = none).
    pub header: Vec<Block>,
    /// First-page running header content (empty = none or default applies).
    pub first_header: Vec<Block>,
    /// Even-page running header content (empty = none or default applies).
    pub even_header: Vec<Block>,
    /// Running footer content (empty = none).
    pub footer: Vec<Block>,
    /// First-page running footer content (empty = none or default applies).
    pub first_footer: Vec<Block>,
    /// Even-page running footer content (empty = none or default applies).
    pub even_footer: Vec<Block>,
    /// Use distinct first-page section behavior (`w:titlePg`).
    pub title_page: bool,
    /// Emit a centered page number (`PAGE` field) in the footer.
    pub page_numbers: bool,
    /// Display page number to start this section at, if explicitly set.
    pub page_number_start: Option<u32>,
    /// Display page-number format for this section, if explicitly set.
    pub page_number_format: Option<PageNumberFormat>,
    /// Number of text columns in this section, if explicitly set.
    pub columns: Option<u16>,
    /// Text flow direction for this section, if explicitly set.
    pub text_direction: Option<TextDirection>,
    /// Document grid settings for this section, if explicitly set.
    pub doc_grid: Option<DocGrid>,
}

/// Document-level layout + metadata, for authoring and rendering. All fields are
/// optional/default so existing read paths are unaffected.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocSetup {
    /// Page geometry.
    pub page: PageSetup,
    /// Paragraph style definitions for generated `.docx` output.
    pub styles: Vec<ParagraphStyle>,
    /// Running header content (empty = none).
    pub header: Vec<Block>,
    /// First-page running header content (empty = none or default applies).
    pub first_header: Vec<Block>,
    /// Even-page running header content (empty = none or default applies).
    pub even_header: Vec<Block>,
    /// Running footer content (empty = none).
    pub footer: Vec<Block>,
    /// First-page running footer content (empty = none or default applies).
    pub first_footer: Vec<Block>,
    /// Even-page running footer content (empty = none or default applies).
    pub even_footer: Vec<Block>,
    /// Use distinct first-page section behavior (`w:titlePg`).
    pub title_page: bool,
    /// Emit a centered page number (`PAGE` field) in the footer.
    pub page_numbers: bool,
    /// Display page number to start the final/current section at, if explicitly set.
    pub page_number_start: Option<u32>,
    /// Display page-number format for the final/current section, if explicitly set.
    pub page_number_format: Option<PageNumberFormat>,
    /// Number of text columns in the final/current section, if explicitly set.
    pub columns: Option<u16>,
    /// Text flow direction for the final/current section, if explicitly set.
    pub text_direction: Option<TextDirection>,
    /// Document grid settings for the final/current section, if explicitly set.
    pub doc_grid: Option<DocGrid>,
    /// Optional document identifier emitted to `word/settings.xml` as `w14:docId`.
    pub document_id: Option<String>,
    /// Authored Office web-extension task panes.
    pub web_extension_task_panes: Vec<WebExtensionTaskPane>,
    /// Document title metadata.
    pub title: Option<String>,
    /// Document subject metadata.
    pub subject: Option<String>,
    /// Document author metadata.
    pub creator: Option<String>,
    /// Document description metadata.
    pub description: Option<String>,
    /// Document keywords metadata.
    pub keywords: Option<String>,
    /// Document category metadata.
    pub category: Option<String>,
    /// Document content-status metadata.
    pub content_status: Option<String>,
    /// Document last-modified-by metadata.
    pub last_modified_by: Option<String>,
    /// Document creation timestamp metadata.
    pub created: Option<String>,
    /// Document last-modified timestamp metadata.
    pub modified: Option<String>,
    /// Document last-printed timestamp metadata.
    pub last_printed: Option<String>,
    /// Document revision-count metadata.
    pub revision: Option<String>,
    /// Document version metadata.
    pub version: Option<String>,
}

impl From<&DocSetup> for SectionSetup {
    fn from(setup: &DocSetup) -> Self {
        SectionSetup {
            section_break: None,
            page: setup.page,
            header: setup.header.clone(),
            first_header: setup.first_header.clone(),
            even_header: setup.even_header.clone(),
            footer: setup.footer.clone(),
            first_footer: setup.first_footer.clone(),
            even_footer: setup.even_footer.clone(),
            title_page: setup.title_page,
            page_numbers: setup.page_numbers,
            page_number_start: setup.page_number_start,
            page_number_format: setup.page_number_format,
            columns: setup.columns,
            text_direction: setup.text_direction,
            doc_grid: setup.doc_grid,
        }
    }
}

#[cfg(all(test, feature = "docx"))]
mod tests {
    use super::{DocGridType, PageNumberFormat, SectionBreakKind, TableBorderStyle, TextDirection};

    #[test]
    fn wml_enum_parsers_trim_ooxml_values() {
        assert_eq!(
            SectionBreakKind::from_wml_value(" evenPage "),
            Some(SectionBreakKind::EvenPage)
        );
        assert_eq!(
            TableBorderStyle::from_wml_value(" dotted "),
            Some(TableBorderStyle::Dotted)
        );
        assert_eq!(
            PageNumberFormat::from_wml_value(" lowerRoman "),
            Some(PageNumberFormat::LowerRoman)
        );
        assert_eq!(
            TextDirection::from_wml_value(" tbRl "),
            Some(TextDirection::TopToBottomRightToLeft)
        );
        assert_eq!(
            DocGridType::from_wml_value(" linesAndChars "),
            Some(DocGridType::LinesAndChars)
        );
    }
}
